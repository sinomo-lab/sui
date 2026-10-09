// State values, derived states, watchers, and the promise helpers.
import assert from "node:assert/strict";
import { test } from "node:test";
import { sui } from "./support.js";

for (const value of [3, 2.5, true, "text"]) {
  test(`state keeps the ${typeof value} value ${value}`, () => {
    const state = new sui.State(value);
    assert.equal(state.get(), value);
    assert.equal(typeof state.get(), typeof value);
  });
}

test("state rejects unsupported values", () => {
  assert.throws(() => new sui.State([1, 2]));
  assert.throws(() => new sui.State({}));
});

test("select and watch follow updates", () => {
  const source = new sui.State(2);
  const doubled = source.select((value) => value * 2);
  const seen = [];
  const subscription = source.watch((value) => seen.push(value));

  source.set(3);
  assert.equal(doubled.get(), 6);
  assert.deepEqual(seen, [3]);

  assert.equal(subscription.unsubscribe(), true);
  source.set(4);
  assert.equal(doubled.get(), 8);
  assert.deepEqual(seen, [3]);
});

test("selection writes back integers", () => {
  const selected = new sui.State(1);
  const app = new sui.App().window(
    new sui.Window("Tabs").root(sui.tabs("Sections", ["Design", "Inspect"], { selected })),
  );
  const running = app.start();
  running.press(running.render().getOne({ role: "tabs" }), "ArrowLeft");
  running.drain();

  assert.equal(selected.get(), 0);
  assert.ok(Number.isInteger(selected.get()));
});

test("changed() resolves with the next value", async () => {
  const state = new sui.State("a");
  const next = state.changed();
  state.set("b");
  assert.equal(await next, "b");

  // Only the next change: later ones need another call.
  state.set("c");
  const after = state.changed();
  state.set("d");
  assert.equal(await after, "d");
});

test("changed() rejects when aborted", async () => {
  const state = new sui.State(1);
  await assert.rejects(state.changed({ signal: AbortSignal.abort() }), { name: "AbortError" });

  const controller = new AbortController();
  const pending = state.changed({ signal: controller.signal });
  controller.abort();
  await assert.rejects(pending, { name: "AbortError" });
  state.set(2);
});

test("changes() iterates until aborted", async () => {
  const state = new sui.State(0);
  const controller = new AbortController();
  const seen = [];

  const loop = (async () => {
    for await (const value of state.changes({ signal: controller.signal })) {
      seen.push(value);
      if (value === 3) {
        controller.abort();
      }
    }
  })();
  // Changes made before the loop reads them are queued, not dropped.
  state.set(1);
  state.set(2);
  await new Promise((resolve) => setImmediate(resolve));
  state.set(3);
  await loop;
  state.set(4);
  assert.deepEqual(seen, [1, 2, 3]);
});

test("changes() stops watching when the loop exits", async () => {
  const state = new sui.State(0);
  const seen = [];
  const loop = (async () => {
    for await (const value of state.changes()) {
      seen.push(value);
      break;
    }
  })();
  state.set(1);
  await loop;
  state.set(2);
  await new Promise((resolve) => setImmediate(resolve));
  assert.deepEqual(seen, [1]);
});

test("watch() stops when its signal aborts", () => {
  const state = new sui.State(0);
  const controller = new AbortController();
  const seen = [];
  state.watch((value) => seen.push(value), { signal: controller.signal });
  state.set(1);
  controller.abort();
  state.set(2);
  assert.deepEqual(seen, [1]);

  state.watch((value) => seen.push(value), { signal: AbortSignal.abort() });
  state.set(3);
  assert.deepEqual(seen, [1]);
});

test("state subscriptions are disposable", () => {
  const state = new sui.State(1);
  const seen = [];
  {
    using subscription = state.watch((value) => seen.push(value));
    assert.equal(typeof subscription[Symbol.dispose], "function");
    state.set(2);
  }
  state.set(3);
  assert.deepEqual(seen, [2]);

  const subscription = state.watch((value) => seen.push(value));
  subscription[Symbol.dispose]();
  state.set(4);
  assert.deepEqual(seen, [2]);
  assert.equal(subscription.unsubscribe(), false);
});
