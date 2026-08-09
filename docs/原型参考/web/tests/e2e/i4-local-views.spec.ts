import type { Page, TestInfo } from 'playwright/test';

import { expect, test } from 'playwright/test';

const viewMeta = {
  data_as_of: '2026-07-24T18:42:00+08:00',
  freshness: 'FRESH',
  generated_at: '2026-07-24T18:42:08+08:00',
  retained_after_failure: false,
  schema_version: 'i4.1',
  status_message: '数据最新',
};

const runtime = {
  current: {
    batch_id: 'batch_visual001',
    confirmed_bytes: 8_400_000_000_000,
    eta_confidence: 'HIGH',
    eta_seconds: 1694,
    progress_percent: 68,
    stage: '下载 · 加密 · 写盘中',
    total_bytes: 12_300_000_000_000,
  },
  display_name: '工厂 A',
  media: {
    completed: 4,
    connected: 16,
    failed: 0,
    running: 4,
    standby: 8,
    warning: 1,
  },
  meta: viewMeta,
  next_action: {
    action_code: 'REPLACE_AFTER_STAGE',
    detail: '温度 52°C，健康警告',
    media_slot: '04',
    priority: 'WARNING',
    requires_role: null,
    serial_suffix: '8F2A',
    title: '完成当前阶段后更换此盘',
  },
  site_id: 'factory-a-001',
  state: 'RUNNING',
  state_label: '自动运行中',
  throughput_bytes_per_second: 1_840_000_000,
};

const transportDiskSerials = [
  '7F22',
  '51C8',
  '0D16',
  '8F2A',
  '44B9',
  '28E6',
  '6C31',
  'B702',
  '94A1',
  'E8C4',
  '33D9',
  'A107',
  '9B50',
  '71DD',
  '4E21',
  'C640',
];

const transportDisks = {
  disks: transportDiskSerials.map((serial, index) => {
    const slot = index + 1;
    const writing = [1, 4, 6, 8].includes(slot);
    const readyToSwap = slot === 7 || slot >= 14;
    const warning = slot === 4;
    const progress = {
      1: 72,
      4: 68,
      6: 61,
      8: 54,
    }[slot as 1 | 4 | 6 | 8];
    let state = 'STANDBY';
    let stateLabel = '待命';
    if (readyToSwap) {
      state = 'READY_TO_SWAP';
      stateLabel = '待换';
    }
    if (writing) {
      state = 'WRITING';
      stateLabel = '写入中';
    }
    return {
      capacity_bytes: 6_000_000_000_000,
      exclusion_reason: null,
      exclusion_state: 'READY',
      filesystem_label: 'FUSTFS_TRANSPORT',
      in_use: writing,
      life_percent: null,
      media_id_suffix: `M${String(slot).padStart(3, '0')}`,
      media_label: '运输盘',
      progress_percent: writing ? progress : null,
      read_only: false,
      serial_suffix: serial,
      slot: String(slot).padStart(2, '0'),
      smart_state: warning ? 'WARNING' : 'READY',
      state,
      state_label: stateLabel,
      temperature_celsius: warning ? 52 : 36 + (slot % 6),
    };
  }),
  last_scan_at: '2026-07-24T14:32:08+08:00',
  meta: viewMeta,
  site_id: 'factory-a-001',
  summary: {
    connected: 16,
    failed: 0,
    healthy: 15,
    warning: 1,
  },
};

const serverStatus = {
  capabilities: [
    {
      code: 'AGENT',
      detail: '正常',
      label: 'Agent',
      state: 'READY',
    },
    {
      code: 'RUSTFS',
      detail: '已连接',
      label: 'RustFS',
      state: 'READY',
    },
    {
      code: 'AUTO_DISCOVERY',
      detail: '正常',
      label: '自动发现',
      state: 'READY',
    },
    {
      code: 'READ_ONLY_ACCESS',
      detail: '有效',
      label: '只读权限',
      state: 'READY',
    },
    {
      code: 'CRYPTO_SIGNING',
      detail: '就绪',
      label: '加密与签名',
      state: 'READY',
    },
  ],
  health_trend: [
    93, 94, 94, 92, 90, 94, 91, 94, 92, 92, 95, 94, 90, 88, 92, 88, 87, 92, 88,
    92, 91, 92, 91, 91, 93, 91, 92, 93, 91, 92,
  ],
  last_scan_at: '2026-07-24T18:07:42+08:00',
  meta: viewMeta,
  overall: 'WARNING',
  overall_label: '存在 1 块注意盘',
  pending_object_versions: 1286,
  site_id: 'factory-a-001',
  storage: {
    available_bytes: 39_400_000_000_000,
    healthy_disks: 7,
    recognized_disks: 8,
    total_bytes: 144_000_000_000_000,
    unknown_disks: 0,
    warning_disks: 1,
  },
};

const serverStatusUnknown = {
  ...serverStatus,
  capabilities: [
    ...serverStatus.capabilities.slice(0, 4),
    {
      code: 'CRYPTO_SIGNING',
      detail: '尚未获得测试介质授权',
      label: '加密与签名',
      state: 'UNKNOWN',
    },
  ],
};

const edgeRecords = {
  meta: viewMeta,
  records: [
    {
      batch_id: 'A-20260720-008',
      completed_at: '2026-07-24T18:42:09+08:00',
      destination_label: '中心 B 归档区',
      events: [
        {
          at: '2026-07-24T18:42:09+08:00',
          label: '停止写入',
          result: '未生成可送出标记',
          state: 'PASSED',
        },
      ],
      failure_reason: '整盘完成证据的签名摘要不一致',
      failure_stage: '发布整盘证据',
      logical_bytes: 12_300_000_000_000,
      media_serial_suffix: '8F2A',
      result_label: '批次失败',
      retry_result: '等待人工复核',
      stages: [
        {
          at: '2026-07-24T18:10:00+08:00',
          label: '读取完成',
          state: 'PASSED',
        },
        {
          at: '2026-07-24T18:26:00+08:00',
          label: '加密写入完成',
          state: 'PASSED',
        },
        {
          at: '2026-07-24T18:42:09+08:00',
          label: '写入校验失败',
          state: 'FAILED',
        },
        {
          at: null,
          label: '生成可送出标记',
          state: 'PENDING',
        },
      ],
      state: 'FAILED',
    },
    {
      batch_id: 'A-20260720-007',
      completed_at: '2026-07-24T17:32:00+08:00',
      destination_label: '中心 B 归档区',
      events: [],
      failure_reason: null,
      failure_stage: null,
      logical_bytes: 11_800_000_000_000,
      media_serial_suffix: '91C4',
      result_label: '装盘成功',
      retry_result: null,
      stages: [
        {
          at: '2026-07-24T17:06:00+08:00',
          label: '读取完成',
          state: 'PASSED',
        },
        {
          at: '2026-07-24T17:20:00+08:00',
          label: '加密写入完成',
          state: 'PASSED',
        },
        {
          at: '2026-07-24T17:29:00+08:00',
          label: '写入校验通过',
          state: 'PASSED',
        },
        {
          at: '2026-07-24T17:32:00+08:00',
          label: '发布整盘证据',
          state: 'PASSED',
        },
      ],
      state: 'PACKED',
    },
  ],
  site_id: 'factory-a-001',
  summary: {
    closed: 0,
    failed: 1,
    packed: 1,
    total: 2,
    waiting_receipt: 0,
  },
  transport_media_connected: false,
};

const managedSettings = {
  collection: {
    endpoint_state: 'READY',
    last_collection_at: '2026-07-24T18:42:00+08:00',
    last_snapshot_label: '完整快照 #42',
    trusted_control_label: '中心 B',
    trusted_control_state: 'READY',
  },
  discovery: {
    auto_discovery: 'READY',
    health_scan_interval_label: '每 5 分钟',
    scan_scope_label: '已授权 RustFS 只读范围',
  },
  identity: {
    access_label: '最小只读权限有效',
    access_state: 'READY',
    policy_source_label: '可信管控中心',
    site_role_label: 'EDGE 子工厂',
  },
  meta: viewMeta,
  policy_state: 'READY',
  site_id: 'factory-a-001',
};

const registration = {
  can_generate_identity: true,
  capabilities: [
    {
      detail: '系统密钥存储可用',
      label: '本机身份生成',
      state: 'READY',
    },
    {
      detail: '注册包签名链有效',
      label: '可信根校验',
      state: 'READY',
    },
    {
      detail: '站点 ID 与本机预期一致',
      label: '站点绑定',
      state: 'READY',
    },
  ],
  meta: viewMeta,
  package: {
    control_label: '中心 B',
    expires_at: '2026-07-25T18:42:00+08:00',
    package_id: 'regpkg_20260724_001',
    signature_valid: true,
    site_display_name: '工厂 A',
    site_id: 'factory-a-001',
    site_role: 'EDGE',
    state: 'VALID',
  },
  phase: 'CONFIRM',
  site_id: 'factory-a-001',
};

const site = {
  active_alerts: 1,
  can_trigger_collection: false,
  central: {
    conflict_locked_versions: 0,
    ingesting_batches: 1,
    issued_receipts: 231,
    target_verified_versions: 1024,
  },
  collection_blocked_reason: '需要 CONTROL_ADMIN 权限',
  data_as_of: '2026-07-24T18:42:00+08:00',
  disks: { connected: 12, label: '正常', state: 'READY' },
  display_name: '工厂 A-001',
  in_transit_bytes: 12_000_000_000_000,
  site_id: 'factory-a-001',
  snapshot_state: 'COLLECTION_FAILED',
  source: {
    in_transit_versions: 16,
    local_failed_versions: 2,
    new_object_versions: 128,
    packed_waiting_transport_versions: 32,
    waiting_for_media_versions: 64,
  },
  unsynced_object_versions: 128,
};

const siteDetail = {
  display_timezone: 'Asia/Shanghai',
  latest_complete_snapshot_id: 'snap-a001-0042',
  latest_complete_snapshot_seq: 42,
  meta: viewMeta,
  period_end_inclusive: '2026-07-24T18:42:00+08:00',
  period_start_exclusive: '2026-07-23T18:42:00+08:00',
  recent_batches: [],
  site,
};

