import type { Component } from 'vue';

import { createApp, nextTick } from 'vue';
import { createMemoryHistory, createRouter } from 'vue-router';

import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const { getControlIngestOverviewView } = vi.hoisted(() => ({
  getControlIngestOverviewView: vi.fn(),
}));

vi.mock('#/api/local-views', async (importOriginal) => ({
  ...(await importOriginal<typeof import('#/api/local-views')>()),
  getControlIngestOverviewView,
}));

import ConflictsPage from '../control/conflicts.vue';
import FactoryAddPage from '../control/factory-add.vue';
import FactoryAdminPage from '../control/factory-admin.vue';
import FactoryRegistrationPage from '../control/factory-registration.vue';
import HistoryPage from '../control/history.vue';
import MediaDetailPage from '../control/media-detail.vue';
import OverviewPage from '../control/overview.vue';
import SettingsPage from '../control/settings.vue';

const mounted: Array<ReturnType<typeof createApp>> = [];
const controlIngestView = {
  meta: { schema_version: 'i4.1' },
  site_id: 'control-b',
  summary: {
    connected_media: 1,
    conflict_locked: 0,
    failed: 0,
    importing: 1,
    queued: 0,
    source_sites: 1,
    verified: 0,
  },
  tasks: [
    {
      batch_id: 'legacy-transport-record-001',
      completed_at: null,
      failure_reason: null,
      logical_bytes: 1024,
      media_id: 'media-a',
      media_label: 'transport-a',
      media_serial_suffix: 'A01',
      object_count: 1,
      progress_percent: 50,
      receipt_id: null,
      result_label: 'importing',
      source_site_id: 'factory-a',
      started_at: '2026-08-03T09:00:00Z',
      stage_label: '解密归档中',
      state: 'IMPORTING',
      transport_record_id: 'transport_record-001',
      updated_at: '2026-08-03T12:00:00Z',
      verified_bytes: 512,
    },
  ],
};

beforeEach(() => {
  getControlIngestOverviewView.mockResolvedValue(controlIngestView);
});
const routes = [
  '/control',
  '/control/media',
  '/control/conflicts',
  '/control/history',
  '/control/settings',
  '/control/settings/factories',
  '/control/settings/factories/new',
  '/control/settings/factories/:siteId/registration',
].map((path) => ({
  component: { template: '<div />' },
  path,
}));

async function mountPage(
  component: Component,
  initialPath: string,
  props?: Record<string, unknown>,
) {
  const container = document.createElement('div');
  document.body.append(container);
  const router = createRouter({
    history: createMemoryHistory(),
    routes,
  });
  await router.push(initialPath);
  await router.isReady();
  const app = createApp(component, props);
  app.use(router);
  app.mount(container);
  mounted.push(app);
  await nextTick();
  await nextTick();
  return container;
}

function expectFrozenBaseline(container: HTMLElement, baselineKey: string) {
  const root = container.querySelector(`[data-baseline-key="${baselineKey}"]`);
  expect(root).not.toBeNull();
  expect((root as HTMLElement | null)?.dataset.viewSource).toBe(
    'frozen-baseline-fixture',
  );
}

afterEach(() => {
  mounted.splice(0).forEach((app) => app.unmount());
  document.body.replaceChildren();
});

