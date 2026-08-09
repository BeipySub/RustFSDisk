<!--
  @file nas-panel.vue
  @description A-06 运输 NAS 运行状态面板，展示实际接入盘位、任务摘要和设备状态。
  @usage 嵌入 runtime.vue 的运输设备聚焦场景。
  @baseline A-06-transport-status · 1672×941
-->
<script setup lang="ts">
import type { EchartsUIType } from '@vben/plugins/echarts';

import type { EdgeRuntimeView } from '#/api/local-views';

import { computed, onMounted, ref, watch } from 'vue';
import { useRouter } from 'vue-router';

import { VbenCountToAnimator } from '@vben/common-ui';
import { EchartsUI, useEcharts } from '@vben/plugins/echarts';

import { usePreferredReducedMotion } from '@vueuse/core';
import { Badge, Progress, TabPane, Tabs } from 'ant-design-vue';

import { formatBytes, formatEta } from '../model';

const props = defineProps<{ view: EdgeRuntimeView }>();
const router = useRouter();
const trendChartRef = ref<EchartsUIType>();
const { renderEcharts } = useEcharts(trendChartRef);
const preferredReducedMotion = usePreferredReducedMotion();

function handleTabChange(key: number | string) {
  if (key === 'disks') void router.push('/edge/nas/disks');
}

const speedDisplay = computed(() => {
  if (props.view.throughput_bytes_per_second === null) return null;
  const [value = '', unit = ''] = formatBytes(
    props.view.throughput_bytes_per_second,
  ).split(' ');
  const numericValue = Number(value);
  return {
    decimals: value.includes('.') ? (value.split('.')[1]?.length ?? 0) : 0,
    label: `${value} ${unit}/s`,
    unit: `${unit}/s`,
    value: Number.isFinite(numericValue) ? numericValue : 0,
  };
});

const countAnimationDuration = computed(() =>
  preferredReducedMotion.value === 'reduce' ? 0 : 1200,
);

const taskFactIcons = {
  confidence: '/assets/fustfs-baseline/icons/task-confidence-shield.svg',
  confirmed: '/assets/fustfs-baseline/icons/task-confirmed-database.svg',
  eta: '/assets/fustfs-baseline/icons/task-eta-clock.svg',
} as const;

function confidenceLabel(value: null | string) {
  const labels: Record<string, string> = {
    HIGH: '高',
    LOW: '低',
    MEDIUM: '中',
  };
  return value ? (labels[value] ?? value) : '未知';
}

function formatBytePair(confirmed: number, total: null | number) {
  const confirmedText = formatBytes(confirmed);
  if (total === null) return `${confirmedText} / 总量未知`;

  const totalText = formatBytes(total);
  const confirmedParts = confirmedText.split(' ');
  const totalParts = totalText.split(' ');
  return confirmedParts[1] === totalParts[1]
    ? `${confirmedParts[0]} / ${totalText}`
    : `${confirmedText} / ${totalText}`;
}

const visibleSlots = computed(() => {
  const groups: Array<
    ['completed' | 'failed' | 'running' | 'standby', number]
  > = [
    ['running', props.view.media.running],
    ['standby', props.view.media.standby],
    ['completed', props.view.media.completed],
    ['failed', props.view.media.failed],
  ];
  return groups
    .flatMap(([state, count]) => Array.from({ length: count }, () => state))
    .slice(0, props.view.media.connected);
});

const slotMeterStyle = computed(() => {
  const slotCount = visibleSlots.value.length;
  const slotWidth = Math.min(30, Math.max(16, 36 - slotCount));
  return {
    '--nas-slot-count': slotCount,
    '--nas-slot-width': `${slotWidth}px`,
  };
});

