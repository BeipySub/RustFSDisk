import type { Component } from 'vue';

import type {
  ControlSitesView,
  EdgeMediaCandidatesView,
  EdgeManagedSettingsView,
  EdgeRuntimeView,
  EdgeSyncRecordsView,
  EdgeTransportDisksView,
} from '#/api/local-views';

import { createApp, nextTick } from 'vue';
import { createMemoryHistory, createRouter } from 'vue-router';

import { afterEach, describe, expect, it } from 'vitest';

import SitesPanel from '../control/sites-panel.vue';
import NasDisksPanel from '../edge/nas-disks-panel.vue';
import { defaultParticleDebugSettings } from '../edge/particle-debug';
import ParticleDebugPanel from '../edge/particle-debug-panel.vue';
import RecordsPanel from '../edge/records-panel.vue';
import RuntimePanel from '../edge/runtime-panel.vue';
import ServerTabs from '../edge/server-tabs.vue';

const mounted: Array<ReturnType<typeof createApp>> = [];

async function mount(
  component: Component,
  props: Record<string, unknown>,
  initialPath = '/',
) {
  const container = document.createElement('div');
  document.body.append(container);
  const router = createRouter({
    history: createMemoryHistory(),
    routes: [
      { component: { template: '<div />' }, path: '/' },
      { component: { template: '<div />' }, path: '/control/sites/:siteId' },
      { component: { template: '<div />' }, path: '/edge/server' },
      { component: { template: '<div />' }, path: '/edge/server/records' },
      {
        component: { template: '<div />' },
        path: '/edge/server/records/:batchId',
      },
      { component: { template: '<div />' }, path: '/edge/nas/disks' },
    ],
  });
  await router.push(initialPath);
  await router.isReady();
  router.afterEach((to) => {
    container.dataset.routePath = to.path;
  });
  container.dataset.routePath = router.currentRoute.value.path;
  const app = createApp(component, props);
  app.use(router);
  app.mount(container);
  mounted.push(app);
  await nextTick();
  return container;
}

afterEach(() => {
  mounted.splice(0).forEach((app) => app.unmount());
  document.body.replaceChildren();
});

const meta = {
  data_as_of: '2026-07-24T18:42:00+08:00',
  freshness: 'FRESH',
  generated_at: '2026-07-24T18:42:08+08:00',
  retained_after_failure: false,
  schema_version: 'i4.1',
  status_message: '数据最新',
} as const;

