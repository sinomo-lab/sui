// Callback errors: the error handler, its console.error default, and async callbacks.
import assert from "node:assert/strict";
import { test } from "node:test";
import { capturingErrors, sui } from "./support.js";

function startWithButtons(buttons) {
  const root = sui.column(
    Object.entries(buttons).map(([name, onPress]) => sui.button(name, { onPress })),
  );
  const running = new sui.App().window(new sui.Window("Callbacks").root(root)).start();
  return [running, running.render()];
}

function fail() {
  throw new RangeError("press failed");
}

/** Replace console.error while `body` runs and return what it logged. */
function capturingConsoleErrors(t, body) {
  const logged = [];
  t.mock.method(console, "error", (...args) => logged.push(args));
  try {
    body();
  } finally {
    console.error.mock.restore();
  }
  return logged;
}

test("a thrown error reaches the handler with its stack", async () => {
  const errors = await capturingErrors(() => {
    const [running, snapshot] = startWithButtons({ Fail: fail });
    running.click(snapshot.getOne({ role: "button", name: "Fail" }));
  });
  assert.equal(errors.length, 1);
  assert.ok(errors[0] instanceof RangeError);
  assert.equal(errors[0].message, "press failed");
  assert.match(errors[0].stack, /\bfail\b/);
});

test("an async rejection reaches the handler after idle", async () => {
  const errors = await capturingErrors(async () => {
    const [running, snapshot] = startWithButtons({
      Fail: async () => {
        await new Promise((resolve) => setTimeout(resolve, 5));
        throw new RangeError("async press failed");
      },
    });
    running.click(snapshot.getOne({ role: "button", name: "Fail" }));
    await running.idle();
  });
  assert.equal(errors.length, 1);
  assert.equal(errors[0].message, "async press failed");
});

test("the default handler logs with console.error", (t) => {
  sui.setErrorHandler(null);
  const logged = capturingConsoleErrors(t, () => {
    const [running, snapshot] = startWithButtons({ Fail: fail });
    running.click(snapshot.getOne({ role: "button", name: "Fail" }));
  });
  assert.equal(logged.length, 1);
  const error = logged[0].find((arg) => arg instanceof Error);
  assert.equal(error.message, "press failed");
});

test("a failing handler still logs the original error", (t) => {
  sui.setErrorHandler(() => {
    throw new Error("handler failed");
  });
  let logged;
  try {
    logged = capturingConsoleErrors(t, () => {
      const [running, snapshot] = startWithButtons({ Fail: fail });
      running.click(snapshot.getOne({ role: "button", name: "Fail" }));
    });
  } finally {
    sui.setErrorHandler(null);
  }
  const messages = logged.map((args) => args.find((arg) => arg instanceof Error)?.message);
  assert.deepEqual(messages.sort(), ["handler failed", "press failed"]);
});

test("setErrorHandler validates its argument", () => {
  assert.throws(() => sui.setErrorHandler(42), TypeError);
  assert.throws(() => sui.setErrorHandler("log"), TypeError);
  sui.setErrorHandler(() => {});
  sui.setErrorHandler(null);
  sui.setErrorHandler(undefined);
});

test("state watchers and app listeners report errors", async () => {
  const errors = await capturingErrors(() => {
    const state = new sui.State("a");
    const subscription = state.watch(() => {
      throw new RangeError("watched");
    });
    state.set("b");
    subscription.unsubscribe();

    const app = new sui.App().window(new sui.Window("Events").root(sui.label("x")));
    app.on("ping", () => {
      throw new SyntaxError("listened");
    });
    const running = app.start();
    running.uiHandle().emit("ping", 1);
    running.drain();
  });
  assert.deepEqual(
    errors.map((error) => [error.constructor, error.message]),
    [
      [RangeError, "watched"],
      [SyntaxError, "listened"],
    ],
  );
});

test("requestExit without a running loop returns false", () => {
  const [running] = startWithButtons({});
  assert.equal(running.uiHandle().requestExit(), false);
});
