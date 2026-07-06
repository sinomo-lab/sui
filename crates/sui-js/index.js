"use strict";

const path = require("path");

const platform = process.platform;
const arch = process.arch;
const abi = process.versions.modules;

const candidates = [
  "sui_js.node",
  "sui-js.node",
  `sui_js.${platform}-${arch}.node`,
  `sui-js.${platform}-${arch}.node`,
  `sui_js.${platform}-${arch}-abi${abi}.node`,
  `sui-js.${platform}-${arch}-abi${abi}.node`,
];

const errors = [];

for (const candidate of candidates) {
  const nativePath = path.join(__dirname, candidate);
  try {
    module.exports = require(nativePath);
    return;
  } catch (error) {
    if (error && error.code !== "MODULE_NOT_FOUND") {
      throw error;
    }
    errors.push(nativePath);
  }
}

throw new Error(
  [
    "Could not load the SUI native binding.",
    "Build it from crates/sui-js with `napi build --platform --release`",
    "or place the generated .node file next to index.js.",
    `Tried: ${errors.join(", ")}`,
  ].join(" ")
);
