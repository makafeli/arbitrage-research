import { expect, test as base } from '@playwright/test';
import type { Page } from '@playwright/test';
import { resolve } from 'node:path';
import { pathToFileURL } from 'node:url';

// A runtime exception or an attempted external connection fails every scenario.
// This checks the exercised dashboard flows; it is not an egress security audit.
const test = base.extend<{ runtimeChecks: void }>({
  runtimeChecks: [async ({ page, context }, use) => {
    const errors: string[] = [];
    const externalConnections: string[] = [];
    page.on('pageerror', error => errors.push(error.message));
    const inspectUrl = (raw: string) => {
      const url = new URL(raw);
      if (['http:', 'https:', 'ws:', 'wss:'].includes(url.protocol)
          && !['127.0.0.1', 'localhost'].includes(url.hostname)) {
        externalConnections.push(url.origin);
      }
    };
    context.on('request', request => inspectUrl(request.url()));
    page.on('websocket', socket => inspectUrl(socket.url()));
    await use();
    expect(errors, 'Uncaught browser errors').toEqual([]);
    expect(externalConnections, 'Unexpected external connections').toEqual([]);
  }, { auto: true }],
});

const pages = [
  ['Overview', 'Research overview'],
  ['Opportunities', 'Opportunity explorer'],
  ['Experiments', 'Compare experiments'],
  ['Runs', 'Runs and control'],
  ['Strategies', 'Strategy workspace'],
  ['System', 'System and data quality'],
] as const;

function controls(page: Page) {
  return page.getByRole('region', { name: 'Local demo controls for both paper sessions' });
}

test('six views retain synthetic provenance and support both themes', async ({ page }) => {
  await page.goto('/');
  for (const [navigation, heading] of pages) {
    const button = page.getByRole('navigation').getByRole('button', { name: navigation, exact: true });
    await button.click();
    await expect(button).toHaveAttribute('aria-current', 'page');
    await expect(page.getByRole('heading', { name: heading, level: 1 })).toBeVisible();
    await expect(page.getByText('LOCAL SYNTHETIC DEMO', { exact: true })).toBeVisible();
    await expect(controls(page)).toContainText('SYNTHETIC RUN GROUP');
  }
  await page.getByRole('button', { name: 'Switch to light theme' }).click();
  await expect(page.locator('body')).toHaveClass(/light/);
  await page.getByRole('button', { name: 'Switch to dark theme' }).click();
  await expect(page.locator('body')).not.toHaveClass(/light/);
});

test('a chain filter changes observations while both session scopes remain intact', async ({ page }) => {
  await page.goto('/');
  await page.getByRole('combobox', { name: 'View chain' }).selectOption('base');
  await expect(page.getByRole('region', { name: 'Base synthetic observations' })).toBeVisible();
  await expect(page.getByRole('region', { name: 'Solana synthetic observations' })).toHaveCount(0);
  await expect(controls(page)).toContainText('Solana: STOPPED');
  await expect(controls(page)).toContainText('Base: STOPPED');
  await page.getByRole('button', { name: 'Start demo', exact: true }).click();
  await expect(controls(page)).toContainText('Solana: RUNNING');
  await expect(controls(page)).toContainText('Base: RUNNING');
  await page.getByRole('navigation').getByRole('button', { name: 'Opportunities', exact: true }).click();
  await expect(page.getByRole('combobox', { name: 'View chain' })).toHaveValue('base');
  await expect(page.getByRole('button', { name: /Inspect synthetic opportunity BASE-/ })).toHaveCount(3);
  await expect(page.getByRole('button', { name: /Inspect synthetic opportunity SOL-/ })).toHaveCount(0);
});

test('opportunity detail remains hypothetical and returns keyboard focus on Escape', async ({ page }) => {
  await page.goto('/');
  const inspect = page.getByRole('button', { name: 'Inspect synthetic opportunity SOL-001', exact: true });
  await inspect.focus();
  await page.keyboard.press('Enter');
  const dialog = page.getByRole('dialog', { name: 'SOL-001 · Solana' });
  await expect(dialog).toBeVisible();
  await expect(dialog).toContainText('Not applicable — paper demo');
  await expect(dialog).toContainText('No simulation artifacts are produced by this interface.');
  await expect(dialog.getByRole('button', { name: 'Close opportunity detail' })).toBeFocused();
  await page.keyboard.press('Tab');
  await expect(dialog.getByRole('button', { name: 'Close opportunity detail' })).toBeFocused();
  await page.keyboard.press('Escape');
  await expect(dialog).not.toBeVisible();
  await expect(inspect).toBeFocused();
});

