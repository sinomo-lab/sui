// Real-window event-loop tests. Each case runs in its own process because
// winit allows one event loop per process. Enable with SUI_DESKTOP_TESTS=1
// on a machine with a display.
import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import { test } from "node:test";
import { fileURLToPath } from "node:url";
import { promisify } from "node:util";
import { desktop } from "./support.js";

const skip = !desktop && "opens desktop windows; set SUI_DESKTOP_TESTS=1";
const cases = fileURLToPath(new URL("desktop-cases.js", import.meta.url));

/** Run a case from desktop-cases.js and return its JSON result. */
async function runCase(name) {
  const { stdout } = await promisify(execFile)(process.execPath, [cases, name], {
    env: process.env,
    timeout: 60_000,
  });
  return JSON.parse(stdout.trim().split("\n").at(-1));
}

test("run() resolves after requestExit", { skip }, async () => {
  assert.equal(await runCase("request-exit"), "returned");
});

test("run() resolves when its signal aborts", { skip }, async () => {
  assert.equal(await runCase("abort-signal"), "returned");
});

test("a throwing onReady exits the app and rejects run()", { skip }, async () => {
  assert.equal(await runCase("throwing-on-ready"), "setup failed");
});

test("Node timers keep firing while a window is open", { skip }, async () => {
  const ticks = await runCase("timers");
  assert.ok(ticks >= 25, `only ${ticks} ticks of a 20 ms interval in about 1 s`);
});

test("run() rejects while another app is running", { skip }, async () => {
  assert.match(await runCase("run-twice"), /already running/);
});
