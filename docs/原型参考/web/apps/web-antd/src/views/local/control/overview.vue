<!--
  @file overview.vue
  @description B-01 默认入库总览，使用冻结演示夹具呈现多来源运输盘向中心 RustFS 入库。
  @route 由 CONTROL 入库总览路由装配
  @baseline B-01-ingest-overview · 1672×941
  @scope 仅表达已确认的入库、等待验签和冲突锁定状态；不调用接口，不修改业务状态。
-->
<script setup lang="ts">
import {
  getTransportRecordId,
  type ControlIngestOverviewView,
} from '#/api/local-views';

import { computed, ref, watch } from 'vue';

import { Progress, Tag } from 'ant-design-vue';

import DataFlowField from '../components/data-flow-field.vue';
import ProductRuntimeHeader from '../components/product-runtime-header.vue';
import ProductShell from '../components/product-shell.vue';

interface IngestOverviewFixture {
  archiveStorage: {
    available: string;
    total: string;
    used: string;
  } | null;
  connected: number;
  conflict: {
    count: number;
    site: string;
    slot: string;
  };
  eta: string;
  progress: number;
  runningBatches: number;
  sourceCount: number;
  states: ReadonlyArray<{
    count: number;
    label: string;
    tone: 'danger' | 'running' | 'waiting';
  }>;
  throughput: string;
  total: string;
}

interface SourceDevice {
  label: string;
  stateClass: 'source-device-danger' | 'source-device-running' | 'source-device-waiting';
  status: string;
  to: string;
}

const props = withDefaults(
  defineProps<{
    embedded?: boolean;
    fixture?: IngestOverviewFixture;
    paused?: boolean;
    /** A validated CONTROL local-view projection; absent only for frozen visual review. */
    view?: ControlIngestOverviewView;
  }>(),
  {
    embedded: false,
    fixture: undefined,
    paused: false,
    view: undefined,
  },
);

const rackAsset = '/assets/fustfs-baseline/source-rack-cutout-v3.webp';
const nasAsset = '/assets/fustfs-baseline/transport-nas-cutout-v3.webp';
const baselineFixture = Object.freeze<IngestOverviewFixture>({
  archiveStorage: null,
  connected: 12,
  conflict: {
    count: 1,
    site: 'A-006',
    slot: '11',
  },
  eta: '02:08:31',
  progress: 72,
  runningBatches: 3,
  sourceCount: 6,
  states: [
    { count: 7, label: '入库', tone: 'running' },
    { count: 4, label: '待处理', tone: 'waiting' },
    { count: 1, label: '锁定', tone: 'danger' },
  ],
  throughput: '2.06',
  total: '19.8 / 27.4 TB',
});

const observedRate = ref<number | null>(null);
let previousObservation: { bytes: number; observedAt: number } | undefined;

watch(
  () => props.view?.tasks,
  (tasks) => {
    if (!tasks) {
      previousObservation = undefined;
      observedRate.value = null;
      return;
    }
    const bytes = tasks
      .filter((task) => task.state === 'IMPORTING')
      .reduce((total, task) => total + task.verified_bytes, 0);
    const observedAt = Date.now();
    if (previousObservation && observedAt > previousObservation.observedAt && bytes >= previousObservation.bytes) {
      observedRate.value = (bytes - previousObservation.bytes) / ((observedAt - previousObservation.observedAt) / 1000);
    }
    previousObservation = { bytes, observedAt };
  },
  { deep: true },
);

