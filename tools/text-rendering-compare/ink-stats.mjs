// Align only captured rows. Keep SUI's layout, glyph phases, and RGB samples intact.
export const ALIGNMENT_RADIUS_PHYSICAL_PIXELS = 2;
const SAMPLE_MARGIN_CSS_PIXELS = 6;

function image(width, height, background) {
  const data = Buffer.alloc(width * height * 4);
  for (let i = 0; i < data.length; i += 4) {
    data[i] = background[0];
    data[i + 1] = background[1];
    data[i + 2] = background[2];
    data[i + 3] = 255;
  }
  return { width, height, data };
}

function rowCrop(source, rect, background) {
  const padding = ALIGNMENT_RADIUS_PHYSICAL_PIXELS;
  const crop = image(rect.width + padding * 2, rect.height + padding * 2, background);
  for (let y = 0; y < rect.height; y += 1) {
    const start = ((rect.y + y) * source.width + rect.x) * 4;
    source.data.copy(crop.data, ((y + padding) * crop.width + padding) * 4,
      start, start + rect.width * 4);
  }
  return crop;
}

function measureInk(sui, browser, background, dx = 0, dy = 0) {
  const backdrop = 0.2126 * background[0] + 0.7152 * background[1] + 0.0722 * background[2];
  let unionPixels = 0;
  let differingInkPixels = 0;
  let suiInkMass = 0;
  let browserInkMass = 0;
  let absoluteInkChannelError = 0;

  for (let y = 0; y < browser.height; y += 1) {
    for (let x = 0; x < browser.width; x += 1) {
      // dx/dy translate SUI toward Chrome: negative dy moves SUI upward.
      const sx = x - dx;
      const sy = y - dy;
      const inside = sx >= 0 && sx < sui.width && sy >= 0 && sy < sui.height;
      const si = (sy * sui.width + sx) * 4;
      const bi = (y * browser.width + x) * 4;
      const sr = inside ? sui.data[si] : background[0];
      const sg = inside ? sui.data[si + 1] : background[1];
      const sb = inside ? sui.data[si + 2] : background[2];
      const br = browser.data[bi];
      const bg = browser.data[bi + 1];
      const bb = browser.data[bi + 2];
      const suiDarkness = Math.abs(0.2126 * sr + 0.7152 * sg + 0.0722 * sb - backdrop);
      const browserDarkness = Math.abs(0.2126 * br + 0.7152 * bg + 0.0722 * bb - backdrop);
      if (suiDarkness <= 2 && browserDarkness <= 2) continue;

      unionPixels += 1;
      absoluteInkChannelError += Math.abs(sr - br) + Math.abs(sg - bg) + Math.abs(sb - bb);
      suiInkMass += suiDarkness;
      browserInkMass += browserDarkness;
      if (Math.abs(suiDarkness - browserDarkness) > 12) differingInkPixels += 1;
    }
  }

  return {
    unionPixels,
    differingInkPixels,
    differingInkRatio: unionPixels === 0 ? 0 : differingInkPixels / unionPixels,
    suiInkMass: Math.round(suiInkMass),
    browserInkMass: Math.round(browserInkMass),
    inkMassRatio: browserInkMass <= 0 ? 1 : suiInkMass / browserInkMass,
    absoluteInkChannelError,
    meanInkChannelError: unionPixels === 0 ? 0 : absoluteInkChannelError / (unionPixels * 3)
  };
}

function translated(source, dx, dy, background) {
  const result = image(source.width, source.height, background);
  const left = Math.max(0, -dx);
  const right = Math.min(source.width, source.width - dx);
  for (let y = Math.max(0, -dy); y < Math.min(source.height, source.height - dy); y += 1) {
    source.data.copy(result.data, ((y + dy) * result.width + left + dx) * 4,
      (y * source.width + left) * 4, (y * source.width + right) * 4);
  }
  return result;
}

function stackRows(rows, background) {
  const width = Math.max(1, ...rows.map(row => row.width));
  const result = image(width, rows.reduce((height, row) => height + row.height, 0), background);
  let top = 0;
  for (const row of rows) {
    for (let y = 0; y < row.height; y += 1) {
      row.data.copy(result.data, (top + y) * width * 4,
        y * row.width * 4, (y + 1) * row.width * 4);
    }
    top += row.height;
  }
  return result;
}

