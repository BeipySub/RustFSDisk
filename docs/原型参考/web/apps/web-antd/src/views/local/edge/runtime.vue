<!--
  @file runtime.vue
  @description A 工厂连续场景编排器，统一加载首页、服务器详情和运输 NAS 详情。
  @route /edge · /edge/server · /edge/server/{records,settings} · /edge/nas
  @scope 保持设备 DOM 连续与渐显转场；粒子调节面板仅在开发环境异步加载。
-->
<script setup lang="ts">
import type { ParticleDebugSettings } from './particle-debug';
import type { EdgeDeviceFocus } from './runtime-panel.vue';

import {
  computed,
  defineAsyncComponent,
  nextTick,
  onBeforeUnmount,
  onMounted,
  ref,
  watch,
} from 'vue';
import { useRoute, useRouter } from 'vue-router';

import {
  getEdgeMediaCandidatesView,
  getEdgeRuntimeView,
  getEdgeServerStatusView,
  getEdgeSyncRecordsView,
  isEdgeRuntimeViewProjection,
  isEdgeSyncRecordsViewProjection,
} from '#/api/local-views';

import ProductRuntimeHeader from '../components/product-runtime-header.vue';
import ProductShell from '../components/product-shell.vue';
import ViewState from '../components/view-state.vue';
import { formatBytes } from '../model';
import { useLocalView } from '../use-local-view';
import { useLocalEventStream } from '../use-local-event-stream';
import NasDisksPanel from './nas-disks-panel.vue';
import { defaultParticleDebugSettings } from './particle-debug';
import RecordsPanel from './records-panel.vue';
import RuntimePanel from './runtime-panel.vue';
import ServerPanel from './server-panel.vue';

const ParticleDebugPanel = import.meta.env.DEV
  ? defineAsyncComponent(() => import('./particle-debug-panel.vue'))
  : null;

const route = useRoute();
const router = useRouter();
const {
  data,
  error,
  loading,
  reload: reloadRuntime,
} = useLocalView(getEdgeRuntimeView, {
  isValidPayload: isEdgeRuntimeViewProjection,
});
const {
  data: nasDisks,
  error: nasDisksError,
  loading: nasDisksLoading,
  reload: reloadNasDisks,
} = useLocalView(getEdgeMediaCandidatesView);
const {
  data: server,
  error: serverError,
  loading: serverLoading,
  reload: reloadServer,
} = useLocalView(getEdgeServerStatusView, { refreshIntervalMs: 5_000 });
const {
  data: records,
  error: recordsError,
  loading: recordsLoading,
  reload: reloadRecords,
} = useLocalView(getEdgeSyncRecordsView, {
  isValidPayload: isEdgeSyncRecordsViewProjection,
});

let eventRefreshTimer: ReturnType<typeof setTimeout> | undefined;
const { status: eventStreamStatus } = useLocalEventStream({
  onEvent(event) {
    // Coalesce bursts from a physical insert/remove and its worker follow-up.
    if (eventRefreshTimer) return;
    eventRefreshTimer = setTimeout(() => {
      eventRefreshTimer = undefined;
      if (event.topic === 'media') {
        void reloadNasDisks({ background: true });
      }
      if (event.topic === 'runtime' || event.topic === 'task') {
        void reloadRuntime({ background: true });
      }
    }, 150);
  },
});
let fallbackRefreshTimer: ReturnType<typeof setInterval> | undefined;

watch(
  eventStreamStatus,
  (status) => {
    if (fallbackRefreshTimer) {
      clearInterval(fallbackRefreshTimer);
      fallbackRefreshTimer = undefined;
    }
    if (status === 'CONNECTED') return;
    // SSE is advisory. A disconnected stream falls back to a deliberately
    // low-frequency snapshot read instead of restoring the former 2s poll.
    fallbackRefreshTimer = setInterval(() => {
      void reloadRuntime({ background: true });
      void reloadNasDisks({ background: true });
    }, 30_000);
  },
  { immediate: true },
);

const requestedFocus = computed<EdgeDeviceFocus | null>(() => {
  if (route.path.startsWith('/edge/server')) return 'server';
  if (route.path.startsWith('/edge/nas')) return 'nas';
  return null;
});
const serverSection = computed(() => {
  if (route.path.startsWith('/edge/server/records/')) return 'detail';
  return 'status';
});

const visualFocus = ref<EdgeDeviceFocus | null>(null);
const mountedFocus = ref<EdgeDeviceFocus | null>(null);
const particleDebug = ref<ParticleDebugSettings>(
  defaultParticleDebugSettings(),
);
const showParticleDebug = computed(
  () =>
    import.meta.env.DEV &&
    route.path === '/edge' &&
    mountedFocus.value === null,
);
let leaveTimer: ReturnType<typeof setTimeout> | undefined;
let enterFrame: number | undefined;

const headerSpeed = computed(() => {
  const speed = data.value?.throughput_bytes_per_second;
  if (speed === null || speed === undefined) {
    return null;
  }

  const [value = '', unit = ''] = formatBytes(speed).split(' ');
  const numericValue = Number(value);
  if (!Number.isFinite(numericValue)) return null;

  return {
    decimals: value.includes('.') ? (value.split('.')[1]?.length ?? 0) : 0,
    unit: `${unit}/s`,
    value: numericValue,
  };
});

