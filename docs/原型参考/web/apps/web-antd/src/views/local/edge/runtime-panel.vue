<!--
  @file runtime-panel.vue
  @description A-01 运行首页舞台，展示服务器、运输 NAS、粒子数据流和业务状态底栏。
  @usage 由 runtime.vue 持续挂载，并根据路由切换设备聚焦状态。
  @baseline A-01-running · A-01-idle-no-media · 1672×941
-->
<script setup lang="ts">
import type { ParticleDebugSettings } from './particle-debug';

import type { EdgeMediaCandidatesView, EdgeRuntimeView } from '#/api/local-views';

import { computed } from 'vue';

import DataFlowField from '../components/data-flow-field.vue';
import { formatBytes, formatCount, formatEta } from '../model';

export type EdgeDeviceFocus = 'nas' | 'server';

const props = withDefaults(
  defineProps<{
    focus?: EdgeDeviceFocus | null;
    mountedFocus?: EdgeDeviceFocus | null;
    particleDebug?: ParticleDebugSettings;
    transportCandidates?: EdgeMediaCandidatesView;
    view: EdgeRuntimeView;
  }>(),
  {
    focus: null,
    mountedFocus: null,
    particleDebug: () => ({
      color: null,
      glowStrength: 1,
      speedMultiplier: 1,
      state: null,
    }),
  },
);

const emit = defineEmits<{
  open: [focus: EdgeDeviceFocus];
}>();

const sourceRackAsset = '/assets/fustfs-baseline/source-rack-cutout-v3.webp';
const transportNasAsset =
  '/assets/fustfs-baseline/transport-nas-cutout-v3.webp';
const connectedMediaCount = computed(
  () => props.transportCandidates?.candidates.length ?? props.view.media.connected,
);

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
  if (total === null) {
    return `${confirmedText} / 总量未知`;
  }

  const totalText = formatBytes(total);
  const confirmedParts = confirmedText.split(' ');
  const totalParts = totalText.split(' ');

  return confirmedParts[1] === totalParts[1]
    ? `${confirmedParts[0]} / ${totalText}`
    : `${confirmedText} / ${totalText}`;
}
</script>