function aggregate(rows) {
  const unionPixels = rows.reduce((sum, row) => sum + row.unionPixels, 0);
  const absoluteInkChannelError = rows.reduce((sum, row) => sum + row.absoluteInkChannelError, 0);
  return {
    unionPixels,
    absoluteInkChannelError,
    meanInkChannelError: unionPixels === 0 ? 0 : absoluteInkChannelError / (unionPixels * 3)
  };
}

export function compareTextRows(sui, browser, { samples, background, dpiScale }) {
  if (sui.width !== browser.width || sui.height !== browser.height) {
    throw new Error('Text comparison requires equal image dimensions');
  }
  const radius = ALIGNMENT_RADIUS_PHYSICAL_PIXELS;
  const rowInkStats = [];
  const alignedRowInkStats = [];
  const suiRows = [];
  const browserRows = [];
  let sheetTop = 0;

  for (const sample of samples) {
    if (![sample.x, sample.y, sample.width, sample.lineHeight].every(Number.isFinite) ||
        sample.width < 0 || sample.lineHeight < 0) {
      throw new Error(`Invalid text sample bounds: ${sample.text}`);
    }
    const margin = SAMPLE_MARGIN_CSS_PIXELS;
    const left = Math.max(0, Math.min(sui.width, Math.floor((sample.x - margin) * dpiScale)));
    const right = Math.max(left, Math.min(sui.width, Math.ceil((sample.x + sample.width + margin) * dpiScale)));
    const top = Math.max(0, Math.min(sui.height, Math.floor((sample.y - margin) * dpiScale)));
    const bottom = Math.max(top, Math.min(sui.height, Math.ceil((sample.y + sample.lineHeight + margin) * dpiScale)));
    const sourceRect = { x: left, y: top, width: right - left, height: bottom - top };
    // Padding retains ink translated beyond the crop/image edge instead of dropping
    // difficult pixels from the score. Each isolated row is scored independently.
    const suiRow = rowCrop(sui, sourceRect, background);
    const browserRow = rowCrop(browser, sourceRect, background);
    const raw = measureInk(suiRow, browserRow, background);
    let best = raw;
    let bestX = 0;
    let bestY = 0;
    for (let dy = -radius; dy <= radius; dy += 1) {
      for (let dx = -radius; dx <= radius; dx += 1) {
        if (dx === 0 && dy === 0) continue;
        const candidate = measureInk(suiRow, browserRow, background, dx, dy);
        // Minimize total error, not MAE: shifting can change the ink union size.
        // Resolve ties toward no movement, including blank and identical rows.
        if (candidate.absoluteInkChannelError < best.absoluteInkChannelError ||
            (candidate.absoluteInkChannelError === best.absoluteInkChannelError &&
             Math.abs(dx) + Math.abs(dy) < Math.abs(bestX) + Math.abs(bestY))) {
          best = candidate;
          bestX = dx;
          bestY = dy;
        }
      }
    }
    rowInkStats.push({ text: sample.text, sourceRect, ...raw });
    alignedRowInkStats.push({
      text: sample.text,
      sourceRect,
      ...best,
      alignment: {
        suiShiftX: bestX,
        suiShiftY: bestY,
        atSearchBoundary: Math.abs(bestX) === radius || Math.abs(bestY) === radius
      },
      absoluteErrorReduction: raw.absoluteInkChannelError === 0 ? 0 :
        1 - best.absoluteInkChannelError / raw.absoluteInkChannelError,
      alignedImageRect: { x: 0, y: sheetTop, width: suiRow.width, height: suiRow.height }
    });
    suiRows.push(translated(suiRow, bestX, bestY, background));
    browserRows.push(browserRow);
    sheetTop += suiRow.height;
  }

  return {
    rowInkStats,
    alignedRowInkStats,
    alignment: {
      method: 'whole-row integer translation',
      units: 'physical pixels',
      searchRadius: radius,
      objective: 'minimum absoluteInkChannelError; ties prefer the smallest shift',
      shiftedRows: alignedRowInkStats.filter(row => row.alignment.suiShiftX !== 0 || row.alignment.suiShiftY !== 0).length,
      boundaryRows: alignedRowInkStats.filter(row => row.alignment.atSearchBoundary).length
    },
    textQuality: {
      sampleMarginCssPixels: SAMPLE_MARGIN_CSS_PIXELS,
      raw: aggregate(rowInkStats),
      aligned: aggregate(alignedRowInkStats)
    },
    alignedSui: stackRows(suiRows, background),
    alignedBrowser: stackRows(browserRows, background)
  };
}