function renderTrendChart() {
  const throughput = props.view.throughput_bytes_per_second;
  const currentGigabytes = (throughput ?? 0) / 1024 ** 3;
  const samples: Array<null | number> = Array.from({ length: 31 }, () => null);
  if (throughput !== null) samples[30] = currentGigabytes;
  const maximum = Math.max(2.5, Math.ceil(currentGigabytes * 1.25 * 10) / 10);

  renderEcharts(
    {
      animation: false,
      aria: {
        description: '运输 NAS 当前瞬时吞吐量；没有历史采样时不补绘曲线',
        enabled: true,
      },
      grid: {
        bottom: 28,
        left: 84,
        right: 0,
        top: 12,
      },
      series: [
        {
          connectNulls: false,
          data: samples,
          itemStyle: { color: '#23b9ff' },
          label: {
            color: '#c9d4df',
            formatter: speedDisplay.value?.label ?? '',
            fontSize: 12,
            position: 'left',
            show: false,
          },
          lineStyle: {
            color: '#23b9ff',
            width: 2,
          },
          showSymbol: true,
          symbol: 'circle',
          symbolSize: 8,
          type: 'line',
        },
      ],
      tooltip: { show: false },
      xAxis: {
        axisLabel: {
          color: '#7e8995',
          fontSize: 13,
          formatter: (_value: string, index: number) => {
            if (index === 0) return '30 秒前';
            if (index === 15) return '15 秒前';
            if (index === 30) return '现在';
            return '';
          },
          interval: 0,
          margin: 8,
        },
        axisLine: { lineStyle: { color: '#20303e' } },
        axisTick: { show: false },
        boundaryGap: false,
        data: samples.map((_sample, index) => index),
        splitLine: {
          interval: (index: number) =>
            index === 0 || index === 15 || index === 30,
          lineStyle: { color: 'rgba(80, 112, 142, 0.08)' },
          show: true,
        },
        type: 'category',
      },
      yAxis: {
        axisLabel: {
          color: '#7e8995',
          fontSize: 13,
          formatter: '{value}',
          margin: 12,
        },
        axisLine: { show: false },
        axisTick: { show: false },
        max: maximum,
        min: 0,
        splitLine: {
          lineStyle: { color: 'rgba(80, 112, 142, 0.08)' },
          show: true,
        },
        type: 'value',
      },
    },
    true,
  );
}

onMounted(renderTrendChart);
watch(() => props.view.throughput_bytes_per_second, renderTrendChart);
</script>

