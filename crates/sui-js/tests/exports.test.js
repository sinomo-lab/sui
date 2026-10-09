// index.d.ts and the runtime module agree on names and methods.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { sui } from "./support.js";

const declarations = readFileSync(new URL("../index.d.ts", import.meta.url), "utf8");

/** Top-level `export class|function|const NAME` declarations. */
function declaredExports() {
  const pattern = /^export\s+(?:declare\s+)?(class|function|const)\s+([A-Za-z_$][\w$]*)/gm;
  return new Map([...declarations.matchAll(pattern)].map(([, kind, name]) => [name, kind]));
}

/** Method names declared in each `export class` body, split by static. */
function declaredMethods() {
  const classes = new Map();
  const lines = declarations.split(/\r?\n/);
  let current = null;
  for (const line of lines) {
    const start = /^export\s+class\s+(\w+)/.exec(line);
    if (start) {
      current = { instance: new Set(), statics: new Set() };
      classes.set(start[1], current);
      continue;
    }
    if (current === null) {
      continue;
    }
    if (/^}/.test(line)) {
      current = null;
      continue;
    }
    // Members are indented two spaces; deeper lines are parameter lists.
    const member = /^ {2}(static\s+)?([A-Za-z_$][\w$]*)\s*[(<]/.exec(line);
    if (member && member[2] !== "constructor") {
      (member[1] ? current.statics : current.instance).add(member[2]);
    }
  }
  return classes;
}

test("every declared export exists at runtime", () => {
  const missing = [...declaredExports()]
    .filter(([name]) => sui[name] === undefined)
    .map(([name, kind]) => `${kind} ${name}`);
  assert.deepEqual(missing, [], `declared in index.d.ts but undefined: ${missing.join(", ")}`);
});

test("every runtime export is declared", () => {
  const declared = declaredExports();
  const undeclared = Object.keys(sui).filter((name) => !declared.has(name));
  assert.deepEqual(undeclared, [], `exported but not declared: ${undeclared.join(", ")}`);
});

test("declared class methods exist", () => {
  const mismatches = [];
  for (const [name, { instance, statics }] of declaredMethods()) {
    const Class = sui[name];
    if (typeof Class !== "function") {
      mismatches.push(`${name} is not a class`);
      continue;
    }
    for (const method of instance) {
      if (typeof Class.prototype[method] !== "function") {
        mismatches.push(`${name}.prototype.${method}`);
      }
    }
    for (const method of statics) {
      if (typeof Class[method] !== "function") {
        mismatches.push(`${name}.${method}`);
      }
    }
  }
  assert.deepEqual(mismatches, [], `declared methods missing at runtime:\n${mismatches.join("\n")}`);
});
