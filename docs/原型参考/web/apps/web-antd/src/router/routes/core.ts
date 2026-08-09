import type { RouteRecordRaw } from 'vue-router';

import { $t } from '#/locales';

const fallbackNotFoundRoute: RouteRecordRaw = {
  component: () => import('#/views/_core/fallback/not-found.vue'),
  meta: {
    hideInBreadcrumb: true,
    hideInMenu: true,
    hideInTab: true,
    title: '404',
  },
  name: 'FallbackNotFound',
  path: '/:path(.*)*',
};

const coreRoutes: RouteRecordRaw[] = [
  {
    component: () => import('#/views/local/role-gate.vue'),
    meta: {
      hideInBreadcrumb: true,
      hideInMenu: true,
      hideInTab: true,
      title: $t('page.localViews.entry'),
    },
    name: 'LocalRoleGate',
    path: '/',
  },
  {
    component: () => import('#/views/local/edge/runtime.vue'),
    meta: {
      authority: ['EDGE'],
      hideInBreadcrumb: true,
      title: $t('page.localViews.edgeRuntime'),
    },
    name: 'EdgeRuntime',
    path: '/edge',
  },
  {
    component: () => import('#/views/local/edge/registration.vue'),
    meta: {
      authority: ['EDGE'],
      hideInBreadcrumb: true,
      title: 'EDGE 首次注册',
    },
    name: 'EdgeRegistration',
    path: '/edge/register',
  },
  {
    component: () => import('#/views/local/edge/runtime.vue'),
    meta: {
      authority: ['EDGE'],
      hideInBreadcrumb: true,
      title: $t('page.localViews.edgeServer'),
    },
    name: 'EdgeServer',
    path: '/edge/server',
  },
  {
    component: () => import('#/views/local/edge/runtime.vue'),
    meta: {
      authority: ['EDGE'],
      hideInBreadcrumb: true,
      title: '同步批次详情',
    },
    name: 'EdgeServerRecordDetail',
    path: '/edge/server/records/:batchId',
  },
  {
    component: () => import('#/views/local/edge/runtime.vue'),
    meta: {
      authority: ['EDGE'],
      hideInBreadcrumb: true,
      title: '运输盘位',
    },
    name: 'EdgeNasDisks',
    path: '/edge/nas/disks',
  },
  {
    component: () => import('#/views/local/control/runtime.vue'),
    meta: {
      authority: ['CONTROL'],
      hideInBreadcrumb: true,
      title: '入库总览',
    },
    name: 'ControlOverview',
    path: '/control',
  },
  {
    component: () => import('#/views/local/control/runtime.vue'),
    meta: {
      authority: ['CONTROL'],
      hideInBreadcrumb: true,
      title: $t('page.localViews.controlSites'),
    },
    name: 'ControlSites',
    path: '/control/sites',
  },
  {
    component: () => import('#/views/local/control/runtime.vue'),
    meta: {
      authority: ['CONTROL'],
      hideInBreadcrumb: true,
      title: $t('page.localViews.controlSiteDetail'),
    },
    name: 'ControlSiteDetail',
    path: '/control/sites/:siteId',
  },
  {
    component: () => import('#/views/local/control/runtime.vue'),
    meta: {
      authority: ['CONTROL'],
      hideInBreadcrumb: true,
      title: $t('page.localViews.controlCollection'),
    },
    name: 'ControlCollection',
    path: '/control/sites/:siteId/collection',
  },
  {
    component: () => import('#/views/local/control/media-detail.vue'),
    meta: {
      authority: ['CONTROL'],
      hideInBreadcrumb: true,
      title: '运输盘详情',
    },
    name: 'ControlMedia',
    path: '/control/media',
  },
  {
    component: () => import('#/views/local/control/conflicts.vue'),
    meta: {
      authority: ['CONTROL'],
      hideInBreadcrumb: true,
      title: '冲突锁定',
    },
    name: 'ControlConflicts',
    path: '/control/conflicts',
  },
  {
    component: () => import('#/views/local/control/runtime.vue'),
    meta: {
      authority: ['CONTROL'],
      hideInBreadcrumb: true,
      title: '同步记录',
    },
    name: 'ControlHistory',
    path: '/control/history',
  },
  {
    component: () => import('#/views/local/control/runtime.vue'),
    meta: {
      authority: ['CONTROL'],
      hideInBreadcrumb: true,
      title: '中控配置',
    },
    name: 'ControlSettings',
    path: '/control/settings',
  },
  {
    component: () => import('#/views/local/control/factory-admin.vue'),
    meta: {
      authority: ['CONTROL_ADMIN'],
      hideInBreadcrumb: true,
      title: '子工厂管理',
    },
    name: 'ControlFactoryAdmin',
    path: '/control/settings/factories',
  },
  {
    component: () => import('#/views/local/control/factory-add.vue'),
    meta: {
      authority: ['CONTROL_ADMIN'],
      hideInBreadcrumb: true,
      title: '新增子工厂',
    },
    name: 'ControlFactoryAdd',
    path: '/control/settings/factories/new',
  },
  {
    component: () => import('#/views/local/control/factory-registration.vue'),
    meta: {
      authority: ['CONTROL_ADMIN'],
      hideInBreadcrumb: true,
      title: '注册包与连接验证',
    },
    name: 'ControlFactoryRegistration',
    path: '/control/settings/factories/:siteId/registration',
  },
];

export { coreRoutes, fallbackNotFoundRoute };