const failedCollection = {
  collection_job_id: 'collect_visual001',
  completed_at: '2026-07-24T18:42:09+08:00',
  failure_reason: '子工厂签名摘要与传输摘要不一致',
  failure_stage: '校验快照',
  meta: {
    ...viewMeta,
    freshness: 'FAILED_RETAINED',
    retained_after_failure: true,
    status_message: '采集失败，保留最近完整快照',
  },
  next_scheduled_at: '2026-07-24T19:00:00+08:00',
  queued_at: '2026-07-24T18:41:30+08:00',
  site_id: 'factory-a-001',
  snapshot_id: null,
  snapshot_seq: 42,
  source_delta: null,
  stage: 'FAILED',
  stages: [
    {
      at: '2026-07-24T18:41:30+08:00',
      label: '排队',
      stage: 'QUEUED',
      state: 'COMPLETED',
    },
    {
      at: '2026-07-24T18:41:32+08:00',
      label: '建立连接',
      stage: 'CONNECT',
      state: 'COMPLETED',
    },
    {
      at: '2026-07-24T18:41:36+08:00',
      label: '读取源端事实',
      stage: 'READ_SOURCE',
      state: 'COMPLETED',
    },
    {
      at: '2026-07-24T18:41:50+08:00',
      label: '读取中心事实',
      stage: 'READ_CENTRAL',
      state: 'COMPLETED',
    },
    {
      at: '2026-07-24T18:42:09+08:00',
      label: '校验快照',
      stage: 'VALIDATE',
      state: 'FAILED',
    },
    { at: null, label: '重建投影', stage: 'REBUILD', state: 'PENDING' },
    { at: null, label: '发布快照', stage: 'PUBLISH', state: 'PENDING' },
  ],
  trigger: 'ON_DEMAND',
  validation: null,
};

async function context(page: Page, role: 'CONTROL' | 'EDGE') {
  await page.route('**/api/local/v1/context', (route) => {
    expect(route.request().headers()['x-request-id']).toMatch(
      /^req_[A-Za-z0-9_-]{8,120}$/,
    );
    route.fulfill({
      body: JSON.stringify({
        display_name: role === 'EDGE' ? '工厂 A' : '中心 B',
        meta: viewMeta,
        permissions: ['VIEW_LOCAL_STATUS'],
        role,
        site_id: role === 'EDGE' ? 'factory-a-001' : 'control-b',
      }),
      contentType: 'application/json',
    });
  });
}

async function attachVisual(
  page: Page,
  testInfo: TestInfo,
  name: string,
  animations: 'allow' | 'disabled' = 'disabled',
) {
  await expect(page.locator('#nprogress')).toHaveCount(0);
  await expect(page.locator('#__app-loading__')).toBeHidden();
  const path = testInfo.outputPath(`${name}.png`);
  const image = await page.screenshot({
    animations,
    fullPage: true,
    path,
  });
  expect(image.byteLength).toBeGreaterThan(40_000);
  await testInfo.attach(name, { contentType: 'image/png', path });
}

test('A-01 shows connected media and one authoritative action at 1672×941', async ({
  page,
}, testInfo) => {
  await page.setViewportSize({ height: 941, width: 1672 });
  await page.emulateMedia({ reducedMotion: 'no-preference' });
  await context(page, 'EDGE');
  await page.route('**/api/local/v1/edge/runtime-overview', (route) =>
    route.fulfill({
      body: JSON.stringify(runtime),
      contentType: 'application/json',
    }),
  );

  await page.goto('/#/edge');
  await expect(page.getByText('运输硬盘', { exact: true })).toBeVisible();
  await expect(
    page.getByRole('button', { name: '查看 RustFS 服务器' }),
  ).toBeVisible();
  await expect(page.locator('.media-slots i')).toHaveCount(16);
  await expect(
    page.locator('[data-particle-renderer="path-aether"]'),
  ).toBeVisible();
  await expect(page.getByText('完成当前阶段后更换此盘')).toHaveCount(1);
  await expect(page.getByRole('progressbar')).toHaveAttribute('value', '68');
  await attachVisual(page, testInfo, 'A-01-1672x941');
});