<template>
  <section
    aria-labelledby="edge-runtime-title"
    class="runtime-view"
    :class="{
      'focus-nas': focus === 'nas',
      'focus-server': focus === 'server',
      'has-device-focus': mountedFocus !== null,
      'is-detail-leaving': mountedFocus !== null && focus === null,
    }"
    :data-device-focus="focus ?? 'home'"
  >
    <h1 id="edge-runtime-title" class="screen-reader-only">运行首页</h1>
    <p class="screen-reader-only" role="status">
      {{ view.state_label }}；{{ view.meta.status_message }}；
      {{
        view.meta.data_as_of
          ? `数据截至 ${view.meta.data_as_of}`
          : '尚无权威采样时间'
      }}
    </p>

    <div
      class="runtime-stage"
      :class="`state-${view.state.toLowerCase().replaceAll('_', '-')}`"
      role="img"
      aria-label="本地 RustFS 服务器正在向运输 NAS 加密写入数据"
    >
      <DataFlowField
        :custom-color="particleDebug.color"
        :debug-state="particleDebug.state"
        :glow-strength="particleDebug.glowStrength"
        :paused="mountedFocus !== null"
        :speed-multiplier="particleDebug.speedMultiplier"
        :state="view.state"
      />
      <button
        aria-label="查看 RustFS 服务器"
        class="scene-device scene-source"
        type="button"
        @click="emit('open', 'server')"
      >
        <img class="scene-device-art" :src="sourceRackAsset" alt="" />
      </button>
      <button
        aria-label="查看运输 NAS"
        class="scene-device scene-nas"
        type="button"
        @click="emit('open', 'nas')"
      >
        <img alt="" class="scene-device-art" :src="transportNasAsset" />
      </button>
    </div>

    <div class="runtime-dashboard">
      <section class="media-summary" aria-labelledby="media-summary-title">
        <div
          class="media-slots"
          :aria-label="`当前接入 ${connectedMediaCount} 块运输盘`"
        >
          <i
            v-for="index in connectedMediaCount"
            :key="index"
            :class="{
              completed: index > view.media.running + view.media.standby,
              running: index <= view.media.running,
            }"
          ></i>
        </div>
        <div class="media-status-row">
          <dl class="count-grid" aria-label="运输盘状态分布">
            <div>
              <dt>运行</dt>
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
            <div class="danger">
              <dt>异常</dt>
              <dd>{{ view.media.failed }}</dd>
            </div>
            <div class="warning">
              <dt>健康警告</dt>
              <dd>{{ view.media.warning }}</dd>
            </div>
          </dl>
          <div class="connected-count">
            <div class="connected-main">
              <span>已接入</span>
              <strong>{{ connectedMediaCount }}</strong>
              <small>块</small>
            </div>
            <h2 id="media-summary-title">运输硬盘</h2>
          </div>
        </div>
      </section>

      <section class="current-stage" aria-labelledby="current-stage-title">
        <h2 id="current-stage-title">当前装盘</h2>
        <template v-if="view.current">
          <div class="stage-heading">
            <strong class="stage-name">{{ view.current.stage }}</strong>
            <span class="batch-id">
              {{ view.current.batch_id ?? '批次未分配' }}
            </span>
          </div>
          <div
            v-if="view.current.progress_percent !== null"
            class="progress-row"
          >
            <progress
              :aria-label="`当前阶段进度 ${view.current.progress_percent}%`"
              max="100"
              :value="view.current.progress_percent"
            ></progress>
            <span class="progress-value">
              {{ view.current.progress_percent }}%
            </span>
          </div>
          <div v-else class="indeterminate-progress" role="status">
            总量未知，正在确认已完成数据
          </div>
          <dl class="current-metrics">
            <div>
              <dt>已确认</dt>
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
              <dt>预计剩余</dt>
              <dd>{{ formatEta(view.current.eta_seconds) }}</dd>
            </div>
            <div>
              <dt>ETA 置信度</dt>
              <dd>{{ confidenceLabel(view.current.eta_confidence) }}</dd>
            </div>
          </dl>
        </template>
        <p v-else class="empty-current">
          当前没有装盘任务，系统继续自动发现可同步对象。
        </p>
        <dl
          v-if="view.source_export"
          class="source-export-summary"
          aria-label="源数据拷贝状态"
        >
          <div>
            <dt>未拷贝</dt>
            <dd>{{ formatCount(view.source_export.not_copied_versions) }}</dd>
            <small>{{ formatBytes(view.source_export.not_copied_bytes) }}</small>
          </div>
          <div>
            <dt>已拷贝</dt>
            <dd>{{ formatCount(view.source_export.copied_versions) }}</dd>
            <small>{{ formatBytes(view.source_export.copied_bytes) }}</small>
          </div>
        </dl>
        <p v-else class="source-export-unavailable">源数据拷贝状态暂不可用</p>
      </section>

      <section
        aria-labelledby="next-action-title"
        class="next-action"
        :class="`priority-${view.next_action.priority.toLowerCase()}`"
      >
        <h2 id="next-action-title">下一步</h2>
        <div class="next-device">
          <span v-if="view.next_action.media_slot">
            <i aria-hidden="true"></i>
            盘位 {{ view.next_action.media_slot }} · SN …{{
              view.next_action.serial_suffix ?? '未知'
            }}
          </span>
          <p>{{ view.next_action.detail }}</p>
        </div>
        <div class="next-command">
          <small>建议操作</small>
          <strong>
            {{ view.next_action.title }} <b aria-hidden="true">→</b>
          </strong>
        </div>
        <small v-if="view.next_action.requires_role">
          需要 {{ view.next_action.requires_role }} 管理角色
        </small>
        <span class="screen-reader-only">
          连接盘数 {{ formatCount(connectedMediaCount) }}
        </span>
      </section>
    </div>

    <section
      v-if="mountedFocus"
      :aria-label="mountedFocus === 'server' ? '服务器详情' : '运输 NAS 详情'"
      aria-live="polite"
      class="device-detail-layer"
      :class="`detail-${mountedFocus}`"
    >
      <slot name="detail"></slot>
    </section>
  </section>
</template>

