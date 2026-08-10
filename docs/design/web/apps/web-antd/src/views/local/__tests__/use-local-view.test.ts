import { createApp, defineComponent, h, nextTick } from 'vue';

import { afterEach, describe, expect, it } from 'vitest';

import {
  isControlIngestOverviewViewProjection,
  isEdgeSyncRecordsViewProjection,
} from '#/api/local-views';

import { useLocalView } from '../use-local-view';

const mounted: Array<ReturnType<typeof createApp>> = [];

afterEach(() => {
  mounted.splice(0).forEach((app) => app.unmount());
  document.body.replaceChildren();
});

async function settle() {
  await new Promise((resolve) => setTimeout(resolve, 0));
  await nextTick();
}

describe('local view response boundary', () => {
  const emptyControlIngestView = {
    meta: { schema_version: 'i4.1' },
    site_id: 'control-b',
    summary: {
      connected_media: 0,
      conflict_locked: 0,
      failed: 0,
      importing: 0,
      queued: 0,
      source_sites: 0,
      verified: 0,
    },
    tasks: [],
  };

  it('accepts an explicit empty CONTROL import queue', () => {
    expect(isControlIngestOverviewViewProjection(emptyControlIngestView)).toBe(
      true,
    );
  });

  it('accepts absent or explicitly unavailable B archive capacity during collector rollout', () => {
    expect(isControlIngestOverviewViewProjection(emptyControlIngestView)).toBe(
      true,
    );
    expect(
      isControlIngestOverviewViewProjection({
        ...emptyControlIngestView,
        storage: {
          available_bytes: null,
          reported_at: null,
          total_bytes: null,
        },
      }),
    ).toBe(true);
  });

  it('rejects B archive capacity that exceeds its reported total', () => {
    expect(
      isControlIngestOverviewViewProjection({
        ...emptyControlIngestView,
        storage: {
          available_bytes: 11,
          reported_at: '2026-08-04T01:00:00Z',
          total_bytes: 10,
        },
      }),
    ).toBe(false);
  });

  it('rejects CONTROL import tasks whose progress or failure evidence is incomplete', () => {
    expect(
      isControlIngestOverviewViewProjection({
        ...emptyControlIngestView,
        tasks: [
          {
            batch_id: 'batch-001',
            completed_at: null,
            failure_reason: null,
            logical_bytes: 1024,
            media_label: 'transport-01',
            media_serial_suffix: '01',
            object_count: 1,
            progress_percent: 101,
            receipt_id: null,
            result_label: 'importing',
            source_site_id: 'factory-a',
            started_at: '2026-08-03T09:00:00Z',
            stage_label: 'importing',
            state: 'IMPORTING',
            updated_at: '2026-08-03T10:00:00Z',
            verified_bytes: 100,
          },
        ],
      }),
    ).toBe(false);
  });

  it('rejects an HTML fallback instead of rendering it as view data', async () => {
    const container = document.createElement('div');
    document.body.append(container);
    const component = defineComponent({
      setup() {
        const { data, error, loading } = useLocalView(() =>
          Promise.resolve('<!doctype html><html></html>'),
        );
        return () =>
          h(
            'div',
            loading.value
              ? 'loading'
              : error.value || (data.value ? 'data' : 'empty'),
          );
      },
    });
    const app = createApp(component);
    app.mount(container);
    mounted.push(app);
    await settle();

    expect(container.textContent).toContain('本机只读视图暂不可用');
    expect(container.textContent).not.toContain('data');
  });

  it('recovers the fail-closed screen when polling later receives a valid local view', async () => {
    const container = document.createElement('div');
    document.body.append(container);
    let attempts = 0;
    const component = defineComponent({
      setup() {
        const { data, error, loading } = useLocalView(
          () => {
            attempts += 1;
            if (attempts === 1) return Promise.reject(new Error('Agent unavailable'));
            return Promise.resolve({ meta: { schema_version: 'i4.1' } });
          },
          { refreshIntervalMs: 5 },
        );
        return () =>
          h(
            'div',
            loading.value
              ? 'loading'
              : error.value || (data.value ? 'data' : 'empty'),
          );
      },
    });
    const app = createApp(component);
    app.mount(container);
    mounted.push(app);
    await settle();

    expect(container.textContent).toContain('本机只读视图暂不可用');

    await new Promise((resolve) => setTimeout(resolve, 15));
    await settle();

    expect(container.textContent).toBe('data');
  });

  it('rejects a schema-valid but incomplete runtime projection', async () => {
    const container = document.createElement('div');
    document.body.append(container);
    const component = defineComponent({
      setup() {
        const { data, error, loading } = useLocalView(
          () => Promise.resolve({ meta: { schema_version: 'i4.1' } }),
          { isValidPayload: (value) => 'state' in (value as object) },
        );
        return () =>
          h(
            'div',
            loading.value
              ? 'loading'
              : error.value || (data.value ? 'data' : 'empty'),
          );
      },
    });
    const app = createApp(component);
    app.mount(container);
    mounted.push(app);
    await settle();

    expect(container.textContent).toContain(
      'The local view response has an unsupported schema version.',
    );
    expect(container.textContent).not.toContain('data');
  });

  it('rejects a sync projection with malformed stage evidence', async () => {
    const container = document.createElement('div');
    document.body.append(container);
    const malformedRecordsView = {
      meta: { schema_version: 'i4.1' },
      records: [
        {
          batch_id: 'transport-001',
          completed_at: null,
          destination_label: 'Control',
          events: [],
          failure_reason: null,
          failure_stage: null,
          logical_bytes: 1,
          media_serial_suffix: '001',
          result_label: 'Waiting',
          retry_result: null,
          stages: [{ at: null, label: 'Write', state: 'UNVERIFIED' }],
          state: 'PACKED',
        },
      ],
      site_id: 'factory-a-001',
      summary: {
        closed: 0,
        failed: 0,
        packed: 1,
        total: 1,
        waiting_receipt: 0,
      },
      transport_media_connected: false,
    };
    const component = defineComponent({
      setup() {
        const { data, error, loading } = useLocalView(
          () => Promise.resolve(malformedRecordsView),
          { isValidPayload: isEdgeSyncRecordsViewProjection },
        );
        return () =>
          h(
            'div',
            loading.value
              ? 'loading'
              : error.value || (data.value ? 'data' : 'empty'),
          );
      },
    });
    const app = createApp(component);
    app.mount(container);
    mounted.push(app);
    await settle();

    expect(container.textContent).toContain(
      'The local view response has an unsupported schema version.',
    );
    expect(container.textContent).not.toContain('data');
  });
});