<template>
  <section aria-labelledby="nas-title" class="nas-baseline">
    <h1 id="nas-title" class="screen-reader-only">运输 NAS 运行状态</h1>

    <Tabs
      active-key="status"
      :animated="false"
      aria-label="运输 NAS 页面"
      class="nas-tabs"
      @change="handleTabChange"
    >
      <TabPane key="status" tab="运行状态" />
      <TabPane key="disks" tab="硬盘" />
    </Tabs>

    <section class="nas-overview" aria-label="运输 NAS 运行概览">
      <article class="array-state">
        <h2><i aria-hidden="true"></i>{{ view.state_label }}</h2>
        <p>
          {{ view.media.connected }} 块已接入 · {{ view.media.running }} 块写入
          ·
          <b>{{ view.media.warning }} 块注意</b>
        </p>
        <dl>
          <div>
            <dt>容量</dt>
            <dd>尚无权威数据</dd>
          </div>
          <div>
            <dt>数据时间</dt>
            <dd>{{ view.meta.status_message }}</dd>
          </div>
        </dl>
      </article>

      <article class="write-state">
        <h2>当前写入</h2>
        <strong>{{ view.media.running }}<small> 块</small></strong>
        <hr />
        <h2>实时速度</h2>
        <div
          class="runtime-header-activity nas-speed-indicator"
          aria-label="运输 NAS 当前写入速度"
        >
          <i aria-hidden="true" data-runtime-heartbeat>
            <svg role="presentation" viewBox="0 0 44 18">
              <path
                class="heartbeat-track"
                d="M1 9h8l3-6 5 13 5-11 4 7 3-3h14"
                pathLength="100"
              />
              <path
                class="heartbeat-pulse"
                d="M1 9h8l3-6 5 13 5-11 4 7 3-3h14"
                pathLength="100"
              />
            </svg>
          </i>
          <template v-if="speedDisplay">
            <VbenCountToAnimator
              class="runtime-header-speed"
              :decimals="speedDisplay.decimals"
              :duration="countAnimationDuration"
              :end-val="speedDisplay.value"
              transition="easeOutCubic"
            />
            <em>{{ speedDisplay.unit }}</em>
          </template>
          <strong v-else>速率未知</strong>
        </div>
        <Badge
          class="nas-write-state"
          :color="view.state === 'RUNNING' ? '#2dd591' : '#7e8999'"
          :text="view.state === 'RUNNING' ? '加密写入正常' : view.state_label"
        />
      </article>

      <article class="trend-state">
        <h2>实时速度趋势</h2>
        <p class="screen-reader-only">
          当前契约仅提供瞬时吞吐量，不推测历史曲线。
        </p>
        <EchartsUI
          ref="trendChartRef"
          aria-label="运输 NAS 实时速度趋势，缺失历史采样不补值"
          class="nas-trend-chart"
          height="218px"
          role="img"
        />
      </article>
    </section>

    <section class="nas-status-row" aria-label="运输设备状态">
      <span><i></i>设备已连接</span>
      <span><i></i>本机只读视图</span>
      <span><i></i>运行状态可追溯</span>
      <span v-if="view.media.warning > 0" class="warning">
        <i></i>{{ view.media.warning }} 块介质需要注意
      </span>
    </section>

    <section class="nas-persistent-strip">
      <article class="nas-current-task">
        <h2>当前任务</h2>
        <template v-if="view.current">
          <strong>{{ view.current.batch_id ?? '批次未分配' }}</strong>
          <span>{{ view.current.stage }}</span>
          <div
            v-if="view.current.progress_percent !== null"
            class="nas-task-progress-row"
          >
            <Progress
              class="nas-task-progress"
              :percent="view.current.progress_percent"
              :show-info="false"
              :stroke-color="{ '0%': '#087ae0', '100%': '#38c9ff' }"
              stroke-linecap="butt"
              :stroke-width="8"
              trail-color="#29323b"
            />
            <b>{{ view.current.progress_percent }}%</b>
          </div>
          <dl>
            <div>
              <dt>
                <img :src="taskFactIcons.confirmed" alt="" />
                <span>已确认</span>
              </dt>
              <dd>
                {{
                  formatBytePair(
                    view.current.confirmed_bytes,
                    view.current.total_bytes,
                  )
                }}
              </dd>
            </div>
            <div>
              <dt>
                <img :src="taskFactIcons.eta" alt="" />
                <span>预计剩余时间</span>
              </dt>
              <dd>{{ formatEta(view.current.eta_seconds) }}</dd>
            </div>
            <div>
              <dt>
                <img :src="taskFactIcons.confidence" alt="" />
                <span>预计剩余时间可信度</span>
              </dt>
              <dd>{{ confidenceLabel(view.current.eta_confidence) }}</dd>
            </div>
          </dl>
        </template>
        <p v-else>当前没有装盘任务。</p>
      </article>

      <article class="nas-pool-state">
        <h2>盘组状态</h2>
        <dl>
          <div>
            <dt>已接入</dt>
            <dd>{{ view.media.connected }}</dd>
          </div>
          <div>
            <dt>写入</dt>
            <dd>{{ view.media.running }}</dd>
          </div>
          <div>
            <dt>待命</dt>
            <dd>{{ view.media.standby }}</dd>
          </div>
          <div>
            <dt>待换</dt>
            <dd>{{ view.media.completed }}</dd>
          </div>
          <div>
            <dt>异常</dt>
            <dd>{{ view.media.failed }}</dd>
          </div>
          <div class="warning">
            <dt>警告</dt>
            <dd>{{ view.media.warning }}</dd>
          </div>
        </dl>
        <div
          class="nas-slot-meter"
          :aria-label="`当前接入 ${view.media.connected} 块运输盘`"
          :style="slotMeterStyle"
        >
          <i
            v-for="(state, index) in visibleSlots"
            :key="index"
            :class="state"
          ></i>
        </div>
      </article>
    </section>
  </section>
</template>

<style scoped>
.nas-baseline {
  position: relative;
  width: 100%;
  height: 100%;
  overflow: hidden;
  color: #d8dee6;
}

.nas-tabs {
  position: absolute;
  top: 16px;
  left: 80px;
  z-index: 2;
  width: 290px;
  height: 50px;
  color: #87909b;
}

.nas-tabs :deep(.ant-tabs-nav) {
  height: 50px;
  margin: 0;
}

.nas-tabs :deep(.ant-tabs-nav),
.nas-tabs :deep(.ant-tabs-nav-wrap),
.nas-tabs :deep(.ant-tabs-nav-list),
.nas-tabs :deep(.ant-tabs-tab),
.nas-tabs :deep(.ant-tabs-tab-btn),
.nas-tabs :deep(.ant-tabs-ink-bar) {
  transition: none !important;
  animation: none !important;
}

.nas-tabs :deep(.ant-tabs-nav::before) {
  border-color: rgb(116 132 148 / 25%);
}

.nas-tabs :deep(.ant-tabs-nav-list) {
  display: grid;
  grid-template-columns: repeat(2, 145px);
  width: 100%;
  transform: none !important;
}

.nas-tabs :deep(.ant-tabs-tab) {
  justify-content: center;
  padding: 0;
  margin: 0 !important;
}

