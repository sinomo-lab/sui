// Shared test setup: load the package from the cargo-built native library.
import { existsSync } from "node:fs";
import { fileURLToPath } from "node:url";

/**
 * Which native module the tests run on: `"node"` (the addon, the default) or
 * `"web"` (the WebAssembly build from `cargo xtask web build`, run in Node).
 */
export const backend = process.env.SUI_JS_BACKEND ?? "node";

async function loadNode() {
  if (!process.env.SUI_JS_NATIVE) {
    const library = {
      darwin: "libsui_js.dylib",
      linux: "libsui_js.so",
      win32: "sui_js.dll",
    }[process.platform];
    if (!library) {
      throw new Error(`unsupported native test platform: ${process.platform}`);
    }
    const path = fileURLToPath(new URL(`../../../target/debug/${library}`, import.meta.url));
    if (!existsSync(path)) {
      throw new Error(`native SUI binding not found at ${path}; run cargo build --package sinomo-ui-js`);
    }
    process.env.SUI_JS_NATIVE = path;
  }
  return import("../index.js");
}

async function loadWeb() {
  if (!existsSync(fileURLToPath(new URL("../web/sui_web_bg.wasm", import.meta.url)))) {
    throw new Error("the WebAssembly build is missing; run cargo xtask web build");
  }
  return (await import("./web-backend.js")).default;
}

export const sui = backend === "web" ? await loadWeb() : await loadNode();

/** Whether tests may open real desktop windows. */
export const desktop = backend === "node" && process.env.SUI_DESKTOP_TESTS === "1";

/** Start `widget` in a one-window app without a platform. */
export function start(widget, title = "Test") {
  const app = new sui.App().window(new sui.Window(title).root(widget));
  const running = app.start();
  running.render();
  return running;
}

/** Collect errors reported to the error handler while `body` runs. */
export async function capturingErrors(body) {
  const errors = [];
  sui.setErrorHandler((error) => errors.push(error));
  try {
    await body(errors);
  } finally {
    sui.setErrorHandler(null);
  }
  return errors;
}
