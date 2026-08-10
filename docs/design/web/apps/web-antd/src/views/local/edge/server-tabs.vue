<script setup lang="ts">
import ProductDeviceTabs from '../components/product-device-tabs.vue';

type ServerTab = 'settings' | 'status';

defineProps<{ active: ServerTab }>();

const items = [
  { key: 'status', label: '运行状态', path: '/edge/server' },
] as const;
</script>

<template>
  <ProductDeviceTabs
    :active="active"
    class="server-tabs"
    :items="items"
    tabs-label="服务器页面"
  />
</template>

<style scoped>
.server-tabs {
  --fd-device-tabs-top: var(--fd-server-tabs-top, 16px);
  --fd-device-tabs-left: var(--fd-server-content-left, 535px);
}
</style>
