import assert from 'node:assert/strict';
import test from 'node:test';
import { compareTextRows } from './ink-stats.mjs';

const white = [255, 255, 255];
const points = [[5, 8], [5, 9], [5, 10], [6, 10], [7, 10], [10, 9], [11, 9], [10, 10], [10, 11], [12, 11]];
const sample = { text: 'colored glyphs', y: 4, lineHeight: 8 };

function picture(background, marks = [], color = [0, 0, 0]) {
  const width = 24;
  const height = 56;
  const data = Buffer.alloc(width * height * 4);
  for (let i = 0; i < width * height; i += 1) data.set([...background, 255], i * 4);
  for (const [x, y] of marks) data.set([...color, 255], (y * width + x) * 4);
  return { width, height, data };
}

function compare(sui, browser, background = white, samples = [sample], dpiScale = 1) {
  return compareTextRows(sui, browser, { background, samples, dpiScale });
}

test('integer alignment recovers colored pixels on both surfaces at physical DPI', () => {
  for (const background of [white, [18, 22, 31]]) {
    const color = background === white ? [8, 124, 164] : [125, 211, 252];
    for (const dpiScale of [1, 1.5, 2]) {
      for (const [dx, dy] of [[0, 1], [1, -1], [-2, 2]]) {
        const browser = picture(background, points, color);
        const sui = picture(background, points.map(([x, y]) => [x + dx, y + dy]), color);
        const original = Buffer.from(sui.data);
        const result = compare(sui, browser, background, [sample], dpiScale);
        const row = result.alignedRowInkStats[0];
        assert.equal(row.alignment.suiShiftX, -dx || 0);
        assert.equal(row.alignment.suiShiftY, -dy || 0);
        assert.ok(result.rowInkStats[0].absoluteInkChannelError > 0);
        assert.equal(row.meanInkChannelError, 0);
        assert.equal(row.inkMassRatio, 1);
        assert.equal(row.absoluteErrorReduction, 1);
        assert.deepEqual(result.alignedSui.data, result.alignedBrowser.data);
        assert.deepEqual(sui.data, original, 'the captured image must not be modified');
      }
    }
  }
});

test('identical and empty rows choose no movement and finite zero errors', () => {
  for (const marks of [[], points]) {
    const source = picture(white, marks);
    const result = compare(source, source);
    const row = result.alignedRowInkStats[0];
    assert.equal(row.alignment.suiShiftX, 0);
    assert.equal(row.alignment.suiShiftY, 0);
    assert.equal(row.meanInkChannelError, 0);
    assert.equal(row.absoluteErrorReduction, 0);
    assert.equal(result.alignment.shiftedRows, 0);
    assert.equal(result.textQuality.aligned.meanInkChannelError, 0);
  }
});

test('alignment preserves color differences instead of correcting them', () => {
  const browser = picture(white, points, [17, 126, 171]);
  const sui = picture(white, points.map(([x, y]) => [x, y + 1]), [28, 146, 200]);
  const row = compare(sui, browser).alignedRowInkStats[0];
  assert.equal(row.alignment.suiShiftY, -1);
  assert.equal(row.meanInkChannelError, 20);
  assert.equal(row.unionPixels, points.length);
});

test('one translation per row cannot hide internal glyph spacing differences', () => {
  const browser = picture(white, points);
  const sui = picture(white, points.map(([x, y]) => [x >= 10 ? x + 1 : x, y + 1]));
  const row = compare(sui, browser).alignedRowInkStats[0];
  assert.ok(row.absoluteInkChannelError > 0);
  assert.ok(row.absoluteErrorReduction < 1);
});

test('independent rows retain their own offsets and image-sheet coordinates', () => {
  const browser = picture(white, [...points, ...points.map(([x, y]) => [x, y + 30])]);
  const sui = picture(white, [
    ...points.map(([x, y]) => [x, y + 1]),
    ...points.map(([x, y]) => [x - 1, y + 29])
  ]);
  const result = compare(sui, browser, white, [sample, { ...sample, y: 34 }]);
  assert.deepEqual(result.alignedRowInkStats.map(row => [row.alignment.suiShiftX, row.alignment.suiShiftY]), [[0, -1], [1, 1]]);
  assert.equal(result.alignment.shiftedRows, 2);
  const [first, second] = result.alignedRowInkStats.map(row => row.alignedImageRect);
  assert.equal(first.y, 0);
  assert.equal(second.y, first.height);
  assert.equal(second.y + second.height, result.alignedSui.height);
  assert.deepEqual(result.alignedSui.data, result.alignedBrowser.data);
});

test('padding retains mismatching ink shifted outside the original image', () => {
  const browser = picture(white, [[2, 8]]);
  const sui = picture(white, [[1, 8], [23, 8]]);
  const result = compare(sui, browser);
  const row = result.alignedRowInkStats[0];
  assert.equal(row.alignment.suiShiftX, 1);
  assert.equal(row.absoluteInkChannelError, 255 * 3);
  assert.equal(row.unionPixels, 2);
  assert.notDeepEqual(result.alignedSui.data, result.alignedBrowser.data);
});

test('search minimizes absolute error instead of enlarging the MAE denominator', () => {
  const browser = picture(white, [[8, 8]]);
  const sui = picture(white, [[8, 8]], [200, 200, 200]);
  const row = compare(sui, browser).alignedRowInkStats[0];
  // Misalignment would give a lower MAE (155), but greater total RGB error.
  assert.equal(row.alignment.suiShiftX, 0);
  assert.equal(row.alignment.suiShiftY, 0);
  assert.equal(row.meanInkChannelError, 200);
  assert.equal(row.absoluteInkChannelError, 600);
});

test('search remains bounded and reports a best offset at the boundary', () => {
  const block = Array.from({ length: 32 }, (_, i) => [4 + i % 8, 8 + Math.floor(i / 8)]);
  const browser = picture(white, block);
  const sui = picture(white, block.map(([x, y]) => [x + 3, y]));
  const result = compare(sui, browser);
  const row = result.alignedRowInkStats[0];
  assert.equal(row.alignment.suiShiftX, -2);
  assert.equal(row.alignment.suiShiftY, 0);
  assert.equal(row.alignment.atSearchBoundary, true);
  assert.equal(result.alignment.boundaryRows, 1);
  assert.ok(row.absoluteInkChannelError > 0);
});
