// Run the test suite on one backend: `node tests/run.js [node|web]`.
import { spawnSync } from "node:child_process";
import { readdirSync } from "node:fs";
import { fileURLToPath } from "node:url";

const backend = process.argv[2] ?? "node";
// Name the files: Node 24's `--test` loads a directory argument as a module
// instead of searching it.
const files = readdirSync(new URL(".", import.meta.url))
  .filter((name) => name.endsWith(".test.js"))
  .sort()
  .map((name) => `tests/${name}`);
const result = spawnSync(process.execPath, ["--test", ...files], {
  cwd: fileURLToPath(new URL("..", import.meta.url)),
  env: { ...process.env, SUI_JS_BACKEND: backend },
  stdio: "inherit",
});
process.exit(result.status ?? 1);