<style scoped>
.runtime-view {
  --fd-edge-footer-height: 31.6%;
  --fd-edge-footer-start: 68.4%;
  --fd-edge-footer-background: #06101a;
  --fd-detail-footer-border: 1px solid rgb(112 130 149 / 28%);
  --fd-detail-footer-divider: 1px solid rgb(112 130 149 / 22%);
  --fd-detail-footer-padding: 25px 48px;
  --fd-server-content-left: 535px;
  --fd-server-content-right: 55px;
  --fd-server-heading-note-color: #8d96a0;
  --fd-server-heading-note-line-height: 24px;
  --fd-server-heading-note-size: 15px;
  --fd-server-section-title-color: #d8e0e7;
  --fd-server-section-title-line-height: 36px;
  --fd-server-section-title-size: 30px;
  --fd-server-tabs-top: 16px;

  position: relative;
  display: grid;
  grid-template-rows:
    var(--fd-edge-footer-start)
    var(--fd-edge-footer-height);
  width: 100%;
  height: 100%;
  min-height: 0;
  overflow: hidden;
  background-color: #05070a;
  transition: background-color 360ms ease;
}

.runtime-stage {
  position: relative;
  overflow: hidden;
  background:
    linear-gradient(
      180deg,
      rgb(0 2 5 / 28%) 0%,
      rgb(0 3 7 / 20%) 54%,
      rgb(0 5 10 / 18%) 100%
    ),
    url('/assets/fustfs-baseline/factory-environment-v4.webp') center / 100%
      100% no-repeat,
    #010407;
}

.runtime-stage::before {
  position: absolute;
  inset: 0;
  z-index: 1;
  pointer-events: none;
  content: '';
  background:
    radial-gradient(ellipse at 28% 79%, rgb(39 137 222 / 13%), transparent 16%),
    radial-gradient(ellipse at 76% 82%, rgb(34 118 206 / 11%), transparent 15%),
    linear-gradient(
      90deg,
      rgb(0 2 5 / 28%),
      transparent 15%,
      transparent 84%,
      rgb(0 2 5 / 36%)
    );
}

.runtime-stage::after {
  position: absolute;
  inset: 0;
  z-index: 2;
  pointer-events: none;
  content: '';
  background:
    radial-gradient(ellipse at 21% 48%, rgb(12 99 175 / 18%), transparent 31%),
    radial-gradient(ellipse at 62% 78%, rgb(11 72 126 / 8%), transparent 34%),
    linear-gradient(180deg, rgb(0 3 7 / 58%), rgb(1 7 13 / 36%));
  opacity: 0;
  transition: opacity 360ms ease;
}

.scene-device {
  position: absolute;
  z-index: 8;
  display: block;
  padding: 0;
  background: transparent;
  border: 0;
  border-radius: 8px;
  filter: none;
  transition:
    inset 420ms cubic-bezier(0.22, 1, 0.36, 1),
    width 420ms cubic-bezier(0.22, 1, 0.36, 1),
    height 420ms cubic-bezier(0.22, 1, 0.36, 1),
    transform 420ms cubic-bezier(0.22, 1, 0.36, 1),
    filter 200ms ease,
    opacity 350ms ease;
}

.scene-device::before {
  content: none;
}

.scene-device img {
  display: block;
  width: 100%;
  height: 100%;
  object-fit: contain;
}

.scene-device:focus-visible {
  outline: 2px solid var(--fd-cyan);
  outline-offset: 5px;
}

.scene-source {
  bottom: 6.8%;
  left: 2.3%;
  width: 41%;
  height: 91%;
  transform-origin: left bottom;
}

.scene-source .scene-device-art {
  transform: scaleX(1.06);
}

.scene-nas {
  right: 9%;
  bottom: 5.5%;
  z-index: 10;
  width: 34%;
  height: 50%;
  transform-origin: right bottom;
}

.scene-nas .scene-device-art {
  transform: scaleX(1.06);
}

.focus-server .runtime-dashboard,
.focus-nas .runtime-dashboard {
  position: absolute;
  right: 0;
  bottom: 0;
  left: 0;
  height: var(--fd-edge-footer-height);
  pointer-events: none;
  opacity: 0;
  transform: translateY(24px);
  transition-delay: 0ms;
}

