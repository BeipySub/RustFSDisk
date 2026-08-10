import type {
  EdgeMediaCandidatesView,
  EdgeRuntimeView,
  EdgeTransportDisksView,
} from '#/api/local-views';

import { createApp, nextTick } from 'vue';
import { createMemoryHistory, createRouter } from 'vue-router';

import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const { initialize, initializeCandidate } = vi.hoisted(() => ({
  initialize: vi.fn(),
  initializeCandidate: vi.fn(),
}));

vi.mock('#/api/local-views', () => ({
  initializeEdgeMediaCandidate: initializeCandidate,
  initializeUnregisteredEdgeTransportDisk: initialize,
}));

import NasDisksPanel from '../nas-disks-panel.vue';

const mounted: Array<ReturnType<typeof createApp>> = [];

const meta = {
  data_as_of: null,
  freshness: 'FRESH' as const,
  generated_at: '2026-08-06T12:00:00+08:00',
  retained_after_failure: false,
  schema_version: 'i4.1' as const,
  status_message: '最新扫描完成',
};

const runtime: EdgeRuntimeView = {
  current: null,
  display_name: 'A 工厂',
  media: {
    completed: 0,
    connected: 1,
    failed: 0,
    running: 0,
    standby: 0,
    warning: 0,
  },
  meta,
  next_action: {
    action_code: 'INITIALIZE_NEW_MEDIA',
    detail: '等待本地管理员确认',
    media_slot: '01',
    priority: 'INFO',
    requires_role: 'EDGE',
    serial_suffix: '1234',
    title: '确认初始化新运输盘',
  },
  site_id: 'factory-a-001',
  state: 'IDLE',
  state_label: '空闲',
  throughput_bytes_per_second: null,
};

function diskView(
  initialization: EdgeTransportDisksView['disks'][number]['initialization'],
): EdgeTransportDisksView {
  return {
    disks: [
      {
        capacity_bytes: 1_000_000_000_000,
        exclusion_reason: null,
        exclusion_state: 'READY',
        filesystem_label: null,
        in_use: false,
        initialization,
        life_percent: null,
        media_id_suffix: null,
        media_label: '新运输盘',
        progress_percent: null,
        read_only: false,
        serial_suffix: '1234',
        slot: '01',
        smart_state: 'READY',
        state: 'UNINITIALIZED',
        state_label: '未初始化',
        temperature_celsius: null,
      },
    ],
    last_scan_at: '2026-08-06T12:00:00+08:00',
    meta,
    site_id: 'factory-a-001',
    summary: { connected: 1, failed: 0, healthy: 1, warning: 0 },
  };
}

