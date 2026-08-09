import type { Locator, Page, TestInfo } from 'playwright/test';

import { expect, test } from 'playwright/test';

const viewMeta = {
  data_as_of: '2026-07-29T19:40:00+08:00',
  freshness: 'FRESH',
  generated_at: '2026-07-29T19:40:08+08:00',
  retained_after_failure: false,
  schema_version: 'i4.1',
  status_message: '页面评审固定上下文',
};

const scenes = [
  {
    baselineKey: 'B-01-ingest-overview',
    path: '/#/control',
    review(page: Page, container: Locator) {
      return Promise.all([
        expect(page.locator('.overview-navigation')).toHaveCount(0),
        expect(page.locator('.runtime-header-activity')).toBeVisible(),
        expect(
          page.getByRole('button', { name: '打开中控服务器同步记录' }),
        ).toBeVisible(),
        expect(
          container.locator('.current-ingest .ant-progress'),
        ).toBeVisible(),
        expect(container.locator('.source-device')).toHaveCount(3),
        expect(container.locator('.data-flow-canvas')).toHaveAttribute(
          'data-particle-renderer',
          'path-aether',
        ),
        expect(container.locator('.data-flow-canvas')).toHaveAttribute(
          'data-particle-state',
          'running',
        ),
        expect(container.locator('.data-flow-canvas')).toHaveAttribute(
          'data-particle-path-start',
          '0.215,0.304',
        ),
        expect(container.locator('.data-flow-canvas')).toHaveAttribute(
          'data-particle-path-end',
          '0.704,0.470',
        ),
        expect(container.locator('.stage-caption')).toHaveCount(0),
      ]);
    },
  },
  {
    baselineKey: 'B-03-collection-snapshot',
    path: '/#/control/sites/factory-a-001/collection',
    review(page: Page, container: Locator) {
      return Promise.all([
        expect(page.locator('.control-server-tabs .ant-tabs-tab')).toHaveCount(
          4,
        ),
        expect(container.locator('.stage-panel li')).toHaveCount(7),
        expect(container.locator('.stage-panel .state-completed')).toHaveCount(
          7,
        ),
        expect(container.locator('.snapshot-card .success')).not.toHaveCount(0),
        expect(container.locator('.failure-banner')).toHaveCount(0),
      ]);
    },
  },
  {
    baselineKey: 'B-04-media-detail',
    path: '/#/control/media',
    review(_page: Page, container: Locator) {
      return Promise.all([
        expect(container.locator('.media-progress.ant-progress')).toBeVisible(),
        expect(container.locator('.ingest-steps.ant-steps')).toBeVisible(),
        expect(container.locator('.ingest-steps .ant-steps-item')).toHaveCount(
          5,
        ),
      ]);
    },
  },
  {
    baselineKey: 'B-05-conflict-lock',
    path: '/#/control/conflicts',
    review(_page: Page, container: Locator) {
      return Promise.all([
        expect(container.locator('.conflict-steps.ant-steps')).toBeVisible(),
        expect(container.locator('.locked-button')).toBeDisabled(),
        expect(container.locator('.diagnostic-button')).toBeEnabled(),
      ]);
    },
  },
  {
    baselineKey: 'B-06-records',
    path: '/#/control/history',
    review(page: Page, container: Locator) {
      return Promise.all([
        expect(page.locator('.control-server-tabs .ant-tabs-tab')).toHaveCount(
          4,
        ),
        expect(container.locator('.history-workspace')).toHaveAttribute(
          'data-layout-reference',
          'edge-server-records',
        ),
        expect(
          container.locator('.history-heading-row .history-heading-copy h2'),
        ).toBeVisible(),
        expect(
          container.locator('.history-heading-row .history-note.ant-alert'),
        ).toBeVisible(),
        expect(
          container.locator('.history-summary .summary-filter.ant-btn'),
        ).toHaveCount(6),
        expect(
          container.locator('.history-summary .summary-filter.ant-btn').first(),
        ).toHaveCSS('align-items', 'center'),
        expect(
          container
            .locator('.history-summary .summary-filter.ant-btn > span')
            .first(),
        ).toHaveCSS('align-items', 'center'),
        expect(
          container.locator('.history-summary .summary-filter strong').first(),
        ).toHaveCSS('color', 'rgb(231, 235, 239)'),
        expect(container.locator('.history-heading-copy h2')).toHaveCSS(
          'font-size',
          '30px',
        ),
        expect(container.locator('.history-heading-copy p')).toHaveCSS(
          'font-size',
          '15px',
        ),
        expect(
          container.locator('.history-controls .state-buttons'),
        ).toHaveCount(0),
        expect(container.locator('.history-controls .ant-btn')).toHaveCount(0),
        expect(container.locator('.history-controls .range-select')).toHaveCSS(
          'grid-column-start',
          '2',
        ),
        expect(
          container.locator('.history-controls .history-search'),
        ).toHaveCSS('grid-column-start', '3'),
        expect(
          container.locator('.history-table.ant-table-wrapper'),
        ).toBeVisible(),
        expect(container.locator('.history-controls .ant-select')).toHaveCount(
          2,
        ),
        expect(
          container.locator('.history-search.ant-input-affix-wrapper'),
        ).toBeVisible(),
        expect(container.locator('.row-action.ant-btn')).toHaveCount(5),
        expect(container.locator('.history-table strong').first()).toHaveCSS(
          'font-weight',
          '400',
        ),
        expect(container.locator('.history-rack')).toHaveCount(0),
        expect(
          page.locator(
            '.control-runtime-rack[data-device-id="control-center-rack"]',
          ),
        ).toBeVisible(),
        expect(container.locator('.history-footer .ant-btn').first()).toHaveCSS(
          'width',
          '34px',
        ),
        expect(container.locator('.history-footer .ant-btn').first()).toHaveCSS(
          'border-top-width',
          '1px',
        ),
      ]);
    },
  },
  {
    baselineKey: 'B-07-control-config',
    path: '/#/control/settings',
    review(page: Page, container: Locator) {
      return Promise.all([
        expect(page.locator('.control-server-tabs .ant-tabs-tab')).toHaveCount(
          4,
        ),
        expect(
          page.locator(
            '.control-runtime-rack[data-device-id="control-center-rack"]',
          ),
        ).toBeVisible(),
        expect(container.locator('.settings-status .ant-tag')).toBeVisible(),
        expect(
          container.locator('input, textarea, [contenteditable="true"]'),
        ).toHaveCount(0),
        expect(container.locator('button[type="submit"]')).toHaveCount(0),
      ]);
    },
  },
] as const;