.has-device-focus :deep(.data-flow-canvas) {
  opacity: 0.045;
  transition: opacity 200ms ease;
}

.focus-server .scene-source {
  top: 53px;
  bottom: auto;
  left: 136px;
  width: 385px;
  height: 466px;
  transform: none;
}

.focus-server .scene-source::before {
  right: 5%;
  bottom: -1.5%;
  left: 7%;
  z-index: 0;
  height: 7%;
  content: '';
  background: radial-gradient(
    ellipse at center,
    rgb(8 20 32 / 88%) 0%,
    rgb(25 111 184 / 26%) 42%,
    transparent 76%
  );
  opacity: 0.92;
  filter: blur(9px);
  transform: perspective(110px) rotateX(68deg);
}

.focus-server .scene-source .scene-device-art {
  position: relative;
  z-index: 1;
  filter: drop-shadow(0 13px 11px rgb(0 0 0 / 68%));
}

.runtime-view.focus-server,
.runtime-view.focus-nas {
  background-color: #020508;
}

.focus-server .runtime-stage::after,
.focus-nas .runtime-stage::after {
  opacity: 1;
}

.focus-server .scene-nas {
  right: -8%;
  pointer-events: none;
  opacity: 0;
  transform: translateX(10vw) scale(0.72);
}

.focus-nas .scene-nas {
  top: 156px;
  right: 4.2%;
  bottom: auto;
  width: 25.5%;
  height: 62%;
  filter: none;
  transform: none;
}

.focus-nas .scene-nas::before {
  content: none;
}

.focus-nas .scene-nas .scene-device-art {
  transform: scaleX(1);
}

.focus-nas .scene-source {
  visibility: hidden;
  pointer-events: none;
  opacity: 0;
  transform: translateX(-10vw) scale(0.72);
}

.is-detail-leaving .scene-device {
  transition-duration: 320ms;
}

.device-detail-layer {
  --fd-detail-enter-x: 0;

  position: absolute;
  inset: 0;
  z-index: 12;
  pointer-events: none;
  opacity: 0;
  transform: translateX(var(--fd-detail-enter-x));
  transition:
    opacity 320ms ease,
    transform 420ms cubic-bezier(0.22, 1, 0.36, 1);
  will-change: opacity, transform;
}

.device-detail-layer > * {
  pointer-events: auto;
}

.device-detail-layer.detail-server {
  --fd-detail-enter-x: 42px;
}

.device-detail-layer.detail-nas {
  --fd-detail-enter-x: -42px;
}

.focus-server .device-detail-layer.detail-server,
.focus-nas .device-detail-layer.detail-nas {
  opacity: 1;
  transform: translateX(0);
  transition-delay: 0ms;
}

.is-detail-leaving .device-detail-layer {
  pointer-events: none;
  opacity: 0;
  transition-delay: 0ms;
  transition-duration: 320ms;
}

.state-idle .scene-device,
.state-no-media .scene-device,
.state-paused .scene-device,
.state-permission-denied .scene-device {
  filter: saturate(0.55) brightness(0.76);
}

.state-risk-locked .scene-device {
  filter: saturate(0.72) brightness(0.72);
}

.runtime-dashboard {
  display: grid;
  grid-template-columns: 38% 33.1% 28.9%;
  min-height: 0;
  background: var(--fd-edge-footer-background);
  border-top: 1px solid rgb(103 124 145 / 24%);
  opacity: 1;
  transform: translateY(0);
  transition:
    opacity 320ms ease,
    transform 320ms cubic-bezier(0.22, 1, 0.36, 1) 0ms;
  will-change: opacity, transform;
}

.runtime-dashboard > section {
  position: relative;
  min-width: 0;
  overflow: hidden;
}

.runtime-dashboard > section + section {
  border-left: 1px solid rgb(111 125 140 / 32%);
}

.runtime-dashboard .current-stage {
  padding: 18px 30px 12px;
}

.runtime-dashboard .next-action {
  padding: 22px 42px 18px;
}

.runtime-dashboard .media-summary {
  padding: 18px 48px 16px;
}

