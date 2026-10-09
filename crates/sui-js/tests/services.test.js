// Platform services: the clipboard and file dialogs.
import assert from "node:assert/strict";
import { afterEach, beforeEach, test } from "node:test";
import { sui } from "./support.js";

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

test("file dialogs reject unknown modes", async () => {
  const app = new sui.App().window(new sui.Window("Dialogs").root(sui.label("x")));
  const ui = app.start().uiHandle();
  const result = ui.showFileDialog({ mode: "bogus" });
  assert.ok(result instanceof Promise);
  await assert.rejects(result, /unknown file dialog mode/);
});