const fixture = computed<IngestOverviewFixture>(() => {
  if (!props.view) return props.fixture ?? baselineFixture;
  const { summary, tasks } = props.view;
  const active = tasks.filter((task) => task.state === 'IMPORTING');
  const activeBytes = active.reduce((sum, task) => sum + task.logical_bytes, 0);
  const verifiedBytes = active.reduce((sum, task) => sum + task.verified_bytes, 0);
  const conflict = tasks.find((task) => task.state === 'CONFLICT');
  const storage = props.view.storage;
  const archiveStorage =
    storage?.total_bytes !== null &&
    storage?.total_bytes !== undefined &&
    storage.available_bytes !== null &&
    storage.available_bytes !== undefined &&
    storage.available_bytes <= storage.total_bytes
      ? {
          available: formatBytes(storage.available_bytes),
          total: formatBytes(storage.total_bytes),
          used: formatBytes(storage.total_bytes - storage.available_bytes),
        }
      : null;
  return {
    archiveStorage,
    connected: summary.connected_media,
    conflict: {
      count: summary.conflict_locked,
      site: conflict?.source_site_id ?? '—',
      slot: conflict?.media_serial_suffix ?? '—',
    },
    eta: observedRate.value && observedRate.value > 0 ? formatEta(Math.ceil((activeBytes - verifiedBytes) / observedRate.value)) : '暂未上报',
    progress:
      activeBytes > 0 ? Math.round((verifiedBytes / activeBytes) * 100) : 0,
    runningBatches: summary.importing,
    sourceCount: summary.source_sites,
    states: [
      { count: summary.importing, label: '入库', tone: 'running' },
      { count: summary.queued + summary.verified, label: '待处理', tone: 'waiting' },
      { count: summary.conflict_locked + summary.failed, label: '锁定/失败', tone: 'danger' },
    ],
    throughput: observedRate.value === null ? '暂未上报' : `${formatBytes(observedRate.value)}/s`,
    total: `${formatBytes(verifiedBytes)} / ${formatBytes(activeBytes)}`,
  };
});

/**
 * Frozen review uses the three baseline devices.  A live CONTROL projection
 * must never show those illustrative records: it only renders the media the
 * local Agent actually reported.
 */
const sourceDevices = computed<SourceDevice[]>(() => {
  if (!props.view) {
    return [
      { label: 'A-001', stateClass: 'source-device-running', status: '解密入库中', to: '/control/media' },
      { label: 'A-002', stateClass: 'source-device-waiting', status: '等待验签', to: '/control/media' },
      { label: 'A-006', stateClass: 'source-device-danger', status: '冲突锁定', to: '/control/conflicts' },
    ];
  }
  return props.view.tasks.slice(0, 3).map((task) => ({
    label: task.source_site_id,
    stateClass:
      task.state === 'FAILED' || task.state === 'CONFLICT'
        ? 'source-device-danger'
        : task.state === 'IMPORTING'
          ? 'source-device-running'
          : 'source-device-waiting',
    status:
      task.failure_reason ??
      `${task.stage_label} · ${formatBytes(task.verified_bytes)} / ${formatBytes(task.logical_bytes)}`,
    to:
      task.state === 'CONFLICT'
        ? '/control/conflicts'
        : `/control/media?transport_record_id=${encodeURIComponent(getTransportRecordId(task))}`,
  }));
});

function formatBytes(bytes: number) {
  if (bytes === 0) return '0 B';
  const units = ['B', 'KB', 'MB', 'GB', 'TB'];
  const index = Math.min(Math.floor(Math.log(bytes) / Math.log(1024)), units.length - 1);
  return `${(bytes / 1024 ** index).toFixed(index < 3 ? 0 : 1)} ${units[index]}`;
}
function formatEta(seconds: number) {
  if (!Number.isFinite(seconds) || seconds < 0) return '暂未上报';
  const minutes = Math.floor(seconds / 60);
  return minutes > 59 ? `${Math.floor(minutes / 60)}小时${minutes % 60}分钟` : `${minutes}分钟`;
}
const ingestFlowPath = Object.freeze({
  control1: { x: 0.362, y: 0.279 },
  control2: { x: 0.506, y: 0.53 },
  end: { x: 0.704, y: 0.47 },
  spread: 0.068,
  start: { x: 0.215, y: 0.304 },
});
</script>

