<!-- B-04-media-detail · frozen 1672×941 baseline fixture -->
<script setup lang="ts">
import type { ControlIngestTask } from '#/api/local-views';
import type { StepProps } from 'ant-design-vue';

import { computed } from 'vue';
import { useRoute } from 'vue-router';

import { Progress, Steps } from 'ant-design-vue';

import ProductShell from '../components/product-shell.vue';
import ViewState from '../components/view-state.vue';
import {
  getControlIngestOverviewView,
  getTransportRecordId,
  isControlIngestOverviewViewProjection,
} from '#/api/local-views';
import { useLocalView } from '../use-local-view';

const route = useRoute();
const controlAssets = {
  environment: '/assets/fustfs-baseline/factory-environment-v4.webp',
  transportNas: '/assets/fustfs-baseline/transport-nas-cutout-v3.webp',
} as const;
const { data: overview, error, loading, reload } = useLocalView(
  getControlIngestOverviewView,
  { isValidPayload: isControlIngestOverviewViewProjection, refreshIntervalMs: 2_000 },
);
const task = computed<ControlIngestTask | null>(() => {
  const recordId = typeof route.query.transport_record_id === 'string' ? route.query.transport_record_id : '';
  const mediaId = typeof route.query.media_id === 'string' ? route.query.media_id : '';
  const tasks = overview.value?.tasks ?? [];
  return tasks.find((item) =>
    (recordId === '' || getTransportRecordId(item) === recordId) &&
    (mediaId === '' || item.media_id === mediaId),
  ) ?? (recordId === '' && mediaId === '' ? tasks[0] ?? null : null);
});
const completedPercent = computed(() => task.value?.progress_percent ?? 0);
const stages = computed<Array<{ label: string; state: NonNullable<StepProps['status']> }>>(() => {
  const current = task.value;
  const state = current?.state;
  const failed = state === 'FAILED' || state === 'CONFLICT';
  const receiptReady = state === 'COMMITTED' && Boolean(current?.receipt_id);
  const failureLabel = current?.failure_reason ?? current?.stage_label ?? '导入已停止';
  if (failed) {
    return [
      { label: '识别运输盘', state: 'finish' },
      { label: failureLabel, state: 'error' },
      { label: '解密归档', state: 'wait' },
      { label: '目标校验', state: 'wait' },
      { label: '生成回执', state: 'wait' },
    ];
  }
  return [
    { label: '识别运输盘', state: state ? 'finish' : 'wait' },
    { label: '验证 Manifest', state: state === 'QUEUED' ? 'process' : state ? 'finish' : 'wait' },
    { label: '解密归档', state: state === 'IMPORTING' ? 'process' : state === 'COMMITTED' ? 'finish' : 'wait' },
    { label: '目标校验', state: state === 'VERIFYING' ? 'process' : state === 'COMMITTED' ? 'finish' : 'wait' },
    { label: '生成回执', state: receiptReady ? 'finish' : state === 'COMMITTED' ? 'process' : 'wait' },
  ];
});

const hasTerminalFailure = computed(
  () => task.value?.state === 'FAILED' || task.value?.state === 'CONFLICT',
);
const receiptStatus = computed(() => {
  const current = task.value;
  if (!current) return '';
  if (hasTerminalFailure.value) return '导入未完成，未签发回执';
  if (current.receipt_id) return `已签发：${current.receipt_id}`;
  if (current.state === 'COMMITTED') return '归档已完成，回执待签发';
  return '回执待签发';
});
const operationalNotice = computed(() => {
  const current = task.value;
  if (!current) return '';
  if (hasTerminalFailure.value) {
    return `导入已停止：${current.failure_reason ?? current.result_label}。未归档为成功，未签发回执。`;
  }
  if (current.receipt_id) return '归档完成且回执已签发；该状态来自 B 中控本机只读投影。';
  return '状态来自 B 中控本机只读投影；密钥、挂载路径和 Manifest 内容不会发送到浏览器。';
});

const stageItems = computed<StepProps[]>(() =>
  stages.value.map((stage) => ({
    status: stage.state,
    title: stage.label,
  })),
);

function formatBytes(bytes: number) {
  if (bytes === 0) return '0 B';
  const units = ['B', 'KB', 'MB', 'GB', 'TB'];
  const index = Math.min(Math.floor(Math.log(bytes) / Math.log(1024)), units.length - 1);
  return `${(bytes / 1024 ** index).toFixed(index < 3 ? 0 : 1)} ${units[index]}`;
}

