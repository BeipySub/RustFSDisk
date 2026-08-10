import { createReadStream, existsSync, statSync } from 'node:fs';
import { createServer } from 'node:http';
import { extname, join, normalize, resolve } from 'node:path';
import process from 'node:process';

const host = '127.0.0.1';
const port = Number.parseInt(process.env.FUSTFS_REVIEW_PORT ?? '4315', 10);
const distRoot = resolve(import.meta.dirname, '../../../../build/web');

const meta = {
  data_as_of: '2026-07-24T18:42:00+08:00',
  freshness: 'FRESH',
  generated_at: '2026-07-24T18:42:08+08:00',
  retained_after_failure: false,
  schema_version: 'i4.1',
  status_message: '评审数据 · 非真实运行状态',
};

const edgeContext = {
  display_name: '工厂 A',
  meta,
  permissions: ['VIEW_LOCAL_STATUS'],
  role: 'EDGE',
  site_id: 'factory-a-001',
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
    }[slot];
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
  meta,
  site_id: 'factory-a-001',
  summary: {
    connected: 16,
    failed: 0,
    healthy: 15,
    warning: 1,
  },
};

const controlContext = {
  display_name: '中心 B',
  meta,
  permissions: ['VIEW_LOCAL_STATUS'],
  role: 'CONTROL',
  site_id: 'control-b',
};

const controlSites = [
  ['A-001', 'LATEST', 128, 1024, 128, 12_000_000_000_000, 12, 'READY', 0],
  ['A-002', 'UPDATING', 64, 838, 64, 8_600_000_000_000, 8, 'READY', 1],
  ['A-003', 'LATEST', 0, 1420, 0, 0, 16, 'READY', 0],
  ['A-004', 'STALE', 23, 615, 23, 0, 6, 'UNKNOWN', 1],
  ['A-005', 'LATEST', 47, 790, 47, 4_100_000_000_000, 10, 'READY', 0],
  [
    'A-006',
    'COLLECTION_FAILED',
    31,
    504,
    31,
    4_300_000_000_000,
    11,
    'FAILED',
    2,
  ],
].map(
  (
    [
      id,
      snapshotState,
      newVersions,
      verifiedVersions,
      unsyncedVersions,
      transitBytes,
      disks,
      diskState,
      alerts,
    ],
    index,
  ) => ({
    active_alerts: alerts,
    can_trigger_collection: index === 0,
    central: {
      conflict_locked_versions: 0,
      ingesting_batches: snapshotState === 'UPDATING' ? 1 : 0,
      issued_receipts: 231 + index,
      target_verified_versions: verifiedVersions,
    },
    collection_blocked_reason: index === 0 ? null : '当前评审夹具仅开放 A-001',
    data_as_of: `2026-07-24T${String(18 - Math.min(index, 2)).padStart(2, '0')}:${String(42 - index * 6).padStart(2, '0')}:00+08:00`,
    disks: {
      connected: disks,
      label: {
        FAILED: '异常',
        READY: '正常',
        UNKNOWN: '未知',
      }[diskState],
      state: diskState,
    },
    display_name: `工厂 ${id}`,
    in_transit_bytes: transitBytes,
    site_id: `factory-${id.toLowerCase()}`,
    snapshot_state: snapshotState,
    source: {
      in_transit_versions: index,
      local_failed_versions: snapshotState === 'COLLECTION_FAILED' ? 2 : 0,
      new_object_versions: newVersions,
      packed_waiting_transport_versions: Math.ceil(unsyncedVersions / 4),
      waiting_for_media_versions: Math.ceil(unsyncedVersions / 2),
    },
    unsynced_object_versions: unsyncedVersions,
  }),
);

const completedCollection = {
  collection_job_id: 'COL-A001-20260721-1842',
  completed_at: '2026-07-21T18:42:08+08:00',
  failure_reason: null,
  failure_stage: null,
  meta,
  next_scheduled_at: '2026-07-22T01:30:00+08:00',
  queued_at: '2026-07-21T18:40:00+08:00',
  site_id: 'factory-a-001',
  snapshot_id: 'snap-A001-1024',
  snapshot_seq: 1024,
  source_delta: {
    in_transit_versions: 0,
    local_failed_versions: 2,
    new_object_versions: 128,
    packed_waiting_transport_versions: 32,
    waiting_for_media_versions: 64,
  },
  stage: 'COMPLETED',
  stages: [
    ['QUEUED', '排队', '18:40:00'],
    ['CONNECT', '连接', '18:40:03'],
    ['READ_SOURCE', '扫描', '18:40:08'],
    ['READ_CENTRAL', '生成快照', '18:41:32'],
    ['VALIDATE', '下载', '18:41:36'],
    ['REBUILD', '对账', '18:42:01'],
    ['PUBLISH', '完成', '18:42:08'],
  ].map(([stage, label, time]) => ({
    at: `2026-07-21T${time}+08:00`,
    label,
    stage,
    state: 'COMPLETED',
  })),
  trigger: 'SCHEDULED',
  validation: {
    completeness: 'COMPLETE',
    digest_valid: true,
    mtls_valid: true,
    policy_version: 'policy-v7',
    projection_rebuilt: true,
    scope_label: '12 个存储桶 · 1,152 个对象版本',
    site_match: true,
  },
};