<template>
  <component
    :is="embedded ? 'div' : ProductShell"
    :class="embedded ? 'control-overview-embedded' : 'control-overview-shell'"
    v-bind="
      embedded
        ? {}
        : {
            displayName: '中心 B · 中控',
            immersive: true,
            role: 'CONTROL',
          }
    "
  >
    <template v-if="!embedded" #header-end>
      <ProductRuntimeHeader
        activity-label="中心当前入库吞吐量"
        :decimals="2"
        label="中心数据入库"
        :speed="view ? undefined : Number(fixture.throughput)"
        unit="GB/s"
      />
    </template>

    <section
      aria-labelledby="control-overview-title"
      class="control-overview"
      data-baseline-key="B-01-ingest-overview"
      :data-view-source="view ? 'local-control-api' : 'frozen-baseline-fixture'"
    >
      <h1 id="control-overview-title" class="screen-reader-only">
        默认入库总览
      </h1>
      <p class="screen-reader-only" role="status">
        {{ view ? '本页展示 B 中控本机只读导入投影；后端不可用时会失败关闭。' : '本页为 B-01 冻结基线视觉夹具，不代表生产实时数据。' }}
      </p>

      <section
        aria-label="三台来源运输 NAS 向中心 RustFS 入库；第一台解密入库中，第二台等待验签，第三台冲突锁定"
        class="ingest-stage"
        role="img"
      >
        <DataFlowField
          :path="ingestFlowPath"
          :paused="paused"
          state="RUNNING"
        />
        <svg
          aria-hidden="true"
          class="flow-field"
          preserveAspectRatio="none"
          viewBox="0 0 1672 596"
        >
          <defs>
            <linearGradient id="overview-idle-flow" x1="0" x2="1">
              <stop offset="0" stop-color="#80cfff" stop-opacity=".24" />
              <stop offset="1" stop-color="#66bdf2" stop-opacity=".12" />
            </linearGradient>
          </defs>

          <path class="waiting-line" d="M257 349 C535 349 866 351 1160 349" />
          <path class="waiting-points" d="M257 349 C535 349 866 351 1160 349" />
          <path class="danger-line" d="M395 485 H504" />
        </svg>

        <RouterLink
          v-for="device in sourceDevices"
          :key="`${device.label}-${device.status}`"
          :aria-label="`打开 ${device.label} 运输盘详情`"
          class="source-device"
          :class="device.stateClass"
          :to="device.to"
        >
          <img :src="nasAsset" alt="" />
          <p><strong>{{ device.label }}</strong> · {{ device.status }}</p>
          <span v-if="device.stateClass === 'source-device-danger'" class="lock-mark" aria-hidden="true">
            <i></i>
          </span>
        </RouterLink>

        <RouterLink
          v-if="!embedded"
          aria-label="打开中控服务器同步记录"
          class="center-rack"
          to="/control/history"
        >
          <img :src="rackAsset" alt="" />
        </RouterLink>
      </section>

      <section class="overview-dashboard" aria-label="入库指标">
        <article class="source-summary" aria-labelledby="source-summary-title">
          <h2 id="source-summary-title">接入与来源</h2>
          <dl class="summary-totals">
            <div>
              <dt>已接入</dt>
              <dd>{{ fixture.connected }}</dd>
            </div>
            <div>
              <dt>来源工厂</dt>
              <dd>{{ fixture.sourceCount }}</dd>
            </div>
          </dl>
          <div class="state-tags" aria-label="运输盘状态分布">
            <template
              v-for="(state, index) in fixture.states"
              :key="state.label"
            >
              <Tag
                :bordered="false"
                class="state-tag"
                :class="`state-${state.tone}`"
              >
                {{ state.label }} {{ state.count }}
              </Tag>
              <span
                v-if="index < fixture.states.length - 1"
                aria-hidden="true"
                class="state-separator"
              >
                ·
              </span>
            </template>
          </div>
        </article>

        <article class="current-ingest" aria-labelledby="current-ingest-title">
          <h2 id="current-ingest-title">当前入库</h2>
          <strong>{{ fixture.runningBatches }} 个批次运行</strong>
          <div class="ingest-progress-heading">
            <span>已完成 {{ fixture.total }}</span>
            <b>{{ fixture.progress }}%</b>
          </div>
          <Progress
            :percent="fixture.progress"
            :show-info="false"
            :stroke-color="{ '0%': '#087de0', '100%': '#3bc9ff' }"
            stroke-linecap="round"
            :stroke-width="8"
            trail-color="#2b3137"
          />
          <dl class="ingest-facts">
            <div>
              <dt>预计剩余</dt>
              <dd>{{ fixture.eta }}</dd>
            </div>
            <div>
              <dt>实时</dt>
              <dd>{{ view ? `${fixture.throughput}（浏览器观测）` : `${fixture.throughput} GB/s` }}</dd>
            </div>
          </dl>
          <dl v-if="view" class="archive-storage" aria-label="B 中控归档存储容量">
            <div>
              <dt>B 归档存储</dt>
              <dd v-if="fixture.archiveStorage">
                已用 {{ fixture.archiveStorage.used }} / {{ fixture.archiveStorage.total }}
              </dd>
              <dd v-else>暂未上报</dd>
            </div>
            <div>
              <dt>可用空间</dt>
              <dd>{{ fixture.archiveStorage?.available ?? '暂未上报' }}</dd>
            </div>
          </dl>
        </article>

        <article class="conflict-summary" aria-labelledby="conflict-title">
          <h2 id="conflict-title">冲突与异常</h2>
          <dl>
            <div>
              <dt>目标冲突</dt>
              <dd>{{ fixture.conflict.count }}</dd>
            </div>
            <div>
              <dt class="screen-reader-only">锁定来源和盘位</dt>
              <dd>
                {{ fixture.conflict.site }} · 盘位 {{ fixture.conflict.slot }}
              </dd>
            </div>
          </dl>
          <p class="conflict-action">
            保持锁定并查看冲突 <span aria-hidden="true">→</span>
          </p>
          <p>其他对象继续入库</p>
        </article>
      </section>
    </section>
  </component>