test('stop stays pending until each separate session acknowledges its fence', async ({ page }) => {
  // Install before navigation. Pause before the control interactions so CI
  // scheduling cannot skip the intermediate one-chain-acknowledged state.
  await page.clock.install({ time: new Date('2026-09-12T12:00:00Z') });
  await page.goto('/');
  await page.clock.pauseAt(new Date('2026-09-12T12:01:00Z'));
  await page.getByRole('button', { name: 'Start demo', exact: true }).click();
  await page.getByRole('button', { name: 'Pause demo', exact: true }).click();
  await expect(controls(page)).toContainText('Solana: PAUSED · APPLIED');
  await expect(controls(page)).toContainText('Base: PAUSED · APPLIED');
  await page.getByRole('button', { name: 'Resume demo', exact: true }).click();
  await page.getByRole('button', { name: 'Stop demo', exact: true }).click();
  await expect(controls(page)).toContainText('Solana: PAUSING · PENDING');
  await expect(controls(page)).toContainText('Base: PAUSING · PENDING');
  await expect(page.getByRole('button', { name: 'Start demo', exact: true })).toBeDisabled();
  await page.clock.runFor(700);
  await expect(controls(page)).toContainText('Solana: STOPPED · APPLIED');
  await expect(controls(page)).toContainText('Base: PAUSING · PENDING');
  await expect(controls(page)).toContainText('Stop requested');
  await page.clock.runFor(400);
  await expect(controls(page)).toContainText('Base: STOPPED · APPLIED');
  await expect(page.getByRole('button', { name: 'Start demo', exact: true })).toBeEnabled();
  await expect(page.getByRole('button', { name: 'Stop demo', exact: true })).toBeDisabled();
  await page.reload();
  await expect(controls(page)).toContainText('Solana: STOPPED · NONE');
  await expect(controls(page)).toContainText('Base: STOPPED · NONE');
});

test('an acknowledged stop with an unresolved illustrative outcome remains DRAINING', async ({ page }) => {
  await page.goto('/');
  await page.getByRole('navigation').getByRole('button', { name: 'Runs', exact: true }).click();
  await page.getByRole('button', { name: 'Simulate pending outcome', exact: true }).click();
  await expect(page.getByText(/Stop APPLIED: the worker admission fence/)).toContainText('State DRAINING');
  await expect(page.getByText(/A separate illustration of a previously emitted live attempt/)).toBeVisible();
  await expect(controls(page)).toContainText('Solana: STOPPED · NONE');
  await expect(controls(page)).toContainText('Base: STOPPED · NONE');
  await page.getByRole('button', { name: 'Resolve demo outcome', exact: true }).click();
  await expect(page.getByText(/Outcome reconciled as no trade. Pending count zero/)).toContainText('STOPPED');
  await expect(page.getByRole('button', { name: 'Resolve demo outcome', exact: true })).toBeDisabled();
});

for (const width of [320, 390, 768, 1440]) {
  test(`all views fit a ${width}px viewport and preserve visible controls`, async ({ page }, testInfo) => {
    await page.setViewportSize({ width, height: 1000 });
    await page.goto('/');
    for (const [navigation, heading] of pages) {
      await page.getByRole('navigation').getByRole('button', { name: navigation, exact: true }).click();
      await expect(page.getByRole('heading', { name: heading, level: 1 })).toBeVisible();
      await expect(page.getByText('LOCAL SYNTHETIC DEMO', { exact: true })).toBeVisible();
      await expect(page.getByRole('button', { name: 'Start demo', exact: true })).toBeVisible();
      const overflow = await page.evaluate(() => Math.max(
        document.documentElement.scrollWidth,
        document.body.scrollWidth,
      ) - document.documentElement.clientWidth);
      expect(overflow, `${navigation} horizontal overflow at ${width}px`).toBeLessThanOrEqual(1);
    }
    await page.getByRole('navigation').getByRole('button', { name: 'Overview', exact: true }).click();
    await testInfo.attach(`overview-${width}-dark`, { body: await page.screenshot({ fullPage: true }), contentType: 'image/png' });
    await page.getByRole('button', { name: 'Switch to light theme' }).click();
    await testInfo.attach(`overview-${width}-light`, { body: await page.screenshot({ fullPage: true }), contentType: 'image/png' });
  });
}

test('capture the approved reference for visual comparison', async ({ page }, testInfo) => {
  const reference = resolve(process.cwd(), '../../design/dashboard-wireframe.html');
  await page.setViewportSize({ width: 1440, height: 1000 });
  await page.goto(pathToFileURL(reference).href);
  await testInfo.attach('approved-reference-1440-dark', {
    body: await page.screenshot({ fullPage: true }),
    contentType: 'image/png',
  });
});