const closeLabel = computed(() =>
  mountedFocus.value === 'nas'
    ? '关闭运输 NAS 详情并返回运行首页'
    : '关闭服务器详情并返回运行首页',
);

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

async function showDetail(focus: EdgeDeviceFocus) {
  if (
    mountedFocus.value === focus &&
    (visualFocus.value === focus || enterFrame !== undefined)
  ) {
    return;
  }

  clearTransitionTimers();
  mountedFocus.value = focus;
  visualFocus.value = null;
  await nextTick();
  enterFrame = requestAnimationFrame(() => {
    visualFocus.value = focus;
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

function openDevice(focus: EdgeDeviceFocus) {
  void showDetail(focus);
  const target = focus === 'server' ? '/edge/server' : '/edge/nas/disks';
  if (route.path !== target) void router.push(target);
}

function closeDetail() {
  if (!mountedFocus.value) return;
  hideDetail();
  if (route.path !== '/edge') void router.push('/edge');
}

function handleEscape(event: KeyboardEvent) {
  if (event.key === 'Escape' && mountedFocus.value) {
    closeDetail();
  }
}

watch(
  requestedFocus,
  (nextFocus) => {
    if (nextFocus) {
      void showDetail(nextFocus);
      return;
    }
    hideDetail();
  },
  { immediate: true },
);

onMounted(() => window.addEventListener('keydown', handleEscape));
onBeforeUnmount(() => {
  clearTransitionTimers();
  if (eventRefreshTimer) clearTimeout(eventRefreshTimer);
  if (fallbackRefreshTimer) clearInterval(fallbackRefreshTimer);
  window.removeEventListener('keydown', handleEscape);
});
</script>

<template>
  <ProductShell
    :close-label="closeLabel"
    close-to="/edge"
    :display-name="data?.display_name ?? 'EDGE 本机站点'"
    :hide-navigation="mountedFocus !== null"
    immersive
    role="EDGE"
    :show-close="mountedFocus !== null"
  >
    <template v-if="mountedFocus === null" #header-end>
      <ProductRuntimeHeader
        activity-label="当前装盘吞吐量"
        :decimals="headerSpeed?.decimals"
        label="离线数据装盘"
        :speed="headerSpeed?.value"
        :unit="headerSpeed?.unit"
      />
    </template>

    <ViewState
      v-if="loading"
      kind="loading"
      message="正在读取装盘状态、运输盘分布与唯一行动。"
    />
    <ViewState
      v-else-if="error || !data"
      kind="error"
      :message="error || '未返回运行视图'"
      @retry="reloadRuntime"
    />
    <RuntimePanel
      v-else
      :focus="visualFocus"
      :mounted-focus="mountedFocus"
      :particle-debug="particleDebug"
      :transport-candidates="nasDisks"
      :view="data"
      @open="openDevice"
    >
      <template #detail>
        <ViewState
          v-if="
            mountedFocus === 'server' &&
            serverSection === 'detail' &&
            recordsLoading
          "
          kind="loading"
          message="正在读取本机同步记录与批次闭环状态。"
        />
        <ViewState
          v-else-if="
            mountedFocus === 'server' &&
            serverSection === 'detail' &&
            (recordsError || !records)
          "
          kind="error"
          :message="recordsError || '未返回同步记录视图'"
          @retry="reloadRecords"
        />
        <RecordsPanel
          v-else-if="
            mountedFocus === 'server' && serverSection === 'detail' && records
          "
          :view="records"
        />
        <ViewState
          v-else-if="
            mountedFocus === 'server' && (serverLoading || recordsLoading)
          "
          kind="loading"
          message="正在读取服务器容量、发现状态和安全能力。"
        />
        <ViewState
          v-else-if="
            mountedFocus === 'server' &&
            (serverError || !server || recordsError || !records)
          "
          kind="error"
          :message="serverError || recordsError || '未返回服务器或同步记录视图'"
          @retry="reloadServer"
        />
        <ServerPanel
          v-else-if="mountedFocus === 'server' && server && records"
          embedded
          :records="records"
          :view="server"
        />
        <ViewState
          v-else-if="
            mountedFocus === 'nas' && nasDisksLoading
          "
          kind="loading"
          message="正在读取运输 NAS 盘位、介质身份和健康状态。"
        />
        <ViewState
          v-else-if="
            mountedFocus === 'nas' &&
            (nasDisksError || !nasDisks)
          "
          kind="error"
          :message="nasDisksError || '未返回运输盘位视图'"
          @retry="reloadNasDisks"
        />
        <NasDisksPanel
          v-else-if="
            mountedFocus === 'nas' && nasDisks
          "
          :runtime="data"
          :candidates="nasDisks"
          :event-stream-status="eventStreamStatus"
          :view="nasDisks"
        />
      </template>
    </RuntimePanel>
    <ParticleDebugPanel v-if="showParticleDebug" v-model="particleDebug" />
  </ProductShell>
</template>
