import type { EdgeRegistrationView } from '#/api/local-views';

import { createApp, nextTick, ref } from 'vue';
import { createMemoryHistory, createRouter } from 'vue-router';

import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const { exportRequest, importResponse, reload } = vi.hoisted(() => ({
  exportRequest: vi.fn(),
  importResponse: vi.fn(),
  reload: vi.fn(),
}));

const localView = {
  data: ref<EdgeRegistrationView>(),
  error: ref(''),
  loading: ref(false),
  reload,
};

vi.mock('#/api/local-views', () => ({
  exportIsolatedTrialRegistrationRequest: exportRequest,
  getIsolatedTrialRegistrationView: vi.fn(),
  importIsolatedTrialRegistrationResponse: importResponse,
}));

vi.mock('../../components/product-shell.vue', () => ({
  default: { template: '<main data-product-shell><slot /></main>' },
}));

vi.mock('../../components/view-state.vue', () => ({
  default: {
    emits: ['retry'],
    props: ['kind', 'message'],
    template:
      '<button data-view-state type="button" @click="$emit(\'retry\')">{{ kind }}: {{ message }}</button>',
  },
}));

vi.mock('../../use-local-view', () => ({
  useLocalView: () => localView,
}));

import Registration from '../registration.vue';

const mounted: Array<ReturnType<typeof createApp>> = [];

function registrationView(
  overrides: Partial<EdgeRegistrationView> = {},
): EdgeRegistrationView {
  return {
    can_generate_identity: false,
    capabilities: [],
    meta: {
      data_as_of: null,
      freshness: 'FRESH',
      generated_at: '2026-08-03T00:00:00+08:00',
      retained_after_failure: false,
      schema_version: 'i4.1',
      status_message: 'ready',
    },
    package: {
      control_label: 'Control B',
      expires_at: '2026-08-04T00:00:00+08:00',
      package_id: 'isolated-trial-package',
      signature_valid: true,
      site_display_name: 'Factory A',
      site_id: 'factory-a-001',
      site_role: 'EDGE',
      state: 'VALID',
    },
    phase: 'IMPORT',
    site_id: 'factory-a-001',
    trial_mode: 'ISOLATED_READ_ONLY',
    ...overrides,
  };
}

async function mountRegistration() {
  const container = document.createElement('div');
  document.body.append(container);
  const router = createRouter({
    history: createMemoryHistory(),
    routes: [
      { component: Registration, path: '/edge/register' },
      { component: { template: '<div>edge home</div>' }, path: '/edge' },
    ],
  });
  await router.push('/edge/register');
  await router.isReady();
  const app = createApp({ template: '<RouterView />' });
  app.use(router);
  app.mount(container);
  mounted.push(app);
  await nextTick();
  return { container, router };
}

afterEach(() => {
  mounted.splice(0).forEach((app) => app.unmount());
  document.body.replaceChildren();
});

beforeEach(() => {
  exportRequest.mockReset();
  importResponse.mockReset();
  reload.mockReset();
  exportRequest.mockResolvedValue(undefined);
  importResponse.mockResolvedValue(undefined);
  reload.mockResolvedValue(undefined);
  localView.data.value = registrationView();
  localView.error.value = '';
  localView.loading.value = false;
});

describe('edge isolated-trial registration actions', () => {
  it('uses empty POST actions and switches from request export to response import', async () => {
    const { container } = await mountRegistration();
    const exportButton = container.querySelector<HTMLButtonElement>(
      '.registration-actions button',
    );
    expect(exportButton?.disabled).toBe(false);

    exportButton?.click();
    await nextTick();
    await Promise.resolve();
    await nextTick();

    expect(exportRequest).toHaveBeenCalledOnce();
    expect(reload).toHaveBeenCalledOnce();
    const importButton = container.querySelector<HTMLButtonElement>(
      '.registration-actions button',
    );
    expect(importButton?.disabled).toBe(false);

    importButton?.click();
    await Promise.resolve();
    await nextTick();

    expect(importResponse).toHaveBeenCalledOnce();
    expect(reload).toHaveBeenCalledTimes(2);
  });

  it('fails closed when the explicit isolated projection is unavailable', async () => {
    localView.data.value = registrationView({ trial_mode: 'UNAVAILABLE' });
    const { container } = await mountRegistration();

    expect(container.querySelector('[data-view-state]')).not.toBeNull();
    expect(container.querySelector('.registration-actions button')).toBeNull();
    expect(exportRequest).not.toHaveBeenCalled();
    expect(importResponse).not.toHaveBeenCalled();
  });

  it('keeps the action stage closed after an export failure', async () => {
    exportRequest.mockRejectedValueOnce(new Error('Agent unavailable'));
    const { container } = await mountRegistration();
    const action = container.querySelector<HTMLButtonElement>(
      '.registration-actions button',
    );
    action?.click();
    await Promise.resolve();
    await nextTick();

    expect(container.querySelector('.action-status')).not.toBeNull();
    expect(importResponse).not.toHaveBeenCalled();
    expect(reload).not.toHaveBeenCalled();
  });

  it('renders the edge-home navigation only after complete state', async () => {
    localView.data.value = registrationView({ phase: 'COMPLETE' });
    const { container } = await mountRegistration();

    const navigation = container.querySelector<HTMLAnchorElement>(
      '.registration-actions a',
    );
    expect(navigation?.getAttribute('href')).toBe('/edge');
    expect(container.querySelector('.registration-actions button')).toBeNull();
  });
});
