// Font registration: TrueType, WOFF, and WOFF2 data, family names, and URLs.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { createServer } from "node:http";
import { test } from "node:test";
import { sui } from "./support.js";

const asset = (name) =>
  readFileSync(new URL(`../../sui-text/assets/${name}`, import.meta.url));

const FONTS = {
  ttf: asset("NotoSans-Regular.ttf"),
  woff: asset("NotoSans-Ag-subset.woff"),
  woff2: asset("NotoSans-Ag-subset.woff2"),
};

test("fonts register from TrueType, WOFF, and WOFF2 data", () => {
  const app = new sui.App();
  for (const [format, data] of Object.entries(FONTS)) {
    const font = app.fontBytes(new Uint8Array(data));
    assert.equal(font.family, "Noto Sans", format);
  }
  assert.equal(app.fontResourceCount, 3);
});

test("registered fonts render text", () => {
  const app = new sui.App();
  const font = app.fontBytes(new Uint8Array(FONTS.woff2));
  const running = app
    .window(new sui.Window("Fonts").root(sui.richText([new sui.TextSpan("Ag", undefined, 18, undefined, font)])))
    .start();
  assert.ok(running.render().commandCount > 0);
});

test("invalid font data is rejected", () => {
  const app = new sui.App();
  assert.throws(() => app.fontBytes(new Uint8Array([0, 1, 2, 3])), { code: "InvalidArg" });
  const truncated = new Uint8Array(FONTS.woff2.subarray(0, 64));
  assert.throws(() => app.fontBytes(truncated), /WOFF/);
  assert.equal(app.fontResourceCount, 0);
});

test("handles made from an id have no family", () => {
  assert.equal(new sui.FontHandle("7").family, null);
});

test("fontUrl fetches and registers a font", async (t) => {
  const server = createServer((request, response) => {
    if (request.url === "/font.woff2") {
      response.writeHead(200, { "content-type": "font/woff2" });
      response.end(FONTS.woff2);
    } else {
      response.writeHead(404);
      response.end();
    }
  });
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  t.after(() => server.close());
  const base = `http://127.0.0.1:${server.address().port}`;

  const app = new sui.App();
  const font = await app.fontUrl(`${base}/font.woff2`);
  assert.equal(font.family, "Noto Sans");
  await assert.rejects(app.fontUrl(new URL("/missing.woff2", base)), /404/);
  assert.equal(app.fontResourceCount, 1);
});
