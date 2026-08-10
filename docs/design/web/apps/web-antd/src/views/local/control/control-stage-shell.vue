<script setup lang="ts">
import type { ControlServerTab } from './control-server-tabs.vue';

import ProductShell from '../components/product-shell.vue';
import ControlServerTabs from './control-server-tabs.vue';
import { controlAssets } from './ops-fixtures';

withDefaults(
  defineProps<{
    activeTab: ControlServerTab;
    adminEnabled?: boolean;
    baselineKey: string;
    closeLabel: string;
  }>(),
  { adminEnabled: false },
);
</script>

<template>
  <ProductShell
    :close-label="closeLabel"
    close-to="/control"
    display-name="中心 B · 中控"
    hide-navigation
    immersive
    role="CONTROL"
    show-close
  >
    <ControlServerTabs :active="activeTab" :admin-enabled="adminEnabled" />
    <section
      class="control-stage"
      :data-baseline-key="baselineKey"
      data-view-source="frozen-baseline-fixture"
    >
      <img
        :src="controlAssets.environment"
        alt=""
        class="control-stage-environment"
        draggable="false"
      />
      <div class="control-stage-content">
        <slot></slot>
      </div>
      <aside class="control-stage-rack" aria-label="中心 RustFS 归档机柜">
        <img :src="controlAssets.rack" alt="" draggable="false" />
      </aside>
      <p class="screen-reader-only" role="status">
        当前页面使用冻结基线视觉夹具，不代表生产实时数据。
      </p>
    </section>
  </ProductShell>
</template>

<style scoped>
.control-stage {
  position: absolute;
  inset: 0;
  overflow: hidden;
  background:
    radial-gradient(circle at 73% 44%, rgb(26 109 180 / 18%), transparent 31%),
    linear-gradient(135deg, #020407, #07111b 64%, #020507);
}

.control-stage-environment {
  position: absolute;
  inset: 0;
  width: 100%;
  height: 100%;
  pointer-events: none;
  object-fit: cover;
  opacity: 0.42;
  filter: saturate(0.82) brightness(0.7);
}

.control-stage-content {
  position: absolute;
  inset: 58px 0 0;
  z-index: 2;
}

.control-stage-rack {
  position: absolute;
  right: 34px;
  bottom: 40px;
  z-index: 1;
  width: 430px;
  height: 650px;
  pointer-events: none;
}

.control-stage[data-baseline-key='B-07-control-config'] .control-stage-rack {
  top: 145px;
  right: 132px;
  bottom: auto;
  width: 380px;
  height: 567px;
}

.control-stage[data-baseline-key='B-07-control-config']
  .control-stage-rack
  img {
  transform: scaleX(-1);
}

.control-stage-rack::after {
  position: absolute;
  right: -45px;
  bottom: -30px;
  width: 520px;
  height: 180px;
  content: '';
  background: radial-gradient(ellipse, rgb(26 118 197 / 25%), transparent 70%);
  filter: blur(8px);
}

.control-stage-rack img {
  position: relative;
  z-index: 1;
  width: 100%;
  height: 100%;
  object-fit: contain;
  filter: drop-shadow(0 34px 28px rgb(0 0 0 / 66%));
}

@media (prefers-reduced-motion: reduce) {
  .control-stage *,
  .control-stage *::before,
  .control-stage *::after {
    transition-duration: 0.01ms !important;
    animation-duration: 0.01ms !important;
  }
}
</style>
