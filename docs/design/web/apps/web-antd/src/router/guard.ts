import type { Router } from 'vue-router';

import { preferences } from '@vben/preferences';
import { startProgress, stopProgress } from '@vben/utils';

import { getLocalViewContext } from '#/api/local-views';

/**
 * I0 installs navigation progress only.
 *
 * Authentication and authorization guards enter with their frozen contract
 * instead of retaining the upstream demo login behavior.
 */
function createRouterGuard(router: Router) {
  const loadedPaths = new Set<string>();
  let installedRole: null | string = null;

  router.beforeEach(async (to) => {
    to.meta.loaded = loadedPaths.has(to.path);
    if (!to.meta.loaded && preferences.transition.progress) {
      await startProgress();
    }
    const expectedRoles = to.meta.authority;
    // CONTROL_ADMIN has no installed-role identity contract yet. Keep these
    // frozen review scenes unreachable even when the context endpoint fails.
    if (expectedRoles?.includes('CONTROL_ADMIN')) {
      return '/control';
    }
    if (expectedRoles?.length) {
      try {
        if (installedRole === null) {
          const context = await getLocalViewContext();
          if (context.role !== 'EDGE' && context.role !== 'CONTROL') {
            throw new Error('The installed local role is unavailable.');
          }
          installedRole = context.role;
        }
        if (!expectedRoles.includes(installedRole)) {
          return installedRole === 'EDGE' ? '/edge' : '/control';
        }
      } catch {
        // The target page renders the fail-closed unavailable state.
      }
    }
    return true;
  });

  router.afterEach(async (to) => {
    loadedPaths.add(to.path);
    if (preferences.transition.progress) {
      await stopProgress();
    }
  });
}

export { createRouterGuard };
