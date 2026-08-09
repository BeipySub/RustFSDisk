<!--
  @file runtime.vue
  @description B 中控连续场景编排器，保持首页中心机柜 DOM，并在同步记录展开时移动到右侧聚焦位。
  @route /control · /control/history
  @baseline B-01-ingest-overview · B-06-records · 1672×941
-->
<script setup lang="ts">
import {
  computed,
  nextTick,
  onBeforeUnmount,
  onMounted,
  ref,
  watch,
} from 'vue';
import { useRoute, useRouter } from 'vue-router';

import {
  getControlIngestOverviewView,
  getControlIngestRecordsView,
  isControlIngestOverviewViewProjection,
} from '#/api/local-views';

import ProductRuntimeHeader from '../components/product-runtime-header.vue';
import ProductShell from '../components/product-shell.vue';
import ViewState from '../components/view-state.vue';
import { useLocalView } from '../use-local-view';
import ControlServerTabs from './control-server-tabs.vue';
import ExperiencePanel from './experience.vue';
import HistoryPanel from './history.vue';
import OverviewPanel from './overview.vue';
import SettingsPanel from './settings.vue';

type ControlDeviceFocus = 'server';
type ControlServerSection = 'history' | 'settings' | 'sites';

const route = useRoute();
const router = useRouter();
const {
  data: ingestOverview,
  error: ingestOverviewError,
  loading: ingestOverviewLoading,
  reload: reloadIngestOverview,
} = useLocalView(getControlIngestOverviewView, {
  isValidPayload: isControlIngestOverviewViewProjection,
  refreshIntervalMs: 2_000,
});
const {
  data: ingestRecords,
  error: ingestRecordsError,
  loading: ingestRecordsLoading,
  reload: reloadIngestRecords,
} = useLocalView(getControlIngestRecordsView, {
  isValidPayload: isControlIngestOverviewViewProjection,
  refreshIntervalMs: 5_000,
});
const requestedFocus = computed<ControlDeviceFocus | null>(() =>
  route.path.startsWith('/control/history') ||
  route.path.startsWith('/control/settings') ||
  route.path.startsWith('/control/sites')
    ? 'server'
    : null,
);
const serverSection = computed<ControlServerSection>(() => {
  if (route.path.startsWith('/control/settings')) return 'settings';
  if (route.path.startsWith('/control/sites')) return 'sites';
  return 'history';
});
const visualFocus = ref<ControlDeviceFocus | null>(null);
const mountedFocus = ref<ControlDeviceFocus | null>(null);
const rackAsset = '/assets/fustfs-baseline/source-rack-cutout-v3.webp';

let leaveTimer: ReturnType<typeof setTimeout> | undefined;
let enterFrame: number | undefined;

function clearTransitionTimers() {
  if (leaveTimer) {
    clearTimeout(leaveTimer);
    leaveTimer = undefined;
  }
  if (enterFrame !== undefined) {
    cancelAnimationFrame(enterFrame);
    enterFrame = undefined;
  }
}

async function showDetail() {
  if (
    mountedFocus.value === 'server' &&
    (visualFocus.value === 'server' || enterFrame !== undefined)
  ) {
    return;
  }

  clearTransitionTimers();
  mountedFocus.value = 'server';
  visualFocus.value = null;
  await nextTick();
  enterFrame = requestAnimationFrame(() => {
    visualFocus.value = 'server';
    enterFrame = undefined;
  });
}

function hideDetail() {
  if (
    mountedFocus.value === null ||
    (visualFocus.value === null && leaveTimer !== undefined)
  ) {
    return;
  }

  if (enterFrame !== undefined) {
    cancelAnimationFrame(enterFrame);
    enterFrame = undefined;
  }
  visualFocus.value = null;
  leaveTimer = setTimeout(() => {
    mountedFocus.value = null;
    leaveTimer = undefined;
  }, 320);
}

function openServer() {
  void showDetail();
  if (route.path !== '/control/history') {
    void router.push('/control/history');
  }
}

function closeDetail() {
  if (mountedFocus.value === null) return;
  hideDetail();
  if (route.path !== '/control') {
    void router.push('/control');
  }
}

function handleEscape(event: KeyboardEvent) {
  if (event.key === 'Escape' && mountedFocus.value !== null) {
    closeDetail();
  }
}

watch(
  requestedFocus,
  (nextFocus) => {
    if (nextFocus === 'server') {
      void showDetail();
      return;
    }
    hideDetail();
  },
  { immediate: true },
);

onMounted(() => window.addEventListener('keydown', handleEscape));
onBeforeUnmount(() => {
  clearTransitionTimers();
  window.removeEventListener('keydown', handleEscape);
});
</script>

