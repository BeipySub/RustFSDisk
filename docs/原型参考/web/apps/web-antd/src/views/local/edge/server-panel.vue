<!--
  @file server-panel.vue
  @description A-02 服务器运行状态面板，展示数据发现与同步记录摘要。
  @usage 嵌入 runtime.vue 的连续设备场景，也可由 server.vue 独立装配。
  @baseline A-02-server-status · 1672×941
-->
<script setup lang="ts">
import type { EdgeServerStatusView, EdgeSyncRecordsView } from '#/api/local-views';

import { computed } from 'vue';

import { VbenCountToAnimator } from '@vben/common-ui';
import { usePreferredReducedMotion } from '@vueuse/core';
import RecordsPanel from './records-panel.vue';
import ServerTabs from './server-tabs.vue';

const props = defineProps<{
  embedded?: boolean;
  records: EdgeSyncRecordsView;
  view: EdgeServerStatusView;
}>();

const rackAsset = '/assets/fustfs-baseline/source-rack-cutout-v3.webp';
const preferredReducedMotion = usePreferredReducedMotion();
const countAnimationDuration = computed(() =>
  preferredReducedMotion.value === 'reduce' ? 0 : 1200,
);

function formatScanTime(value: null | string | undefined) {
  if (!value) return '未知';
  const timestamp = new Date(value);
  if (Number.isNaN(timestamp.getTime())) return '未知';
  return timestamp.toLocaleTimeString('zh-CN', {
    hour: '2-digit',
    hour12: false,
    minute: '2-digit',
    second: '2-digit',
  });
}

</script>

<template>
  <section
    aria-labelledby="server-title"
    class="server-baseline"
    :class="{ 'server-embedded': embedded }"
  >
    <h1 id="server-title" class="screen-reader-only">服务器运行状态</h1>

    <img :src="rackAsset" alt="" class="server-rack" />

    <ServerTabs active="status" />

    <section
      class="readiness-area server-section-enter flex justify-between"
      aria-label="数据发现概览"
    >
      <article class="server-metric discovery-metric min-w-0">
        <h2>数据发现</h2>
        <div class="discovery-summary">
          <strong>
            <VbenCountToAnimator
              v-if="view.pending_object_versions !== null"
              class="metric-count"
              :duration="countAnimationDuration"
              :end-val="view.pending_object_versions"
              separator=","
              transition="easeOutCubic"
            />
            <template v-else>未知</template>
          </strong>
          <span>新增待拷贝版本</span>
        </div>
        <div class="metric-details">
          <p>最近扫描 {{ formatScanTime(view.last_scan_at) }}</p>
          <p><i></i>扫描已完成</p>
        </div>
      </article>

    </section>

    <RecordsPanel embedded :view="records" />
  </section>
</template>

