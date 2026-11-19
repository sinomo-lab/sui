// The package: version, exported names, and runnable examples.
import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import { readdirSync, readFileSync } from "node:fs";
import { test } from "node:test";
import { join } from "node:path";
import { promisify } from "node:util";
import { backend, packageRoot, sui } from "./support.js";

const packageJson = JSON.parse(readFileSync(join(packageRoot, "package.json"), "utf8"));

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
const examples = readdirSync(join(packageRoot, "examples"))
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
