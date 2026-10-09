// Run the test suite on one backend: `node tests/run.js [node|web]`.
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const backend = process.argv[2] ?? "node";
const result = spawnSync(process.execPath, ["--test", "tests/"], {
  cwd: fileURLToPath(new URL("..", import.meta.url)),
  env: { ...process.env, SUI_JS_BACKEND: backend },
  stdio: "inherit",
});
process.exit(result.status ?? 1);
