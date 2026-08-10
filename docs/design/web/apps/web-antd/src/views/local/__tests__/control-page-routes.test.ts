import { createApp, nextTick } from 'vue';
import { createMemoryHistory, createRouter } from 'vue-router';

import { afterEach, describe, expect, it } from 'vitest';

import { coreRoutes } from '#/router/routes/core';

import ProductShell from '../components/product-shell.vue';

const mounted: Array<ReturnType<typeof createApp>> = [];

async function mountShell(path: string) {
  const container = document.createElement('div');
  document.body.append(container);
  const router = createRouter({
    history: createMemoryHistory(),
    routes: [
      { component: { template: '<div />' }, path: '/control' },
      { component: { template: '<div />' }, path: '/control/sites' },
      { component: { template: '<div />' }, path: '/control/media' },
      { component: { template: '<div />' }, path: '/control/conflicts' },
      { component: { template: '<div />' }, path: '/control/history' },
      { component: { template: '<div />' }, path: '/control/settings' },
      {
        component: { template: '<div />' },
        path: '/control/settings/factories',
      },
    ],
  });
  await router.push(path);
  await router.isReady();
  const app = createApp(ProductShell, {
    baselineCanvas: true,
    displayName: '中心 B · 中控',
    role: 'CONTROL',
  });
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

describe('control page route map', () => {
  it('keeps runtime pages behind control and admin scenes fail closed', () => {
    const paths = [
      '/control',
      '/control/sites',
      '/control/sites/:siteId',
      '/control/sites/:siteId/collection',
      '/control/media',
      '/control/conflicts',
      '/control/history',
      '/control/settings',
    ];

    for (const path of paths) {
      const route = coreRoutes.find((candidate) => candidate.path === path);
      expect(route, `${path} is missing`).toBeDefined();
      expect(route?.meta?.authority).toContain('CONTROL');
    }

    for (const path of [
      '/control/settings/factories',
      '/control/settings/factories/new',
      '/control/settings/factories/:siteId/registration',
    ]) {
      const route = coreRoutes.find((candidate) => candidate.path === path);
      expect(route, `${path} is missing`).toBeDefined();
      expect(route?.meta?.authority).toEqual(['CONTROL_ADMIN']);
    }
  });

  it('groups CONTROL navigation around home, media, and the control server', async () => {
    const overview = await mountShell('/control/sites');
    expect(
      [...overview.querySelectorAll('.nav-link')].map((item) =>
        item.textContent?.trim(),
      ),
    ).toEqual(['首页／入库总览', '运输盘', '中控服务器']);
    expect(overview.querySelector('.nav-link.disabled')).toBeNull();
    expect(overview.querySelector('.nav-link.active')?.textContent).toContain(
      '中控服务器',
    );
    expect(overview.querySelector('.brand')?.getAttribute('href')).toBe(
      '/control',
    );

    const settings = await mountShell('/control/settings/factories');
    expect(settings.querySelector('.nav-link.active')?.textContent).toContain(
      '中控服务器',
    );
  });
});