const edgeRecords = {
  meta,
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
  meta,
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
  meta,
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

const fixtures = new Map([
  [
    '/api/local/v1/control/sites',
    {
      failed_sites: 1,
      latest_sites: 3,
      meta: {
        ...meta,
        retained_after_failure: true,
        status_message: '采集失败保留最近完整快照',
      },
      sites: controlSites,
      stale_sites: 1,
      total_sites: 6,
      updating_sites: 1,
    },
  ],
  ['/api/local/v1/control/sites/factory-a-001/collection', completedCollection],
  ['/api/local/v1/edge/managed-settings', managedSettings],
  ['/api/local/v1/edge/registration', registration],
  [
    '/api/local/v1/edge/runtime-overview',
    {
      current: {
        batch_id: 'A-20260720-008',
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
      meta,
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
    },
  ],
  ['/api/local/v1/edge/nas-disks', transportDisks],
  [
    '/api/local/v1/edge/server-status',
    {
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
        93, 94, 94, 92, 90, 94, 91, 94, 92, 92, 95, 94, 90, 88, 92, 88, 87, 92,
        88, 92, 91, 92, 91, 91, 93, 91, 92, 93, 91, 92,
      ],
      last_scan_at: '2026-07-24T18:07:42+08:00',
      meta,
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
    },
  ],
  ['/api/local/v1/edge/sync-records', edgeRecords],
]);

const contentTypes = {
  '.css': 'text/css; charset=utf-8',
  '.html': 'text/html; charset=utf-8',
  '.ico': 'image/x-icon',
  '.js': 'text/javascript; charset=utf-8',
  '.json': 'application/json; charset=utf-8',
  '.png': 'image/png',
  '.svg': 'image/svg+xml',
  '.webp': 'image/webp',
  '.woff': 'font/woff',
  '.woff2': 'font/woff2',
};

function sendJson(response, status, value) {
  response.writeHead(status, {
    'cache-control': 'no-store',
    'content-type': 'application/json; charset=utf-8',
    'x-fustfs-data-source': 'isolated-review-fixture',
  });
  response.end(JSON.stringify(value));
}

function resolveStaticPath(pathname) {
  const candidate = resolve(
    distRoot,
    normalize(pathname).replace(/^[/\\]+/, ''),
  );
  return candidate.startsWith(`${distRoot}\\`) || candidate === distRoot
    ? candidate
    : null;
}

if (!existsSync(join(distRoot, 'index.html'))) {
  throw new Error(`缺少前端构建产物：${distRoot}`);
}

createServer((request, response) => {
  const url = new URL(request.url ?? '/', `http://${host}:${port}`);

  const reviewRoutes = new Map([
    ['/review/control', '/#/control/sites'],
    ['/review/control/collection', '/#/control/sites/factory-a-001/collection'],
    ['/review/edge', '/#/edge/server'],
    ['/review/edge/disks', '/#/edge/nas/disks'],
    ['/review/edge/records', '/#/edge/server/records'],
    ['/review/edge/register', '/#/edge/register'],
    ['/review/edge/settings', '/#/edge/server/settings'],
  ]);
  const reviewLocation = reviewRoutes.get(url.pathname);

  if (reviewLocation) {
    const role = url.pathname.startsWith('/review/control')
      ? 'CONTROL'
      : 'EDGE';
    response.writeHead(302, {
      'cache-control': 'no-store',
      location: reviewLocation,
      'set-cookie': `fustfs_review_role=${role}; Path=/; SameSite=Strict`,
    });
    response.end();
    return;
  }

  if (url.pathname === '/api/local/v1/context') {
    const role = request.headers.cookie?.includes('fustfs_review_role=CONTROL')
      ? 'CONTROL'
      : 'EDGE';
    sendJson(response, 200, role === 'CONTROL' ? controlContext : edgeContext);
    return;
  }

  const fixture = fixtures.get(url.pathname);

  if (fixture) {
    sendJson(response, 200, fixture);
    return;
  }

  if (url.pathname.startsWith('/api/')) {
    sendJson(response, 404, {
      error_code: 'REVIEW_FIXTURE_NOT_FOUND',
      message: '评审模式未为该接口配置夹具',
      retryable: false,
    });
    return;
  }

  const requestedPath = resolveStaticPath(url.pathname);
  const filePath =
    requestedPath &&
    existsSync(requestedPath) &&
    statSync(requestedPath).isFile()
      ? requestedPath
      : join(distRoot, 'index.html');

  response.writeHead(200, {
    'cache-control': filePath.endsWith('index.html')
      ? 'no-store'
      : 'public, max-age=3600',
    'content-type':
      contentTypes[extname(filePath)] ?? 'application/octet-stream',
  });
  createReadStream(filePath).pipe(response);
}).listen(port, host, () => {
  console.warn(`A-02 评审入口：http://${host}:${port}/review/edge`);
  console.warn(`A-06 评审入口：http://${host}:${port}/review/edge/disks`);
  console.warn(`A-04 评审入口：http://${host}:${port}/review/edge/records`);
  console.warn(`A-05 评审入口：http://${host}:${port}/review/edge/settings`);
  console.warn(`A-07 评审入口：http://${host}:${port}/review/edge/register`);
  console.warn(`B-02 评审入口：http://${host}:${port}/review/control`);
  console.warn(
    `B-03 评审入口：http://${host}:${port}/review/control/collection`,
  );
});