const runtime: EdgeRuntimeView = {
  current: {
    batch_id: 'batch_synthetic001',
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
  source_export: {
    copied_bytes: 8_400_000_000_000,
    copied_versions: 12,
    not_copied_bytes: 3_900_000_000_000,
    not_copied_versions: 4,
  },
  throughput_bytes_per_second: 1_840_000_000,
};

const transportDisks: EdgeTransportDisksView = {
  disks: [
    {
      active_task: {
        confirmed_bytes: 4_320_000_000_000,
        eta_confidence: 'HIGH',
        eta_seconds: 960,
        progress_percent: 72,
        stage: 'ENCRYPT_AND_WRITE',
        throughput_bytes_per_second: 1_200_000_000,
        total_bytes: 6_000_000_000_000,
        transport_record_id: 'transport-record-media-001',
      },
      capacity_bytes: 6_000_000_000_000,
      exclusion_reason: null,
      exclusion_state: 'READY',
      filesystem_label: 'FUSTFS_TRANSPORT',
      in_use: true,
      life_percent: null,
      media_id_suffix: 'M001',
      media_label: '运输盘',
      progress_percent: 72,
      read_only: false,
      serial_suffix: '7F22',
      slot: '01',
      smart_state: 'READY',
      state: 'WRITING',
      state_label: '写入中',
      temperature_celsius: 38,
    },
    {
      capacity_bytes: 6_000_000_000_000,
      exclusion_reason: null,
      exclusion_state: 'READY',
      filesystem_label: 'FUSTFS_TRANSPORT',
      in_use: false,
      life_percent: null,
      media_id_suffix: 'M002',
      media_label: '运输盘',
      progress_percent: null,
      read_only: false,
      serial_suffix: '51C8',
      slot: '02',
      smart_state: 'READY',
      state: 'STANDBY',
      state_label: '待命',
      temperature_celsius: 37,
    },
    {
      active_task: {
        confirmed_bytes: 4_080_000_000_000,
        eta_confidence: 'MEDIUM',
        eta_seconds: 1_680,
        progress_percent: 68,
        stage: 'ENCRYPT_AND_WRITE',
        throughput_bytes_per_second: 640_000_000,
        total_bytes: 6_000_000_000_000,
        transport_record_id: 'transport-record-media-004',
      },
      capacity_bytes: 6_000_000_000_000,
      exclusion_reason: null,
      exclusion_state: 'READY',
      filesystem_label: 'FUSTFS_TRANSPORT',
      in_use: true,
      life_percent: null,
      media_id_suffix: 'M004',
      media_label: '运输盘',
      progress_percent: 68,
      read_only: false,
      serial_suffix: '8F2A',
      slot: '04',
      smart_state: 'WARNING',
      state: 'WRITING',
      state_label: '写入中',
      temperature_celsius: 52,
    },
  ],
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

const sites: ControlSitesView = {
  failed_sites: 1,
  latest_sites: 0,
  meta: {
    ...meta,
    freshness: 'FAILED_RETAINED',
    retained_after_failure: true,
    status_message: '采集失败，保留最近完整快照',
  },
  sites: [
    {
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
    },
  ],
  stale_sites: 0,
  total_sites: 1,
  updating_sites: 0,
};

const records: EdgeSyncRecordsView = {
  meta,
  records: [
    {
      batch_id: 'A-20260720-008',
      completed_at: '2026-07-29T10:20:00+08:00',
      destination_label: '管控中心',
      events: [
        {
          at: '2026-07-29T10:20:00+08:00',
          label: '写盘失败',
          result: '介质锁定',
          state: 'FAILED',
        },
      ],
      failure_reason: '介质健康检查未通过',
      failure_stage: '写盘',
      logical_bytes: 8_400_000_000_000,
      media_serial_suffix: '8F2A',
      result_label: '失败',
      retry_result: '未重试',
      stages: [
        {
          at: '2026-07-29T10:02:00+08:00',
          label: '读取完成',
          state: 'PASSED',
        },
        {
          at: '2026-07-29T10:14:00+08:00',
          label: '加密写入完成',
          state: 'PASSED',
        },
        {
          at: '2026-07-29T10:20:00+08:00',
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
      completed_at: '2026-07-29T09:32:00+08:00',
      destination_label: '管控中心',
      events: [],
      failure_reason: null,
      failure_stage: null,
      logical_bytes: 7_200_000_000_000,
      media_serial_suffix: '91C4',
      result_label: '装盘成功',
      retry_result: null,
      stages: [
        {
          at: '2026-07-29T09:03:00+08:00',
          label: '读取完成',
          state: 'PASSED',
        },
        {
          at: '2026-07-29T09:17:00+08:00',
          label: '加密写入完成',
          state: 'PASSED',
        },
        {
          at: '2026-07-29T09:28:00+08:00',
          label: '写入校验通过',
          state: 'PASSED',
        },
        {
          at: '2026-07-29T09:32:00+08:00',
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

const settings: EdgeManagedSettingsView = {
  collection: {
    endpoint_state: 'READY',
    last_collection_at: null,
    last_snapshot_label: '尚未采集',
    trusted_control_label: '管控中心',
    trusted_control_state: 'READY',
  },
  discovery: {
    auto_discovery: 'READY',
    health_scan_interval_label: '每 5 分钟',
    scan_scope_label: '企业级 SSD',
  },
  identity: {
    access_label: '只读',
    access_state: 'READY',
    policy_source_label: '可信管控中心',
    site_role_label: 'EDGE',
  },
  meta,
  policy_state: 'READY',
  site_id: 'factory-a-001',
};

void settings;

describe('i4 local view components', () => {
  it('renders only connected media and the unique next action', async () => {
    const container = await mount(RuntimePanel, { view: runtime });
    expect(container.querySelectorAll('.media-slots i')).toHaveLength(16);
    expect(
      container.querySelector('[data-particle-renderer="path-aether"]'),
    ).not.toBeNull();
    expect(container.querySelector('h1')?.textContent).toContain('运行首页');
    expect(container.textContent).toContain('完成当前阶段后更换此盘');
    expect(container.textContent).toContain('盘位 04');
    expect(container.textContent).toContain('未拷贝');
    expect(container.textContent).toContain('已拷贝');
    expect(container.querySelectorAll('progress')).toHaveLength(1);
  });

  it('uses the NAS candidate projection for the homepage connected-media count', async () => {
    const transportCandidates: EdgeMediaCandidatesView = {
      candidates: Array.from({ length: 2 }, (_, index) => ({
        candidate_id: `candidate-${index}`,
        candidate_session_id: `session-${index}`,
        capacity_bytes: null,
        class: 'CANDIDATE',
        filesystem_type: null,
        mounted_filesystems: 0,
        read_only: false,
        registration_state: 'UNREGISTERED',
        registration_detail: null,
        rejection: null,
        serial_hex: '',
        serial_suffix: `${index}`,
        trusted_slot: null,
      })),
      disks: [],
      last_scan_at: null,
      meta,
      site_id: runtime.site_id,
      summary: { connected: 2, failed: 0, healthy: 0, warning: 2 },
    };
    const container = await mount(RuntimePanel, {
      transportCandidates,
      view: runtime,
    });

    expect(container.querySelectorAll('.media-slots i')).toHaveLength(2);
    expect(container.textContent).toContain('已接入2块');
  });

  it('does not invent a percentage when total progress is unknown', async () => {
    const view: EdgeRuntimeView = {
      ...runtime,
      current: runtime.current
        ? {
            ...runtime.current,
            progress_percent: null,
            total_bytes: null,
          }
        : null,
    };
    const container = await mount(RuntimePanel, { view });
    expect(container.querySelector('progress')).toBeNull();
    expect(container.textContent).toContain('总量未知');
    expect(container.textContent).toContain('未知');
  });

  it('keeps both scene devices interactive during continuous detail focus', async () => {
    const container = await mount(RuntimePanel, {
      focus: 'server',
      mountedFocus: 'server',
      view: runtime,
    });
    expect(
      container.querySelector('[data-device-focus="server"]'),
    ).not.toBeNull();
    expect(
      container.querySelector('button[aria-label="查看 RustFS 服务器"]'),
    ).not.toBeNull();
    expect(
      container.querySelector('button[aria-label="查看运输 NAS"]'),
    ).not.toBeNull();
    expect(
      container.querySelector(
        '.scene-nas .scene-device-art[src*="transport-nas-cutout-v3.webp"]',
      ),
    ).not.toBeNull();
    expect(
      container.querySelectorAll(
        '.scene-nas img[src*="transport-nas-cutout-v3.webp"]',
      ),
    ).toHaveLength(1);
    expect(container.querySelector('.device-detail-layer')).not.toBeNull();
  });

  it('keeps particle debugging separate from page state', async () => {
    const updates: unknown[] = [];
    const container = await mount(ParticleDebugPanel, {
      modelValue: defaultParticleDebugSettings(),
      'onUpdate:modelValue': (value: unknown) => updates.push(value),
    });
    const toggle = container.querySelector<HTMLButtonElement>('.debug-toggle');
    toggle?.click();
    await nextTick();

    expect(container.textContent).toContain('仅覆盖粒子，不修改页面状态');
    const errorButton = [
      ...container.querySelectorAll<HTMLButtonElement>('.state-options button'),
    ].find((button) => button.textContent?.trim() === '异常');
    errorButton?.click();

    expect(updates).toContainEqual({
      color: null,
      glowStrength: 1,
      speedMultiplier: 1,
      state: 'error',
    });
    expect(
      container.querySelector('input[aria-label="选择自定义粒子颜色"]'),
    ).not.toBeNull();
  });

  it('passes particle-only overrides to the renderer', async () => {
    const container = await mount(RuntimePanel, {
      particleDebug: {
        color: '#ff00aa',
        glowStrength: 1.4,
        speedMultiplier: 1.5,
        state: 'error',
      },
      view: runtime,
    });
    const canvas = container.querySelector('[data-particle-renderer]');
    expect((canvas as HTMLElement | null)?.dataset.particleState).toBe('error');
    expect((canvas as HTMLElement | null)?.dataset.particleColor).toBe(
      '#ff00aa',
    );
    expect(container.textContent).toContain('自动运行中');
  });

  it('renders the A-06 selected transport disk detail with Ant page header and cards', async () => {
    const container = await mount(
      NasDisksPanel,
      { runtime, view: transportDisks },
      '/edge/nas/disks',
    );

    expect(
      container.querySelector('.nas-disks-header.ant-page-header'),
    ).not.toBeNull();
    expect(container.querySelectorAll('.nas-disk-card.ant-card')).toHaveLength(
      3,
    );
    expect(container.querySelector('.nas-disk-scroll')).not.toBeNull();
    expect(container.querySelectorAll('.nas-section-enter')).toHaveLength(3);
    expect(container.textContent).toContain('运输盘位');
    expect(container.textContent).toContain('SN …8F2A');
    expect(container.textContent).toContain('温度注意');
    expect(container.textContent).not.toContain('当前显示');
    expect(
      container.querySelector('.ant-page-header-heading-extra')?.textContent,
    ).toContain('最近扫描 14:32:08');
    expect(
      container.querySelector('[data-safety-facts*="FUSTFS_TRANSPORT"]'),
    ).not.toBeNull();
    expect(
      container.querySelector('.nas-disk-card.is-selected')?.textContent,
    ).toContain('04');
    expect(container.textContent).toContain('运输盘 04 信息');
    expect(container.textContent).toContain('数据传输');
    expect(container.textContent).toContain('4.08 / 6 TB');
    expect(container.textContent).toContain('transport-record-media-004');
    expect(container.textContent).toContain('任务进度');
    expect(container.textContent).toContain('运输盘容量');
    expect(container.textContent).toContain('任务耗时');
    expect(container.textContent).toContain('数据源未提供');
    expect(
      container.querySelector<HTMLImageElement>('.transport-disk-cutout')?.src,
    ).toContain('transport-disk-cutout-v1.png');

    container
      .querySelector<HTMLElement>('.nas-disk-card[aria-label^="盘位 01"]')
      ?.click();
    await nextTick();
    expect(
      container.querySelector('.nas-disk-card.is-selected')?.textContent,
    ).toContain('01');
    expect(container.textContent).toContain('运输盘 01 信息');
    expect(container.textContent).toContain('transport-record-media-001');
    expect(container.textContent).not.toContain('transport-record-media-004');

  });

  it('keeps only the server status tab', async () => {
    const container = await mount(
      ServerTabs,
      { active: 'status' },
      '/edge/server',
    );
    const tabs = container.querySelector('.server-tabs');
    expect(tabs?.classList.contains('animate-in')).toBe(false);
    expect(tabs?.classList.contains('slide-in-from-right-12')).toBe(false);
    const tabLabels = [
      ...container.querySelectorAll<HTMLElement>('[role="tab"]'),
    ].map((tab) => tab.textContent?.trim());
    expect(tabLabels).toEqual(['运行状态']);
    expect(container.textContent).not.toContain('硬盘');
  });

  it('renders A-04 failure evidence without implying successful closure', async () => {
    const container = await mount(
      RecordsPanel,
      { view: records },
      '/edge/server/records/A-20260720-008',
    );
    expect(container.textContent).toContain('批次详情');
    expect(container.textContent).toContain('已停止危险写入');
    expect(container.textContent).toContain('未生成可送出标记');
    expect(container.textContent).not.toContain('端到端闭环完成');
    const pageHeader = container.querySelector(
      '.detail-page-header.ant-page-header',
    );
    expect(pageHeader).not.toBeNull();
    expect(
      pageHeader?.querySelector('.ant-page-header-back-button'),
    ).not.toBeNull();
    expect(pageHeader?.querySelector('.detail-back-label')).toBeNull();
    expect(
      pageHeader?.querySelector('.ant-page-header-heading-title')?.textContent,
    ).toContain('批次详情');
    expect(
      pageHeader?.querySelector('.ant-page-header-heading-sub-title')
        ?.textContent,
    ).toContain('A-20260720-008');
    expect(
      pageHeader?.querySelector('.ant-page-header-heading-tags')?.textContent,
    ).toContain('失败');
    expect(
      container.querySelector('.record-stage-steps.ant-steps'),
    ).not.toBeNull();
    expect(container.querySelectorAll('.ant-steps-item')).toHaveLength(4);
    expect(container.querySelector('.stage-track')).toBeNull();
    expect(
      container.querySelector(
        'img[src="/assets/fustfs-baseline/a04-failed-lock-v1.png"]',
      ),
    ).not.toBeNull();
    expect(
      container.querySelector(
        'img[src="/assets/fustfs-baseline/a04-failed-lock-small-v1.png"]',
      ),
    ).not.toBeNull();
    expect(
      container.querySelector('.event-table.ant-table-wrapper'),
    ).not.toBeNull();
    expect(container.querySelector('.event-table table')).not.toBeNull();
    expect(container.querySelectorAll('.event-table tbody tr')).toHaveLength(1);
  });

  it('renders the A-04 packed detail with the baseline shield and Ant Steps', async () => {
    const container = await mount(
      RecordsPanel,
      { view: records },
      '/edge/server/records/A-20260720-007',
    );
    expect(container.textContent).toContain('本机装盘完成，等待后续确认');
    expect(container.textContent).toContain('尚未确认中控接收或回执');
    expect(container.textContent).not.toContain('端到端闭环已确认');
    expect(
      container.querySelector('.record-stage-steps.ant-steps'),
    ).not.toBeNull();
    expect(container.querySelectorAll('.ant-steps-item')).toHaveLength(4);
    expect(container.querySelector('.stage-track')).toBeNull();
    expect(
      container.querySelector(
        'img[src="/assets/fustfs-baseline/a04-packed-shield-v1.png"]',
      ),
    ).not.toBeNull();
  });

  it('keeps the A-04 heading notice on one row and paginates long history', async () => {
    const record = records.records[0];
    if (!record) throw new Error('A-04 test fixture must include one record');
    const longHistory: EdgeSyncRecordsView = {
      ...records,
      records: Array.from({ length: 10 }, (_, index) => ({
        ...record,
        batch_id: `A-20260720-${String(index + 1).padStart(3, '0')}`,
        media_serial_suffix: String(8000 + index),
      })),
      summary: {
        ...records.summary,
        failed: 10,
        total: 10,
      },
    };
    const container = await mount(
      RecordsPanel,
      { view: longHistory },
      '/edge/server',
    );

    expect(
      container.querySelector('.records-heading-copy')?.textContent,
    ).toContain('同步记录');
    expect(
      container.querySelector('.records-heading-copy')?.textContent,
    ).toContain('本机历史独立保存');
    expect(
      container.querySelector('.records-heading .media-notice'),
    ).not.toBeNull();
    expect(container.textContent).not.toContain('待回执');
    expect(container.textContent).not.toContain('闭环完成');
    expect(
      container.querySelectorAll('.record-scroll .record-row'),
    ).toHaveLength(8);

    const nextPage = container.querySelector<HTMLButtonElement>(
      'button[aria-label="下一页"]',
    );
    expect(nextPage?.disabled).toBe(false);
    nextPage?.click();
    await nextTick();

    expect(
      container.querySelectorAll('.record-scroll .record-row'),
    ).toHaveLength(2);
    expect(container.querySelector('.pagination-count b')?.textContent).toBe(
      '2',
    );
    expect(
      container.querySelector('.pagination-count span:last-child')?.textContent,
    ).toBe('2');
  });

  it('retains the last complete values after collection failure', async () => {
    const container = await mount(SitesPanel, { view: sites });
    expect(container.textContent).toContain('采集失败保留最近完整快照');
    expect(container.textContent).toContain('1,024');
    expect(container.textContent).toContain('128');
    const trigger = container.querySelector<HTMLButtonElement>(
      'button[aria-label*="立即获取"]',
    );
    expect(trigger?.disabled).toBe(true);
    expect(trigger?.title).toBe('需要 CONTROL_ADMIN 权限');
  });

  it('provides named controls, headings and a real data table', async () => {
    const container = await mount(SitesPanel, { view: sites });
    expect(container.querySelectorAll('h1')).toHaveLength(1);
    expect(
      container.querySelector(
        '.sites-workspace[data-layout-reference="control-history"]',
      ),
    ).not.toBeNull();
    expect(container.querySelector('.sites-note.ant-alert')).not.toBeNull();
    expect(
      container.querySelectorAll('.site-summary-filter.ant-btn'),
    ).toHaveLength(5);
    expect(
      container.querySelector('.sites-search.ant-input-affix-wrapper'),
    ).not.toBeNull();
    expect(
      container.querySelector('.sites-table.ant-table-wrapper'),
    ).not.toBeNull();
    expect(container.querySelector('.sites-table table')).not.toBeNull();
    expect(container.querySelector('.filters')).toBeNull();
    const controls = [...container.querySelectorAll('button, input, a')];
    expect(controls.length).toBeGreaterThan(5);
    expect(
      controls.every(
        (control) =>
          control.getAttribute('aria-label') ||
          control.textContent?.trim() ||
          control.getAttribute('placeholder'),
      ),
    ).toBe(true);
  });
});
