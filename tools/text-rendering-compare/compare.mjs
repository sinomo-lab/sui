import { createHash } from 'node:crypto';
import { spawnSync } from 'node:child_process';
import { createRequire } from 'node:module';
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import pixelmatch from 'pixelmatch';
import { chromium } from 'playwright';
import { compareTextRows } from './ink-stats.mjs';

const require = createRequire(import.meta.url);
const { PNG } = require('pngjs');

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const outputDir =
  process.env.SUI_TEXT_COMPARE_OUTPUT ??
  path.join(repoRoot, 'target', 'text-rendering-compare');
let width;
let height;
const dark = process.env.SUI_TEXT_COMPARE_SURFACE === 'dark';
let background;
const coveragePolicy = process.env.SUI_TEXT_COMPARE_COVERAGE ?? 'perceptual';
const renderMode = (process.env.SUI_TEXT_COMPARE_MODE ?? 'grayscale').toLowerCase();
const orderName = (process.env.SUI_TEXT_COMPARE_SUBPIXEL_ORDER ?? (renderMode === 'lcd' ? 'rgb' : 'none')).toLowerCase();
const subpixelOrder = orderName === 'off' ? 'none' : orderName;
if (!['grayscale', 'lcd'].includes(renderMode) || !['rgb', 'bgr', 'none'].includes(subpixelOrder)) {
  throw new Error('Expected mode grayscale/lcd and subpixel order rgb/bgr/none/off');
}
const browserChannel = process.env.SUI_TEXT_COMPARE_BROWSER ?? 'chrome';
const dpiScale = Number.parseFloat(process.env.SUI_TEXT_COMPARE_DPI_SCALE ?? '1');
if (!Number.isFinite(dpiScale) || dpiScale <= 0) {
  throw new Error(`invalid SUI_TEXT_COMPARE_DPI_SCALE: ${process.env.SUI_TEXT_COMPARE_DPI_SCALE}`);
}
const fontPath = process.env.SUI_TEXT_COMPARE_FONT ?? path.join(repoRoot, 'crates', 'sui-text', 'assets', 'NotoSans-Regular.ttf');

let samples;
let browserVersion;

function run(command, args) {
  const result = spawnSync(command, args, {
    cwd: repoRoot,
    encoding: 'utf8',
    stdio: ['ignore', 'pipe', 'pipe']
  });
  if (result.status !== 0) {
    throw new Error(
      `${command} ${args.join(' ')} failed with status ${result.status}\n${result.stdout}\n${result.stderr}`
    );
  }
  return result;
}

function readPng(file) {
  return PNG.sync.read(readFileSync(file));
}

function writePng(file, image) {
  writeFileSync(file, PNG.sync.write(image));
}

function channelDeltaStats(a, b) {
  let maxDelta = 0;
  let totalDelta = 0;

  for (let i = 0; i < a.data.length; i += 4) {
    for (let c = 0; c < 4; c += 1) {
      const delta = Math.abs(a.data[i + c] - b.data[i + c]);
      maxDelta = Math.max(maxDelta, delta);
      totalDelta += delta;
    }
  }

  return {
    maxChannelDelta: maxDelta,
    meanChannelDelta: totalDelta / a.data.length
  };
}

function detectsLcdEdges(image) {
  const probe = samples.find(sample => sample.text.startsWith('RGB edge probe'));
  if (!probe) return null;
  const linear = value => value <= 0.04045 ? value / 12.92 : ((value + 0.055) / 1.055) ** 2.4;
  const foreground = probe.color.slice(0, 3).map(value => linear(value / 255));
  const backdrop = background.map(value => linear(value / 255));
  for (let y = Math.floor(probe.y * dpiScale); y < Math.min(image.height, Math.ceil((probe.y + probe.lineHeight) * dpiScale)); y += 1) {
    for (let x = 0; x < image.width; x += 1) {
      const index = (y * image.width + x) * 4;
      const coverage = foreground.map((value, channel) =>
        (linear(image.data[index + channel] / 255) - backdrop[channel]) / (value - backdrop[channel]));
      if (Math.max(...coverage) - Math.min(...coverage) > 0.04) return true;
    }
  }
  return false;
}

function writeDiff(sui, browser, filename) {
  const diff = new PNG({ width: sui.width, height: sui.height });
  const count = pixelmatch(sui.data, browser.data, diff.data, sui.width, sui.height, {
    threshold: 0.12,
    includeAA: true,
    alpha: 0.25,
    diffColor: [255, 0, 96],
    diffColorAlt: [0, 128, 255]
  });
  writePng(path.join(outputDir, filename), diff);
  return count;
}

