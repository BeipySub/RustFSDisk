<!-- B-05-conflict-lock · frozen 1672×941 baseline fixture -->
<script setup lang="ts">
import type { StepProps } from 'ant-design-vue';

import { computed, ref } from 'vue';

import { Button, Steps } from 'ant-design-vue';

import ProductShell from '../components/product-shell.vue';
import {
  conflictQueue,
  conflictSummary,
  conflictTimeline,
  controlAssets,
} from './ops-fixtures';

const selectedKey = ref(conflictQueue[0]?.batchId ?? '');
const exportNotice = ref('');
const selected = computed(
  () =>
    conflictQueue.find((item) => item.batchId === selectedKey.value) ??
    conflictQueue[0],
);
const selectedIsConflict = computed(() => selected.value?.state === 'CONFLICT');
const timelineItems = computed<StepProps[]>(() =>
  conflictTimeline.map((stage) => ({
    description: stage.at,
    status: stage.status,
    title: stage.label,
  })),
);

function selectQueueItem(batchId: string) {
  selectedKey.value = batchId;
  exportNotice.value = '';
}

function queueStateClass(state: string) {
  return `state-${state.toLowerCase().replaceAll('_', '-')}`;
}

function explainFixtureExport() {
  exportNotice.value = '冻结基线夹具未连接诊断导出服务，未生成文件。';
}
</script>

<template>
  <ProductShell
    close-label="关闭冲突页并返回入库总览"
    close-to="/control"
    display-name="中心 B · 中控"
    hide-navigation
    immersive
    role="CONTROL"
    show-close
  >
    <section
      class="conflict-page"
      aria-labelledby="conflict-title"
      data-baseline-key="B-05-conflict-lock"
      data-view-source="frozen-baseline-fixture"
    >
      <p class="screen-reader-only" role="status">
        本页为 B-05 冻结基线视觉夹具，不代表生产实时数据。
      </p>
      <header class="conflict-heading">
        <span class="lock-mark" aria-hidden="true">▣</span>
        <div>
          <h1 id="conflict-title">冲突锁定</h1>
          <p>系统已阻止目标覆盖，等待受控核对</p>
        </div>
      </header>

      <section class="conflict-summary" aria-label="当前冲突摘要">
        <div class="danger">
          <span aria-hidden="true">!</span>
          <p>
            目标冲突<strong>{{ conflictSummary.conflictCount }}</strong>
          </p>
        </div>
        <div class="warning">
          <span aria-hidden="true">⌕</span>
          <p>
            等待密钥<strong>{{ conflictSummary.waitingKeys }}</strong>
          </p>
        </div>
        <div class="success">
          <span aria-hidden="true">▶</span>
          <p>
            其他对象继续<strong>{{ conflictSummary.continuingObjects }}</strong>
          </p>
        </div>
        <div class="danger receipt-blocked">
          <span aria-hidden="true">▤</span>
          <p>receipt <strong>暂不签发</strong></p>
        </div>
      </section>

      <div class="conflict-workspace">
        <aside class="queue-panel" aria-labelledby="queue-title">
          <h2 id="queue-title">锁定项</h2>
          <button
            v-for="item in conflictQueue"
            :key="item.batchId"
            :aria-pressed="selectedKey === item.batchId"
            class="queue-item"
            :class="[
              queueStateClass(item.state),
              { selected: selectedKey === item.batchId },
            ]"
            type="button"
            @click="selectQueueItem(item.batchId)"
          >
            <span class="queue-icon" aria-hidden="true">
              {{ item.state === 'CONFLICT' ? '!' : '⌕' }}
            </span>
            <strong>{{ item.label }}</strong>
            <small>{{ item.batchId }}</small>
            <b>{{ item.reason }}</b>
          </button>
        </aside>

        <article class="conflict-detail" aria-labelledby="detail-title">
          <h2 id="detail-title">
            {{ selectedIsConflict ? '冲突详情' : '等待密钥' }}
          </h2>
          <dl class="conflict-facts">
            <div>
              <dt>来源</dt>
              <dd>
                {{
                  selectedIsConflict
                    ? 'A-006 / batch A-20260720-021'
                    : 'A-002 / 盘位 07'
                }}
              </dd>
            </div>
            <div>
              <dt>目标</dt>
              <dd>
                {{
                  selectedIsConflict
                    ? 'fustfs-archive / A-006 / 2026-07-20 / …'
                    : '受控密钥服务'
                }}
              </dd>
            </div>
            <div>
              <dt>现有对象</dt>
              <dd>{{ selectedIsConflict ? '已存在' : '未进入读取' }}</dd>
            </div>
            <div>
              <dt>摘要比较</dt>
              <dd>{{ selectedIsConflict ? 'SHA-256 不同' : '等待密钥' }}</dd>
            </div>
            <div>
              <dt>锁定范围</dt>
              <dd>
                {{
                  selectedIsConflict
                    ? '当前对象与归档地址'
                    : '当前介质的解密阶段'
                }}
              </dd>
            </div>
            <div>
              <dt>其他对象</dt>
              <dd class="success">继续入库</dd>
            </div>
          </dl>

          <div class="conflict-visual" aria-hidden="true">
            <img :src="controlAssets.rack" alt="" draggable="false" />
            <span class="shield-lock">▣</span>
          </div>

          <aside
            class="lock-banner"
            :class="{ waiting: !selectedIsConflict }"
            role="status"
          >
            <span aria-hidden="true">{{ selectedIsConflict ? '▣' : '⌕' }}</span>
            <div>
              <strong>
                {{
                  selectedIsConflict
                    ? '禁止覆盖 · 对象保持锁定'
                    : '等待受控密钥 · 保持介质现场'
                }}
              </strong>
              <p>
                {{
                  selectedIsConflict
                    ? '整盘暂不签发完成 receipt'
                    : '密钥就绪前不进入解密或目标写入'
                }}
              </p>
            </div>
          </aside>

          <Steps
            v-if="selectedIsConflict"
            aria-label="冲突锁定阶段"
            class="conflict-steps"
            :items="timelineItems"
            label-placement="vertical"
            :responsive="false"
            size="small"
          />
          <p v-else class="waiting-explanation">
            已完成介质身份识别；解密密钥不可得时保持静止，不伪造处理进度。
          </p>

          <footer class="detail-action">
            <div>
              <h3>唯一建议动作</h3>
              <p>
                {{
                  selectedIsConflict
                    ? '联系数据负责人核对来源与现有目标内容。'
                    : '联系受控密钥负责人核对介质密钥状态。'
                }}
              </p>
              <small aria-live="polite">{{ exportNotice }}</small>
            </div>
            <Button class="diagnostic-button" @click="explainFixtureExport">
              导出诊断信息
            </Button>
            <Button
              class="locked-button"
              disabled
              title="普通人员不得解除系统锁定"
            >
              保持锁定
            </Button>
          </footer>
        </article>
      </div>
    </section>
  </ProductShell>
