import type { RouteRecordRaw } from 'vue-router';

import { traverseTreeValues } from '@vben/utils';

import { coreRoutes, fallbackNotFoundRoute } from './core';

const routes: RouteRecordRaw[] = [...coreRoutes, fallbackNotFoundRoute];
const coreRouteNames = traverseTreeValues(coreRoutes, (route) => route.name);

// Product routes are introduced only from frozen page contracts.
const accessRoutes: RouteRecordRaw[] = [];

export { accessRoutes, coreRouteNames, routes };
