// Host-driven test helpers: scoped queries, time control, and screenshots.
import assert from "node:assert/strict";
import { mkdtemp, readFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { backend, sui } from "./support.js";

function start(root, width = 320, height = 200) {
  const window = new sui.Window("Helpers").setInitialSize(new sui.Size(width, height)).root(root);
  return new sui.App().window(window).start();
}

test("queries can be scoped to a subtree", () => {
  const running = start(
    sui.row([
      sui.semanticRegion("Left", sui.button("Save")),
      sui.semanticRegion("Right", sui.button("Save")),
    ]),
  );
  const snapshot = running.render();
  assert.equal(snapshot.find({ role: "button", name: "Save" }).length, 2);
  const right = snapshot.getOne({ name: "Right" });
  const save = snapshot.getOne({ role: "button", name: "Save", within: right.id });
  const ids = (within) => snapshot.find({ within: within.id }).map((node) => node.id);
  assert.ok(ids(right).includes(save.id));
  assert.ok(!ids(snapshot.getOne({ name: "Left" })).includes(save.id));
  assert.deepEqual(snapshot.find({ within: save.id }), []);
});

test("role queries accept any spelling", () => {
  const snapshot = start(sui.textInput("Name")).render();
  for (const role of ["text_input", "text-input", "textInput"]) {
    assert.equal(snapshot.getOne({ role }).name, "Name");
  }
});

test("advanceTime and settleAnimations", () => {
  const running = start(sui.switchControl("Wi-Fi"));
  const snapshot = running.render();
  assert.equal(running.frameTime, 0);
  running.click(snapshot.getOne({ role: "switch" }));
  const elapsed = running.settleAnimations();
  assert.ok(elapsed > 0 && elapsed < 2);
  assert.ok(running.frameTime >= elapsed);
  assert.equal(running.settleAnimations(), 0);

  running.advanceTime(0.5);
  assert.ok(Math.abs(running.frameTime - (elapsed + 0.5)) <= 0.02);
  assert.throws(() => running.advanceTime(-1));
});

test("motion preference off finishes transitions immediately", () => {
  sui.setMotionPreference("off");
  try {
    const running = start(sui.switchControl("Wi-Fi"));
    running.click(running.render().getOne({ role: "switch" }));
    assert.equal(running.settleAnimations(), 0);
  } finally {
    sui.setMotionPreference(null);
  }
  assert.throws(() => sui.setMotionPreference("slow"), /unknown motion preference/);
});

/**
 * Capture PNG data, skipping where no GPU adapter is available and in the
 * WebAssembly build, which cannot render offscreen synchronously.
 */
function screenshot(t, running) {
  if (backend === "web") {
    t.skip("screenshots need the Node addon");
    return null;
  }
  try {
    return running.screenshotPng();
  } catch (error) {
    if (String(error?.message).includes("failed to acquire wgpu adapter")) {
      t.skip("no wgpu adapter is available on this machine");
      return null;
    }
    throw error;
  }
}

function pngSize(data) {
  assert.deepEqual([...data.subarray(0, 8)], [0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]);
  const view = new DataView(data.buffer, data.byteOffset, data.byteLength);
  return [view.getUint32(16), view.getUint32(20)];
}

test("screenshots render the window", async (t) => {
  const running = start(sui.button("Capture"), 160, 90);
  const data = screenshot(t, running);
  if (data === null) {
    return;
  }
  assert.deepEqual(pngSize(data), [160, 90]);

  const directory = await mkdtemp(join(tmpdir(), "sui-js-"));
  try {
    const path = join(directory, "window.png");
    await running.saveScreenshot(path);
    assert.deepEqual(pngSize(await readFile(path)), [160, 90]);
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});

test("unsized windows are captured at their content size", (t) => {
  const app = new sui.App().window(new sui.Window("No size").root(sui.button("Content")));
  const data = screenshot(t, app.start());
  if (data === null) {
    return;
  }
  const [width, height] = pngSize(data);
  assert.ok(width > 0 && width < 320);
  assert.ok(height > 0 && height < 200);
});