</template>

<style scoped>
.conflict-page {
  position: absolute;
  inset: 0;
  padding: 25px 28px 45px;
  overflow: hidden;
  color: #c8ced5;
  background:
    radial-gradient(circle at 85% 52%, rgb(14 65 108 / 15%), transparent 34%),
    linear-gradient(145deg, #03070b, #08121d 65%, #07101a);
}

.conflict-heading {
  display: flex;
  gap: 18px;
  align-items: center;
  height: 84px;
}

.lock-mark {
  display: grid;
  place-items: center;
  width: 56px;
  height: 56px;
  font-size: 26px;
  color: #ff4d61;
  background: rgb(255 77 97 / 10%);
  border: 1px solid #ff4d61;
  border-radius: 50%;
}

.conflict-heading h1 {
  margin: 0;
  font-size: 32px;
  font-weight: 550;
  color: #f2f5f9;
}

.conflict-heading p {
  margin: 4px 0 0;
  color: #9ca5af;
}

.conflict-summary {
  display: grid;
  grid-template-columns: repeat(4, 1fr);
  height: 92px;
  border: 1px solid rgb(91 123 155 / 35%);
  border-radius: 12px;
}

.conflict-summary > div {
  position: relative;
  display: flex;
  gap: 18px;
  align-items: center;
  padding: 0 60px;
}

.conflict-summary > div + div::before {
  position: absolute;
  top: 24px;
  bottom: 24px;
  left: 0;
  width: 1px;
  content: '';
  background: rgb(91 123 155 / 38%);
}

.conflict-summary span {
  display: grid;
  place-items: center;
  width: 49px;
  height: 49px;
  font-size: 26px;
  color: currentcolor;
  background: rgb(255 255 255 / 3%);
  border: 1px solid currentcolor;
  border-radius: 50%;
}

.conflict-summary p {
  display: grid;
  gap: 2px;
  margin: 0;
  color: #aeb7c0;
  white-space: nowrap;
}

.conflict-summary strong {
  font-size: 31px;
  font-weight: 500;
  color: currentcolor;
}

.danger {
  color: #ff4d61;
}

.warning {
  color: #ffad18;
}

.success {
  color: #09d891 !important;
}

.receipt-blocked {
  padding-left: 64px !important;
}

.receipt-blocked p {
  display: block;
  font-size: 18px;
  color: #ff4d61;
}

.receipt-blocked strong {
  font-size: 21px;
}

.conflict-workspace {
  display: grid;
  grid-template-columns: 497px minmax(0, 1fr);
  gap: 14px;
  height: 612px;
  margin-top: 13px;
}

.queue-panel,
.conflict-detail {
  min-width: 0;
  background: rgb(7 16 26 / 58%);
  border: 1px solid rgb(91 123 155 / 36%);
  border-radius: 12px;
}

.queue-panel {
  padding: 15px 18px;
}

.queue-panel h2,
.conflict-detail h2 {
  margin: 0 0 14px;
  font-size: 22px;
  font-weight: 550;
  color: #f2f5f9;
}

.queue-item {
  position: relative;
  display: grid;
  grid-template-columns: 44px 1fr;
  gap: 3px 14px;
  width: 100%;
  min-height: 110px;
  padding: 17px 16px;
  color: #b9c1ca;
  text-align: left;
  cursor: pointer;
  background: rgb(11 20 31 / 68%);
  border: 1px solid rgb(91 123 155 / 48%);
  border-left: 5px solid #516276;
  border-radius: 10px;
}

.queue-item + .queue-item {
  margin-top: 18px;
}

.queue-item.selected.state-conflict {
  background: linear-gradient(110deg, rgb(78 20 31 / 62%), rgb(35 22 31 / 45%));
  border: 1px solid #ff4d61;
  border-left: 5px solid #ff4d61;
}

.queue-item:focus-visible {
  outline: 2px solid #58dcff;
  outline-offset: 3px;
}

.queue-icon {
  display: grid;
  grid-row: 1 / 4;
  place-items: center;
  align-self: start;
  width: 34px;
  height: 34px;
  font-size: 21px;
  color: #ff4d61;
  border: 1px solid currentcolor;
  border-radius: 50%;
}

.state-waiting-key .queue-icon {
  color: #ffad18;
  border: 0;
}

.queue-item strong {
  font-size: 21px;
  font-weight: 500;
  color: #e4e9ee;
}

.queue-item small {
  font-size: 15px;
  color: #a6aeb8;
}

.queue-item b {
  font-weight: 500;
  color: #ff4d61;
}

.state-waiting-key b {
  color: #7f8b97;
}

.conflict-detail {
  position: relative;
  padding: 16px 20px;
  overflow: hidden;
}

.conflict-facts {
  display: grid;
  grid-template-columns: 124px 1fr;
  gap: 12px 0;
  width: 620px;
  margin: 0;
}

.conflict-facts div {
  display: contents;
}

.conflict-facts dt {
  color: #8f99a5;
}

.conflict-facts dd {
  margin: 0;
  color: #cbd2d9;
}

.conflict-visual {
  position: absolute;
  top: 29px;
  right: 44px;
  width: 310px;
  height: 235px;
  background:
    linear-gradient(rgb(255 77 97 / 12%) 1px, transparent 1px),
    linear-gradient(90deg, rgb(255 77 97 / 12%) 1px, transparent 1px);
  background-size: 44px 34px;
  mask-image: radial-gradient(ellipse, #000, transparent 76%);
}

.conflict-visual img {
  position: absolute;
  right: 62px;
  bottom: 0;
  width: 145px;
  height: 205px;
  object-fit: contain;
  filter: drop-shadow(0 20px 22px rgb(0 0 0 / 68%));
}

.shield-lock {
  position: absolute;
  right: 8px;
  bottom: 8px;
  display: grid;
  place-items: center;
  width: 106px;
  height: 126px;
  font-size: 42px;
  color: #ff7180;
  background: linear-gradient(
    145deg,
    rgb(255 77 97 / 38%),
    rgb(80 14 26 / 82%)
  );
  border: 2px solid #ff4d61;
  border-radius: 52% 52% 40% 40%;
  box-shadow: 0 0 28px rgb(255 77 97 / 22%);
}

.lock-banner {
  display: grid;
  grid-template-columns: 80px 1fr;
  gap: 20px;
  align-items: center;
  height: 104px;
  padding: 10px 24px;
  margin-top: 18px;
  background: linear-gradient(90deg, rgb(75 18 28 / 44%), rgb(50 25 34 / 62%));
  border: 1px solid #9c3644;
  border-radius: 10px;
}

.lock-banner > span {
  display: grid;
  place-items: center;
  width: 68px;
  height: 68px;
  font-size: 32px;
  color: #ff4d61;
  border: 1px solid #ff4d61;
  border-radius: 50%;
}

.lock-banner strong {
  font-size: 24px;
  color: #ff5769;
}

.lock-banner p {
  margin: 4px 0 0;
  color: #a9a4aa;
}

.lock-banner.waiting {
  background: rgb(98 64 10 / 20%);
  border-color: rgb(255 173 24 / 50%);
}

.lock-banner.waiting > span,
.lock-banner.waiting strong {
  color: #ffad18;
  border-color: #ffad18;
}

.conflict-steps {
  height: 91px;
  padding: 8px 34px 0;
  margin-top: 10px;
}

.conflict-steps :deep(.ant-steps-item-title) {
  font-size: 14px;
  color: #c8ced5 !important;
}

.conflict-steps :deep(.ant-steps-item-description) {
  font-size: 12px;
  color: #818c98 !important;
}

.conflict-steps :deep(.ant-steps-item-finish .ant-steps-item-icon) {
  background: #09d891;
  border-color: #09d891;
}

.conflict-steps
  :deep(.ant-steps-item-finish .ant-steps-item-icon > .ant-steps-icon) {
  color: #fff;
}

.conflict-steps :deep(.ant-steps-item-error .ant-steps-item-icon) {
  background: rgb(255 77 97 / 18%);
  border-color: #ff4d61;
}

.conflict-steps
  :deep(.ant-steps-item-error .ant-steps-item-icon > .ant-steps-icon) {
  color: #ff4d61;
}

.conflict-steps :deep(.ant-steps-item-tail::after) {
  background: linear-gradient(90deg, #09d891 56%, #ff4d61 56%);
}

.waiting-explanation {
  display: grid;
  place-items: center;
  height: 91px;
  margin: 10px 0 0;
  color: #ffbd45;
  border-top: 1px solid rgb(91 123 155 / 25%);
}

.detail-action {
  position: absolute;
  right: 20px;
  bottom: 16px;
  left: 20px;
  display: grid;
  grid-template-columns: 1fr 228px 228px;
  gap: 18px;
  align-items: center;
  height: 97px;
  padding-top: 12px;
  border-top: 1px solid rgb(91 123 155 / 27%);
}

.detail-action h3 {
  margin: 0 0 6px;
  font-size: 20px;
  color: #eef2f6;
}

.detail-action p {
  margin: 0;
  color: #9ca5af;
}

.detail-action small {
  display: block;
  min-height: 17px;
  margin-top: 4px;
  color: #ffad18;
}

.diagnostic-button,
.locked-button {
  height: 55px;
  font-size: 16px;
  color: #cbd5df;
  background: rgb(18 31 45 / 82%);
  border-color: rgb(116 145 175 / 55%);
}

.locked-button:disabled {
  color: #5c6672;
  background: rgb(45 54 66 / 58%);
  border-color: rgb(91 105 122 / 28%);
}

@media (prefers-reduced-motion: reduce) {
  .conflict-page * {
    transition: none !important;
  }
}
</style>
