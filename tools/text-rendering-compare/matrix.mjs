// Runs compare.mjs over surfaces x scales x modes and prints one table, so a
// change can be judged across the whole matrix instead of one capture.
import { spawnSync } from 'node:child_process';
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const here = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(here, '../..');
const list = (name, fallback) =>
  (process.env[name] ?? fallback).split(',').map(value => value.trim()).filter(Boolean);
const surfaces = list('SUI_TEXT_COMPARE_MATRIX_SURFACES', 'light,dark');
const scales = list('SUI_TEXT_COMPARE_MATRIX_SCALES', '1,1.25,1.5,2');
const modes = list('SUI_TEXT_COMPARE_MATRIX_MODES', 'grayscale,lcd');
const outputRoot =
  process.env.SUI_TEXT_COMPARE_OUTPUT ??
  path.join(repoRoot, 'target', 'text-rendering-compare', 'matrix');

const runs = [];
for (const mode of modes) {
  for (const surface of surfaces) {
    for (const scale of scales) {
      const name = `${surface}-${scale}x-${mode}`;
      const outputDir = path.join(outputRoot, name);
      mkdirSync(outputDir, { recursive: true });
      process.stderr.write(`comparing ${name}\n`);
      const result = spawnSync(process.execPath, [path.join(here, 'compare.mjs')], {
        cwd: repoRoot,
        encoding: 'utf8',
        env: {
          ...process.env,
          SUI_TEXT_COMPARE_SURFACE: surface,
          SUI_TEXT_COMPARE_DPI_SCALE: scale,
          SUI_TEXT_COMPARE_MODE: mode,
          SUI_TEXT_COMPARE_OUTPUT: outputDir
        }
      });
      if (result.status !== 0) {
        throw new Error(`${name} failed\n${result.stdout}\n${result.stderr}`);
      }
      const summary = JSON.parse(readFileSync(path.join(outputDir, 'summary.json'), 'utf8'));
      runs.push({ name, summary });
    }
  }
}

const mode = lcd => lcd === null ? '?' : lcd ? 'lcd' : 'gray';
const shifts = rows => Object.entries(rows)
  .sort(([a], [b]) => Number(a) - Number(b))
  .map(([shift, count]) => `${shift}:${count}`)
  .join(' ');
const table = runs.map(({ name, summary }) => {
  const { raw, aligned } = summary.textQuality;
  return {
    run: name,
    'sui/browser': `${mode(summary.suiLcdChromaticEdges)}/${mode(summary.browserLcdChromaticEdges)}`,
    weight: aligned.inkMassRatio.toFixed(3),
    'differing ink': `${(aligned.differingInkRatio * 100).toFixed(1)}%`,
    'error raw': raw.meanInkChannelError.toFixed(1),
    'error aligned': aligned.meanInkChannelError.toFixed(1),
    'y shifts': shifts(summary.alignment.suiShiftYRows),
    'x shifts': shifts(summary.alignment.suiShiftXRows)
  };
});

const columns = Object.keys(table[0]);
const widths = columns.map(column =>
  Math.max(column.length, ...table.map(row => row[column].length)));
const line = values => values.map((value, i) => value.padEnd(widths[i])).join('  ');
console.log(line(columns));
for (const row of table) console.log(line(columns.map(column => row[column])));
console.log(
  '\nweight: SUI ink / browser ink (below 1 = SUI lighter). ' +
  'y shifts: rows per vertical offset (negative = SUI drew lower).'
);
const mixed = runs.filter(({ summary }) => summary.likeForLike === false);
if (mixed.length > 0) {
  console.warn(`warning: mixed antialiasing in ${mixed.map(({ name }) => name).join(', ')}`);
}
writeFileSync(path.join(outputRoot, 'matrix.json'), `${JSON.stringify(table, null, 2)}\n`);