</template>

<style scoped>
.control-overview {
  display: grid;
  grid-template-rows: 596px 273px;
  width: 100%;
  height: 100%;
  overflow: hidden;
  color: #c6ccd3;
  background: #030609;
}

.control-overview-embedded {
  position: absolute;
  inset: 0;
}

.ingest-stage {
  position: relative;
  min-height: 0;
  overflow: hidden;
  background:
    linear-gradient(180deg, rgb(0 2 5 / 22%), rgb(1 6 11 / 10%)),
    url('/assets/fustfs-baseline/factory-environment-v4.webp') center / 100%
      100% no-repeat,
    #02070c;
}

.ingest-stage::before {
  position: absolute;
  inset: 0;
  z-index: 1;
  pointer-events: none;
  content: '';
  background:
    radial-gradient(ellipse at 78% 83%, rgb(42 144 229 / 17%), transparent 22%),
    radial-gradient(ellipse at 18% 65%, rgb(26 110 185 / 8%), transparent 25%),
    linear-gradient(
      90deg,
      rgb(0 3 7 / 26%),
      transparent 18%,
      transparent 82%,
      rgb(0 3 7 / 30%)
    );
}

.flow-field {
  position: absolute;
  inset: 0;
  z-index: 4;
  width: 100%;
  height: 100%;
  overflow: visible;
  pointer-events: none;
  mix-blend-mode: screen;
}

.waiting-line {
  filter: drop-shadow(0 0 5px rgb(85 186 245 / 26%));
  fill: none;
  stroke: url('#overview-idle-flow');
  stroke-width: 3;
}

.waiting-points {
  fill: none;
  stroke: rgb(169 220 251 / 62%);
  stroke-width: 6;
  stroke-linecap: round;
  stroke-dasharray: 1 53;
}

.danger-line {
  filter: drop-shadow(0 0 4px rgb(255 61 78 / 54%));
  fill: none;
  stroke: #ff5261;
  stroke-width: 5;
  stroke-linecap: round;
  stroke-dasharray: 1 16;
}

.source-device {
  position: absolute;
  z-index: 6;
  color: inherit;
  text-decoration: none;
}

.source-device:focus-visible,
.center-rack:focus-visible {
  outline: 2px solid #58dcff;
  outline-offset: 5px;
}

.source-device img {
  position: absolute;
  object-fit: fill;
  filter: drop-shadow(0 22px 20px rgb(0 0 0 / 66%));
  transform: scaleX(-1);
}