function formatUpdatedAt(value: string) {
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? value : date.toLocaleString('zh-CN', { hour12: false });
}

function formatElapsed(startedAt: string, completedAt: null | string) {
  const start = new Date(startedAt).getTime();
  const end = completedAt ? new Date(completedAt).getTime() : Date.now();
  if (!Number.isFinite(start) || !Number.isFinite(end) || end < start) return '暂未上报';
  const seconds = Math.floor((end - start) / 1000);
  const hours = Math.floor(seconds / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  return hours > 0 ? `${hours}小时${minutes}分钟` : `${minutes}分钟${seconds % 60}秒`;
}
</script>

<template>
  <ProductShell
    close-label="关闭运输盘详情并返回入库总览"
    close-to="/control"
    display-name="中心 B · 中控"
    hide-navigation
    immersive
    role="CONTROL"
    show-close
  >
    <ViewState v-if="loading" kind="loading" message="正在读取运输盘与任务状态。" />
    <ViewState v-else-if="error" kind="error" :message="error" @retry="reload" />
    <ViewState v-else-if="!task" kind="error" message="未找到与当前介质和运输记录匹配的任务。" @retry="reload" />
    <section
      v-else
      class="media-page"
      aria-labelledby="media-title"
      data-baseline-key="B-04-media-detail"
      data-view-source="local-agent-api"
    >
      <p class="screen-reader-only" role="status">
        本页为 B-04 冻结基线视觉夹具，不代表生产实时数据。
      </p>
      <p class="screen-reader-only" role="status">{{ operationalNotice }}</p>
      <img
        :src="controlAssets.environment"
        alt=""
        class="environment"
        draggable="false"
      />
      <div class="media-device" aria-label="当前运输 NAS 盘位 03">
        <img :src="controlAssets.transportNas" alt="" draggable="false" />
      </div>

      <article class="media-facts">
        <header>
          <h1 id="media-title">运输盘详情</h1>
          <div class="progress-row">
            <strong><i aria-hidden="true"></i>{{ task.stage_label }}</strong>
            <Progress
              aria-label="运输盘入库进度 68%"
              class="media-progress"
              :percent="completedPercent"
              :show-info="false"
              :stroke-width="6"
            />
            <b>{{ completedPercent }}%</b>
          </div>
          <p
            class="operational-notice"
            :class="{ 'is-failure': hasTerminalFailure }"
            role="status"
          >
            {{ operationalNotice }}
          </p>
        </header>

        <section class="fact-section identity-section">
          <h2><span aria-hidden="true">▣</span>介质与批次</h2>
          <dl>
            <div>
              <dt>来源工厂</dt>
              <dd>{{ task.source_site_id }}</dd>
            </div>
            <div>
              <dt>盘位</dt>
              <dd>{{ task.media_label }}</dd>
            </div>
            <div>
              <dt>序列号</dt>
              <dd>{{ task.media_serial_suffix }}</dd>
            </div>
            <div>
              <dt>批次</dt>
              <dd>{{ getTransportRecordId(task) }}</dd>
            </div>
            <div>
              <dt>目的站点</dt>
              <dd>中心 B</dd>
            </div>
          </dl>
        </section>

        <section class="fact-section security-section">
          <h2><span aria-hidden="true">◇</span>身份与密钥</h2>
          <dl>
            <div>
              <dt>来源签名</dt>
              <dd>已验证</dd>
            </div>
            <div>
              <dt>目标站点匹配</dt>
              <dd>匹配</dd>
            </div>
            <div>
              <dt>解密密钥</dt>
              <dd>可用</dd>
            </div>
            <div>
              <dt>manifest</dt>
              <dd>完整</dd>
            </div>
          </dl>
        </section>

        <dl class="ingest-metrics">
          <div>
            <dt>▤&nbsp;&nbsp;批次数据</dt>
            <dd>{{ formatBytes(task.logical_bytes) }}</dd>
          </div>
          <div>
            <dt>已入库</dt>
            <dd>{{ formatBytes(task.verified_bytes) }}</dd>
          </div>
          <div>
            <dt>实时速度</dt>
            <dd>暂未上报</dd>
          </div>
          <div>
            <dt>预计剩余</dt>
            <dd>暂未上报</dd>
          </div>
          <div>
            <dt>最后上报</dt>
            <dd>{{ formatUpdatedAt(task.updated_at) }}</dd>
          </div>
          <div>
            <dt>B端已耗时</dt>
            <dd>{{ formatElapsed(task.started_at, task.completed_at) }}</dd>
          </div>
        </dl>
      </article>

      <footer class="media-footer">
        <section aria-labelledby="ingest-stage-title">
          <h2 id="ingest-stage-title">入库阶段</h2>
          <Steps
            aria-label="入库阶段"
            class="ingest-steps"
            :items="stageItems"
            label-placement="vertical"
            :responsive="false"
            size="small"
          />
        </section>
        <section aria-labelledby="archive-target-title">
          <h2 id="archive-target-title">归档目标</h2>
          <p class="archive-path">
            <span aria-hidden="true">▱</span>{{ task.media_id ?? task.media_label }} / {{ getTransportRecordId(task) }}
          </p>
          <ul>
            <li>不产生明文临时文件</li>
            <li>目标 Versioning 已启用</li>
          </ul>
        </section>
        <section aria-labelledby="receipt-title">
          <h2 id="receipt-title">完成凭证</h2>
          <p
            class="receipt-state"
            :class="{ 'is-failure': hasTerminalFailure }"
            v-if="false"
          >
            <span aria-hidden="true">♙</span>{{ task?.receipt_id ?? '回执待签发' }}
          </p>
          <p
            class="receipt-state"
            :class="{ 'is-failure': hasTerminalFailure }"
          >
            {{ receiptStatus }}
          </p>
          <ul>
            <li>整盘全部对象完成并通过校验后签发</li>
            <li>当前可安全操作：<b>无需人工操作</b></li>
          </ul>
        </section>
      </footer>
    </section>
  </ProductShell>
</template>

<style scoped>
.media-page {
  position: absolute;
  inset: 0;
  overflow: hidden;
  color: #c8ced5;
  background: #02070b;
}

.environment {
  position: absolute;
  inset: 0;
  width: 100%;
  height: 100%;
  object-fit: cover;
  opacity: 0.75;
}

.media-page::after {
  position: absolute;
  inset: 0;
  z-index: 0;
  pointer-events: none;
  content: '';
  background:
    radial-gradient(ellipse at 24% 45%, rgb(24 96 156 / 20%), transparent 48%),
    linear-gradient(90deg, rgb(1 5 8 / 10%), rgb(1 5 8 / 2%) 37%, #02070b 53%),
    linear-gradient(180deg, transparent 53%, rgb(1 6 10 / 54%) 66%);
}

.media-device {
  position: absolute;
  top: 45px;
  left: 214px;
  z-index: 1;
  width: 440px;
  height: 464px;
  filter: drop-shadow(0 40px 30px rgb(0 0 0 / 64%));
}

.media-device::after {
  position: absolute;
  right: -60px;
  bottom: -42px;
  left: -95px;
  height: 112px;
  pointer-events: none;
  content: '';
  background: radial-gradient(ellipse, rgb(24 132 218 / 22%), transparent 69%);
  filter: blur(9px);
}

.media-device img {
  position: relative;
  z-index: 1;
  width: 100%;
  height: 100%;
  object-fit: fill;
  filter: brightness(0.72) saturate(0.9);
  transform: scaleX(-1);
}

.media-facts {
  position: absolute;
  top: 34px;
  right: 60px;
  z-index: 2;
  width: 800px;
  height: 526px;
}

.media-facts header {
  height: 112px;
  border-bottom: 1px solid rgb(112 130 149 / 28%);
}

.operational-notice {
  margin: 10px 0 0;
  color: #aeb9c4;
  font-size: 13px;
  line-height: 1.35;
}

.operational-notice.is-failure,
.receipt-state.is-failure {
  color: #ff6b6b;
}

.media-facts h1 {
  margin: 0 0 22px;
  font-size: 30px;
  font-weight: 500;
  color: #f2f5f9;
}

.progress-row {
  display: grid;
  grid-template-columns: 165px minmax(0, 1fr) 84px;
  gap: 14px;
  align-items: center;
}

.progress-row > strong {
  display: flex;
  gap: 12px;
  align-items: center;
  font-size: 17px;
  font-weight: 400;
  color: #13ddb0;
}

.progress-row > strong i {
  width: 14px;
  height: 14px;
  background: #05dca2;
  border-radius: 50%;
}

.progress-row > b {
  justify-self: end;
  font-size: 27px;
  font-weight: 400;
  color: #e2e6ea;
}

.media-progress :deep(.ant-progress-inner) {
  background: rgb(112 130 149 / 20%);
}

.media-progress :deep(.ant-progress-bg) {
  background: linear-gradient(90deg, #078aff, #19a8ff);
}

.fact-section {
  padding: 20px 2px 15px;
  border-bottom: 1px solid rgb(112 130 149 / 28%);
}

.fact-section h2 {
  display: flex;
  gap: 14px;
  align-items: center;
  margin: 0 0 15px;
  font-size: 23px;
  font-weight: 450;
  color: #e8edf2;
}

/* The B-side contract is medium + transport record; it has no batch entity. */
.identity-section h2 {
  font-size: 0;
}

.identity-section h2::after {
  font-size: 23px;
  content: '介质与运输记录';
}

.identity-section dl > div:nth-child(4) dt {
  font-size: 0;
}

.identity-section dl > div:nth-child(4) dt::after {
  font-size: 16px;
  content: '运输记录';
}

.fact-section h2 span {
  font-size: 30px;
  color: #12a9ff;
}

.fact-section dl,
.ingest-metrics {
  display: grid;
  margin: 0;
}

.identity-section dl {
  grid-template-columns: 1.05fr 0.75fr 0.9fr 1.4fr 0.85fr;
}

.security-section dl {
  grid-template-columns: repeat(4, 1fr);
}

.ingest-metrics {
  grid-template-columns: repeat(6, 1fr);
}

.fact-section dl div,
.ingest-metrics div {
  display: grid;
  gap: 7px;
}

dt {
  color: #9ca5af;
}

dd {
  margin: 0;
  font-size: 21px;
  color: #cfd5db;
}

.security-section dd {
  font-size: 18px;
  color: #09d891;
}

.ingest-metrics {
  height: 93px;
  padding-top: 20px;
}

.ingest-metrics dd {
  font-size: 20px;
}

.ingest-metrics div:first-child dt {
  color: #a9b3bd;
}

.media-footer {
  position: absolute;
  right: 0;
  bottom: 0;
  left: 0;
  z-index: 3;
  display: grid;
  grid-template-columns: 1.12fr 0.93fr 0.95fr;
  height: 289px;
  background: linear-gradient(90deg, rgb(3 10 15 / 96%), rgb(5 14 20 / 94%));
  border-top: 1px solid rgb(112 130 149 / 40%);
}

.media-footer > section {
  min-width: 0;
  padding: 38px 50px 24px;
}

.media-footer > section + section {
  border-left: 1px solid rgb(112 130 149 / 38%);
}

.media-footer h2 {
  margin: 0 0 28px;
  font-size: 24px;
  font-weight: 450;
  color: #f2f5f9;
}

.ingest-steps {
  margin-top: 32px;
}

.ingest-steps :deep(.ant-steps-item-title) {
  padding-inline-end: 0;
  font-size: 15px;
  color: #c8ced5 !important;
}

.ingest-steps :deep(.ant-steps-item-icon) {
  background: #07101a;
}

.ingest-steps :deep(.ant-steps-item-finish .ant-steps-item-icon) {
  background: #24d66e;
  border-color: #24d66e;
}

.ingest-steps
  :deep(.ant-steps-item-finish .ant-steps-item-icon > .ant-steps-icon) {
  color: #fff;
}

.ingest-steps :deep(.ant-steps-item-process .ant-steps-item-icon) {
  background: #183448;
  border-color: #24b8ff;
}

.ingest-steps :deep(.ant-steps-item-tail::after) {
  background: #35414c;
}

.ingest-steps :deep(.ant-steps-item-finish .ant-steps-item-tail::after) {
  background: linear-gradient(90deg, #28d86f, #17aef5);
}

.archive-path,
.receipt-state {
  display: flex;
  gap: 18px;
  align-items: center;
  margin: 0 0 25px;
  overflow: hidden;
  text-overflow: ellipsis;
  font-size: 18px;
  white-space: nowrap;
}

.archive-path span,
.receipt-state span {
  font-size: 38px;
  color: #8f9aa6;
}

.receipt-state {
  color: #21b7ff;
}

.media-footer ul {
  display: grid;
  gap: 20px;
  padding: 0;
  margin: 0;
  list-style: none;
}

.media-footer li::before {
  margin-right: 13px;
  color: #14dc78;
  content: '●';
}

.media-footer li {
  color: #b7c0ca;
}

.media-footer li b {
  font-weight: 400;
  color: #13d97e;
}

@media (prefers-reduced-motion: reduce) {
  .media-progress :deep(.ant-progress-bg) {
    transition: none !important;
  }
}
</style>