const adminPaths = [
  '/#/control/settings/factories',
  '/#/control/settings/factories/new',
  '/#/control/settings/factories/factory-a-007/registration',
] as const;

async function provideControlContext(page: Page) {
  await page.route('**/api/local/v1/context', (route) =>
    route.fulfill({
      body: JSON.stringify({
        display_name: '中心 B',
        meta: viewMeta,
        permissions: ['VIEW_LOCAL_STATUS'],
        role: 'CONTROL',
        site_id: 'control-b',
      }),
      contentType: 'application/json',
    }),
  );
}

async function waitForStableVisual(page: Page) {
  await expect(page.locator('#nprogress')).toHaveCount(0);
  await expect(page.locator('#__app-loading__')).toBeHidden();
  await page.waitForFunction(async () => {
    await document.fonts.ready;
    return [...document.images].every((image) => image.complete);
  });
}

async function attachReviewScreenshot(
  page: Page,
  testInfo: TestInfo,
  name: string,
) {
  const path = testInfo.outputPath(`${name}.png`);
  const image = await page.screenshot({
    animations: 'disabled',
    fullPage: true,
    path,
  });
  expect(image.byteLength).toBeGreaterThan(40_000);
  await testInfo.attach(name, { contentType: 'image/png', path });
}

async function expectBaselineCanvasFits(
  page: Page,
  viewport: { height: number; width: number },
) {
  const canvas = page.locator('.product-shell-baseline-canvas');
  await expect(canvas).toBeVisible();
  const box = await canvas.boundingBox();
  expect(box).not.toBeNull();
  if (!box) {
    throw new Error('The frozen baseline canvas has no bounding box.');
  }
  expect(box.width / box.height).toBeCloseTo(1672 / 941, 2);
  expect(box.x).toBeGreaterThanOrEqual(-1);
  expect(box.y).toBeGreaterThanOrEqual(-1);
  expect(box.x + box.width).toBeLessThanOrEqual(viewport.width + 1);
  expect(box.y + box.height).toBeLessThanOrEqual(viewport.height + 1);
}

for (const scene of scenes) {
  test(`${scene.baselineKey} keeps its frozen review contract at all review viewports`, async ({
    page,
  }, testInfo) => {
    await page.emulateMedia({ reducedMotion: 'reduce' });
    await provideControlContext(page);

    for (const viewport of [
      { height: 941, name: '1672x941', width: 1672 },
      { height: 1080, name: '1920x1080', width: 1920 },
      { height: 720, name: '1280x720', width: 1280 },
    ]) {
      await page.setViewportSize(viewport);
      await page.goto(scene.path);
      const root = page.locator(
        `[data-baseline-key="${scene.baselineKey}"][data-view-source="frozen-baseline-fixture"]`,
      );
      await expect(root).toBeVisible();
      await scene.review(page, root);
      await waitForStableVisual(page);
      await expectBaselineCanvasFits(page, viewport);
      await attachReviewScreenshot(
        page,
        testInfo,
        `${scene.baselineKey}-${viewport.name}`,
      );
    }
  });
}

test('B-01 animates the reused A-01 particle renderer between the disk slot and server', async ({
  page,
}, testInfo) => {
  await page.setViewportSize({ height: 941, width: 1672 });
  await provideControlContext(page);
  await page.goto('/#/control');
  await waitForStableVisual(page);

  const canvas = page.locator('.data-flow-canvas');
  await expect(canvas).toHaveAttribute('data-particle-state', 'running');
  await expect(canvas).toHaveAttribute(
    'data-particle-path-start',
    '0.215,0.304',
  );
  await expect(canvas).toHaveAttribute('data-particle-path-end', '0.704,0.470');

  const firstFrame = await canvas.evaluate((element) =>
    (element as HTMLCanvasElement).toDataURL(),
  );
  await page.waitForTimeout(160);
  const secondFrame = await canvas.evaluate((element) =>
    (element as HTMLCanvasElement).toDataURL(),
  );
  expect(firstFrame.length).toBeGreaterThan(10_000);
  expect(secondFrame).not.toBe(firstFrame);
  await attachReviewScreenshot(
    page,
    testInfo,
    'B-01-ingest-overview-animated-1672x941',
  );
});

