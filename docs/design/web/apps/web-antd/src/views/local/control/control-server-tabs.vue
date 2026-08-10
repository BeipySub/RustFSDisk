<script setup lang="ts">
import { computed } from 'vue';

import ProductDeviceTabs from '../components/product-device-tabs.vue';

export type ControlServerTab = 'factories' | 'history' | 'settings' | 'sites';

const props = withDefaults(
  defineProps<{
    active: ControlServerTab;
    adminEnabled?: boolean;
  }>(),
  { adminEnabled: false },
);

const items = computed(() => [
  { key: 'history', label: '同步记录', path: '/control/history' },
  { key: 'settings', label: '中控配置', path: '/control/settings' },
  { key: 'sites', label: '子工厂', path: '/control/sites' },
  {
    disabled: !props.adminEnabled,
    key: 'factories',
    label: '子工厂管理',
    path: '/control/settings/factories',
  },
]);
</script>

<template>
  <ProductDeviceTabs
    :active="active"
    class="control-server-tabs"
    :items="items"
    tabs-label="中控服务器页面"
  />
</template>

<style scoped>
.control-server-tabs {
  --fd-device-tab-count: 4;
  --fd-device-tabs-left: 430px;
  --fd-device-tabs-width: 736px;
}
</style>