.nas-tabs :deep(.ant-tabs-tab-btn) {
  font-size: 17px;
  font-weight: 400;
  color: #87909b;
}

.nas-tabs :deep(.ant-tabs-tab-active .ant-tabs-tab-btn) {
  color: #e7edf3;
}

.nas-tabs :deep(.ant-tabs-ink-bar) {
  height: 2px;
  background: #18baff;
}

.nas-tabs :deep(.ant-tabs-content-holder) {
  display: none;
}

.nas-overview {
  position: absolute;
  top: 116px;
  left: 80px;
  display: grid;
  grid-template-columns: 360px 260px 440px;
  height: 255px;
}

.nas-overview > article {
  padding: 0 32px;
  animation: nas-info-reveal-from-left 420ms cubic-bezier(0.22, 1, 0.36, 1) both;
}

.nas-overview > article:first-child {
  padding-left: 5px;
  animation-delay: 80ms;
}

.nas-overview > article:nth-child(2) {
  animation-delay: 130ms;
}

.nas-overview > article:nth-child(3) {
  animation-delay: 180ms;
}

.nas-overview > article + article {
  border-left: 1px solid rgb(112 130 149 / 28%);
}

.nas-overview h2 {
  margin: 0 0 12px;
  font-size: 18px;
  font-weight: 400;
  color: #c5ccd5;
}

.array-state > h2 {
  display: flex;
  gap: 20px;
  align-items: center;
  margin-bottom: 10px;
  font-size: 26px;
  line-height: 34px;
  color: #e2e8ee;
}

.array-state > h2 i,
.nas-status-row i {
  width: 10px;
  height: 10px;
  background: var(--fd-success);
  border-radius: 50%;
  box-shadow: 0 0 10px rgb(57 223 166 / 65%);
}

.array-state > p {
  font-size: 16px;
  color: #aab3bd;
}

.array-state > p b,
.warning {
  font-weight: 400;
  color: var(--fd-warning);
}

.array-state dl {
  display: grid;
  gap: 18px;
  margin-top: 28px;
}

.array-state dl div {
  display: grid;
  gap: 6px;
}

.array-state dt,
.nas-current-task dt {
  font-size: 15px;
  color: #7f8995;
}

.array-state dd,
.nas-current-task dd {
  margin: 0;
}

.write-state > strong {
  display: block;
  font-size: 44px;
  font-weight: 350;
  line-height: 1.1;
}

.write-state > strong small {
  font-size: 18px;
  color: #919ba6;
}

.write-state hr {
  margin: 16px 0 22px;
  border: 0;
  border-top: 1px solid rgb(112 130 149 / 25%);
}

.write-state > h2 {
  margin-bottom: 0;
}

.nas-speed-indicator {
  justify-content: flex-start;
  min-width: 0;
  margin: 11px 0 8px;
}

.nas-speed-indicator > i {
  flex: 0 0 36px;
  margin-left: 0;
}

.nas-speed-indicator .runtime-header-speed {
  font-size: 38px;
  font-weight: 400;
}

.nas-speed-indicator em {
  font-size: 18px;
}

.trend-state > p {
  font-size: 12px;
  color: #74808d;
}

.trend-state {
  position: relative;
}

.trend-state > h2 {
  margin-left: 18px;
}

.nas-write-state {
  display: inline-flex;
  align-items: center;
  margin-top: 2px;
}

.nas-write-state :deep(.ant-badge-status-dot) {
  width: 8px;
  height: 8px;
  box-shadow: 0 0 9px currentcolor;
}

.nas-write-state :deep(.ant-badge-status-text) {
  margin-left: 10px;
  font-size: 15px;
  color: #c5ccd5;
}

.nas-trend-chart {
  position: absolute;
  inset: 34px 0 auto;
}

.nas-status-row {
  position: absolute;
  top: 398px;
  left: 80px;
  display: flex;
  width: 1060px;
  height: 82px;
  border-top: 1px solid rgb(112 130 149 / 28%);
  animation: nas-info-reveal-from-left 420ms cubic-bezier(0.22, 1, 0.36, 1)
    220ms both;
}

.nas-status-row span {
  display: flex;
  flex: 1;
  gap: 12px;
  align-items: center;
  justify-content: center;
  min-width: 0;
  padding: 0 12px;
  font-size: 16px;
  text-align: center;
}

.nas-status-row span + span {
  border-left: 1px solid rgb(112 130 149 / 24%);
}

.nas-status-row .warning i {
  background: var(--fd-warning);
  box-shadow: 0 0 10px rgb(255 177 74 / 45%);
}