<style scoped>
.server-baseline {
  position: relative;
  width: 100%;
  height: 100%;
  overflow: hidden;
  background:
    radial-gradient(ellipse at 21% 48%, rgb(12 99 175 / 15%), transparent 25%),
    linear-gradient(180deg, #020508 0 61.5%, #06101a 61.6% 100%);
}

.server-baseline.server-embedded {
  background: transparent;
}

.server-embedded .server-rack {
  display: none;
}

.server-rack {
  position: absolute;
  top: 4px;
  left: 116px;
  width: 430px;
  height: 486px;
  object-fit: contain;
  filter: drop-shadow(0 24px 28px rgb(0 0 0 / 65%));
  transform: scaleX(1.05);
}

.readiness-area {
  position: absolute;
  top: 96px;
  right: var(--fd-server-content-right, 55px);
  left: var(--fd-server-content-left, 535px);
  display: grid;
  grid-template-columns: minmax(0, 1fr);
  min-width: 0;
  height: 118px;
  overflow: hidden;
  background:
    linear-gradient(120deg, rgb(8 25 38 / 78%), rgb(3 13 21 / 62%)),
    radial-gradient(circle at 8% 100%, rgb(20 134 222 / 15%), transparent 44%);
  border: 1px solid rgb(67 143 193 / 24%);
  border-radius: 10px;
  box-shadow: inset 0 1px 0 rgb(163 222 255 / 5%);
}

.readiness-area > article {
  position: relative;
}

.health-ring {
  position: relative;
  flex: 1 1 0;
  min-width: 0;
  padding-top: 10px;
}

.health-visual {
  position: relative;
  flex: 0 0 184px;
  width: 184px;
  height: 184px;
}

.health-progress {
  position: absolute;
  inset: 0;
  width: 184px;
  height: 184px;
}

.health-progress :deep(.ant-progress-inner) {
  box-shadow:
    inset 0 0 18px rgb(28 160 255 / 22%),
    0 0 18px rgb(28 160 255 / 18%);
}

.health-progress :deep(.ant-progress-circle-path) {
  filter: drop-shadow(0 0 5px rgb(28 160 255 / 28%));
}

.health-progress-content {
  position: absolute;
  inset: 0;
  z-index: 1;
  width: 184px;
  text-align: center;
}

.health-progress-content span {
  font-size: 18px;
  font-weight: 600;
  color: #c5ccd4;
}

.health-progress-content strong {
  font-size: 57px;
  font-weight: 350;
  line-height: 1.08;
}

.health-progress-content small {
  font-size: 23px;
}

.health-ring p {
  width: 100%;
  margin: 32px 0 0;
  font-size: 15px;
  color: #c2c9d1;
  text-align: center;
}

.health-ring p b {
  font-weight: 400;
  color: var(--fd-warning);
}

.server-metric {
  display: grid;
  grid-template-rows: 20px 42px 8px 1fr;
  min-width: 0;
  padding: 14px 24px 12px;
}

.server-metric > h2,
.server-metric > strong,
.server-metric > .discovery-summary,
.server-metric > .metric-details {
  margin-right: 0;
  margin-left: 0;
}

.server-metric h2 {
  margin: 0;
  font-size: 14px;
  font-weight: 600;
  color: #c2c8d0;
}

.server-metric p,
.server-metric span {
  font-size: 12px;
  color: #a0a9b3;
}

.server-metric span.metric-count {
  font-size: inherit;
  font-weight: inherit;
  line-height: inherit;
  color: inherit;
}

.metric-details {
  display: grid;
  grid-row: 4;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 6px 18px;
  align-content: end;
  padding-top: 4px;
}

.metric-details p {
  margin: 0;
  white-space: nowrap;
}

.discovery-summary {
  display: grid;
  grid-row: 2 / 4;
  align-content: center;
}

.discovery-summary strong {
  max-width: 100%;
  overflow: hidden;
  text-overflow: ellipsis;
  font-size: 28px;
  line-height: 1.25;
  font-weight: 350;
  line-height: 1.1;
  white-space: nowrap;
}

.discovery-metric .metric-details p:last-child {
  display: inline-flex;
  gap: 8px;
  align-items: center;
  width: max-content;
  padding: 3px 8px;
  color: #d0d7df;
  background: rgb(39 150 112 / 10%);
  border: 1px solid rgb(64 197 149 / 16%);
  border-radius: 999px;
}

.discovery-metric .metric-details p i,
.transport-summary dd i {
  display: inline-block;
  width: 8px;
  height: 8px;
  margin-right: 8px;
  background: var(--fd-success);
  border-radius: 50%;
}

.trend-metric {
  display: block;
  flex: 1 1 0;
  overflow: hidden;
}

.trend-chart {
  position: absolute;
  inset: 28px 0 auto;
}

.trend-metric > .trend-latest {
  position: absolute;
  top: 31px;
  right: 0;
  z-index: 2;
  font-size: 12px;
  font-weight: 400;
  color: #28aaff;
}

.persistent-strip {
  position: absolute;
  inset: var(--fd-edge-footer-start) 0 0;
  display: grid;
  grid-template-columns: 1fr 1fr;
  background: var(--fd-edge-footer-background);
  border-top: var(--fd-detail-footer-border);
}

.persistent-strip > article {
  position: relative;
  padding: var(--fd-detail-footer-padding);
}

.persistent-strip > article + article {
  border-left: var(--fd-detail-footer-divider);
}

.persistent-strip h2 {
  margin: 0;
  font-size: 18px;
  font-weight: 400;
  color: #22b8ff;
}

.current-task {
  padding-left: 48px !important;
}

.task-icon {
  position: absolute;
  top: 80px;
  left: 47px;
  width: 150px;
  height: 150px;
  object-fit: contain;
  filter: drop-shadow(0 9px 14px rgb(0 0 0 / 42%));
}

.current-task > strong,
.current-task > span,
.current-task > progress,
.current-task > b,
.current-task > p {
  margin-left: 158px;
}

.current-task > strong {
  display: block;
  margin-top: 18px;
  font-size: 23px;
  font-weight: 400;
}

.current-task > span {
  display: block;
  margin-top: 6px;
  font-size: 15px;
  color: #7e8996;
}

.current-task progress {
  width: 460px;
  height: 8px;
  margin-top: 22px;
  appearance: none;
}

.current-task progress::-webkit-progress-bar {
  background: #29323b;
}

.current-task progress::-webkit-progress-value {
  background: linear-gradient(90deg, #087ae0, #38c9ff);
}

.current-task > b {
  display: inline-block;
  margin-left: 30px;
  font-size: 34px;
  font-weight: 350;
  transform: translateY(-7px);
}

.current-task > p {
  margin-top: 12px;
  color: #9ba5b0;
}

.task-facts {
  display: flex;
  align-items: center;
  font-size: 15px;
  white-space: nowrap;
}

.task-facts > span {
  display: inline-flex;
  gap: 6px;
  align-items: center;
}

.task-facts svg {
  width: 15px;
  height: 15px;
  fill: none;
  stroke: #8e9ba8;
  stroke-width: 1.2;
  stroke-linecap: round;
  stroke-linejoin: round;
}

.task-facts > i {
  width: 1px;
  height: 14px;
  margin: 0 18px;
  background: #56616d;
}

.transport-summary {
  padding-left: 315px !important;
}

.transport-summary > h2 {
  position: absolute;
  left: 45px;
}

.transport-summary > img {
  position: absolute;
  top: 80px;
  bottom: auto;
  left: 47px;
  width: 150px;
  height: 150px;
  object-fit: contain;
  transform: none;
}

.transport-summary dl {
  display: grid;
  gap: 12px;
  margin: 42px 0 0;
}

.runtime-unavailable {
  margin: 64px 0 0;
  color: #87929f;
}

.transport-summary dl div {
  display: grid;
  grid-template-columns: 195px 1fr;
}

.transport-summary dt {
  color: #87929f;
}

.transport-summary dd {
  margin: 0;
  color: #e1e7ed;
}
</style>
