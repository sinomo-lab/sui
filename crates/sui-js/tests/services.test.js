// Platform services: the clipboard, opening URLs, and file dialogs.
import assert from "node:assert/strict";
import { afterEach, beforeEach, test } from "node:test";
import { capturingErrors, sui } from "./support.js";

// The clipboard may be the user's system clipboard; restore it.
let previousClipboard;
beforeEach(() => {
  previousClipboard = sui.clipboardText();
});
afterEach(() => {
  sui.setClipboardText(previousClipboard ?? "");
});

test("clipboard round trip", () => {
  sui.setClipboardText("sui clipboard test");
  assert.equal(sui.clipboardText(), "sui clipboard test");
});

test("a widget's clipboard matches the module functions", () => {
  const copied = [];
  const copier = new sui.Widget({
    measure: (constraints) => constraints.clamp(new sui.Size(40, 20)),
    event(event, context) {
      if (event.kind === "pointer" && event.action === "down") {
        context.setClipboardText("from a widget");
        copied.push(true);
      }
    },
  });
  const running = new sui.App().window(new sui.Window("Clipboard").root(copier)).start();
  running.render();
  running.handleEvent(sui.Event.pointer("down", new sui.Point(10, 10)));

  assert.deepEqual(copied, [true]);
  assert.equal(sui.clipboardText(), "from a widget");
});

// Allowed URLs would launch the real browser, so only refusals are tested.
test("openUrl refuses URLs outside the allowed schemes", () => {
  assert.equal(sui.openUrl("file:///etc/passwd"), false);
  assert.equal(sui.openUrl("javascript:alert(1)"), false);
  assert.equal(sui.openUrl("docs/intro.md"), false);
});

test("the URL policy decides what opens", () => {
  const asked = [];
  sui.setUrlPolicy((url) => {
    asked.push(url);
    return false;
  });
  try {
    assert.equal(sui.openUrl("  https://example.com  "), false);
    assert.deepEqual(asked, ["https://example.com"]);
  } finally {
    sui.setUrlPolicy(null);
  }
  assert.equal(sui.openUrl("file:///etc/passwd"), false);
  assert.deepEqual(asked, ["https://example.com"]);
});

test("the URL policy must be a function that returns a boolean", () => {
  assert.throws(() => sui.setUrlPolicy("https"), TypeError);
  sui.setUrlPolicy(() => "yes");
  try {
    assert.throws(() => sui.openUrl("https://example.com"), /must return a boolean/);
  } finally {
    sui.setUrlPolicy(null);
  }
});

test("urlAllowedByDefault extends into custom policies", () => {
  assert.equal(sui.urlAllowedByDefault("https://example.com"), true);
  assert.equal(sui.urlAllowedByDefault("mailto:team@example.com"), true);
  assert.equal(sui.urlAllowedByDefault("myapp://settings"), false);
  assert.equal(sui.urlAllowedByDefault("docs/intro.md"), false);
});

test("openUrl can be a link's onOpen callback", async () => {
  // The policy refuses `file:`, so the click opens nothing.
  const link = sui.link("Passwords", "file:///etc/passwd", { onOpen: sui.openUrl });
  const errors = await capturingErrors(() => {
    const running = new sui.App().window(new sui.Window("Links").root(link)).start();
    running.click(running.render().getOne({ role: "link", name: "Passwords" }));
  });
  assert.deepEqual(errors, []);
});

test("file dialogs reject unknown modes", async () => {
  const app = new sui.App().window(new sui.Window("Dialogs").root(sui.label("x")));
  const ui = app.start().uiHandle();
  const result = ui.showFileDialog({ mode: "bogus" });
  assert.ok(result instanceof Promise);
  await assert.rejects(result, /unknown file dialog mode/);
});
