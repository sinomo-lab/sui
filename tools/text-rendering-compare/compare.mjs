import { createHash } from 'node:crypto';
import { spawnSync } from 'node:child_process';
import { createRequire } from 'node:module';
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import pixelmatch from 'pixelmatch';
import { chromium } from 'playwright';

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

function luminance(data, index) {
  return 0.2126 * data[index] + 0.7152 * data[index + 1] + 0.0722 * data[index + 2];
}

function darkness(data, index) {
  return Math.abs(luminance(data, index) - (0.2126*background[0] + 0.7152*background[1] + 0.0722*background[2]));
}

function rowInkStats(sui, browser) {
  return samples.map((sample) => {
    const top = Math.max(0, Math.floor((sample.y - 6) * dpiScale));
    const bottom = Math.min(
      sui.height,
      Math.ceil((sample.y + sample.lineHeight + 6) * dpiScale)
    );
    let unionPixels = 0;
    let differingInkPixels = 0;
    let suiInkMass = 0;
    let browserInkMass = 0;
    let channelError = 0;

    for (let y = top; y < bottom; y += 1) {
      for (let x = 0; x < sui.width; x += 1) {
        const index = (y * sui.width + x) * 4;
        const suiDarkness = darkness(sui.data, index);
        const browserDarkness = darkness(browser.data, index);
        if (suiDarkness <= 2 && browserDarkness <= 2) {
          continue;
        }

        unionPixels += 1;
        for (let c = 0; c < 3; c += 1) {
          channelError += Math.abs(sui.data[index + c] - browser.data[index + c]);
        }
        suiInkMass += suiDarkness;
        browserInkMass += browserDarkness;
        if (Math.abs(suiDarkness - browserDarkness) > 12) {
          differingInkPixels += 1;
        }
      }
    }

    return {
      text: sample.text,
      unionPixels,
      differingInkPixels,
      differingInkRatio: unionPixels === 0 ? 0 : differingInkPixels / unionPixels,
      suiInkMass: Math.round(suiInkMass),
      browserInkMass: Math.round(browserInkMass),
      inkMassRatio: browserInkMass <= 0 ? 1 : suiInkMass / browserInkMass,
      meanInkChannelError: unionPixels === 0 ? 0 : channelError / (unionPixels * 3)
    };
  });
}

function comparisonSummary(sui, browser, diffPixels) {
  const stats = channelDeltaStats(sui, browser);
  const totalPixels = sui.width * sui.height;
  return {
    dpiScale,
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
    rowInkStats: rowInkStats(sui, browser)
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
  const diffPath = path.join(outputDir, 'diff.png');
  const summaryPath = path.join(outputDir, 'summary.json');

  const sui = readPng(suiPath);
  const browser = readPng(browserPath);
  if (sui.width !== browser.width || sui.height !== browser.height) {
    throw new Error(
      `image dimensions differ: sui=${sui.width}x${sui.height} browser=${browser.width}x${browser.height}`
    );
  }

  const diff = new PNG({ width: sui.width, height: sui.height });
  const diffPixels = pixelmatch(sui.data, browser.data, diff.data, sui.width, sui.height, {
    threshold: 0.12,
    includeAA: true,
    alpha: 0.25,
    diffColor: [255, 0, 96],
    diffColorAlt: [0, 128, 255]
  });
  writePng(diffPath, diff);

  const summary = comparisonSummary(sui, browser, diffPixels);
  writeFileSync(summaryPath, `${JSON.stringify(summary, null, 2)}\n`);
  console.log(JSON.stringify({ ...summary, outputDir }, null, 2));
}

main().catch((error) => {
  console.error(error);
  process.exit(1);
});