function comparisonSummary(sui, browser, diffPixels, rowStats) {
  const stats = channelDeltaStats(sui, browser);
  const totalPixels = sui.width * sui.height;
  return {
    dpiScale,
    requestedRenderMode: renderMode,
    suiLcdChromaticEdges: detectsLcdEdges(sui),
    browserLcdChromaticEdges: detectsLcdEdges(browser),
    subpixelOrder,
    hinting: process.env.SUI_TEXT_COMPARE_HINTING ?? 'slight',
    surface: dark ? 'dark' : 'light',
    browserChannel,
    browserVersion,
    fontSha256: createHash('sha256').update(readFileSync(fontPath)).digest('hex'),
    coveragePolicy,
    cssWidth: width,
    cssHeight: height,
    width: sui.width,
    height: sui.height,
    totalPixels,
    diffPixels,
    diffRatio: diffPixels / totalPixels,
    ...stats,
    ...rowStats
  };
}

async function writeBrowserReference() {
  const browserPath = path.join(outputDir, 'browser.png');
  const fontDataUrl = `data:font/ttf;base64,${readFileSync(fontPath).toString('base64')}`;
  const sampleHtml = samples
    .map(
      (sample) => `<div class="sample" style="
        left:${sample.x}px;
        top:${sample.y}px;
        width:${sample.width}px;
        height:${sample.lineHeight}px;
        font-size:${sample.fontSize}px;
        line-height:${sample.lineHeight}px;
        color:rgba(${sample.color.join(',')});
      ">${sample.text}</div>`
    )
    .join('\n');

  const html = `<!doctype html>
<html>
<head>
  <meta charset="utf-8">
  <style>
    @font-face {
      font-family: "SuiNotoSans";
      src: url("${fontDataUrl}") format("truetype");
      font-weight: 400;
      font-style: normal;
      font-display: block;
    }
    html, body {
      margin: 0;
      width: ${width}px;
      height: ${height}px;
      overflow: hidden;
      background: rgb(${background.join(',')});
    }
    body {
      font-family: "SuiNotoSans", sans-serif;
      font-synthesis: none;
      font-kerning: normal;
      font-variant-ligatures: normal;
    }
    .sample {
      position: absolute;
      white-space: nowrap;
      overflow: hidden;
      letter-spacing: 0;
      word-spacing: 0;
    }
  </style>
</head>
<body>${sampleHtml}</body>
</html>`;

  const browser = await chromium.launch({ channel: browserChannel });
  browserVersion = browser.version();
  const page = await browser.newPage({
    viewport: { width, height },
    deviceScaleFactor: dpiScale
  });
  await page.setContent(html, { waitUntil: 'load' });
  await page.evaluate(async () => {
    await document.fonts.ready;
  });
  const fontLoaded = await page.evaluate(() => document.fonts.check('16px SuiNotoSans'));
  if (!fontLoaded) {
    await browser.close();
    throw new Error('Chromium did not load the embedded SuiNotoSans font');
  }
  await page.screenshot({ path: browserPath, animations: 'disabled', caret: 'hide' });
  await browser.close();
  return browserPath;
}

async function main() {
  mkdirSync(outputDir, { recursive: true });

  run('cargo', [
    'run',
    '-p',
    'sinomo-ui-demo',
    '--bin',
    'sui-text-render-snapshot',
    '--',
    '--output',
    outputDir
  ]);

  const manifest = JSON.parse(readFileSync(path.join(outputDir, 'samples.json'), 'utf8'));
  ({ width, height, background, samples } = manifest);
  const browserPath = await writeBrowserReference();
  const suiPath = path.join(outputDir, 'sui.png');
  const summaryPath = path.join(outputDir, 'summary.json');

  const sui = readPng(suiPath);
  const browser = readPng(browserPath);
  if (sui.width !== browser.width || sui.height !== browser.height) {
    throw new Error(
      `image dimensions differ: sui=${sui.width}x${sui.height} browser=${browser.width}x${browser.height}`
    );
  }

  const { alignedSui, alignedBrowser, ...rowStats } = compareTextRows(sui, browser, {
    samples, background, dpiScale
  });
  const diffPixels = writeDiff(sui, browser, 'diff.png');
  writePng(path.join(outputDir, 'aligned-sui.png'), alignedSui);
  writePng(path.join(outputDir, 'aligned-browser.png'), alignedBrowser);
  const alignedDiffPixels = writeDiff(alignedSui, alignedBrowser, 'aligned-diff.png');

  const summary = {
    ...comparisonSummary(sui, browser, diffPixels, rowStats),
    alignedImageStats: {
      width: alignedSui.width,
      height: alignedSui.height,
      diffPixels: alignedDiffPixels,
      diffRatio: alignedDiffPixels / (alignedSui.width * alignedSui.height)
    }
  };
  writeFileSync(summaryPath, `${JSON.stringify(summary, null, 2)}\n`);
  console.log(JSON.stringify({ ...summary, outputDir }, null, 2));
}

main().catch((error) => {
  console.error(error);
  process.exit(1);
});
