import { createHash } from "node:crypto";
import { readdir, readFile, stat, writeFile } from "node:fs/promises";
import { dirname, extname, join, relative, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";
import { promisify } from "node:util";
import {
  brotliCompress,
  constants as zlibConstants,
  gzip,
} from "node:zlib";

const brotli = promisify(brotliCompress);
const gzipFile = promisify(gzip);
const scriptDirectory = dirname(fileURLToPath(import.meta.url));
const distDirectory = resolve(process.argv[2] ?? join(scriptDirectory, "dist"));
const compressibleExtensions = new Set([
  ".css",
  ".html",
  ".js",
  ".json",
  ".otf",
  ".svg",
  ".ttf",
  ".wasm",
]);
const textExtensions = new Set([".css", ".html", ".js", ".json", ".svg"]);
const fontExtensions = new Set([".otf", ".ttf"]);

const toPosixPath = (path) => path.split(sep).join("/");

async function filesUnder(directory) {
  const entries = await readdir(directory, { withFileTypes: true });
  const files = [];
  for (const entry of entries) {
    const path = join(directory, entry.name);
    if (entry.isDirectory()) {
      files.push(...(await filesUnder(path)));
    } else if (entry.isFile()) {
      files.push(path);
    }
  }
  return files;
}

function brotliMode(extension) {
  if (textExtensions.has(extension)) return zlibConstants.BROTLI_MODE_TEXT;
  if (fontExtensions.has(extension)) return zlibConstants.BROTLI_MODE_FONT;
  return zlibConstants.BROTLI_MODE_GENERIC;
}

function brotliQuality(extension) {
  // Quality 11 materially helps Wasm and JavaScript. Font containers see very
  // little additional reduction above 9, but take much longer to encode.
  return fontExtensions.has(extension) ? 9 : 11;
}

function immutableHeaders() {
  return `# Used by hosts that support the Netlify/Cloudflare Pages _headers format.
/index.html
  Cache-Control: no-cache

/compression-manifest.json
  Cache-Control: no-cache

/compression-loader.js
  Cache-Control: public, max-age=600, must-revalidate

/asset-cache-worker.js
  Cache-Control: no-cache

/sinomo-ui-demo-*
  Cache-Control: public, max-age=31536000, immutable

/NotoSansCJKsc-Regular.otf*
  Cache-Control: public, max-age=31536000, immutable

/NotoColorEmoji.ttf*
  Cache-Control: public, max-age=31536000, immutable

/sui-logo.svg
  Cache-Control: public, max-age=86400
`;
}

const distStat = await stat(distDirectory).catch(() => null);
if (!distStat?.isDirectory()) {
  throw new Error(`Web distribution directory does not exist: ${distDirectory}`);
}

const files = (await filesUnder(distDirectory))
  .filter((path) => {
    const extension = extname(path);
    return (
      path !== join(distDirectory, "compression-manifest.json") &&
      compressibleExtensions.has(extension) &&
      !path.endsWith(".br") &&
      !path.endsWith(".gz")
    );
  })
  .sort();
const assets = {};
const revisionParts = [];

for (const path of files) {
  const source = await readFile(path);
  if (source.byteLength < 1024) {
    continue;
  }

  const extension = extname(path);
  const relativePath = toPosixPath(relative(distDirectory, path));
  const [brotliBytes, gzipBytes] = await Promise.all([
    brotli(source, {
      params: {
        [zlibConstants.BROTLI_PARAM_MODE]: brotliMode(extension),
        [zlibConstants.BROTLI_PARAM_QUALITY]: brotliQuality(extension),
        [zlibConstants.BROTLI_PARAM_SIZE_HINT]: source.byteLength,
      },
    }),
    gzipFile(source, { level: 9 }),
  ]);

  const entry = {
    bytes: source.byteLength,
    sha256: createHash("sha256").update(source).digest("hex"),
  };
  if (brotliBytes.byteLength < source.byteLength) {
    await writeFile(`${path}.br`, brotliBytes);
    entry.br = `${relativePath}.br`;
    entry.brBytes = brotliBytes.byteLength;
  }
  if (gzipBytes.byteLength < source.byteLength) {
    await writeFile(`${path}.gz`, gzipBytes);
    entry.gzip = `${relativePath}.gz`;
    entry.gzipBytes = gzipBytes.byteLength;
  }
  if (entry.br || entry.gzip) {
    assets[relativePath] = entry;
    revisionParts.push(`${relativePath}:${entry.sha256}`);
  }
}

const revision = createHash("sha256")
  .update(revisionParts.join("\n"))
  .digest("hex")
  .slice(0, 16);
const manifest = {
  version: 1,
  revision,
  assets,
};
await writeFile(
  join(distDirectory, "compression-manifest.json"),
  `${JSON.stringify(manifest, null, 2)}\n`,
);
await writeFile(join(distDirectory, "_headers"), immutableHeaders());

for (const [path, entry] of Object.entries(assets)) {
  const br = entry.brBytes ? `${entry.brBytes} B br` : "no br";
  const gz = entry.gzipBytes ? `${entry.gzipBytes} B gzip` : "no gzip";
  console.log(`${path}: ${entry.bytes} B -> ${br}, ${gz}`);
}
console.log(`Compression manifest revision: ${revision}`);
