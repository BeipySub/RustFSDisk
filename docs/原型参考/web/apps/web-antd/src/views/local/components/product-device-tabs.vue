<script setup lang="ts">
import { useRouter } from 'vue-router';

import { TabPane, Tabs } from 'ant-design-vue';

export interface ProductDeviceTab {
  disabled?: boolean;
  key: string;
  label: string;
  path: string;
}

const props = defineProps<{
  active: string;
  items: readonly ProductDeviceTab[];
  tabsLabel: string;
}>();

const router = useRouter();

function changeTab(key: number | string) {
  const item = props.items.find((candidate) => candidate.key === String(key));
  if (!item || item.disabled) return;
  void router.push(item.path);
}
</script>

<template>
  <Tabs
    :active-key="active"
    :animated="false"
    :aria-label="tabsLabel"
    class="device-tabs"
    @change="changeTab"
  >
    <TabPane
      v-for="item in items"
      :key="item.key"
      :disabled="item.disabled"
      :tab="item.label"
    />
  </Tabs>
</template>

<style scoped>
.device-tabs {
  position: absolute;
  top: var(--fd-device-tabs-top, 16px);
  left: var(--fd-device-tabs-left, 535px);
  z-index: 20;
  width: var(--fd-device-tabs-width, 552px);
  height: 50px;
  color: #87909b;
}

.device-tabs :deep(.ant-tabs-nav) {
  height: 50px;
  margin: 0;
}

.device-tabs :deep(.ant-tabs-nav),
.device-tabs :deep(.ant-tabs-nav-wrap),
.device-tabs :deep(.ant-tabs-nav-list),
.device-tabs :deep(.ant-tabs-tab),
.device-tabs :deep(.ant-tabs-tab-btn),
.device-tabs :deep(.ant-tabs-ink-bar) {
  transition: none !important;
  animation: none !important;
}

.device-tabs :deep(.ant-tabs-nav::before) {
  border-color: rgb(116 132 148 / 25%);
}

.device-tabs :deep(.ant-tabs-nav-list) {
  display: grid;
  grid-template-columns: repeat(var(--fd-device-tab-count, 3), minmax(0, 1fr));
  width: 100%;
  transform: none !important;
}

.device-tabs :deep(.ant-tabs-tab) {
  justify-content: center;
  padding: 0;
  margin: 0 !important;
}

.device-tabs :deep(.ant-tabs-tab-btn) {
  font-size: 18px;
  font-weight: 400;
  color: #87909b;
}

.device-tabs :deep(.ant-tabs-tab-active .ant-tabs-tab-btn) {
  color: #e7edf3;
}

.device-tabs :deep(.ant-tabs-tab-disabled .ant-tabs-tab-btn) {
  color: #515b66;
}

.device-tabs :deep(.ant-tabs-ink-bar) {
  height: 2px;
  background: #18baff;
}

.device-tabs :deep(.ant-tabs-content-holder) {
  display: none;
}
</style>