test('A-06 shows the selected transport disk detail with baseline hierarchy at 1672×941', async ({
  page,
}, testInfo) => {
  await page.setViewportSize({ height: 941, width: 1672 });
  await page.emulateMedia({ reducedMotion: 'reduce' });
  await context(page, 'EDGE');
  await page.route('**/api/local/v1/edge/runtime-overview', (route) =>
    route.fulfill({
      body: JSON.stringify(runtime),
      contentType: 'application/json',
    }),
  );
  await page.route('**/api/local/v1/edge/nas-disks', (route) =>
    route.fulfill({
      body: JSON.stringify(transportDisks),
      contentType: 'application/json',
    }),
  );

  await page.goto('/#/edge');
  await page.getByRole('button', { name: '查看运输 NAS' }).click();
  await expect(page).toHaveURL(/#\/edge\/nas\/disks$/);
  await expect(page.getByText('运输盘位', { exact: true })).toBeVisible();
  await expect(page.locator('.nas-disks-header.ant-page-header')).toBeVisible();
  await expect(page.locator('.nas-disk-card.ant-card')).toHaveCount(16);
  await expect(page.getByText('当前显示 01–08')).toHaveCount(0);
  await expect(
    page.locator('.nas-disks-header .ant-page-header-heading-extra'),
  ).toContainText('最近扫描 14:32:08');
  await expect(page.locator('.disk-summary b').first()).toHaveCSS(
    'font-size',
    '24px',
  );
  await expect(page.getByText('温度注意')).toBeVisible();
  await expect(page.locator('.nas-disk-card.is-selected')).toContainText('04');
  await expect(page.getByText('运输盘 04 信息')).toBeVisible();
  await expect(page.getByText('数据传输', { exact: true })).toBeVisible();
  await expect(page.getByText('4.08 / 6 TB')).toBeVisible();
  await expect(page.locator('.transport-disk-cutout')).toBeVisible();
  const nasDisksRuntimeBox = await page.locator('.runtime-view').boundingBox();
  const nasDisksFooterBox = await page
    .locator('.nas-disks-footer')
    .boundingBox();
  const nasDiskScrollBox = await page.locator('.nas-disk-scroll').boundingBox();
  expect(
    ((nasDisksFooterBox?.y ?? 0) - (nasDisksRuntimeBox?.y ?? 0)) /
      (nasDisksRuntimeBox?.height ?? 1),
  ).toBeCloseTo(0.684, 2);
  expect(
    (nasDisksFooterBox?.height ?? 0) / (nasDisksRuntimeBox?.height ?? 1),
  ).toBeCloseTo(0.316, 2);
  expect(
    (nasDisksFooterBox?.y ?? 0) -
      ((nasDiskScrollBox?.y ?? 0) + (nasDiskScrollBox?.height ?? 0)),
  ).toBeCloseTo(20, 0);
  await expect(page.locator('.nas-disks-footer')).toHaveCSS(
    'background-color',
    'rgb(6, 16, 26)',
  );
  await expect(page.locator('.nas-disk-card').first()).toHaveCSS(
    'height',
    '150px',
  );
  await expect(page.locator('.nas-disk-card').first()).toHaveCSS(
    'background-color',
    'rgba(4, 12, 20, 0.18)',
  );
  await expect(page.locator('.nas-disk-card .ant-card-body').first()).toHaveCSS(
    'display',
    'flex',
  );
  await expect(page.locator('.nas-disk-card .ant-card-body').first()).toHaveCSS(
    'flex-direction',
    'column',
  );
  await expect(page.locator('.nas-disk-card .disk-health').first()).toHaveCSS(
    'margin-top',
    'auto',
  );
  expect(
    await page
      .locator('.nas-disk-card')
      .first()
      .evaluate((element) => getComputedStyle(element).backdropFilter),
  ).toContain('blur(10px)');
  expect(
    await page
      .locator('.nas-disk-card')
      .first()
      .evaluate((element) => getComputedStyle(element).backgroundImage),
  ).toBe('none');
  const nasDiskCardMotion = await page
    .locator('.nas-disk-card')
    .first()
    .evaluate((element) => {
      const style = getComputedStyle(element);
      return {
        transitionDuration: style.transitionDuration,
        transitionProperty: style.transitionProperty,
      };
    });
  expect(nasDiskCardMotion.transitionProperty).toContain('transform');
  expect(nasDiskCardMotion.transitionProperty).toContain('box-shadow');
  expect(nasDiskCardMotion.transitionDuration).toContain('0.18s');
  await page.locator('.nas-disk-card').first().hover();
  await expect(page.locator('.nas-disk-card').first()).toHaveCSS(
    'transform',
    'none',
  );
  await expect(page.locator('.nas-disk-card.is-selected')).toHaveCSS(
    'transform',
    'none',
  );
  await expect(page.locator('.nas-disk-card header strong').first()).toHaveCSS(
    'font-size',
    '32px',
  );
  const nasContentMotion = await page
    .locator('.nas-disks-header')
    .evaluate((element) => {
      const style = getComputedStyle(element);
      return {
        animationDuration: style.animationDuration,
        animationName: style.animationName,
      };
    });
  expect(nasContentMotion.animationName).toBe('nas-disks-section-enter');
  expect(Number.parseFloat(nasContentMotion.animationDuration)).toBeCloseTo(
    0.26,
  );
  await expect(
    page.locator('.scene-nas img[src*="transport-nas-cutout-v3.webp"]'),
  ).toHaveCount(1);
  expect(
    await page
      .locator('.scene-nas')
      .evaluate((element) => getComputedStyle(element, '::after').content),
  ).toBe('none');
  expect(
    await page
      .locator('.scene-nas')
      .evaluate((element) => getComputedStyle(element, '::before').content),
  ).toBe('none');
  await expect(page.locator('.scene-nas')).toHaveCSS('filter', 'none');
  await expect(page.locator('.scene-nas')).toHaveCSS('top', '156px');
  await expect(page.locator('.nas-disk-scroll')).toHaveCSS(
    'overflow-y',
    'auto',
  );
  await expect(page.locator('.nas-disk-scroll')).toHaveCSS(
    'scrollbar-color',
    'rgb(33, 184, 255) rgb(28, 44, 57)',
  );
  expect(
    await page.locator('.nas-disk-scroll').evaluate((element) => {
      const button = getComputedStyle(element, '::-webkit-scrollbar-button');
      const track = getComputedStyle(element, '::-webkit-scrollbar-track');
      return {
        buttonDisplay: button.display,
        buttonHeight: button.height,
        trackBackgroundColor: track.backgroundColor,
      };
    }),
  ).toEqual({
    buttonDisplay: 'none',
    buttonHeight: '0px',
    trackBackgroundColor: 'rgb(28, 44, 57)',
  });
  await attachVisual(page, testInfo, 'A-06-transport-disk-detail-1672x941');
});

test('A-01 preserves the baseline ratio and animates at 2048×1111', async ({
  page,
}, testInfo) => {
  await page.setViewportSize({ height: 1111, width: 2048 });
  await context(page, 'EDGE');
  await page.route('**/api/local/v1/edge/runtime-overview', (route) =>
    route.fulfill({
      body: JSON.stringify(runtime),
      contentType: 'application/json',
    }),
  );

  await page.goto('/#/edge');
  const shell = page.locator('.product-shell-immersive');
  const shellBox = await shell.boundingBox();
  expect(shellBox).not.toBeNull();
  expect(
    Math.abs((shellBox?.width ?? 0) / (shellBox?.height ?? 1) - 1672 / 941),
  ).toBeLessThan(0.002);
  expect(shellBox?.x).toBeGreaterThan(30);
  expect((shellBox?.x ?? 0) + (shellBox?.width ?? 0)).toBeLessThan(2018);

  const canvas = page.locator('[data-particle-renderer="path-aether"]');
  await expect(canvas).toBeVisible();
  const heartbeat = page.locator('[data-runtime-heartbeat] .heartbeat-pulse');
  await expect(heartbeat).toBeVisible();
  const firstHeartbeatFrame = await heartbeat.evaluate(
    (element) => getComputedStyle(element).strokeDashoffset,
  );
  const firstFrame = await canvas.evaluate((element) =>
    (element as HTMLCanvasElement).toDataURL(),
  );
  await page.waitForTimeout(250);
  const secondHeartbeatFrame = await heartbeat.evaluate(
    (element) => getComputedStyle(element).strokeDashoffset,
  );
  const secondFrame = await canvas.evaluate((element) =>
    (element as HTMLCanvasElement).toDataURL(),
  );
  expect(secondHeartbeatFrame).not.toBe(firstHeartbeatFrame);
  expect(secondFrame).not.toBe(firstFrame);
  await attachVisual(page, testInfo, 'A-01-running-particles-2048x1111');
});

test('A-01 scales the frozen canvas at 1920×1080 and 1280×720', async ({
  page,
}, testInfo) => {
  await context(page, 'EDGE');
  await page.route('**/api/local/v1/edge/runtime-overview', (route) =>
    route.fulfill({
      body: JSON.stringify(runtime),
      contentType: 'application/json',
    }),
  );

  for (const viewport of [
    { height: 1080, name: '1920x1080', width: 1920 },
    { height: 720, name: '1280x720', width: 1280 },
  ]) {
    await page.setViewportSize(viewport);
    await page.goto('/#/edge');
    const shellBox = await page
      .locator('.product-shell-immersive')
      .boundingBox();
    expect(shellBox).not.toBeNull();
    expect(
      Math.abs((shellBox?.width ?? 0) / (shellBox?.height ?? 1) - 1672 / 941),
    ).toBeLessThan(0.002);
    expect(shellBox?.x).toBeGreaterThanOrEqual(-0.5);
    expect(shellBox?.y).toBeGreaterThanOrEqual(-0.5);
    expect((shellBox?.width ?? 0) + (shellBox?.x ?? 0)).toBeLessThanOrEqual(
      viewport.width + 0.5,
    );
    expect((shellBox?.height ?? 0) + (shellBox?.y ?? 0)).toBeLessThanOrEqual(
      viewport.height + 0.5,
    );
    await attachVisual(page, testInfo, `A-01-${viewport.name}`);
  }
});

test('A-01 keeps device identity through server and NAS detail transitions', async ({
  page,
}, testInfo) => {
  await page.setViewportSize({ height: 941, width: 1672 });
  await context(page, 'EDGE');
  await page.route('**/api/local/v1/edge/runtime-overview', (route) =>
    route.fulfill({
      body: JSON.stringify(runtime),
      contentType: 'application/json',
    }),
  );
  await page.route('**/api/local/v1/edge/server-status', (route) =>
    route.fulfill({
      body: JSON.stringify(serverStatus),
      contentType: 'application/json',
    }),
  );

  await page.goto('/#/edge');
  const source = page.getByRole('button', {
    name: '查看 RustFS 服务器',
  });
  const nas = page.getByRole('button', { name: '查看运输 NAS' });
  await expect(source).toHaveCSS('filter', 'none');
  await expect(nas).toHaveCSS('filter', 'none');
  for (const device of [source, nas]) {
    expect(
      await device.evaluate(
        (element) => getComputedStyle(element, '::before').content,
      ),
    ).toBe('none');
  }
  await source.evaluate((element) => {
    element.dataset.continuityToken = 'source-preserved';
  });
  await nas.evaluate((element) => {
    element.dataset.continuityToken = 'nas-preserved';
  });
  const homeSourceBox = await source.boundingBox();
  const homeNasBox = await nas.boundingBox();
  const homeFooterBox = await page.locator('.runtime-dashboard').boundingBox();
  const homeFooterBackground = await page
    .locator('.runtime-dashboard')
    .evaluate((element) => getComputedStyle(element).backgroundColor);
  await source.evaluate((element) => {
    element.addEventListener(
      'click',
      () => {
        const samples: number[] = [];
        const crossfadeSamples: Array<{
          detail: number;
          home: number;
        }> = [];
        Reflect.set(window, '__fustfsSourceHeightSamples', samples);
        Reflect.set(window, '__fustfsEnterCrossfadeSamples', crossfadeSamples);
        let remainingFrames = 40;
        const sample = () => {
          samples.push(element.getBoundingClientRect().height);
          const dashboard = document.querySelector('.runtime-dashboard');
          const detail = document.querySelector('.device-detail-layer');
          crossfadeSamples.push({
            detail: detail
              ? Number.parseFloat(getComputedStyle(detail).opacity)
              : 0,
            home: dashboard
              ? Number.parseFloat(getComputedStyle(dashboard).opacity)
              : 0,
          });
          remainingFrames -= 1;
          if (remainingFrames > 0) requestAnimationFrame(sample);
        };
        requestAnimationFrame(sample);
      },
      { once: true },
    );
  });

  await source.click();
  await expect(page).toHaveURL(/#\/edge\/server$/);
  await expect(page.locator('[data-device-focus="server"]')).toBeVisible();
  await expect(
    page.getByRole('heading', { name: '服务器运行状态' }),
  ).toBeVisible();
  await expect(source).toHaveAttribute(
    'data-continuity-token',
    'source-preserved',
  );
  await expect(nas).toHaveAttribute('data-continuity-token', 'nas-preserved');
  await expect(source).toHaveCSS(
    'transition-timing-function',
    /cubic-bezier\(0.22, 1, 0.36, 1\)/,
  );
  await expect(source).toHaveCSS('transition-duration', /0.42s/);
  await page.waitForTimeout(520);
  const sourceHeightSamples = await page.evaluate(
    () =>
      Reflect.get(window, '__fustfsSourceHeightSamples') as
        | number[]
        | undefined,
  );
  expect(sourceHeightSamples?.length).toBeGreaterThan(8);
  expect(Math.max(...(sourceHeightSamples ?? []))).toBeLessThanOrEqual(
    (homeSourceBox?.height ?? 0) + 1,
  );
  const enterCrossfadeSamples = await page.evaluate(
    () =>
      Reflect.get(window, '__fustfsEnterCrossfadeSamples') as
        | Array<{ detail: number; home: number }>
        | undefined,
  );
  expect(
    enterCrossfadeSamples?.some(
      ({ detail, home }) => detail > 0.05 && home > 0.05,
    ),
  ).toBe(true);
  expect(
    Math.min(
      ...(enterCrossfadeSamples ?? []).map(({ detail, home }) =>
        Math.max(detail, home),
      ),
    ),
  ).toBeGreaterThan(0.45);
  const serverFooterBox = await page.locator('.persistent-strip').boundingBox();
  const serverFooterStyle = await page
    .locator('.persistent-strip')
    .evaluate((element) => {
      const footer = getComputedStyle(element);
      const article = getComputedStyle(
        element.querySelector('article') as HTMLElement,
      );
      return {
        articlePadding: article.padding,
        background: footer.background,
        backgroundColor: footer.backgroundColor,
        borderTop: footer.borderTop,
      };
    });
  expect(serverFooterStyle.backgroundColor).toBe(homeFooterBackground);
  expect(
    await page
      .locator('.device-detail-layer.detail-server')
      .evaluate((element) =>
        getComputedStyle(element)
          .getPropertyValue('--fd-detail-enter-x')
          .trim(),
      ),
  ).toBe('42px');
  expect(
    Math.abs((serverFooterBox?.y ?? 0) - (homeFooterBox?.y ?? 0)),
  ).toBeLessThan(1);
  expect(
    Math.abs((serverFooterBox?.height ?? 0) - (homeFooterBox?.height ?? 0)),
  ).toBeLessThan(1);
  const focusedSourceBox = await source.boundingBox();
  expect(
    (focusedSourceBox?.y ?? 0) + (focusedSourceBox?.height ?? 0),
  ).toBeGreaterThanOrEqual(598);
  expect(
    (focusedSourceBox?.y ?? 0) + (focusedSourceBox?.height ?? 0),
  ).toBeLessThanOrEqual(601);
  expect(
    await source.evaluate(
      (element) => getComputedStyle(element, '::before').zIndex,
    ),
  ).toBe('0');
  const capabilityBox = await page.locator('.capability-row').boundingBox();
  expect(Math.abs((capabilityBox?.y ?? 0) - 492)).toBeLessThan(3);
  const tabsBox = await page.locator('.server-tabs').boundingBox();
  const readinessBox = await page.locator('.readiness-area').boundingBox();
  expect(Math.abs((tabsBox?.x ?? 0) - (readinessBox?.x ?? 0))).toBeLessThan(1);
  const transportTitleBox = await page
    .locator('.transport-summary > h2')
    .boundingBox();
  const transportImageBox = await page
    .locator('.transport-summary > img')
    .boundingBox();
  const taskImageBox = await page.locator('.current-task > img').boundingBox();
  expect(
    Math.abs((taskImageBox?.width ?? 0) - (transportImageBox?.width ?? 0)),
  ).toBeLessThan(1);
  expect(
    Math.abs((taskImageBox?.height ?? 0) - (transportImageBox?.height ?? 0)),
  ).toBeLessThan(1);
  expect(
    Math.abs((taskImageBox?.y ?? 0) - (transportImageBox?.y ?? 0)),
  ).toBeLessThan(1);
  expect(
    (transportImageBox?.y ?? 0) -
      ((transportTitleBox?.y ?? 0) + (transportTitleBox?.height ?? 0)),
  ).toBeGreaterThan(8);
  await expect(page.locator('.current-task .task-icon')).toBeVisible();
  await expect(page.locator('.current-task .task-icon')).toHaveAttribute(
    'src',
    /task-database-sync-v1\.png$/,
  );
  await expect(page.locator('.task-facts svg')).toHaveCount(3);
  await expect(page.locator('.server-tabs.ant-tabs')).toBeVisible();
  await expect(
    page.locator('.health-progress.ant-progress-circle'),
  ).toBeVisible();
  await expect(
    page.locator('.storage-progress.ant-progress-line'),
  ).toBeVisible();
  await expect(page.locator('.trend-chart canvas')).toHaveCount(1);
  await expect(page.locator('.readiness-area')).toHaveCSS('display', 'flex');
  await expect(page.locator('.readiness-area')).toHaveCSS(
    'justify-content',
    'space-between',
  );
  await expect(page.locator('.health-ring')).toHaveCSS('align-items', 'center');
  await expect(page.locator('.capability-row')).toHaveCSS('display', 'flex');
  await expect(page.locator('.capability-row')).toHaveCSS(
    'justify-content',
    'space-between',
  );
  await expect(page.locator('.capability-row')).toHaveCSS(
    'align-items',
    'center',
  );
  const trendChartBox = await page.locator('.trend-chart').boundingBox();
  const readinessAreaBox = await page.locator('.readiness-area').boundingBox();
  expect(
    (trendChartBox?.y ?? 0) + (trendChartBox?.height ?? 0),
  ).toBeLessThanOrEqual(
    (readinessAreaBox?.y ?? 0) + (readinessAreaBox?.height ?? 0) + 1,
  );
  const storageDetailRows = await page
    .locator('.storage-metric .metric-details p')
    .evaluateAll((elements) =>
      elements.map((element) => element.getBoundingClientRect().y),
    );
  const discoveryDetailRows = await page
    .locator('.discovery-metric .metric-details p')
    .evaluateAll((elements) =>
      elements.map((element) => element.getBoundingClientRect().y),
    );
  expect(storageDetailRows).toHaveLength(2);
  expect(discoveryDetailRows).toHaveLength(2);
  expect(
    Math.max(
      ...storageDetailRows.map((row, index) =>
        Math.abs(row - (discoveryDetailRows[index] ?? row)),
      ),
    ),
  ).toBeLessThan(1);
  const capabilityWidths = await page
    .locator('.capability-row > div')
    .evaluateAll((elements) =>
      elements.map((element) => element.getBoundingClientRect().width),
    );
  expect(
    Math.max(...capabilityWidths) - Math.min(...capabilityWidths),
  ).toBeLessThanOrEqual(1);
  const readinessChildrenFit = await page
    .locator('.readiness-area')
    .evaluate((element) => {
      const container = element.getBoundingClientRect();
      return [...element.children].every((child) => {
        const bounds = child.getBoundingClientRect();
        return (
          bounds.left >= container.left - 1 &&
          bounds.right <= container.right + 1 &&
          bounds.top >= container.top - 1 &&
          bounds.bottom <= container.bottom + 1
        );
      });
    });
  expect(readinessChildrenFit).toBe(true);
  expect(
    await page
      .locator('.runtime-view.focus-server')
      .evaluate((element) =>
        getComputedStyle(element).backgroundImage.includes('529px'),
      ),
  ).toBe(false);
  const serverClose = page.getByRole('link', {
    name: '关闭服务器详情并返回运行首页',
  });
  await expect(serverClose).toHaveCSS('width', '32px');
  await expect(serverClose).toHaveCSS('height', '32px');
  await expect(serverClose).toHaveCSS('border-radius', '50%');
  await expect(serverClose).toHaveCSS(
    'background-color',
    'rgba(72, 83, 98, 0.28)',
  );
  await serverClose.evaluate((element) => {
    element.addEventListener(
      'click',
      () => {
        const crossfadeSamples: Array<{
          detail: number;
          home: number;
        }> = [];
        Reflect.set(window, '__fustfsLeaveCrossfadeSamples', crossfadeSamples);
        let remainingFrames = 24;
        const sample = () => {
          const dashboard = document.querySelector('.runtime-dashboard');
          const detail = document.querySelector('.device-detail-layer');
          crossfadeSamples.push({
            detail: detail
              ? Number.parseFloat(getComputedStyle(detail).opacity)
              : 0,
            home: dashboard
              ? Number.parseFloat(getComputedStyle(dashboard).opacity)
              : 0,
          });
          remainingFrames -= 1;
          if (remainingFrames > 0) requestAnimationFrame(sample);
        };
        requestAnimationFrame(sample);
      },
      { once: true },
    );
  });
  await attachVisual(page, testInfo, 'A-01-to-A-02-server-focus');

  const canvas = page.locator('[data-particle-renderer="path-aether"]');
  await page.waitForTimeout(160);
  const pausedFrame = await canvas.evaluate((element) =>
    (element as HTMLCanvasElement).toDataURL(),
  );
  await page.waitForTimeout(160);
  expect(
    await canvas.evaluate((element) =>
      (element as HTMLCanvasElement).toDataURL(),
    ),
  ).toBe(pausedFrame);

  await serverClose.click();
  await expect(page).toHaveURL(/#\/edge$/);
  await expect(source).toHaveCSS('transition-duration', /0.32s/);
  await expect(source).toHaveAttribute(
    'data-continuity-token',
    'source-preserved',
  );
  await expect(page.locator('.device-detail-layer')).toHaveCount(0, {
    timeout: 1000,
  });
  const leaveCrossfadeSamples = await page.evaluate(
    () =>
      Reflect.get(window, '__fustfsLeaveCrossfadeSamples') as
        | Array<{ detail: number; home: number }>
        | undefined,
  );
  expect(
    leaveCrossfadeSamples?.some(
      ({ detail, home }) => detail > 0.05 && home > 0.05,
    ),
  ).toBe(true);
  expect(
    Math.min(
      ...(leaveCrossfadeSamples ?? [])
        .filter(({ detail }) => detail > 0)
        .map(({ detail, home }) => Math.max(detail, home)),
    ),
  ).toBeGreaterThan(0.45);
  const restoredSourceBox = await source.boundingBox();
  expect(
    Math.abs((restoredSourceBox?.x ?? 0) - (homeSourceBox?.x ?? 0)),
  ).toBeLessThan(2);
  expect(
    Math.abs((restoredSourceBox?.width ?? 0) - (homeSourceBox?.width ?? 0)),
  ).toBeLessThan(2);

  await nas.click();
  await expect(page).toHaveURL(/#\/edge\/nas$/);
  await expect(page.locator('[data-device-focus="nas"]')).toBeVisible();
  await expect(
    page.getByRole('heading', { name: '运输 NAS 运行状态' }),
  ).toBeVisible();
  await expect(
    page.getByRole('link', { name: 'RustFS离线同步中心' }),
  ).toBeVisible();
  await expect(
    page.getByRole('link', {
      name: '关闭运输 NAS 详情并返回运行首页',
    }),
  ).toBeVisible();
  await expect(source).toHaveAttribute(
    'data-continuity-token',
    'source-preserved',
  );
  await expect(nas).toHaveAttribute('data-continuity-token', 'nas-preserved');
  await expect(page.getByText('尚无权威数据')).toBeVisible();
  await expect(page.locator('.nas-tabs.ant-tabs')).toBeVisible();
  await expect(page.locator('.nas-tabs [role="tab"]')).toHaveCount(2);
  await expect(
    page.locator('.nas-tabs').getByRole('tab', { name: '设置' }),
  ).toHaveCount(0);
  await expect(page.locator('.nas-tabs .ant-tabs-nav')).toHaveCSS(
    'transition-duration',
    '0s',
  );
  await expect(page.locator('.nas-tabs .ant-tabs-nav')).toHaveCSS(
    'animation-name',
    'none',
  );
  await expect(page.locator('.nas-tabs .ant-tabs-nav-list')).toHaveCSS(
    'transform',
    'none',
  );
  await expect(
    page.locator('.nas-task-progress.ant-progress-line'),
  ).toBeVisible();
  await expect(page.locator('.nas-trend-chart canvas')).toHaveCount(1);
  await expect(page.locator('.nas-speed-indicator')).toBeVisible();
  await expect(page.locator('.nas-write-state.ant-badge')).toBeVisible();
  await expect(
    page.locator('.nas-speed-indicator [data-runtime-heartbeat]'),
  ).toBeVisible();
  await expect(page.locator('.nas-status-row span').first()).toHaveCSS(
    'justify-content',
    'center',
  );
  const nasRevealSequence = await page
    .locator('.nas-overview > article, .nas-status-row')
    .evaluateAll((elements) =>
      elements.map((element) => {
        const style = getComputedStyle(element);
        return {
          delay: style.animationDelay,
          name: style.animationName,
        };
      }),
    );
  expect(nasRevealSequence).toEqual([
    { delay: '0.08s', name: 'nas-info-reveal-from-left' },
    { delay: '0.13s', name: 'nas-info-reveal-from-left' },
    { delay: '0.18s', name: 'nas-info-reveal-from-left' },
    { delay: '0.22s', name: 'nas-info-reveal-from-left' },
  ]);
  const focusedNasBox = await nas.boundingBox();
  expect(focusedNasBox?.x ?? 0).toBeGreaterThan(homeNasBox?.x ?? 0);
  expect(focusedNasBox?.x ?? 0).toBeGreaterThan(1170);
  expect(focusedNasBox?.width ?? 0).toBeGreaterThan(400);
  expect(focusedNasBox?.width ?? 0).toBeLessThan(440);
  expect(focusedNasBox?.y ?? 0).toBeGreaterThan(110);
  expect(focusedNasBox?.y ?? 0).toBeLessThan(112);
  expect((focusedNasBox?.y ?? 0) + (focusedNasBox?.height ?? 0)).toBeLessThan(
    477,
  );
  expect(
    (focusedNasBox?.y ?? 0) + (focusedNasBox?.height ?? 0),
  ).toBeGreaterThan(475);
  expect(
    Number.parseFloat(
      await source.evaluate((element) => getComputedStyle(element).opacity),
    ),
  ).toBe(0);
  await expect(source).toHaveCSS('visibility', 'hidden');
  expect(
    await nas.evaluate((element) =>
      getComputedStyle(element, '::before').backgroundImage.includes(
        'radial-gradient',
      ),
    ),
  ).toBe(true);
  const nasFooterBox = await page
    .locator('.nas-persistent-strip')
    .boundingBox();
  const nasFooterStyle = await page
    .locator('.nas-persistent-strip')
    .evaluate((element) => {
      const footer = getComputedStyle(element);
      const article = getComputedStyle(
        element.querySelector('article') as HTMLElement,
      );
      return {
        articlePadding: article.padding,
        background: footer.background,
        backgroundColor: footer.backgroundColor,
        borderTop: footer.borderTop,
      };
    });
  expect(nasFooterStyle).toEqual(serverFooterStyle);
  const taskFooterFits = await page
    .locator('.nas-current-task')
    .evaluate((element) => {
      const footer = element.getBoundingClientRect();
      const facts = element.querySelector('dl')?.getBoundingClientRect();
      return facts ? facts.bottom <= footer.bottom - 12 : false;
    });
  expect(taskFooterFits).toBe(true);
  await expect(page.locator('.nas-current-task dt img')).toHaveCount(3);
  await expect(page.getByText('预计剩余时间可信度')).toBeVisible();
  await expect(page.getByText('8.4 / 12.3 TB')).toBeVisible();
  const taskFactAlignment = await page
    .locator('.nas-current-task dl > div')
    .evaluateAll((facts) =>
      facts.map((fact) => {
        const icon = fact.querySelector('dt img')?.getBoundingClientRect();
        const label = fact.querySelector('dt span')?.getBoundingClientRect();
        const value = fact.querySelector('dd')?.getBoundingClientRect();
        return {
          iconWidth: icon?.width ?? 0,
          labelValueOffset: Math.abs((label?.x ?? 0) - (value?.x ?? 0)),
        };
      }),
    );
  expect(taskFactAlignment).toHaveLength(3);
  expect(Math.min(...taskFactAlignment.map(({ iconWidth }) => iconWidth))).toBe(
    22,
  );
  expect(
    Math.max(
      ...taskFactAlignment.map(({ labelValueOffset }) => labelValueOffset),
    ),
  ).toBeLessThan(1);
  const slotDistribution = await page
    .locator('.nas-slot-meter')
    .evaluate((meter) => {
      const container = meter.getBoundingClientRect();
      const slots = [...meter.querySelectorAll('i')].map((slot) =>
        slot.getBoundingClientRect(),
      );
      const gaps = slots.slice(1).map((slot, index) => {
        const previous = slots[index];
        return previous ? slot.left - previous.right : 0;
      });
      const widths = slots.map((slot) => slot.width);
      return {
        firstOffset: slots[0] ? slots[0].left - container.left : -1,
        gapSpread: gaps.length > 0 ? Math.max(...gaps) - Math.min(...gaps) : -1,
        lastOffset: slots.at(-1)
          ? container.right - (slots.at(-1)?.right ?? 0)
          : -1,
        minimumWidth: widths.length > 0 ? Math.min(...widths) : -1,
        slotCount: slots.length,
        widthSpread:
          widths.length > 0 ? Math.max(...widths) - Math.min(...widths) : -1,
      };
    });
  expect(slotDistribution.slotCount).toBe(16);
  expect(slotDistribution.firstOffset).toBeLessThan(1);
  expect(slotDistribution.lastOffset).toBeLessThan(1);
  expect(slotDistribution.gapSpread).toBeLessThan(1);
  expect(slotDistribution.minimumWidth).toBeGreaterThanOrEqual(19);
  expect(slotDistribution.widthSpread).toBeLessThan(1);
  expect(
    await page
      .locator('.device-detail-layer.detail-nas')
      .evaluate((element) =>
        getComputedStyle(element)
          .getPropertyValue('--fd-detail-enter-x')
          .trim(),
      ),
  ).toBe('-42px');
  expect(
    Math.abs((nasFooterBox?.y ?? 0) - (homeFooterBox?.y ?? 0)),
  ).toBeLessThan(1);
  expect(
    Math.abs((nasFooterBox?.height ?? 0) - (homeFooterBox?.height ?? 0)),
  ).toBeLessThan(1);
  await attachVisual(page, testInfo, 'A-01-to-A-06-nas-focus');

  await page.keyboard.press('Escape');
  await expect(page).toHaveURL(/#\/edge$/);
});

test('device details skip displacement when reduced motion is requested', async ({
  page,
}) => {
  await page.emulateMedia({ reducedMotion: 'reduce' });
  await context(page, 'EDGE');
  await page.route('**/api/local/v1/edge/runtime-overview', (route) =>
    route.fulfill({
      body: JSON.stringify(runtime),
      contentType: 'application/json',
    }),
  );
  await page.route('**/api/local/v1/edge/server-status', (route) =>
    route.fulfill({
      body: JSON.stringify(serverStatus),
      contentType: 'application/json',
    }),
  );

  await page.goto('/#/edge');
  const source = page.getByRole('button', {
    name: '查看 RustFS 服务器',
  });
  await source.click();
  await expect(
    page.getByRole('heading', { name: '服务器运行状态' }),
  ).toBeVisible();
  expect(
    await source.evaluate((element) =>
      Number.parseFloat(getComputedStyle(element).transitionDuration),
    ),
  ).toBeLessThanOrEqual(0.001);
});

test('A-03 is retired from the server tabs and direct routes', async ({
  page,
}, testInfo) => {
  await page.setViewportSize({ height: 941, width: 1672 });
  await page.emulateMedia({ reducedMotion: 'reduce' });
  await context(page, 'EDGE');
  await page.route('**/api/local/v1/edge/runtime-overview', (route) =>
    route.fulfill({
      body: JSON.stringify(runtime),
      contentType: 'application/json',
    }),
  );
  await page.route('**/api/local/v1/edge/server-status', (route) =>
    route.fulfill({
      body: JSON.stringify(serverStatus),
      contentType: 'application/json',
    }),
  );
  await page.goto('/#/edge/server');
  await expect(page.getByRole('tab')).toHaveCount(3);
  await expect(page.getByRole('tab', { name: '运行状态' })).toBeVisible();
  await expect(page.getByRole('tab', { name: '同步记录' })).toBeVisible();
  await expect(page.getByRole('tab', { name: '设置' })).toBeVisible();
  await expect(page.getByRole('tab', { name: '硬盘' })).toHaveCount(0);
  await expect(page.getByText('盘位 8 / 8')).toBeVisible();
  await expect(page.getByText('7 健康 · 1 注意 · 0 未知')).toBeVisible();
  await attachVisual(page, testInfo, 'A-02-three-tabs-A-03-retired');

  await page.goto('/#/edge/server/disks');
  await expect(page.getByText('哎呀！未找到页面')).toBeVisible();
  await page.goto('/#/edge/server/disks/01');
  await expect(page.getByText('哎呀！未找到页面')).toBeVisible();
});

test('A-04 opens from A-02 tabs and exposes failed-batch evidence', async ({
  page,
}, testInfo) => {
  await page.setViewportSize({ height: 941, width: 1672 });
  await page.emulateMedia({ reducedMotion: 'no-preference' });
  await context(page, 'EDGE');
  await page.route('**/api/local/v1/edge/runtime-overview', (route) =>
    route.fulfill({
      body: JSON.stringify(runtime),
      contentType: 'application/json',
    }),
  );
  await page.route('**/api/local/v1/edge/server-status', (route) =>
    route.fulfill({
      body: JSON.stringify(serverStatus),
      contentType: 'application/json',
    }),
  );
  await page.route('**/api/local/v1/edge/sync-records', (route) =>
    route.fulfill({
      body: JSON.stringify(edgeRecords),
      contentType: 'application/json',
    }),
  );
  await page.route('**/api/local/v1/edge/managed-settings', (route) =>
    route.fulfill({
      body: JSON.stringify(managedSettings),
      contentType: 'application/json',
    }),
  );

  await page.goto('/#/edge/server');
  const serverTabs = page.locator('.server-tabs');
  const tabsBeforeSwitch = await serverTabs.boundingBox();
  const tabItemsBeforeSwitch = await serverTabs
    .getByRole('tab')
    .evaluateAll((tabs) =>
      tabs.map((tab) => {
        const box = tab.getBoundingClientRect();
        return {
          height: box.height,
          width: box.width,
          x: box.x,
          y: box.y,
        };
      }),
    );
  const tabsMotion = await serverTabs
    .locator('.ant-tabs-nav-list')
    .evaluate((element) => {
      const style = getComputedStyle(element);
      return {
        animationName: style.animationName,
        transform: style.transform,
        transitionDuration: style.transitionDuration,
      };
    });
  expect(tabsMotion).toEqual({
    animationName: 'none',
    transform: 'none',
    transitionDuration: '0s',
  });
  await page.getByRole('tab', { name: '同步记录' }).click();
  await expect(page).toHaveURL(/#\/edge\/server\/records$/);
  await expect(
    page.getByRole('heading', { level: 2, name: '同步记录' }),
  ).toBeVisible();
  const tabsAfterSwitch = await serverTabs.boundingBox();
  expect(
    Math.abs((tabsBeforeSwitch?.x ?? 0) - (tabsAfterSwitch?.x ?? 0)),
  ).toBeLessThan(0.5);
  expect(
    Math.abs((tabsBeforeSwitch?.y ?? 0) - (tabsAfterSwitch?.y ?? 0)),
  ).toBeLessThan(0.5);
  const tabItemsAfterSwitch = await serverTabs
    .getByRole('tab')
    .evaluateAll((tabs) =>
      tabs.map((tab) => {
        const box = tab.getBoundingClientRect();
        return {
          height: box.height,
          width: box.width,
          x: box.x,
          y: box.y,
        };
      }),
    );
  expect(tabItemsAfterSwitch).toEqual(tabItemsBeforeSwitch);
  const contentMotion = await page
    .locator('.record-list')
    .evaluate((element) => {
      const style = getComputedStyle(element);
      return {
        animationDuration: style.animationDuration,
        animationName: style.animationName,
      };
    });
  expect(contentMotion.animationName).toBe('server-section-enter');
  expect(Number.parseFloat(contentMotion.animationDuration)).toBeCloseTo(0.26);
  await page.waitForTimeout(300);
  const recordTabsBox = await page.locator('.server-tabs').boundingBox();
  const recordListBox = await page.locator('.record-list').boundingBox();
  expect(
    Math.abs((recordTabsBox?.x ?? 0) - (recordListBox?.x ?? 0)),
  ).toBeLessThan(1);
  const headingCopyBox = await page
    .locator('.records-heading-copy')
    .boundingBox();
  const mediaNoticeBox = await page.locator('.media-notice').boundingBox();
  expect(
    Math.abs(
      (headingCopyBox?.y ?? 0) +
        (headingCopyBox?.height ?? 0) / 2 -
        ((mediaNoticeBox?.y ?? 0) + (mediaNoticeBox?.height ?? 0) / 2),
    ),
  ).toBeLessThan(2);
  const recordsTableBox = await page.locator('.records-table').boundingBox();
  const summaryAlignment = await page
    .locator('.record-summary')
    .evaluate((summary) => {
      const summaryRect = summary.getBoundingClientRect();
      const summaryCenter = summaryRect.y + summaryRect.height / 2;
      return [...summary.querySelectorAll('button')].map((button) => {
        const labelRect = button.querySelector('span')?.getBoundingClientRect();
        const valueRect = button
          .querySelector('strong')
          ?.getBoundingClientRect();
        return {
          buttonHeight: button.getBoundingClientRect().height,
          labelDelta: labelRect
            ? Math.abs(labelRect.y + labelRect.height / 2 - summaryCenter)
            : Number.POSITIVE_INFINITY,
          valueDelta: valueRect
            ? Math.abs(valueRect.y + valueRect.height / 2 - summaryCenter)
            : Number.POSITIVE_INFINITY,
        };
      });
    });
  expect(
    summaryAlignment.every(
      ({ buttonHeight, labelDelta, valueDelta }) =>
        Math.abs(buttonHeight - 66) < 1 && labelDelta < 1 && valueDelta < 1,
    ),
  ).toBe(true);
  const recordsFooterBox = await page
    .locator('.records-pagination')
    .boundingBox();
  expect(recordsTableBox?.height ?? 0).toBeGreaterThan(430);
  expect(
    Math.abs(
      (recordListBox?.y ?? 0) +
        (recordListBox?.height ?? 0) -
        ((recordsFooterBox?.y ?? 0) + (recordsFooterBox?.height ?? 0)),
    ),
  ).toBeLessThan(1);
  await expect(page.getByRole('button', { name: '上一页' })).toBeDisabled();
  await expect(page.getByRole('button', { name: '下一页' })).toBeDisabled();
  expect(
    await page
      .locator('.record-scroll')
      .evaluate((element) => getComputedStyle(element).overflowY),
  ).toBe('auto');
  expect(
    await page
      .locator('.edge-records')
      .evaluate(
        (element) => getComputedStyle(element, '::before').backgroundImage,
      ),
  ).toContain('linear-gradient');
  await expect(page.getByText('当前未检测到运输盘')).toBeVisible();
  await expect(page.locator('.persistent-strip')).toHaveCount(0);
  await page.emulateMedia({ reducedMotion: 'reduce' });
  await page.waitForTimeout(650);
  await attachVisual(page, testInfo, 'A-04-list-1672x941');
  for (const viewport of [
    { height: 1080, name: '1920x1080', width: 1920 },
    { height: 720, name: '1280x720', width: 1280 },
  ]) {
    await page.setViewportSize({
      height: viewport.height,
      width: viewport.width,
    });
    await page.waitForTimeout(120);
    await attachVisual(page, testInfo, `A-04-list-${viewport.name}`);
  }
  await page.setViewportSize({ height: 941, width: 1672 });

  await page
    .getByRole('button', { name: '查看批次 A-20260720-008 详情' })
    .click();
  await expect(page).toHaveURL(/#\/edge\/server\/records\/A-20260720-008$/);
  await expect(page.getByText('已停止危险写入')).toBeVisible();
  await expect(
    page.getByText('未生成可送出标记', { exact: true }),
  ).toBeVisible();
  await expect(page.getByText('端到端闭环完成')).toHaveCount(0);
  await expect(
    page.locator('.detail-page-header.ant-page-header'),
  ).toBeVisible();
  await expect(
    page.locator('.detail-page-header .ant-page-header-back-button'),
  ).toBeVisible();
  await expect(
    page.locator('.detail-page-header .ant-page-header-back-button'),
  ).toHaveAttribute('aria-label', /返回/);
  await expect(
    page.locator('.detail-page-header .ant-page-header-heading-title'),
  ).toHaveText('批次详情');
  await expect(
    page.locator('.detail-page-header .ant-page-header-heading-sub-title'),
  ).toHaveText('A-20260720-008');
  await expect(
    page.locator('.detail-page-header .ant-page-header-heading-tags'),
  ).toContainText('批次失败');
  await expect(page.locator('.record-stage-steps.ant-steps')).toBeVisible();
  await expect(page.locator('.record-stage-steps .ant-steps-item')).toHaveCount(
    4,
  );
  await expect(page.locator('.stage-track')).toHaveCount(0);
  await expect(
    page.locator('img[src="/assets/fustfs-baseline/a04-failed-lock-v1.png"]'),
  ).toBeVisible();
  await expect(
    page.locator(
      'img[src="/assets/fustfs-baseline/a04-failed-lock-small-v1.png"]',
    ),
  ).toBeVisible();
  await expect(page.locator('.event-table.ant-table-wrapper')).toBeVisible();
  await expect(page.locator('.event-table table')).toHaveCount(1);
  await expect(page.locator('.event-table thead th')).toHaveCount(3);
  await expect(page.locator('.event-table tbody tr')).toHaveCount(1);
  const detailHeadingMetrics = await page
    .locator(
      '.detail-page-header .ant-page-header-back-button .anticon, .detail-page-header .ant-page-header-heading-title, .detail-page-header .ant-page-header-heading-sub-title, .detail-page-header .ant-page-header-heading-tags',
    )
    .evaluateAll((elements) =>
      elements.map((element) => {
        const box = element.getBoundingClientRect();
        return {
          center: box.top + box.height / 2,
          fontSize: Number.parseFloat(getComputedStyle(element).fontSize),
          height: box.height,
          width: box.width,
        };
      }),
    );
  const [backIconMetrics, titleMetrics, ...secondaryHeadingMetrics] =
    detailHeadingMetrics;
  const detailTextCenters = [titleMetrics, ...secondaryHeadingMetrics]
    .filter((metric) => metric !== undefined)
    .map(({ center }) => center);
  expect(
    Math.max(...detailTextCenters) - Math.min(...detailTextCenters),
  ).toBeLessThan(2);
  expect(
    (backIconMetrics?.center ?? 0) - (titleMetrics?.center ?? 0),
  ).toBeCloseTo(6, 0);
  expect(backIconMetrics?.width).toBeCloseTo(titleMetrics?.fontSize ?? 0, 0);
  expect(backIconMetrics?.height).toBeCloseTo(titleMetrics?.fontSize ?? 0, 0);
  await page.waitForTimeout(450);
  await attachVisual(page, testInfo, 'A-04-failed-detail-1672x941');
  for (const viewport of [
    { height: 1080, name: '1920x1080', width: 1920 },
    { height: 720, name: '1280x720', width: 1280 },
  ]) {
    await page.setViewportSize({
      height: viewport.height,
      width: viewport.width,
    });
    await page.waitForTimeout(120);
    await attachVisual(page, testInfo, `A-04-failed-detail-${viewport.name}`);
  }
  await page.setViewportSize({ height: 941, width: 1672 });

  await page
    .locator('.detail-page-header .ant-page-header-back-button')
    .click();
  await page
    .getByRole('button', { name: '查看批次 A-20260720-007 详情' })
    .click();
  await expect(page).toHaveURL(/#\/edge\/server\/records\/A-20260720-007$/);
  await expect(page.getByText('本地阶段流程')).toBeVisible();
  await expect(page.getByText('本地阶段：已完成')).toBeVisible();
  await expect(page.locator('.record-stage-steps.ant-steps')).toBeVisible();
  await expect(page.locator('.record-stage-steps .ant-steps-item')).toHaveCount(
    4,
  );
  await expect(page.locator('.stage-track')).toHaveCount(0);
  await expect(
    page.locator('img[src="/assets/fustfs-baseline/a04-packed-shield-v1.png"]'),
  ).toBeVisible();
  await page.waitForTimeout(450);
  await attachVisual(page, testInfo, 'A-04-packed-detail-1672x941');
  for (const viewport of [
    { height: 1080, name: '1920x1080', width: 1920 },
    { height: 720, name: '1280x720', width: 1280 },
  ]) {
    await page.setViewportSize({
      height: viewport.height,
      width: viewport.width,
    });
    await page.waitForTimeout(120);
    await attachVisual(page, testInfo, `A-04-packed-detail-${viewport.name}`);
  }
});

test('A-05 remains a read-only managed settings tab', async ({
  page,
}, testInfo) => {
  await page.setViewportSize({ height: 941, width: 1672 });
  await page.emulateMedia({ reducedMotion: 'reduce' });
  await context(page, 'EDGE');
  await page.route('**/api/local/v1/edge/runtime-overview', (route) =>
    route.fulfill({
      body: JSON.stringify(runtime),
      contentType: 'application/json',
    }),
  );
  await page.route('**/api/local/v1/edge/server-status', (route) =>
    route.fulfill({
      body: JSON.stringify(serverStatus),
      contentType: 'application/json',
    }),
  );
  await page.route('**/api/local/v1/edge/managed-settings', (route) =>
    route.fulfill({
      body: JSON.stringify(managedSettings),
      contentType: 'application/json',
    }),
  );

  await page.goto('/#/edge/server');
  await page.getByRole('tab', { name: '设置' }).click();
  await expect(page).toHaveURL(/#\/edge\/server\/settings$/);
  await expect(page.getByRole('heading', { name: '服务器设置' })).toBeVisible();
  const settingsTabsBox = await page.locator('.server-tabs').boundingBox();
  const settingsContentBox = await page
    .locator('.settings-content')
    .boundingBox();
  expect(
    Math.abs((settingsTabsBox?.x ?? 0) - (settingsContentBox?.x ?? 0)),
  ).toBeLessThan(1);
  await expect(
    page.getByRole('heading', { level: 2, name: '系统托管设置' }),
  ).toBeVisible();
  await expect(page.locator('.settings-group')).toHaveCount(3);
  await expect(page.locator('.setting-tile.ant-card')).toHaveCount(9);
  await expect(page.locator('.managed-notice')).toHaveCount(0);
  await expect(
    page.getByText('采集计划由中控管理，本机仅提供只读状态快照。'),
  ).toBeVisible();
  await expect(page.locator('.settings-heading .collection-note')).toHaveCount(
    1,
  );
  await expect(
    page.locator('.settings-collection .collection-note'),
  ).toHaveCount(0);
  const settingsHeadingTypography = await page
    .locator('.settings-heading')
    .evaluate((heading) => {
      const headingCopy = heading.querySelector('.settings-heading-copy');
      const note = heading.querySelector('.collection-note');
      const title = heading.querySelector('h2');
      const noteStyle = note ? getComputedStyle(note) : null;
      const titleStyle = title ? getComputedStyle(title) : null;
      return {
        alignment: headingCopy ? getComputedStyle(headingCopy).alignItems : '',
        noteColor: noteStyle?.color ?? '',
        noteLineHeight: noteStyle?.lineHeight ?? '',
        noteSize: noteStyle?.fontSize ?? '',
        titleColor: titleStyle?.color ?? '',
        titleLineHeight: titleStyle?.lineHeight ?? '',
        titleSize: titleStyle?.fontSize ?? '',
        titleWeight: titleStyle?.fontWeight ?? '',
      };
    });
  expect(settingsHeadingTypography).toEqual({
    alignment: 'baseline',
    noteColor: 'rgb(141, 150, 160)',
    noteLineHeight: '24px',
    noteSize: '15px',
    titleColor: 'rgb(216, 224, 231)',
    titleLineHeight: '36px',
    titleSize: '30px',
    titleWeight: '400',
  });
  await expect(page.getByText('可信管控中心', { exact: true })).toBeVisible();
  const settingTileBoxes = await page
    .locator('.setting-tile.ant-card')
    .evaluateAll((tiles) =>
      tiles.map((tile) => {
        const rect = tile.getBoundingClientRect();
        return { height: rect.height, width: rect.width };
      }),
    );
  expect(
    Math.max(...settingTileBoxes.map(({ height }) => height)) -
      Math.min(...settingTileBoxes.map(({ height }) => height)),
  ).toBeLessThan(1);
  expect(
    settingTileBoxes.every(({ height }) => Math.abs(height - 88) < 1),
  ).toBe(true);
  const settingTypography = await page
    .locator('.setting-tile.ant-card')
    .first()
    .evaluate((tile) => {
      const heading = tile.querySelector('h4');
      const description = tile.querySelector('p');
      return {
        descriptionSize: description
          ? getComputedStyle(description).fontSize
          : '',
        headingSize: heading ? getComputedStyle(heading).fontSize : '',
      };
    });
  expect(settingTypography.headingSize).toBe('16px');
  expect(settingTypography.descriptionSize).toBe('14px');
  for (const selector of ['.identity-grid', '.collection-grid']) {
    const columnWidths = await page
      .locator(`${selector} .setting-tile.ant-card`)
      .evaluateAll((tiles) =>
        tiles.map((tile) => tile.getBoundingClientRect().width),
      );
    expect(Math.max(...columnWidths) - Math.min(...columnWidths)).toBeLessThan(
      1,
    );
  }
  const sectionHeadingSizes = await page
    .locator('.settings-group h3')
    .evaluateAll((headings) =>
      headings.map((heading) => getComputedStyle(heading).fontSize),
    );
  expect(new Set(sectionHeadingSizes).size).toBe(1);
  await expect(page.locator('input, textarea')).toHaveCount(0);
  await expect(page.locator('button[type="submit"]')).toHaveCount(0);
  await attachVisual(page, testInfo, 'A-05-read-only-1672x941');
});

test('A-07 validates the registration package without performing writes', async ({
  page,
}, testInfo) => {
  await page.setViewportSize({ height: 941, width: 1672 });
  await page.emulateMedia({ reducedMotion: 'reduce' });
  await context(page, 'EDGE');
  await page.route('**/api/local/v1/edge/registration', (route) =>
    route.fulfill({
      body: JSON.stringify(registration),
      contentType: 'application/json',
    }),
  );

  await page.goto('/#/edge/register');
  await expect(page.getByRole('heading', { name: '首次注册' })).toBeVisible();
  await expect(page.getByText('regpkg_20260724_001')).toBeVisible();
  await expect(page.getByText('签名有效')).toBeVisible();
  await expect(
    page.getByRole('button', { name: '生成本机证书' }),
  ).toBeDisabled();
  await expect(page.getByText('安全写入接口装配前不会执行')).toBeVisible();
  await attachVisual(page, testInfo, 'A-07-confirm-1672x941');
});

test('B-02 keeps old values, exposes permission reason and fits 1280×720', async ({
  page,
}, testInfo) => {
  await page.setViewportSize({ height: 720, width: 1280 });
  await context(page, 'CONTROL');
  await page.route('**/api/local/v1/control/sites', (route) =>
    route.fulfill({
      body: JSON.stringify({
        failed_sites: 1,
        latest_sites: 0,
        meta: {
          ...viewMeta,
          freshness: 'FAILED_RETAINED',
          retained_after_failure: true,
          status_message: '采集失败，保留最近完整快照',
        },
        sites: [site],
        stale_sites: 0,
        total_sites: 1,
        updating_sites: 0,
      }),
      contentType: 'application/json',
    }),
  );

  await page.goto('/#/control/sites');
  await expect(page.getByRole('heading', { name: '子工厂状态' })).toBeVisible();
  await expect(page.getByText('1,024')).toBeVisible();
  await expect(
    page.locator('.sites-workspace[data-layout-reference="control-history"]'),
  ).toBeVisible();
  await expect(page.locator('.sites-note.ant-alert')).toBeVisible();
  await expect(page.locator('.site-summary-filter.ant-btn')).toHaveCount(5);
  await expect(page.locator('.site-summary-filter.ant-btn').first()).toHaveCSS(
    'align-items',
    'center',
  );
  await expect(
    page.locator('.site-summary-filter.ant-btn > span').first(),
  ).toHaveCSS('align-items', 'center');
  await expect(page.locator('.latest-collection')).toHaveCSS(
    'align-items',
    'center',
  );
  await expect(page.locator('.latest-collection strong')).toHaveText('18:42');
  expect(
    await page.locator('.latest-collection').evaluate(
      (element) => element.scrollWidth <= element.clientWidth,
    ),
  ).toBe(true);
  await expect(page.locator('.sites-table.ant-table-wrapper')).toBeVisible();
  await expect(page.locator('.filters')).toHaveCount(0);
  await expect(
    page.getByRole('link', {
      name: '关闭中控服务器并返回入库总览',
    }),
  ).toHaveAttribute('href', '#/control');
  const trigger = page.getByRole('button', { name: /立即获取/ });
  await expect(trigger).toBeDisabled();
  await expect(trigger).toHaveAttribute('title', '需要 CONTROL_ADMIN 权限');
  await attachVisual(page, testInfo, 'B-02-1280x720');
  await page.setViewportSize({ height: 941, width: 1672 });
  await attachVisual(page, testInfo, 'B-02-1672x941');

  const sitesLayout = await page
    .locator('.sites-workspace')
    .evaluate((root) => {
      const rootBox = root.getBoundingClientRect();
      return {
        children: [...root.children].map((child) => {
          const box = child.getBoundingClientRect();
          return {
            height: Math.round(box.height),
            top: Math.round(box.top - rootBox.top),
          };
        }),
        height: Math.round(rootBox.height),
        width: Math.round(rootBox.width),
        x: Math.round(rootBox.x),
        y: Math.round(rootBox.y),
      };
    });
  const sitesTabsLayout = await page
    .locator('.control-server-tabs')
    .evaluate((root) => {
      const box = root.getBoundingClientRect();
      return {
        height: Math.round(box.height),
        width: Math.round(box.width),
        x: Math.round(box.x),
        y: Math.round(box.y),
      };
    });
  const sitesRackBox = await page
    .locator('.control-runtime-rack[data-device-id="control-center-rack"]')
    .boundingBox();

  await page.goto('/#/control/history');
  await expect(page.locator('.history-workspace')).toBeVisible();
  const historyLayout = await page
    .locator('.history-workspace')
    .evaluate((root) => {
      const rootBox = root.getBoundingClientRect();
      return {
        children: [...root.children].map((child) => {
          const box = child.getBoundingClientRect();
          return {
            height: Math.round(box.height),
            top: Math.round(box.top - rootBox.top),
          };
        }),
        height: Math.round(rootBox.height),
        width: Math.round(rootBox.width),
        x: Math.round(rootBox.x),
        y: Math.round(rootBox.y),
      };
    });
  const historyTabsLayout = await page
    .locator('.control-server-tabs')
    .evaluate((root) => {
      const box = root.getBoundingClientRect();
      return {
        height: Math.round(box.height),
        width: Math.round(box.width),
        x: Math.round(box.x),
        y: Math.round(box.y),
      };
    });
  expect(sitesLayout).toEqual(historyLayout);
  expect(sitesTabsLayout).toEqual(historyTabsLayout);
  expect(sitesRackBox).not.toBeNull();
  if (sitesRackBox) {
    expect(sitesLayout.x + sitesLayout.width).toBeLessThanOrEqual(
      Math.round(sitesRackBox.x),
    );
  }
});

test('A-02 distinguishes unknown readiness at 1920×1080', async ({
  page,
}, testInfo) => {
  await page.setViewportSize({ height: 1080, width: 1920 });
  await context(page, 'EDGE');
  await page.route('**/api/local/v1/edge/server-status', (route) =>
    route.fulfill({
      body: JSON.stringify(serverStatusUnknown),
      contentType: 'application/json',
    }),
  );
  await page.route('**/api/local/v1/edge/runtime-overview', (route) =>
    route.fulfill({
      body: JSON.stringify(runtime),
      contentType: 'application/json',
    }),
  );

  await page.goto('/#/edge/server');
  await expect(
    page.getByRole('heading', { name: '服务器运行状态' }),
  ).toBeVisible();
  await expect(page.getByText('加密与签名')).toBeVisible();
  await expect(page.getByText('尚未获得测试介质授权')).toBeVisible();
  await expect(
    page.getByRole('region', { name: '运行能力状态' }).getByText('状态未知'),
  ).toHaveCount(1);
  await attachVisual(page, testInfo, 'A-02-1920x1080');
  await page
    .getByRole('link', { name: '关闭服务器详情并返回运行首页' })
    .click();
  await expect(page).toHaveURL(/#\/edge$/);
});

test('B-03 preserves the latest complete snapshot after collection failure', async ({
  page,
}, testInfo) => {
  await page.setViewportSize({ height: 941, width: 1672 });
  await context(page, 'CONTROL');
  await page.route(
    '**/api/local/v1/control/sites/factory-a-001/collection',
    (route) =>
      route.fulfill({
        body: JSON.stringify(failedCollection),
        contentType: 'application/json',
      }),
  );

  await page.goto('/#/control/sites/factory-a-001/collection');
  await expect(
    page.getByRole('heading', { name: '采集任务与快照详情' }),
  ).toBeVisible();
  await expect(page.locator('.stage-panel li')).toHaveCount(7);
  await expect(page.getByRole('alert')).toContainText(
    '最近完整快照继续作为页面权威值',
  );
  await expect(page.getByText('本次任务未形成可接受快照。')).toBeVisible();
  await expect(page.getByText('#42')).toBeVisible();
  await attachVisual(page, testInfo, 'B-03-1672x941');
  await page
    .getByRole('link', { name: '关闭中控服务器并返回入库总览' })
    .click();
  await expect(page).toHaveURL(/#\/control$/);
});

test('B-02 and B-03 keep one control scene through forward and return transitions', async ({
  page,
}) => {
  await page.setViewportSize({ height: 941, width: 1672 });
  await context(page, 'CONTROL');
  await page.route('**/api/local/v1/control/sites', (route) =>
    route.fulfill({
      body: JSON.stringify({
        failed_sites: 1,
        latest_sites: 0,
        meta: viewMeta,
        sites: [site],
        stale_sites: 0,
        total_sites: 1,
        updating_sites: 0,
      }),
      contentType: 'application/json',
    }),
  );
  await page.route('**/api/local/v1/control/sites/factory-a-001', (route) =>
    route.fulfill({
      body: JSON.stringify(siteDetail),
      contentType: 'application/json',
    }),
  );
  await page.route(
    '**/api/local/v1/control/sites/factory-a-001/collection',
    async (route) => {
      await new Promise((resolve) => setTimeout(resolve, 80));
      await route.fulfill({
        body: JSON.stringify(failedCollection),
        contentType: 'application/json',
      });
    },
  );

  await page.goto('/#/control/sites');
  const rack = page.locator(
    '.control-runtime-rack[data-device-id="control-center-rack"]',
  );
  await rack.evaluate((element) => {
    element.dataset.continuityToken = 'control-rack-preserved';
  });

  await page.getByRole('link', { name: '查看 工厂 A-001 同步详情' }).click();
  await expect(page).toHaveURL(/#\/control\/sites\/factory-a-001$/);
  await expect(rack).toHaveAttribute(
    'data-continuity-token',
    'control-rack-preserved',
  );

  await page.getByRole('link', { name: '快照与采集' }).click();
  await expect(page).toHaveURL(/#\/control\/sites\/factory-a-001\/collection$/);
  const collectionPanel = page.locator(
    '.collection-baseline.control-scene-panel',
  );
  await expect(collectionPanel).toBeVisible();
  await expect(collectionPanel).toHaveCSS(
    'transition-duration',
    /0.32s.*0.42s/,
  );
  await expect(collectionPanel).toHaveCSS(
    'transition-timing-function',
    /cubic-bezier\(0.22, 1, 0.36, 1\)/,
  );
  await expect(rack).toHaveAttribute(
    'data-continuity-token',
    'control-rack-preserved',
  );

  await page.goBack();
  await expect(page).toHaveURL(/#\/control\/sites\/factory-a-001$/);
  await expect(rack).toHaveAttribute(
    'data-continuity-token',
    'control-rack-preserved',
  );
  await page.goBack();
  await expect(page).toHaveURL(/#\/control\/sites$/);
  await expect(rack).toHaveAttribute(
    'data-continuity-token',
    'control-rack-preserved',
  );

  await page.emulateMedia({ reducedMotion: 'reduce' });
  expect(
    await rack.evaluate((element) =>
      Number.parseFloat(getComputedStyle(element).transitionDuration),
    ),
  ).toBeLessThanOrEqual(0.001);
});

test('the installed EDGE role cannot remain on a CONTROL route', async ({
  page,
}) => {
  await context(page, 'EDGE');
  await page.route('**/api/local/v1/edge/runtime-overview', (route) =>
    route.fulfill({
      body: JSON.stringify(runtime),
      contentType: 'application/json',
    }),
  );
  await page.goto('/#/control/sites');
  await expect(page).toHaveURL(/#\/edge$/);
  await expect(page.getByRole('heading', { name: '运行首页' })).toBeVisible();
});

test('an unavailable local source fails closed instead of showing sample data', async ({
  page,
}) => {
  await context(page, 'EDGE');
  await page.route('**/api/local/v1/edge/runtime-overview', (route) =>
    route.fulfill({
      body: JSON.stringify({
        error_code: 'LOCAL_VIEW_UNAVAILABLE',
        message: 'unavailable',
        request_id: 'req_visualfail001',
        retryable: true,
      }),
      contentType: 'application/json',
      status: 503,
    }),
  );
  await page.goto('/#/edge');
  await expect(page.getByRole('alert')).toContainText('本机视图不可用');
  await expect(page.getByText('8.4 TB')).toHaveCount(0);
});

test('a static preview HTML fallback renders the unavailable state', async ({
  page,
}) => {
  await page.route('**/api/local/v1/**', (route) => route.abort());
  await page.goto('/#/edge');
  await expect(page.getByRole('alert')).toContainText('本机视图不可用');
  await expect(page.getByRole('button', { name: '重新读取' })).toBeVisible();
  await expect(page.getByText('8.4 TB')).toHaveCount(0);
});

test('A-02 distributes readiness metrics without overflow', async ({
  page,
}, testInfo) => {
  await page.setViewportSize({ height: 941, width: 1672 });
  await page.emulateMedia({ reducedMotion: 'reduce' });
  await context(page, 'EDGE');
  await page.route('**/api/local/v1/edge/runtime-overview', (route) =>
    route.fulfill({
      body: JSON.stringify(runtime),
      contentType: 'application/json',
    }),
  );
  await page.route('**/api/local/v1/edge/server-status', (route) =>
    route.fulfill({
      body: JSON.stringify(serverStatus),
      contentType: 'application/json',
    }),
  );

  await page.goto('/#/edge/server');
  await expect(
    page.getByRole('heading', { name: '服务器运行状态' }),
  ).toBeVisible();

  const readiness = page.locator('.readiness-area');
  const capabilityRow = page.locator('.capability-row');
  await expect(readiness).toHaveCSS('display', 'flex');
  await expect(readiness).toHaveCSS('justify-content', 'space-between');
  await expect(page.locator('.health-ring')).toHaveCSS('align-items', 'center');
  await expect(capabilityRow).toHaveCSS('display', 'flex');
  await expect(capabilityRow).toHaveCSS('justify-content', 'space-between');
  await expect(capabilityRow).toHaveCSS('align-items', 'center');

  const layout = await readiness.evaluate((element) => {
    const container = element.getBoundingClientRect();
    const children = [...element.children].map((child) =>
      child.getBoundingClientRect(),
    );
    const contentSelectors = [
      '.health-visual',
      '.health-ring p',
      '.storage-metric > strong',
      '.storage-progress',
      '.storage-metric .metric-details',
      '.discovery-summary',
      '.discovery-metric .metric-details',
      '.trend-chart',
      '.trend-latest',
    ];
    return {
      contentFits: contentSelectors.every((selector) => {
        const content = element.querySelector(selector);
        const article = content?.closest('article');
        if (!(content && article)) return false;
        const contentBounds = content.getBoundingClientRect();
        const articleBounds = article.getBoundingClientRect();
        return (
          contentBounds.left >= articleBounds.left - 1 &&
          contentBounds.right <= articleBounds.right + 1
        );
      }),
      equalWidthSpread:
        Math.max(...children.map((bounds) => bounds.width)) -
        Math.min(...children.map((bounds) => bounds.width)),
      fits: children.every(
        (bounds) =>
          bounds.left >= container.left - 1 &&
          bounds.right <= container.right + 1 &&
          bounds.top >= container.top - 1 &&
          bounds.bottom <= container.bottom + 1,
      ),
      ordered: children.every(
        (bounds, index) =>
          index === 0 || bounds.left >= (children[index - 1]?.right ?? 0) - 1,
      ),
    };
  });
  expect(layout.contentFits).toBe(true);
  expect(layout.equalWidthSpread).toBeLessThanOrEqual(1);
  expect(layout.fits).toBe(true);
  expect(layout.ordered).toBe(true);

  const capabilityWidths = await capabilityRow
    .locator(':scope > div')
    .evaluateAll((elements) =>
      elements.map((element) => element.getBoundingClientRect().width),
    );
  expect(
    Math.max(...capabilityWidths) - Math.min(...capabilityWidths),
  ).toBeLessThanOrEqual(1);

  const healthCenters = await page
    .locator('.health-ring')
    .evaluate((element) => {
      const ring = element.getBoundingClientRect();
      const visual = element
        .querySelector('.health-visual')
        ?.getBoundingClientRect();
      return {
        ring: ring.left + ring.width / 2,
        visual: visual ? visual.left + visual.width / 2 : 0,
      };
    });
  expect(Math.abs(healthCenters.ring - healthCenters.visual)).toBeLessThan(1);

  await attachVisual(page, testInfo, 'A-02-flex-distribution');
});