describe('control frozen page review contracts', () => {
  it('keeps B-01 on the frozen fixture and reuses Ant progress', async () => {
    const container = await mountPage(OverviewPage, '/control');

    expectFrozenBaseline(container, 'B-01-ingest-overview');
    expect(container.querySelector('.overview-navigation')).toBeNull();
    expect(container.querySelector('.runtime-header-activity')).not.toBeNull();
    expect(container.querySelector('.center-rack')?.getAttribute('href')).toBe(
      '/control/history',
    );
    expect(
      container.querySelector('.current-ingest .ant-progress'),
    ).not.toBeNull();
    expect(container.querySelectorAll('.source-device')).toHaveLength(3);
    expect(
      [...container.querySelectorAll('.source-device')].map((item) =>
        item.getAttribute('href'),
      ),
    ).toEqual(['/control/media', '/control/media', '/control/conflicts']);
    const particleCanvas =
      container.querySelector<HTMLCanvasElement>('.data-flow-canvas');
    expect(particleCanvas?.dataset.particleRenderer).toBe('path-aether');
    expect(particleCanvas?.dataset.particleState).toBe('running');
    expect(particleCanvas?.dataset.particlePathStart).toBe('0.215,0.304');
    expect(particleCanvas?.dataset.particlePathEnd).toBe('0.704,0.470');
    expect(container.querySelector('.stage-caption')).toBeNull();
    expect(container.textContent).not.toContain('验签 · 内存解密 · 汇聚入库');
    expect(container.textContent).toContain('A-006');
  });

  it('renders only reported B-01 media and never invents import speed for a local projection', async () => {
    const container = await mountPage(OverviewPage, '/control', {
      view: controlIngestView,
    });

    const root = container.querySelector('[data-baseline-key="B-01-ingest-overview"]');
    expect((root as HTMLElement).dataset.viewSource).toBe('local-control-api');
    expect(container.querySelectorAll('.source-device')).toHaveLength(1);
    expect(container.textContent).toContain('factory-a');
    expect(container.textContent).toContain('512 B / 1 KB');
    expect(container.textContent).toContain('暂未上报');
    expect(container.textContent).toContain('B 归档存储');
    expect(container.textContent).not.toContain('A-006');
  });

  it('renders reported B archive capacity from the local CONTROL projection', async () => {
    const container = await mountPage(OverviewPage, '/control', {
      view: {
        ...controlIngestView,
        storage: {
          available_bytes: 3 * 1024 ** 3,
          reported_at: '2026-08-03T12:00:00Z',
          total_bytes: 10 * 1024 ** 3,
        },
      },
    });

    expect(container.textContent).toContain('已用 7.0 GB / 10.0 GB');
    expect(container.textContent).toContain('3.0 GB');
  });

  it('renders B-04 from the local CONTROL API, not a frozen fixture', async () => {
    const container = await mountPage(MediaDetailPage, '/control/media');

    await new Promise((resolve) => setTimeout(resolve, 0));
    await nextTick();
    const root = container.querySelector('[data-baseline-key="B-04-media-detail"]');
    expect(root).not.toBeNull();
    expect((root as HTMLElement).dataset.viewSource).toBe('local-agent-api');
    expect(container.textContent).toContain('transport_record-001');
    expect(container.textContent).not.toContain('A-20260721-009');
    expect(
      container.querySelector('.media-progress.ant-progress'),
    ).not.toBeNull();
    expect(container.querySelector('.ingest-steps.ant-steps')).not.toBeNull();
    expect(
      container.querySelectorAll('.ingest-steps .ant-steps-item'),
    ).toHaveLength(5);
    expect(container.querySelector('.product-close')).not.toBeNull();
    expect(container.textContent).toContain('暂未上报');
    expect(container.textContent).toContain('最后上报');
  });

  it('shows a failed B-04 import as stopped and never as a signed receipt', async () => {
    getControlIngestOverviewView.mockResolvedValue({
      ...controlIngestView,
      tasks: [
        {
          ...controlIngestView.tasks[0],
          failure_reason: 'AES-GCM authentication failed',
          receipt_id: null,
          result_label: 'authentication rejected',
          stage_label: '解密认证失败',
          state: 'FAILED',
        },
      ],
    });
    const container = await mountPage(MediaDetailPage, '/control/media');

    await new Promise((resolve) => setTimeout(resolve, 0));
    await nextTick();
    expect(container.textContent).toContain('AES-GCM authentication failed');
    expect(container.textContent).toContain('未签发回执');
    expect(container.querySelector('.ingest-steps .ant-steps-item-error')).not.toBeNull();
    expect(container.querySelector('.receipt-state')?.classList).toContain('is-failure');
  });

  it('keeps B-04 loading while the local CONTROL projection is unresolved', async () => {
    getControlIngestOverviewView.mockImplementation(
      () => new Promise(() => undefined),
    );
    const container = await mountPage(MediaDetailPage, '/control/media');

    expect(container.querySelector('.view-state')).not.toBeNull();
    expect(container.querySelector('[data-baseline-key="B-04-media-detail"]')).toBeNull();
  });

  it('fails closed when B-04 cannot read the local CONTROL projection', async () => {
    getControlIngestOverviewView.mockRejectedValue(new Error('Agent unavailable'));
    const container = await mountPage(MediaDetailPage, '/control/media');

    await new Promise((resolve) => setTimeout(resolve, 0));
    await nextTick();
    expect(container.querySelector('.view-state')).not.toBeNull();
    expect(container.querySelector('[data-baseline-key="B-04-media-detail"]')).toBeNull();
  });

  it('fails closed when B-04 has no matching transport record', async () => {
    getControlIngestOverviewView.mockResolvedValue({
      ...controlIngestView,
      tasks: [],
    });
    const container = await mountPage(MediaDetailPage, '/control/media');

    await new Promise((resolve) => setTimeout(resolve, 0));
    await nextTick();
    expect(container.querySelector('.view-state')).not.toBeNull();
    expect(container.querySelector('[data-baseline-key="B-04-media-detail"]')).toBeNull();
  });

  it('keeps B-05 conflict resolution locked and export fixture-only', async () => {
    const container = await mountPage(ConflictsPage, '/control/conflicts');

    expectFrozenBaseline(container, 'B-05-conflict-lock');
    expect(container.querySelector('.conflict-steps.ant-steps')).not.toBeNull();
    expect(
      container.querySelector<HTMLButtonElement>('.locked-button')?.disabled,
    ).toBe(true);

    container
      .querySelector<HTMLButtonElement>('.diagnostic-button')
      ?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    await nextTick();
    expect(
      container.querySelector('.detail-action small[aria-live="polite"]')
        ?.textContent,
    ).not.toBe('');
    expect(container.querySelector('a[download]')).toBeNull();
  });

  it('keeps B-06 review filters and records on Ant controls', async () => {
    const container = await mountPage(HistoryPage, '/control/history');

    expectFrozenBaseline(container, 'B-06-records');
    expect(
      container.querySelectorAll('.control-server-tabs .ant-tabs-tab'),
    ).toHaveLength(4);
    expect(
      container.querySelector(
        '.control-server-tabs .ant-tabs-tab-active .ant-tabs-tab-btn',
      )?.textContent,
    ).toContain('同步记录');
    expect(
      container.querySelector<HTMLElement>('.history-workspace')?.dataset
        .layoutReference,
    ).toBe('edge-server-records');
    expect(
      container.querySelector('.history-heading-row .history-heading-copy h2'),
    ).not.toBeNull();
    expect(
      container.querySelector('.history-heading-row .ant-alert'),
    ).not.toBeNull();
    expect(container.querySelector('.history-note.ant-alert')).not.toBeNull();
    expect(
      container.querySelector('.history-table.ant-table-wrapper'),
    ).not.toBeNull();
    expect(
      container.querySelectorAll('.history-controls .ant-select'),
    ).toHaveLength(2);
    expect(
      container.querySelector('.history-search.ant-input-affix-wrapper'),
    ).not.toBeNull();
    expect(
      container.querySelectorAll('.history-summary .summary-filter.ant-btn'),
    ).toHaveLength(6);
    expect(container.querySelector('.history-rack')).toBeNull();
    expect(container.querySelector('.pagination-count')).not.toBeNull();
    expect(
      container.querySelector('.history-controls .state-buttons'),
    ).toBeNull();
    expect(
      container.querySelectorAll('.history-controls .ant-btn'),
    ).toHaveLength(0);
    expect(container.querySelector('.row-action.ant-btn')).not.toBeNull();
  });

  it('keeps B-07 read-only with no editable or submit controls', async () => {
    const container = await mountPage(SettingsPage, '/control/settings');

    expectFrozenBaseline(container, 'B-07-control-config');
    expect(
      container.querySelector(
        '.control-server-tabs .ant-tabs-tab-active .ant-tabs-tab-btn',
      )?.textContent,
    ).toContain('中控配置');
    expect(container.querySelector('.settings-status .ant-tag')).not.toBeNull();
    expect(
      container.querySelector('input, textarea, [contenteditable="true"]'),
    ).toBeNull();
    expect(container.querySelector('button[type="submit"]')).toBeNull();
  });
});