.source-device p {
  position: absolute;
  z-index: 2;
  margin: 0;
  font-size: 18px;
  line-height: 1;
  white-space: nowrap;
}

.source-device p strong {
  font-weight: 400;
}

.source-device-running {
  inset: 85px auto auto 167px;
  width: 500px;
  height: 172px;
}

.source-device-running img {
  inset: 0 auto auto 0;
  width: 194px;
  height: 160px;
}

.source-device-running p {
  top: 46px;
  left: 238px;
  color: #48d1d4;
}

.source-device-waiting {
  inset: 264px auto auto 76px;
  width: 485px;
  height: 169px;
}

.source-device-waiting img {
  inset: 0 auto auto 0;
  width: 181px;
  height: 145px;
}

.source-device-waiting p {
  top: 38px;
  left: 206px;
  color: #ffb934;
}

.source-device-danger {
  inset: 399px auto auto 208px;
  width: 480px;
  height: 154px;
}

.source-device-danger img {
  inset: 0 auto auto 0;
  width: 190px;
  height: 141px;
  filter: saturate(0.88) drop-shadow(0 22px 20px rgb(0 0 0 / 70%));
}

.source-device-danger p {
  top: 35px;
  left: 207px;
  color: #ff5761;
}

.lock-mark {
  position: absolute;
  top: 72px;
  left: 278px;
  display: grid;
  place-items: center;
  width: 35px;
  height: 35px;
  border: 1px solid #ff4d5b;
  border-radius: 50%;
  box-shadow:
    inset 0 0 10px rgb(255 49 68 / 12%),
    0 0 8px rgb(255 49 68 / 20%);
}

.lock-mark i {
  position: relative;
  width: 13px;
  height: 11px;
  background: #ff4d5b;
  border-radius: 2px;
}

.lock-mark i::before {
  position: absolute;
  top: -7px;
  left: 3px;
  width: 7px;
  height: 9px;
  content: '';
  border: 2px solid #ff4d5b;
  border-bottom: 0;
  border-radius: 7px 7px 0 0;
}

.ingest-stage :deep(.data-flow-canvas) {
  z-index: 4;
}

.center-rack {
  position: absolute;
  top: 40px;
  right: 162px;
  z-index: 5;
  display: block;
  width: 350px;
  height: 518px;
}

.center-rack img {
  width: 100%;
  height: 100%;
  object-fit: fill;
  filter: brightness(0.76) drop-shadow(0 28px 28px rgb(0 0 0 / 70%));
  transform: scaleX(-1);
}

.overview-dashboard {
  display: grid;
  grid-template-columns: 36.55% 32.75% 30.7%;
  min-height: 0;
  background:
    linear-gradient(90deg, rgb(6 13 20 / 96%), rgb(5 11 16 / 97%)), #050b10;
  border-top: 1px solid rgb(111 132 152 / 28%);
}

.overview-dashboard > article {
  position: relative;
  min-width: 0;
  padding: 25px 45px 20px 54px;
}

.overview-dashboard > article + article {
  border-left: 1px solid rgb(111 132 152 / 28%);
}

.overview-dashboard h2 {
  margin: 0;
  font-size: 20px;
  font-weight: 600;
  line-height: 1.35;
  color: #e2e6ea;
  letter-spacing: 0.04em;
}

.summary-totals {
  display: grid;
  grid-template-columns: 183px 1fr;
  margin: 25px 0 0;
}

.summary-totals > div {
  min-width: 0;
}

.summary-totals > div + div {
  padding-left: 53px;
  border-left: 1px solid rgb(112 130 149 / 31%);
}

.summary-totals dt {
  font-size: 16px;
  color: #969fa9;
}

.summary-totals dd {
  margin: 5px 0 0;
  font-size: 43px;
  font-weight: 400;
  line-height: 1;
  color: #119cf1;
}

.state-tags {
  display: flex;
  gap: 17px;
  align-items: center;
  margin-top: 28px;
}

.state-tag {
  display: inline-flex;
  gap: 10px;
  align-items: center;
  padding: 0;
  margin: 0;
  font-size: 17px;
  line-height: 1;
  color: #89939e;
  background: transparent;
  border: 0;
}

