// Stage the directories that are published to npm.
//
//   node scripts/npm-stage.mjs platform <rust-target> <library> <out-dir>
//   node scripts/npm-stage.mjs main <out-dir>
//
// `platform` writes `<out-dir>/sinomo-ui-<platform>`, a package holding one
// prebuilt Node addon. `main` writes `<out-dir>/sinomo-ui`, the package users
// install: the JavaScript, the declarations, the WebAssembly build, and an
// optional dependency on every platform package at the same version, so npm
// installs only the addon for the host.
//
// The checked-in package.json stays free of the optional dependencies, so a
// source checkout never asks the registry for them.
import { cpSync, existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const packageRoot = fileURLToPath(new URL("..", import.meta.url));
const workspaceRoot = resolve(packageRoot, "../..");
const manifest = JSON.parse(readFileSync(join(packageRoot, "package.json"), "utf8"));

/** Rust target → the npm platform package built from it. */
export const platforms = {
  "x86_64-unknown-linux-gnu": { suffix: "linux-x64-gnu", os: "linux", cpu: "x64", libc: "glibc" },
  "aarch64-unknown-linux-gnu": { suffix: "linux-arm64-gnu", os: "linux", cpu: "arm64", libc: "glibc" },
  "aarch64-apple-darwin": { suffix: "darwin-arm64", os: "darwin", cpu: "arm64" },
  "x86_64-apple-darwin": { suffix: "darwin-x64", os: "darwin", cpu: "x64" },
  "x86_64-pc-windows-msvc": { suffix: "win32-x64-msvc", os: "win32", cpu: "x64" },
};

const platformName = (platform) => `${manifest.name}-${platform.suffix}`;

function fail(message) {
  console.error(message);
  process.exit(1);
}

function freshDirectory(path) {
  rmSync(path, { recursive: true, force: true });
  mkdirSync(path, { recursive: true });
}

function writeManifest(directory, contents) {
  writeFileSync(join(directory, "package.json"), `${JSON.stringify(contents, null, 2)}\n`);
}

function stagePlatform(target, library, outDir) {
  const platform = platforms[target];
  if (!platform) {
    fail(`unknown target ${target}; expected one of ${Object.keys(platforms).join(", ")}`);
  }
  if (!existsSync(library)) {
    fail(`native library not found: ${library}`);
  }
  const name = platformName(platform);
  const directory = join(outDir, name);
  freshDirectory(directory);
  cpSync(library, join(directory, "sui_js.node"));
  cpSync(join(workspaceRoot, "LICENSE"), join(directory, "LICENSE"));
  writeFileSync(
    join(directory, "README.md"),
    `# ${name}\n\nThe prebuilt ${platform.os} ${platform.cpu} Node addon of ` +
      `[\`${manifest.name}\`](https://www.npmjs.com/package/${manifest.name}). ` +
      `Install \`${manifest.name}\` instead; it depends on this package.\n`,
  );
  writeManifest(directory, {
    name,
    version: manifest.version,
    description: `Prebuilt ${platform.os} ${platform.cpu} Node addon for ${manifest.name}.`,
    license: manifest.license,
    homepage: manifest.homepage,
    repository: manifest.repository,
    bugs: manifest.bugs,
    main: "sui_js.node",
    files: ["sui_js.node"],
    os: [platform.os],
    cpu: [platform.cpu],
    ...(platform.libc ? { libc: [platform.libc] } : {}),
    engines: manifest.engines,
  });
  console.log(`staged ${directory}`);
}

function stageMain(outDir) {
  const directory = join(outDir, manifest.name);
  freshDirectory(directory);
  // Prebuilt addons come from the platform packages, never from this one.
  const files = manifest.files.filter((entry) => entry !== "*.node");
  for (const entry of files) {
    const source = join(packageRoot, entry);
    if (!existsSync(source)) {
      fail(`${entry} is missing; build the WebAssembly module with cargo xtask web build --release`);
    }
    const destination = join(directory, entry);
    mkdirSync(dirname(destination), { recursive: true });
    cpSync(source, destination, { recursive: true });
  }
  cpSync(join(workspaceRoot, "LICENSE"), join(directory, "LICENSE"));
  const { scripts: _scripts, ...published } = manifest;
  writeManifest(directory, {
    ...published,
    files,
    optionalDependencies: Object.fromEntries(
      Object.values(platforms).map((platform) => [platformName(platform), manifest.version]),
    ),
  });
  console.log(`staged ${directory}`);
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const [command, ...args] = process.argv.slice(2);
  if (command === "platform" && args.length === 3) {
    stagePlatform(args[0], resolve(args[1]), resolve(args[2]));
  } else if (command === "main" && args.length === 1) {
    stageMain(resolve(args[0]));
  } else {
    fail(
      "usage: npm-stage.mjs platform <rust-target> <library> <out-dir>\n" +
        "       npm-stage.mjs main <out-dir>",
    );
  }
}