test('B-01, B-06 and B-07 preserve one center rack while server layouts switch', async ({
  page,
}) => {
  await page.setViewportSize({ height: 941, width: 1672 });
  await provideControlContext(page);
  await page.goto('/#/control');
  await waitForStableVisual(page);

  const rack = page.locator(
    '.control-runtime-rack[data-device-id="control-center-rack"]',
  );
  await expect(rack).toBeVisible();
  await rack.evaluate((element) => {
    element.dataset.continuityToken = 'control-center-rack-preserved';
  });
  const homeBox = await rack.boundingBox();
  expect(homeBox).not.toBeNull();

  await rack.click();
  await expect(page).toHaveURL(/#\/control\/history$/);
  await expect(page.locator('.history-workspace')).toBeVisible();
  await expect(rack).toHaveAttribute(
    'data-continuity-token',
    'control-center-rack-preserved',
  );
  await expect(rack).toHaveCSS(
    'transition-timing-function',
    /cubic-bezier\(0.22, 1, 0.36, 1\)/,
  );

  await page.waitForTimeout(460);
  const detailBox = await rack.boundingBox();
  expect(detailBox).not.toBeNull();
  if (homeBox && detailBox) {
    expect(detailBox.x).toBeGreaterThan(homeBox.x);
    expect(detailBox.y).toBeGreaterThan(homeBox.y);
    expect(detailBox.width).toBeLessThan(homeBox.width);
  }

  await page
    .locator('.control-server-tabs .ant-tabs-tab', {
      hasText: '中控配置',
    })
    .click();
  await expect(page).toHaveURL(/#\/control\/settings$/);
  await expect(page.locator('.settings-page')).toBeVisible();
  await expect(rack).toHaveAttribute(
    'data-continuity-token',
    'control-center-rack-preserved',
  );
  await page.waitForTimeout(460);
  const settingsBox = await rack.boundingBox();
  expect(settingsBox).not.toBeNull();
  if (detailBox && settingsBox) {
    expect(settingsBox.x).toBeLessThan(detailBox.x);
    expect(settingsBox.y).toBeLessThan(detailBox.y);
    expect(settingsBox.width).toBeGreaterThan(detailBox.width);
  }

  await page.getByRole('tab', { exact: true, name: '子工厂' }).click();
  await expect(page).toHaveURL(/#\/control\/sites$/);
  await expect(page.locator('.control-experience')).toBeVisible();
  await expect(rack).toHaveAttribute(
    'data-continuity-token',
    'control-center-rack-preserved',
  );
  await page.waitForTimeout(460);
  const sitesBox = await rack.boundingBox();
  expect(sitesBox).not.toBeNull();
  if (settingsBox && sitesBox) {
    expect(sitesBox.x).toBeGreaterThan(settingsBox.x);
    expect(sitesBox.y).toBeGreaterThan(settingsBox.y);
    expect(sitesBox.width).toBeLessThan(settingsBox.width);
  }

  await page
    .getByRole('link', { name: '关闭中控服务器并返回入库总览' })
    .click();
  await expect(page).toHaveURL(/#\/control$/);
  await expect(rack).toHaveAttribute(
    'data-continuity-token',
    'control-center-rack-preserved',
  );
  await expect(page.locator('.control-overview')).toBeVisible();

  await page.emulateMedia({ reducedMotion: 'reduce' });
  expect(
    await rack.evaluate((element) =>
      Number.parseFloat(getComputedStyle(element).transitionDuration),
    ),
  ).toBeLessThanOrEqual(0.001);
});

test('B-08 administrator scenes fail closed for the installed CONTROL role', async ({
  page,
}) => {
  await provideControlContext(page);

  for (const path of adminPaths) {
    await page.goto(path);
    await expect(page).toHaveURL(/\/#\/control$/);
    await expect(
      page.locator('[data-required-role="CONTROL_ADMIN"]'),
    ).toHaveCount(0);
  }
});

test('B-08 administrator scenes stay closed when context is unavailable', async ({
  page,
}) => {
  await page.route('**/api/local/v1/context', (route) =>
    route.fulfill({
      body: JSON.stringify({
        error_code: 'LOCAL_CONTEXT_UNAVAILABLE',
        message: '页面评审模拟上下文不可用',
        retryable: true,
      }),
      contentType: 'application/json',
      status: 503,
    }),
  );

  for (const path of adminPaths) {
    await page.goto(path);
    await expect(page).toHaveURL(/\/#\/control$/);
    await expect(
      page.locator('[data-required-role="CONTROL_ADMIN"]'),
    ).toHaveCount(0);
  }
});
