import assert from 'node:assert/strict';
import { chromium } from 'playwright';

// Exercise the real Wasm app through SUI's semantic DOM automation bridge.
const url = new URL(process.argv[2] ?? 'http://127.0.0.1:8080/');
url.searchParams.set('benchmark', 'dev');
url.searchParams.set('semantic-dom', 'automation');
url.searchParams.set('sui-debug', '1');
url.searchParams.delete('demo');
const browser = await chromium.launch({
  ...(process.env.SUI_BROWSER_EXECUTABLE
    ? { executablePath: process.env.SUI_BROWSER_EXECUTABLE }
    : {}),
  args: ['--enable-unsafe-webgpu', '--enable-features=Vulkan', '--use-angle=vulkan', '--disable-vulkan-surface'],
});
const page = await browser.newPage({ viewport: { width: 1280, height: 900 } });
page.setDefaultTimeout(15_000);
const errors = [];
page.on('pageerror', error => errors.push(String(error)));
page.on('console', message => {
  if (message.type() === 'error') errors.push(message.text());
});
const node = name => page.evaluate(name =>
  window.__suiInteropState?.nodes.find(node => node.name === name), name);
const waitForNode = name => page.waitForFunction(name =>
  window.__suiInteropState?.nodes.some(node => node.name === name), name);
const activate = async name => {
  await waitForNode(name);
  await page.evaluate(name => {
    const node = window.__suiInteropState.nodes.find(node => node.name === name);
    return window.__sui.click({ id: node.id });
  }, name);
};
const waitForMovement = bounds => page.waitForFunction(before => {
  const circle = window.__suiInteropState?.nodes.find(node => node.name === 'Toggle circle 1');
  return circle && JSON.stringify(circle.bounds) !== JSON.stringify(before);
}, bounds);

try {
  await page.goto(url.href, { waitUntil: 'load' });
  await page.waitForFunction(() =>
    window.__suiInteropState?.nodes.some(node => node.name === 'Editorial engine'),
  null, { timeout: 60_000 });
  await activate('Editorial engine');
  await waitForNode('Editorial text flow');
  assert.ok(JSON.stringify((await node('Editorial text flow')).value).includes('The city wakes'));
  await waitForMovement((await node('Toggle circle 1')).bounds);

  await activate('Pause all');
  await page.waitForTimeout(300);
  const paused = (await node('Toggle circle 1')).bounds;
  await page.waitForTimeout(250);
  assert.deepEqual((await node('Toggle circle 1')).bounds, paused, 'Pause must stop circle motion');
  await activate('Play all');
  await waitForMovement(paused);

  await activate('Toggle circle 1');
  await page.waitForFunction(() =>
    window.__suiInteropState?.nodes.find(node => node.name === 'Toggle circle 1')?.state.selected);
  await activate('Reset');
  await page.waitForFunction(() =>
    window.__suiInteropState?.nodes.find(node => node.name === 'Toggle circle 1')?.state.selected === false);
  await page.setViewportSize({ width: 800, height: 700 });
  await waitForMovement((await node('Toggle circle 1')).bounds);

  await activate('Open demo');
  await activate('Shrinkwrap');
  await waitForNode('Animated shrinkwrap conversation');
  await activate('Editorial engine');
  await waitForNode('Editorial text flow');
  await waitForMovement((await node('Toggle circle 1')).bounds);
  assert.deepEqual(errors, [], 'The browser must not panic or report JavaScript errors');
  console.log('PASS: Editorial opens, animates, pauses, resumes, resets, resizes, and reopens without freezing.');
} catch (error) {
  if (errors.length) console.error(errors.join('\n'));
  throw error;
} finally {
  await browser.close();
}
