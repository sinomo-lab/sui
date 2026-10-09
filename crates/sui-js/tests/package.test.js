// The package: version, exported names, and runnable examples.
import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import { readdirSync, readFileSync } from "node:fs";
import { test } from "node:test";
import { fileURLToPath } from "node:url";
import { promisify } from "node:util";
import { backend, sui } from "./support.js";

const packageRoot = fileURLToPath(new URL("..", import.meta.url));
const packageJson = JSON.parse(readFileSync(new URL("../package.json", import.meta.url), "utf8"));

test("version matches package.json", () => {
  assert.equal(sui.version, packageJson.version);
});

test("widget factories are camelCase only", () => {
  assert.equal(typeof sui.button, "function");
  assert.equal(typeof sui.virtualScrollView, "function");
  assert.equal(sui.Button, undefined);
  assert.equal(sui.VirtualScrollView, undefined);
  assert.equal(sui.PumpedApp, undefined);
});

// `window.js` opens a real window.
const examples = readdirSync(new URL("../examples", import.meta.url))
  .filter((name) => name.endsWith(".js") && name !== "window.js")
  .sort();

// Examples run on the Node addon.
for (const example of examples) {
  test(`example ${example} runs`, { skip: backend !== "node" }, async () => {
    await promisify(execFile)(process.execPath, [`examples/${example}`], {
      cwd: packageRoot,
      env: { ...process.env, SUI_JS_NATIVE: process.env.SUI_JS_NATIVE },
      timeout: 120_000,
    });
  });
}