describe('control admin frozen page review contracts', () => {
  it('keeps B-08 factory list role-marked and on Ant table/input/button', async () => {
    const container = await mountPage(
      FactoryAdminPage,
      '/control/settings/factories',
    );

    expectFrozenBaseline(container, 'B-08-factory-admin');
    expect(
      container.querySelector<HTMLElement>('.factory-admin-page')?.dataset
        .requiredRole,
    ).toBe('CONTROL_ADMIN');
    expect(
      container.querySelector('.factory-table.ant-table-wrapper'),
    ).not.toBeNull();
    expect(
      container.querySelector('.admin-tools .ant-input-affix-wrapper'),
    ).not.toBeNull();
    expect(container.querySelectorAll('.state-filters .ant-btn')).toHaveLength(
      5,
    );
  });

  it('keeps B-08 add form fixture-only and never creates a download', async () => {
    const container = await mountPage(
      FactoryAddPage,
      '/control/settings/factories/new',
    );

    expectFrozenBaseline(container, 'B-08-add-factory');
    expect(
      container.querySelector<HTMLElement>('.factory-add-page')?.dataset
        .requiredRole,
    ).toBe('CONTROL_ADMIN');
    expect(container.querySelector('.factory-form.ant-form')).not.toBeNull();
    expect(
      container.querySelectorAll('.factory-form .ant-input').length,
    ).toBeGreaterThan(2);
    expect(
      container.querySelectorAll('.factory-form .ant-select'),
    ).toHaveLength(2);
    expect(
      container.querySelector<HTMLButtonElement>(
        '.factory-form button[type="submit"]',
      ),
    ).not.toBeNull();
    expect(container.querySelector('a[download]')).toBeNull();
  });

  it('keeps B-08 registration actions disabled and download fixture-only', async () => {
    const container = await mountPage(
      FactoryRegistrationPage,
      '/control/settings/factories/factory-a-007/registration',
    );

    expectFrozenBaseline(container, 'B-08-registration-validation');
    expect(
      container.querySelector<HTMLElement>('.registration-page')?.dataset
        .requiredRole,
    ).toBe('CONTROL_ADMIN');
    expect(
      container.querySelector('.registration-progress.ant-steps'),
    ).not.toBeNull();
    expect(
      container.querySelectorAll('.registration-progress .ant-steps-item'),
    ).toHaveLength(5);
    expect(
      container.querySelectorAll<HTMLButtonElement>(
        '.registration-page button:disabled',
      ),
    ).toHaveLength(2);
    expect(container.querySelector('a[download]')).toBeNull();
  });
});
