// Value semantics, builder chaining, and listener registrations.
import assert from "node:assert/strict";
import { test } from "node:test";
import { inspect } from "node:util";
import { sui } from "./support.js";

test("value types compare by value", () => {
  assert.ok(new sui.Point(1, 2).equals(new sui.Point(1, 2)));
  assert.ok(!new sui.Point(1, 2).equals(new sui.Point(2, 1)));
  assert.ok(new sui.Size(0, 1).equals(new sui.Size(-0, 1)));
  assert.ok(new sui.Rect(0, 0, 4, 4).equals(new sui.Rect(0, 0, 4, 4)));
  assert.ok(!new sui.Rect(0, 0, 4, 4).equals(new sui.Rect(0, 0, 4, 5)));
  assert.ok(sui.Color.rgba(1, 0, 0, 1).equals(new sui.Color(1, 0, 0, 1)));
  assert.ok(!new sui.Point(1, 2).equals({ x: 1, y: 2 }));
  assert.ok(!new sui.Point(1, 2).equals(null));
});

test("color helpers", () => {
  assert.ok(sui.Color.rgba(0.5, 0.25, 1).equals(new sui.Color(0.5, 0.25, 1, 1)));
  assert.ok(sui.Color.black().equals(new sui.Color(0, 0, 0, 1)));
  assert.ok(sui.Color.white().equals(new sui.Color(1, 1, 1, 1)));
});

test("value types print their fields", () => {
  assert.equal(String(new sui.Point(1, 2)), "Point(x=1, y=2)");
  assert.equal(String(new sui.Size(3, 4)), "Size(width=3, height=4)");
  assert.equal(String(new sui.Rect(0, 1, 2, 3)), "Rect(x=0, y=1, width=2, height=3)");
  assert.equal(String(sui.Color.black()), "Color(red=0, green=0, blue=0, alpha=1)");
  assert.equal(inspect(new sui.Point(1, 2)), "Point(x=1, y=2)");
  assert.equal(`${new sui.Size(3, 4)}`, "Size(width=3, height=4)");
});

test("builders return this", () => {
  const window = new sui.Window("x");
  assert.equal(window.root(sui.label("x")), window);
  assert.equal(window.setInitialSize(new sui.Size(10, 10)), window);
  assert.equal(window.setInitialPosition(new sui.Point(0, 0)), window);

  const app = new sui.App();
  assert.equal(app.window(window), app);
  assert.equal(app.setTheme(sui.Theme.dark()), app);
  assert.equal(app.configureRendering({ feathering: true }), app);
  assert.equal(app.windowCount, 1);
});

test("app.on returns a disposable subscription", () => {
  const app = new sui.App().window(new sui.Window("Events").root(sui.label("x")));
  const seen = [];
  const subscription = app.on("ping", (payload) => seen.push(payload));
  const running = app.start();
  const ui = running.uiHandle();

  assert.ok(subscription instanceof sui.Subscription);
  assert.equal(subscription.active, true);
  ui.emit("ping", 1);
  running.drain();
  assert.deepEqual(seen, [1]);

  assert.equal(subscription.unsubscribe(), true);
  assert.equal(subscription.active, false);
  assert.equal(subscription.unsubscribe(), false);
  ui.emit("ping", 2);
  running.drain();
  assert.deepEqual(seen, [1]);

  {
    using disposable = app.on("ping", (payload) => seen.push(payload));
    ui.emit("ping", 3);
    running.drain();
    assert.equal(disposable.active, true);
  }
  ui.emit("ping", 4);
  running.drain();
  assert.deepEqual(seen, [1, 3]);

  const controller = new AbortController();
  const aborted = app.on("ping", (payload) => seen.push(payload), { signal: controller.signal });
  controller.abort();
  assert.equal(aborted.active, false);
});

test("app.once resolves with the next payload", async () => {
  const app = new sui.App().window(new sui.Window("Events").root(sui.label("x")));
  const running = app.start();
  const next = app.once("ready");
  running.uiHandle().emit("ready", "payload");
  running.drain();
  assert.equal(await next, "payload");
});

test("app.once rejects when aborted", async () => {
  const app = new sui.App().window(new sui.Window("Events").root(sui.label("x")));
  await assert.rejects(app.once("ready", { signal: AbortSignal.abort() }), { name: "AbortError" });

  const controller = new AbortController();
  const pending = app.once("ready", { signal: controller.signal });
  controller.abort();
  await assert.rejects(pending, { name: "AbortError" });
});