.state-tag::before {
  width: 10px;
  height: 10px;
  content: '';
  background: #19aff6;
  border-radius: 50%;
  box-shadow: 0 0 10px rgb(25 175 246 / 30%);
}

.state-tag.state-waiting::before {
  background: #ffb300;
  box-shadow: 0 0 10px rgb(255 179 0 / 25%);
}

.state-tag.state-danger::before {
  background: #f64d5c;
  box-shadow: 0 0 10px rgb(246 77 92 / 25%);
}

.state-separator {
  color: #68717b;
}

.current-ingest {
  padding-right: 47px !important;
  padding-left: 46px !important;
}

.current-ingest > strong {
  display: block;
  margin-top: 12px;
  font-size: 32px;
  font-weight: 500;
  line-height: 1.25;
  color: #078eff;
  letter-spacing: 0.03em;
}

.ingest-progress-heading {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-top: 9px;
  font-size: 17px;
  color: #8d959e;
}

.ingest-progress-heading b {
  font-weight: 400;
}

.current-ingest :deep(.ant-progress) {
  display: block;
  margin-top: 10px;
  line-height: 1;
}

.current-ingest :deep(.ant-progress-outer) {
  display: block;
  padding: 0;
  margin: 0;
}

.current-ingest :deep(.ant-progress-inner) {
  vertical-align: top;
  border-radius: 99px;
}

.current-ingest :deep(.ant-progress-bg) {
  box-shadow: 0 0 10px rgb(20 154 255 / 45%);
}

.ingest-facts {
  display: grid;
  grid-template-columns: 1fr 1fr;
  margin: 27px 0 0;
}

.ingest-facts > div {
  display: flex;
  gap: 15px;
  align-items: baseline;
}

.ingest-facts > div + div {
  justify-content: flex-end;
  padding-left: 42px;
  border-left: 1px solid rgb(112 130 149 / 31%);
}

.ingest-facts dt {
  font-size: 16px;
  color: #858e98;
}

.ingest-facts dd {
  margin: 0;
  font-size: 19px;
  color: #a9b1b9;
}

.ingest-facts > div:last-child dd {
  color: #15afff;
}

.archive-storage {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 15px;
  padding-top: 17px;
  margin: 17px 0 0;
  border-top: 1px solid rgb(112 130 149 / 31%);
}

.archive-storage > div {
  display: grid;
  gap: 6px;
}

.archive-storage > div + div {
  padding-left: 15px;
  border-left: 1px solid rgb(112 130 149 / 31%);
}

.archive-storage dt {
  font-size: 15px;
  color: #858e98;
}

.archive-storage dd {
  margin: 0;
  font-size: 17px;
  color: #a9b1b9;
}

.conflict-summary {
  padding-left: 41px !important;
}

.conflict-summary dl {
  display: grid;
  grid-template-columns: 150px 1fr;
  margin: 24px 0 0;
}

.conflict-summary dl > div:first-child dt {
  font-size: 16px;
  color: #969fa9;
}

.conflict-summary dl > div:first-child dd {
  margin: 6px 0 0;
  font-size: 43px;
  line-height: 1;
  color: #f04b5a;
}

.conflict-summary dl > div:last-child {
  align-self: end;
}

.conflict-summary dl > div:last-child dd {
  margin: 0 0 2px;
  font-size: 24px;
  color: #858c94;
  white-space: nowrap;
}

.conflict-action {
  margin: 17px 0 0;
  font-size: 19px;
  color: #ff4c59;
}

.conflict-action span {
  margin-left: 8px;
}

.conflict-summary > p:last-child {
  margin: 15px 0 0;
  font-size: 16px;
  color: #7e8791;
}

@media (prefers-reduced-motion: reduce) {
  .control-overview *,
  .control-overview *::before,
  .control-overview *::after {
    transition-duration: 0.01ms !important;
    animation-duration: 0.01ms !important;
    animation-iteration-count: 1 !important;
  }
}

:global(.control-overview-shell) {
  background: #020508;
}

:global(.control-overview-shell::before) {
  display: none;
}
</style>