.runtime-dashboard h2 {
  margin: 0;
  font-size: clamp(16px, 1.05cqw, 18px);
  font-weight: 400;
  line-height: 1.25;
  color: #8f99a5;
  letter-spacing: 0.03em;
}

.connected-count {
  display: flex;
  flex-direction: column;
  align-items: flex-end;
  flex: 1 1 0;
  min-width: 0;
  margin-top: 0;
  padding-top: 1px;
  line-height: 1;
  color: var(--fd-text-muted);
}

.connected-main {
  display: flex;
  align-items: baseline;
}

.connected-count strong {
  margin-left: 14px;
  font-size: clamp(36px, 2.75cqw, 44px);
  font-weight: 700;
  color: var(--fd-running);
}

.connected-count small {
  margin-left: 5px;
  font-size: 13px;
  color: #66717e;
}

.connected-count h2 {
  margin-top: 4px;
  font-size: clamp(18px, 1.35cqw, 23px);
  line-height: 1;
  color: #7f8b98;
}

.media-slots {
  display: flex;
  gap: clamp(10px, 0.95cqw, 16px);
  align-items: flex-end;
  min-height: 52px;
  margin: 14px 0 0;
}

.media-slots i {
  flex: 1;
  max-width: 18px;
  height: clamp(38px, 5.3cqh, 51px);
  background: linear-gradient(#9dadbd, #344b63);
  border: 1px solid #7891a7;
  box-shadow: inset 0 0 10px rgb(199 224 246 / 12%);
}

.media-slots i.running {
  background: linear-gradient(#66e0ff, #0075d7);
  box-shadow: 0 0 12px rgb(32 136 255 / 38%);
}

.media-slots i.completed {
  background: linear-gradient(#dce6ef, #8295a7);
}

.count-grid,
.current-metrics {
  display: flex;
  flex-wrap: wrap;
  gap: 8px 24px;
  margin: 0;
}

.count-grid {
  display: grid;
  grid-template-columns: repeat(3, max-content);
  gap: 15px 42px;
  margin-top: 27px;
  font-size: 15px;
}

.media-status-row {
  display: flex;
  gap: 18px;
  align-items: center;
  margin-top: 27px;
}

.media-status-row .count-grid {
  flex: 2 1 0;
  margin-top: 0;
}

.count-grid div {
  display: flex;
  gap: 8px;
  align-items: center;
}

.count-grid div::before {
  width: 9px;
  height: 9px;
  content: '';
  background: #1cbbff;
  border-radius: 50%;
  box-shadow: 0 0 8px rgb(28 187 255 / 30%);
}

.count-grid div:nth-child(2)::before {
  background: #476c91;
}

.count-grid div:nth-child(3)::before {
  background: #9eadba;
}

.count-grid .danger::before {
  background: var(--fd-danger);
}

.count-grid .warning::before {
  background: var(--fd-warning);
}

dt {
  color: var(--fd-text-muted);
}

dd {
  margin: 0;
  color: var(--fd-text-secondary);
}

.warning dd {
  color: var(--fd-warning);
}

.danger dd {
  color: var(--fd-danger);
}

.stage-name {
  display: block;
  margin: 0;
  font-size: clamp(24px, 2.1cqw, 34px);
  font-weight: 400;
  line-height: 1.18;
  color: #0c8cf5;
  letter-spacing: 0.04em;
}

.stage-heading {
  margin-top: 8px;
}

.batch-id {
  display: block;
  margin-top: 4px;
  font-size: clamp(14px, 0.95cqw, 16px);
  line-height: 1.25;
  color: var(--fd-text-muted);
}

progress {
  display: block;
  width: 100%;
  height: 6px;
  margin: 0;
  overflow: hidden;
  appearance: none;
  border: 0;
  border-radius: 999px;
}

progress::-webkit-progress-bar {
  background: #26313d;
}

progress::-webkit-progress-value {
  background: linear-gradient(90deg, #087ee5, #58dcff);
  box-shadow: 0 0 12px #168fff;
}

.progress-value {
  font-size: 16px;
  line-height: 1;
  color: #9aa2ac;
}

.progress-row {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  gap: 14px;
  align-items: center;
  margin-top: 10px;
}

.indeterminate-progress {
  padding: 10px 12px;
  margin: 18px 0;
  color: var(--fd-warning);
  background: rgb(255 177 74 / 7%);
  border-left: 2px solid var(--fd-warning);
}

.current-metrics {
  display: grid;
  grid-template-columns: 1.12fr 1fr 0.88fr;
  gap: 0;
  margin-top: 13px;
  line-height: 1.35;
}

.current-metrics div {
  display: flex;
  flex-direction: column;
  gap: 5px;
  padding: 0 16px;
  white-space: nowrap;
  border-left: 1px solid rgb(112 130 149 / 30%);
}

.current-metrics div:first-child {
  padding-left: 0;
  border-left: 0;
}

.current-metrics dd {
  font-size: clamp(16px, 1.08cqw, 18px);
  color: #1cbbff;
}

.current-metrics dt {
  font-size: 13px;
  color: #707b88;
}

.empty-current {
  max-width: 330px;
  padding: 18px;
  color: var(--fd-text-secondary);
  border: 1px dashed var(--fd-line);
}

.source-export-summary {
  display: flex;
  gap: 0;
  margin: 9px 0 0;
  overflow: hidden;
  background: rgb(21 40 57 / 44%);
  border: 1px solid rgb(105 152 185 / 24%);
}

.source-export-summary > div {
  display: grid;
  grid-template-columns: auto auto auto;
  gap: 6px;
  align-items: baseline;
  justify-content: center;
  min-width: 0;
  flex: 1 1 0;
  padding: 7px 12px;
}

.source-export-summary > div + div {
  border-left: 1px solid rgb(105 152 185 / 24%);
}

.source-export-summary dt,
.source-export-summary small,
.source-export-unavailable {
  color: var(--fd-text-secondary);
  font-size: 12px;
}

.source-export-summary dd {
  margin: 0;
  color: #d9efff;
  font-size: 20px;
  font-weight: 600;
  line-height: 1.1;
}

.source-export-unavailable {
  margin: 12px 0 0;
}

.next-device {
  margin-top: 19px;
}

.next-device > span {
  display: flex;
  gap: 14px;
  align-items: center;
  font-size: clamp(18px, 1.35cqw, 23px);
  color: #e3b559;
}

.next-device > span i {
  display: inline-block;
  flex: 0 0 12px;
  width: 12px;
  height: 12px;
  margin-right: 10px;
  background: #ffad00;
  border-radius: 50%;
  box-shadow: 0 0 14px rgb(255 173 0 / 45%);
}

.next-device p {
  margin: 12px 0 0 36px;
  font-size: clamp(15px, 1.05cqw, 18px);
  color: #c49a49;
}

.next-command {
  padding-top: 15px;
  margin-top: 22px;
  border-top: 1px solid rgb(255 173 24 / 15%);
}

.next-command small {
  display: block;
  font-size: 12px;
  color: #697480;
  letter-spacing: 0.12em;
}

.next-command strong {
  display: block;
  margin-top: 7px;
  font-size: clamp(17px, 1.25cqw, 21px);
  font-weight: 400;
  color: var(--fd-warning);
  white-space: nowrap;
}

.next-command strong b {
  margin-left: 10px;
  font-weight: 400;
}

.next-action > small {
  display: block;
  margin-top: 8px;
  color: var(--fd-denied);
}

.priority-none .next-command strong {
  color: var(--fd-success);
}

.priority-danger .next-command strong {
  color: var(--fd-danger);
}

@media (max-width: 1279px) {
  .runtime-dashboard {
    grid-template-columns: 37% 36% 27%;
  }

  .runtime-dashboard > section {
    padding: 20px 24px;
  }

  .count-grid {
    gap: 5px 14px;
    margin-top: 22px;
  }

  .current-metrics {
    font-size: 12px;
  }

  .current-metrics div {
    gap: 5px;
    padding: 0 10px;
  }

  .progress-value {
    right: 24px;
  }

  .next-action strong {
    white-space: normal;
  }
}

@media (prefers-reduced-motion: reduce) {
  .runtime-view,
  .runtime-stage::after,
  .scene-device,
  .device-detail-layer,
  .runtime-dashboard {
    transition-delay: 0ms !important;
    transition-duration: 0.01ms !important;
  }
}
</style>
