// The JavaScript layer shared by the Node addon (index.js) and the
// WebAssembly build (web.js): it runs apps from the host's event loop, lets
// callbacks be async, routes callback errors, and adds promise helpers.
import { createFactories, EXPORTS } from "./api.js";

const disposeSymbol = Symbol.dispose ?? Symbol.for("Symbol.dispose");
const inspectSymbol = Symbol.for("nodejs.util.inspect.custom");

/**
 * Longest wait between event-loop turns. Input is only read during a turn,
 * so this bounds the added input latency on hosts that need pumping.
 */
const MAX_TURN_WAIT_MS = 8;

/**
 * Build the public API over a native module.
 *
 * `host` supplies what differs between Node and the browser:
 * `start(app, options)` opens the app's windows and returns its pumped app:
 * `pump(timeoutMs)` runs one event-loop turn and returns the milliseconds
 * until the next one is due (`-1` once the app exited), `uiHandle` is its
 * handle, an optional `maxTurnWaitMs` overrides the polling bound, and an
 * optional `dispose()` runs once the app exited.
 * `writeFile(path, bytes)` saves screenshots where a file system exists.
 */
export function createApi(native, host) {
  let errorHandler = null;
  const pending = new Set();
  let runningApp = null;

  function reportError(error) {
    if (errorHandler !== null) {
      try {
        errorHandler(error);
        return;
      } catch (handlerError) {
        console.error("Error in the SUI error handler:", handlerError);
      }
    }
    console.error("Uncaught error in a SUI callback:", error);
  }

  function setErrorHandler(handler) {
    if (handler != null && typeof handler !== "function") {
      throw new TypeError("setErrorHandler() expects a function or null");
    }
    errorHandler = handler ?? null;
  }

  function track(promise) {
    const tracked = Promise.resolve(promise)
      .then(undefined, reportError)
      .finally(() => pending.delete(tracked));
    pending.add(tracked);
    return tracked;
  }

  function checkCallback(callback) {
    if (typeof callback !== "function") {
      throw new TypeError(`expected a callback function, got ${typeof callback}`);
    }
  }

  // A callback whose result is ignored: it may be async, and its errors go
  // to the error handler instead of the native caller.
  function action(callback) {
    if (callback == null) {
      return callback;
    }
    checkCallback(callback);
    return function (...args) {
      try {
        const result = callback.apply(this, args);
        if (result != null && typeof result.then === "function") {
          track(result);
        }
      } catch (error) {
        reportError(error);
      }
    };
  }

  // A callback whose result the native caller uses: it must be synchronous.
  // Errors are reported and also returned to the caller as a failure.
  function guard(callback, receiver) {
    if (callback == null) {
      return callback;
    }
    checkCallback(callback);
    return function (...args) {
      try {
        return callback.apply(receiver ?? this, args);
      } catch (error) {
        reportError(error);
        throw error;
      }
    };
  }

  function onAbort(signal, abort) {
    if (signal == null) {
      return () => {};
    }
    if (signal.aborted) {
      abort();
      return () => {};
    }
    signal.addEventListener("abort", abort, { once: true });
    return () => signal.removeEventListener("abort", abort);
  }

  function abortReason(signal) {
    return signal.reason ?? new DOMException("The operation was aborted", "AbortError");
  }

  /** A listener registration. Unsubscribing twice is harmless. */
  class Subscription {
    #release;

    constructor(release) {
      this.#release = release;
    }

    /** Stop listening. Returns `false` when already unsubscribed. */
    unsubscribe() {
      const release = this.#release;
      if (release === null) {
        return false;
      }
      this.#release = null;
      release();
      return true;
    }

    /** Whether the listener is still registered. */
    get active() {
      return this.#release !== null;
    }

    [disposeSymbol]() {
      this.unsubscribe();
    }
  }

  // ---- State ----------------------------------------------------------

  const { State, StateSubscription } = native;
  const nativeWatch = State.prototype.watch;
  const nativeSelect = State.prototype.select;

  StateSubscription.prototype[disposeSymbol] = function () {
    this.unsubscribe();
  };

  State.prototype.watch = function (callback, options = {}) {
    const subscription = nativeWatch.call(this, action(callback));
    onAbort(options.signal, () => subscription.unsubscribe());
    return subscription;
  };

  State.prototype.select = function (selector) {
    return nativeSelect.call(this, guard(selector));
  };

  State.prototype.changed = function (options = {}) {
    const { signal } = options;
    return new Promise((resolve, reject) => {
      if (signal?.aborted) {
        reject(abortReason(signal));
        return;
      }
      let release = () => {};
      const subscription = nativeWatch.call(this, (value) => {
        subscription.unsubscribe();
        release();
        resolve(value);
      });
      release = onAbort(signal, () => {
        subscription.unsubscribe();
        reject(abortReason(signal));
      });
    });
  };

  State.prototype.changes = async function* (options = {}) {
    const { signal } = options;
    const queue = [];
    let wake = null;
    let stopped = false;
    const subscription = nativeWatch.call(this, (value) => {
      queue.push(value);
      wake?.();
    });
    const release = onAbort(signal, () => {
      stopped = true;
      wake?.();
    });
    try {
      while (!stopped) {
        if (queue.length > 0) {
          yield queue.shift();
          continue;
        }
        await new Promise((resolve) => {
          wake = resolve;
        });
        wake = null;
      }
    } finally {
      subscription.unsubscribe();
      release();
    }
  };

  // ---- Custom widgets -------------------------------------------------

  const widgetCallbackNames = new Set([
    "measure",
    "measureWithChildren",
    "arrange",
    "event",
    "paint",
    "semantics",
  ]);

  // Report errors from custom widget callbacks; `this` stays the caller's
  // callbacks object.
  function guardWidgetCallbacks(callbacks) {
    if (callbacks == null || typeof callbacks !== "object") {
      return callbacks;
    }
    const guarded = new Map();
    return new Proxy(callbacks, {
      get(target, key, receiver) {
        const value = Reflect.get(target, key, receiver);
        if (typeof value !== "function" || !widgetCallbackNames.has(key)) {
          return value;
        }
        let wrapped = guarded.get(value);
        if (wrapped === undefined) {
          wrapped = guard(value, target);
          guarded.set(value, wrapped);
        }
        return wrapped;
      },
    });
  }

  const Widget = new Proxy(native.Widget, {
    construct(target, [callbacks, ...rest], newTarget) {
      return Reflect.construct(
        target,
        [guardWidgetCallbacks(callbacks), ...rest],
        newTarget === Widget ? target : newTarget,
      );
    },
  });

  // ---- Builders return `this` -----------------------------------------

  function chain(Class, names) {
    for (const name of names) {
      const method = Class.prototype[name];
      Class.prototype[name] = function (...args) {
        method.apply(this, args);
        return this;
      };
    }
  }

  chain(native.Window, [
    "root",
    "setInitialSize",
    "setInitialPosition",
    "setIconSvg",
    "removeIcon",
  ]);
  chain(native.App, ["window", "configureRendering", "setTheme"]);

  // ---- Values compare and print by value ------------------------------

  function valueClass(Class, fields) {
    Class.prototype.equals = function (other) {
      return (
        other instanceof Class && fields.every((field) => this[field] === other[field])
      );
    };
    Class.prototype.toString = function () {
      return `${Class.name}(${fields.map((field) => `${field}=${this[field]}`).join(", ")})`;
    };
    Class.prototype[inspectSymbol] = function () {
      return this.toString();
    };
  }

  valueClass(native.Point, ["x", "y"]);
  valueClass(native.Size, ["width", "height"]);
  valueClass(native.Rect, ["x", "y", "width", "height"]);
  valueClass(native.Color, ["red", "green", "blue", "alpha"]);

  const { Color } = native;
  Color.rgba = (red, green, blue, alpha = 1) => new Color(red, green, blue, alpha);
  Color.black = () => new Color(0, 0, 0, 1);
  Color.white = () => new Color(1, 1, 1, 1);

  // ---- App ------------------------------------------------------------

  const { App, UiHandle, RunningApp } = native;
  const nativeOn = App.prototype.on;
  const appListeners = new WeakMap();

  function listenersFor(app, name) {
    let byName = appListeners.get(app);
    if (byName === undefined) {
      byName = new Map();
      appListeners.set(app, byName);
    }
    let listeners = byName.get(name);
    if (listeners === undefined) {
      listeners = new Set();
      byName.set(name, listeners);
      nativeOn.call(app, name, (payload) => {
        for (const listener of [...listeners]) {
          listener(payload);
        }
      });
    }
    return listeners;
  }

  App.prototype.on = function (name, listener, options = {}) {
    const wrapped = action(listener);
    const listeners = listenersFor(this, name);
    listeners.add(wrapped);
    const subscription = new Subscription(() => listeners.delete(wrapped));
    onAbort(options.signal, () => subscription.unsubscribe());
    return subscription;
  };

  App.prototype.once = function (name, options = {}) {
    const { signal } = options;
    return new Promise((resolve, reject) => {
      if (signal?.aborted) {
        reject(abortReason(signal));
        return;
      }
      const listeners = listenersFor(this, name);
      let release = () => {};
      const listener = (payload) => {
        listeners.delete(listener);
        release();
        resolve(payload);
      };
      listeners.add(listener);
      release = onAbort(signal, () => {
        listeners.delete(listener);
        reject(abortReason(signal));
      });
    });
  };

  App.prototype.run = function (options = {}) {
    if (typeof options === "function") {
      options = { onReady: options };
    }
    if (runningApp !== null) {
      return Promise.reject(new Error("an app is already running"));
    }
    let pumped;
    try {
      if (options.onReady != null) {
        checkCallback(options.onReady);
      }
      pumped = host.start(this, options);
    } catch (error) {
      return Promise.reject(error);
    }
    runningApp = this;
    const ui = pumped.uiHandle;
    const maxWait = pumped.maxTurnWaitMs ?? MAX_TURN_WAIT_MS;
    return new Promise((resolve, reject) => {
      let timer = null;
      let finished = false;
      let setupError;
      const releaseAbort = onAbort(options.signal, () => ui.requestExit());
      const finish = (error) => {
        if (finished) {
          return;
        }
        finished = true;
        clearTimeout(timer);
        releaseAbort();
        pumped.dispose?.();
        runningApp = null;
        if (error === undefined) {
          resolve();
        } else {
          reject(error);
        }
      };
      const turn = () => {
        timer = null;
        let wait;
        try {
          wait = pumped.pump(0);
        } catch (error) {
          finish(error);
          return;
        }
        if (wait < 0) {
          finish(setupError);
          return;
        }
        timer = setTimeout(turn, Math.max(0, Math.min(wait, maxWait)));
      };
      timer = setTimeout(turn, 0);
      if (options.onReady != null) {
        // A failing setup exits the app and rejects `run()`.
        Promise.resolve()
          .then(() => options.onReady(ui))
          .catch((error) => {
            setupError = error;
            if (!ui.requestExit()) {
              finish(error);
            }
          });
      }
    });
  };

  // ---- UI handle ------------------------------------------------------

  const nativeShowFileDialog = UiHandle.prototype.showFileDialog;
  UiHandle.prototype.showFileDialog = function (options) {
    return new Promise((resolve, reject) => {
      try {
        nativeShowFileDialog.call(this, options ?? undefined, (paths, error) => {
          if (error != null) {
            reject(new Error(error));
          } else {
            resolve(paths ?? null);
          }
        });
      } catch (error) {
        reject(error);
      }
    });
  };

  // ---- Headless runs --------------------------------------------------

  RunningApp.prototype.idle = async function () {
    for (;;) {
      this.drain();
      if (pending.size === 0) {
        return;
      }
      await Promise.allSettled([...pending]);
    }
  };

  RunningApp.prototype.saveScreenshot = async function (path, index = 0) {
    if (host.writeFile == null) {
      throw new Error("saveScreenshot() needs a file system; use screenshotPng() instead");
    }
    await host.writeFile(path, this.screenshotPng(index));
  };

  // ---- Public API -----------------------------------------------------

  // Only the names index.d.ts declares; native internals stay private.
  const all = {
    ...native,
    ...createFactories(native, { action, guard }),
    Widget,
    Subscription,
    setErrorHandler,
    version: native.nativeVersion(),
  };
  const api = {};
  for (const name of EXPORTS) {
    if (all[name] === undefined) {
      throw new Error(`the SUI native module does not provide ${name}`);
    }
    api[name] = all[name];
  }
  return api;
}