<template>
  <ProductShell
    close-label="关闭中控服务器并返回入库总览"
    close-to="/control"
    display-name="中心 B · 中控"
    :hide-navigation="mountedFocus !== null"
    immersive
    role="CONTROL"
    :show-close="mountedFocus !== null"
  >
    <template v-if="mountedFocus === null" #header-end>
      <ProductRuntimeHeader
        activity-label="中心当前入库吞吐量"
        :decimals="2"
        label="中心数据入库"
        :speed="2.06"
        unit="GB/s"
      />
    </template>

    <ControlServerTabs v-if="mountedFocus !== null" :active="serverSection" />

    <section
      class="control-runtime"
      :class="{
        'focus-server': visualFocus === 'server',
        'has-device-focus': mountedFocus !== null,
        'is-detail-leaving': mountedFocus !== null && visualFocus === null,
        'section-history': serverSection === 'history',
        'section-settings': serverSection === 'settings',
        'section-sites': serverSection === 'sites',
      }"
      :data-device-focus="visualFocus ?? 'home'"
      data-runtime-owner="control"
    >
      <div class="control-runtime-home">
        <ViewState
          v-if="ingestOverviewLoading"
          kind="loading"
          message="正在读取中控导入任务、运输盘和对账状态。"
        />
        <ViewState
          v-else-if="ingestOverviewError || !ingestOverview"
          kind="error"
          :message="ingestOverviewError || '未返回中控导入状态视图；不会展示冻结评审数据。'"
          @retry="reloadIngestOverview"
        />
        <OverviewPanel
          v-else
          embedded
          :paused="mountedFocus !== null"
          :view="ingestOverview"
        />
      </div>

      <button
        :aria-label="
          mountedFocus === null
            ? '打开中控服务器同步记录'
            : '当前中控服务器同步记录'
        "
        class="control-runtime-rack"
        data-device-id="control-center-rack"
        type="button"
        @click="openServer"
      >
        <img :src="rackAsset" alt="" draggable="false" />
      </button>

      <section
        v-if="mountedFocus === 'server'"
        :aria-label="
          serverSection === 'history'
            ? '中控服务器同步记录'
            : serverSection === 'settings'
              ? '中控服务器配置'
              : '中控服务器子工厂'
        "
        aria-live="polite"
        class="control-detail-layer"
      >
        <ViewState
          v-if="serverSection === 'history' && ingestRecordsLoading"
          kind="loading"
          message="正在读取中控导入记录与完成回执状态。"
        />
        <ViewState
          v-else-if="serverSection === 'history' && (ingestRecordsError || !ingestRecords)"
          kind="error"
          :message="ingestRecordsError || '未返回中控导入记录视图；不会展示冻结评审数据。'"
          @retry="reloadIngestRecords"
        />
        <HistoryPanel
          v-else-if="serverSection === 'history'"
          embedded
          :view="ingestRecords"
        />
        <SettingsPanel v-else-if="serverSection === 'settings'" embedded />
        <ExperiencePanel v-else embedded />
      </section>
    </section>
  </ProductShell>
</template>

<style scoped>
.control-runtime {
  position: relative;
  width: 100%;
  height: 100%;
  min-height: 0;
  overflow: hidden;
  background: #020508;
}

.control-runtime-home {
  position: absolute;
  inset: 0;
  z-index: 1;
  pointer-events: auto;
  opacity: 1;
  transform: translateX(0);
  transition:
    opacity 320ms ease,
    transform 420ms cubic-bezier(0.22, 1, 0.36, 1);
}

.has-device-focus .control-runtime-home {
  pointer-events: none;
  opacity: 0;
  transform: translateX(-28px);
}

.control-runtime-rack {
  position: absolute;
  top: 40px;
  right: 162px;
  z-index: 16;
  display: block;
  width: 350px;
  height: 518px;
  padding: 0;
  cursor: pointer;
  background: transparent;
  border: 0;
  border-radius: 8px;
  filter: brightness(0.76);
  transition:
    top 420ms cubic-bezier(0.22, 1, 0.36, 1),
    right 420ms cubic-bezier(0.22, 1, 0.36, 1),
    width 420ms cubic-bezier(0.22, 1, 0.36, 1),
    height 420ms cubic-bezier(0.22, 1, 0.36, 1),
    filter 220ms ease;
  will-change: top, right, width, height;
}

.control-runtime-rack::after {
  position: absolute;
  right: -32px;
  bottom: -22px;
  left: -24px;
  z-index: 0;
  height: 98px;
  pointer-events: none;
  content: '';
  background: radial-gradient(ellipse, rgb(26 118 197 / 23%), transparent 70%);
  filter: blur(8px);
}

.control-runtime-rack img {
  position: relative;
  z-index: 1;
  display: block;
  width: 100%;
  height: 100%;
  object-fit: fill;
  filter: drop-shadow(0 28px 28px rgb(0 0 0 / 70%));
  transform: scaleX(-1);
}

.control-runtime-rack:focus-visible {
  outline: 2px solid #58dcff;
  outline-offset: 5px;
}

.has-device-focus .control-runtime-rack {
  pointer-events: none;
  cursor: default;
}

.focus-server .control-runtime-rack {
  top: 282px;
  right: 58px;
  width: 260px;
  height: 410px;
  cursor: default;
  filter: brightness(0.9);
}

.focus-server.section-settings .control-runtime-rack {
  top: 137px;
  right: 141px;
  width: 355px;
  height: 570px;
}

.is-detail-leaving .control-runtime-rack {
  transition-duration: 320ms;
}

.control-detail-layer {
  position: absolute;
  inset: 0;
  z-index: 12;
  pointer-events: none;
  opacity: 0;
  transform: translateX(42px);
  transition:
    opacity 320ms ease,
    transform 420ms cubic-bezier(0.22, 1, 0.36, 1);
  will-change: opacity, transform;
}

.control-detail-layer > * {
  pointer-events: auto;
}

.focus-server .control-detail-layer {
  opacity: 1;
  transform: translateX(0);
}

.is-detail-leaving .control-detail-layer {
  pointer-events: none;
  opacity: 0;
  transition-duration: 320ms;
}

@media (prefers-reduced-motion: reduce) {
  .control-runtime-home,
  .control-runtime-rack,
  .control-detail-layer {
    transition-delay: 0ms !important;
    transition-duration: 0.01ms !important;
  }
}
</style>