async function mount(
  view: EdgeTransportDisksView,
  candidates?: EdgeMediaCandidatesView,
) {
  const container = document.createElement('div');
  document.body.append(container);
  const router = createRouter({
    history: createMemoryHistory(),
    routes: [
      { component: { template: '<div />' }, path: '/edge/nas' },
      { component: { template: '<div />' }, path: '/edge/nas/disks' },
    ],
  });
  await router.push('/edge/nas/disks');
  await router.isReady();
  const app = createApp(NasDisksPanel, { candidates, runtime, view });
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

beforeEach(() => {
  initialize.mockReset();
  initializeCandidate.mockReset();
  initialize.mockResolvedValue(undefined);
  initializeCandidate.mockResolvedValue(undefined);
});

describe('A-06 unregistered transport disk initialization', () => {
  it('shows the confirmation entry only for a securely discovered unregistered disk', async () => {
    const container = await mount(
      diskView({
        capability: 'INITIALIZE_UNREGISTERED_MEDIA',
        discovery_token: 'opaque-discovery-token',
        requires_confirmation: true,
      }),
    );

    const listAction = container.querySelector<HTMLButtonElement>(
      '[data-testid="disk-initialization-entry"]',
    );
    expect(container.querySelector('.new-disk-badge')?.textContent).toContain(
      '新硬盘',
    );
    expect(listAction?.textContent).toContain('初始化');
    expect(
      container.querySelector('[data-testid="transport-initialization"]'),
    ).not.toBeNull();
    expect(container.querySelector('input')).toBeNull();
    expect(container.textContent).not.toContain('opaque-discovery-token');

    listAction?.click();
    await nextTick();
    expect(container.textContent).toContain('盘位 01');
    expect(container.textContent).toContain('SN …1234');
    const confirmation = [
      ...container.querySelectorAll<HTMLButtonElement>('button'),
    ].find((button) => button.textContent?.trim() === '确认初始化');
    confirmation?.click();
    await Promise.resolve();
    await nextTick();

    expect(initialize).toHaveBeenCalledWith('opaque-discovery-token');
    expect(container.textContent).toContain('初始化请求已提交');
  });

  it('fails closed without the Agent-issued initialization capability', async () => {
    const container = await mount(diskView(null));

    expect(
      container.querySelector('[data-testid="transport-initialization"]'),
    ).toBeNull();
    expect(
      container.querySelector('[data-testid="disk-initialization-entry"]'),
    ).toBeNull();
    expect(container.querySelector('.new-disk-badge')).toBeNull();
    expect(initialize).not.toHaveBeenCalled();
  });

  it('offers candidate initialization only for an Agent-authorized new non-trusted disk', async () => {
    const candidates: EdgeMediaCandidatesView = {
      ...diskView(null),
      candidates: [
        {
          candidate_id: 'candidate-new',
          candidate_session_id: 'candidate-session-new',
          capacity_bytes: 1_000_000_000_000,
          class: 'CANDIDATE',
          filesystem_type: null,
          mounted_filesystems: 0,
          read_only: false,
          registration_state: 'UNREGISTERED',
          rejection: null,
          serial_hex: '35363738',
          serial_suffix: '5678',
          trusted_slot: null,
        },
        {
          candidate_id: 'candidate-rejected',
          candidate_session_id: 'candidate-session-rejected',
          capacity_bytes: null,
          class: 'REJECTED',
          filesystem_type: null,
          mounted_filesystems: 1,
          read_only: false,
          rejection: 'SYSTEM_DISK',
          serial_hex: '30303031',
          serial_suffix: '0001',
          trusted_slot: null,
        },
        {
          candidate_id: 'candidate-trusted',
          candidate_session_id: 'candidate-session-trusted',
          capacity_bytes: null,
          class: 'TRUSTED_SLOT',
          filesystem_type: null,
          mounted_filesystems: 0,
          read_only: false,
          rejection: null,
          serial_hex: '30303032',
          serial_suffix: '0002',
          trusted_slot: '01',
        },
        {
          candidate_id: 'candidate-registered',
          candidate_session_id: 'candidate-session-registered',
          capacity_bytes: 1_000_000_000_000,
          class: 'CANDIDATE',
          filesystem_type: 'ext4',
          mounted_filesystems: 0,
          read_only: false,
          registration_state: 'REGISTERED',
          rejection: null,
          serial_hex: '30303034',
          serial_suffix: '0004',
          trusted_slot: null,
        },
        {
          candidate_id: 'candidate-mounted',
          candidate_session_id: 'candidate-session-mounted',
          capacity_bytes: null,
          class: 'CANDIDATE',
          filesystem_type: 'ext4',
          mounted_filesystems: 1,
          read_only: false,
          rejection: null,
          serial_hex: '30303033',
          serial_suffix: '0003',
          trusted_slot: null,
        },
      ],
    };
    const container = await mount(diskView(null), candidates);

    // A v2 projection is authoritative: never mix its actual discoveries
    // with the legacy v1-style slot card supplied by the fallback fixture.
    expect(container.textContent).not.toContain('新运输盘');
    expect(container.textContent).toContain('候选硬盘信息');
    expect(
      container.querySelector('[data-testid="candidate-initialize-candidate-new"]'),
    ).not.toBeNull();
    expect(
      container.querySelector('[data-testid="candidate-initialize-candidate-rejected"]'),
    ).toBeNull();
    expect(
      container.querySelector('[data-testid="candidate-initialize-candidate-trusted"]'),
    ).toBeNull();
    expect(
      container.querySelector('[data-testid="candidate-initialize-candidate-registered"]'),
    ).toBeNull();
    expect(
      container.querySelector('[data-testid="candidate-initialize-candidate-mounted"]'),
    ).toBeNull();
    expect(
      container.querySelector('.nas-candidate-card.tone-success'),
    ).not.toBeNull();

    const registeredCard = [...container.querySelectorAll<HTMLElement>('.nas-candidate-card')]
      .find((card) => card.textContent?.includes('0004'));
    registeredCard?.click();
    await nextTick();
    expect(container.textContent).toContain('已注册运输盘信息');
    expect(container.textContent).toContain('Worker 已管理，等待任务');
    expect(container.textContent).toContain('Worker 受控待命');
    expect(container.textContent).toContain('不适用（该盘已注册）');
    expect(container.textContent).toContain('未挂载（桌面未占用）');

    container
      .querySelector<HTMLButtonElement>(
        '[data-testid="candidate-initialize-candidate-new"]',
      )
      ?.click();
    await nextTick();
    const confirmation = [...container.querySelectorAll<HTMLButtonElement>('button')]
      .find((button) => button.textContent?.trim() === '确认初始化并交由 Worker 管理');
    confirmation?.click();
    await Promise.resolve();
    await nextTick();

    expect(initializeCandidate).toHaveBeenCalledWith(
      'candidate-new',
      'candidate-session-new',
    );
    expect(container.textContent).toContain('初始化请求已提交');
  });
});