.nas-persistent-strip {
  position: absolute;
  inset: var(--fd-edge-footer-start) 0 0;
  display: grid;
  grid-template-columns: 1fr 1fr;
  background: var(--fd-edge-footer-background);
  border-top: var(--fd-detail-footer-border);
}

.nas-persistent-strip > article {
  position: relative;
  padding: var(--fd-detail-footer-padding);
}

.nas-persistent-strip > article + article {
  border-left: var(--fd-detail-footer-divider);
}

.nas-persistent-strip h2 {
  margin: 0 0 14px;
  font-size: 18px;
  font-weight: 400;
  color: #22b8ff;
}

.nas-current-task > strong {
  display: block;
  font-size: 30px;
  font-weight: 400;
  line-height: 1.08;
  color: #188fff;
}

.nas-current-task > span {
  display: block;
  margin-top: 4px;
  font-size: 14px;
  color: #9ba5b0;
}

.nas-current-task {
  padding-top: 19px !important;
}

.nas-current-task h2 {
  margin-bottom: 9px;
}

.nas-task-progress-row {
  display: grid;
  grid-template-columns: minmax(0, 1fr) 76px;
  gap: 18px;
  align-items: center;
  margin-top: 10px;
}

.nas-task-progress {
  width: 100%;
  min-width: 0;
}

.nas-task-progress :deep(.ant-progress-outer) {
  display: block;
  padding: 0;
  margin: 0;
}

.nas-task-progress :deep(.ant-progress-inner) {
  vertical-align: top;
  background: #29323b;
  border-radius: 0;
}

.nas-task-progress :deep(.ant-progress-bg) {
  box-shadow: 0 0 10px rgb(19 143 255 / 40%);
}

.nas-task-progress-row > b {
  font-size: 32px;
  font-weight: 350;
  line-height: 1;
  text-align: right;
}

.nas-current-task dl {
  display: grid;
  grid-template-columns: 0.92fr 1.05fr 1.3fr;
  gap: 0;
  margin-top: 9px;
}

.nas-current-task dl div {
  min-width: 0;
  padding: 0 18px;
  border-left: 1px solid rgb(112 130 149 / 24%);
}

.nas-current-task dl div:first-child {
  padding-left: 0;
  border-left: 0;
}

.nas-current-task dt {
  display: flex;
  gap: 10px;
  align-items: center;
  min-height: 22px;
  font-size: 14px;
  line-height: 22px;
  white-space: nowrap;
}

.nas-current-task dt img {
  flex: 0 0 22px;
  width: 22px;
  height: 22px;
}

.nas-current-task dd {
  margin-top: 3px;
  margin-left: 32px;
  overflow: hidden;
  text-overflow: ellipsis;
  font-size: 20px;
  line-height: 1.1;
  white-space: nowrap;
}

.nas-pool-state dl {
  display: grid;
  grid-template-columns: repeat(6, 1fr);
  margin: 0;
}

.nas-pool-state dl div {
  text-align: center;
}

.nas-pool-state dt {
  color: #87929f;
}

.nas-pool-state dd {
  margin: 8px 0 0;
  font-size: 38px;
  font-weight: 350;
  color: #d6dde5;
}

.nas-pool-state dl div:first-child dd,
.nas-pool-state dl div:nth-child(2) dd {
  color: #188fff;
}

.nas-pool-state .warning dd {
  color: var(--fd-warning);
}

.nas-slot-meter {
  display: flex;
  gap: 0;
  align-items: flex-start;
  justify-content: space-between;
  width: 100%;
  height: 58px;
  margin-top: 34px;
}

.nas-slot-meter i {
  flex: 0 1 var(--nas-slot-width);
  width: var(--nas-slot-width);
  min-width: 16px;
  height: 45px;
  background: #3f5b75;
  border-radius: 5px;
}

.nas-slot-meter i.running {
  background: #159df5;
  box-shadow: 0 0 10px rgb(21 157 245 / 45%);
}

.nas-slot-meter i.completed {
  background: #a8b6c3;
}

.nas-slot-meter i.failed {
  background: var(--fd-danger);
}

@keyframes nas-info-reveal-from-left {
  from {
    opacity: 0;
    filter: blur(3px);
    transform: translateX(-48px);
  }

  to {
    opacity: 1;
    filter: blur(0);
    transform: translateX(0);
  }
}

@media (prefers-reduced-motion: reduce) {
  .nas-overview > article,
  .nas-status-row {
    animation: none;
  }
}
</style>
