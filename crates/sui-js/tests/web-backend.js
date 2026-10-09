// Load the WebAssembly build in Node for tests, the way web.js loads it in a
// browser: the same JavaScript layer over the other native module.
import { readFile, writeFile } from "node:fs/promises";
import * as wasm from "../web/sui_web.js";
import { createApi } from "../runtime.js";

wasm.initSync({
  module: await readFile(new URL("../web/sui_web_bg.wasm", import.meta.url)),
});
wasm.__registerClasses(wasm);

export default createApi(wasm, {
  start() {
    throw new Error("running an app needs a browser");
  },
  writeFile,
});
