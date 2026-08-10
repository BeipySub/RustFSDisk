<!-- B-08-registration-validation · frozen 1672×941 baseline fixture -->
<script setup lang="ts">
import type { StepProps } from 'ant-design-vue';

import { computed, ref } from 'vue';
import { useRoute } from 'vue-router';

import { Button, Steps, Tag } from 'ant-design-vue';

import ControlStageShell from './control-stage-shell.vue';

const route = useRoute();
const notice = ref('');
const siteId = computed(() => String(route.params.siteId || 'factory-a-007'));
const displayName = computed(() =>
  siteId.value === 'factory-a-007' ? '子工厂 A-007' : siteId.value,
);
const progressItems: StepProps[] = [
  { description: '已完成', status: 'finish', title: '注册包已生成' },
  { description: '当前步骤', status: 'process', title: '等待 A 导入' },
  { description: '待处理', status: 'wait', title: '接收证书请求' },
  { description: '待处理', status: 'wait', title: '签发站点证书' },
  { description: '待处理', status: 'wait', title: 'mTLS 连接验证' },
];

function explainNoDownload() {
  notice.value = '冻结基线夹具未连接注册服务，没有生成或下载文件。';
}
</script>

<template>
  <ControlStageShell
    active-tab="factories"
    admin-enabled
    baseline-key="B-08-registration-validation"
    close-label="关闭注册验证并返回入库总览"
  >
    <article
      class="registration-page"
      aria-labelledby="registration-title"
      data-required-role="CONTROL_ADMIN"
    >
      <p class="breadcrumb">
        中控配置&nbsp;&nbsp;/&nbsp;&nbsp;子工厂管理&nbsp;&nbsp;/&nbsp;&nbsp;A-007&nbsp;&nbsp;/&nbsp;&nbsp;注册
      </p>
      <header>
        <h1 id="registration-title">注册包与连接验证</h1>
        <p>一次性配对 · 双向证书 · 只读采集</p>
      </header>

      <ol class="registration-steps" aria-label="注册流程">
        <li class="done"><b>1</b><strong>基本信息</strong></li>
        <li class="active"><b>2</b><strong>注册与证书</strong></li>
        <li><b>3</b><strong>连接验证</strong></li>
      </ol>

      <div class="registration-grid">
        <section class="package-card" aria-labelledby="package-title">
          <h2 id="package-title">
            <span aria-hidden="true">⇩</span>一次性注册包
          </h2>
          <dl>
            <div>
              <dt>工厂</dt>
              <dd>{{ displayName }}</dd>
            </div>
            <div>
              <dt>站点 ID</dt>
              <dd>{{ siteId }}</dd>
            </div>
            <div>
              <dt>注册包 ID</dt>
              <dd>REG-A007-20260722</dd>
            </div>
            <div>
              <dt>有效期</dt>
              <dd>24 小时</dd>
            </div>
            <div>
              <dt>使用限制</dt>
              <dd>仅可导入一次</dd>
            </div>
            <div>
              <dt>允许中控</dt>
              <dd>B-CONTROL</dd>
            </div>
          </dl>
          <Button block ghost type="primary" @click="explainNoDownload">
            ↓&nbsp;&nbsp;下载注册包
          </Button>
          <p v-if="notice" class="fixture-notice" role="status">{{ notice }}</p>
          <p class="package-note">
            <span aria-hidden="true">i</span>
            注册包不包含 A 私钥或业务凭据
          </p>
        </section>

        <div class="verification-column">
          <section class="progress-card" aria-labelledby="progress-title">
            <h2 id="progress-title">注册进度</h2>
            <Steps
              class="registration-progress"
              direction="vertical"
              :items="progressItems"
              size="small"
            />
          </section>

          <section class="connection-card" aria-labelledby="connection-title">
            <h2 id="connection-title">
              <span aria-hidden="true">◎</span>连接验证
            </h2>
            <dl>
              <div>
                <dt>管理网地址</dt>
                <dd>https://a-007.mgmt.local:9443</dd>
              </div>
              <div>
                <dt>站点证书</dt>
                <dd>等待签发</dd>
              </div>
              <div>
                <dt>最近测试</dt>
                <dd>尚未执行</dd>
              </div>
            </dl>
            <div class="connection-actions">
              <Button disabled>⌁&nbsp;测试连接</Button>
              <span><i aria-hidden="true">i</i>A 完成注册后才能验证连接</span>
            </div>
          </section>
        </div>
      </div>

      <div class="registration-actions">
        <RouterLink to="/control/settings/factories/new">
          <Button>←&nbsp;&nbsp;返回基本信息</Button>
        </RouterLink>
        <Button disabled>▣&nbsp;&nbsp;完成后启用采集</Button>
        <Tag color="processing">当前：等待 A 导入</Tag>
      </div>

      <footer>
        <span aria-hidden="true">◇</span>
        证书、信任和测试结果全程审计；页面不显示私钥、凭据或注册令牌正文。
      </footer>
    </article>
  </ControlStageShell>
</template>

<style scoped>
.registration-page {
  width: 1070px;
  padding: 20px 0 0;
}

.breadcrumb {
  margin: 0 0 20px;
  color: #99a3ad;
}

.registration-page > header h1 {
  margin: 0;
  font-size: 36px;
  font-weight: 500;
  color: #edf2f6;
}

.registration-page > header p {
  margin: 8px 0 0;
  font-size: 17px;
  color: #9da7b1;
}

.registration-steps {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  width: 820px;
  padding: 0;
  margin: 27px 0 20px 80px;
  list-style: none;
}

.registration-steps li {
  position: relative;
  display: flex;
  gap: 13px;
  align-items: center;
  color: #8e98a3;
}

.registration-steps li:not(:last-child)::after {
  position: absolute;
  top: 19px;
  right: 15px;
  width: 150px;
  height: 2px;
  content: '';
  background: #555d65;
}

.registration-steps .done:not(:last-child)::after {
  background: #47bb31;
}

.registration-steps b {
  display: grid;
  place-items: center;
  width: 38px;
  height: 38px;
  font-size: 19px;
  font-weight: 400;
  border: 2px solid #747c85;
  border-radius: 50%;
}

.registration-steps .done {
  color: #6ad551;
}

.registration-steps .done b {
  background: #2c952b;
  border-color: #6ad551;
}

.registration-steps .active {
  color: #1bb8ff;
}

.registration-steps .active b {
  color: #fff;
  background: #168fe3;
  border-color: #31baff;
}

.registration-grid {
  display: grid;
  grid-template-columns: 488px 536px;
  gap: 16px;
}

.package-card,
.progress-card,
.connection-card {
  background: rgb(7 15 23 / 74%);
  border: 1px solid rgb(94 119 143 / 48%);
  border-radius: 4px;
}

.package-card {
  height: 485px;
  padding: 25px 27px;
}

.package-card h2,
.progress-card h2,
.connection-card h2 {
  margin: 0;
  font-size: 21px;
  font-weight: 500;
  color: #edf2f6;
}

.package-card h2,
.connection-card h2 {
  display: flex;
  gap: 14px;
  align-items: center;
}

.package-card h2 span,
.connection-card h2 span {
  color: #4aaeff;
}

.package-card dl {
  margin: 18px 0;
}

.package-card dl div {
  display: grid;
  grid-template-columns: 175px 1fr;
  align-items: center;
  min-height: 48px;
}

dt {
  color: #929ca7;
}

dd {
  margin: 0;
  color: #e0e6eb;
}

.package-card :deep(.ant-btn-primary) {
  height: 48px;
  color: #4dbfff;
  border-color: #168ed5;
}

.fixture-notice {
  margin: 8px 0 0;
  color: var(--fd-warning);
}

.package-note {
  display: flex;
  gap: 11px;
  align-items: center;
  margin: 18px 0 0;
  color: #9ba5ae;
}

.package-note span {
  display: grid;
  place-items: center;
  width: 18px;
  height: 18px;
  color: #0c131a;
  background: #909ba6;
  border-radius: 50%;
}

.verification-column {
  display: grid;
  grid-template-rows: 275px 194px;
  gap: 16px;
}

.progress-card,
.connection-card {
  padding: 20px 24px;
}

.registration-progress {
  height: 210px;
  margin-top: 12px;
}

.registration-progress :deep(.ant-steps-item) {
  min-height: 40px;
}

.registration-progress :deep(.ant-steps-item-title) {
  width: 355px;
  color: #dde4e9 !important;
}

.registration-progress :deep(.ant-steps-item-description) {
  position: absolute;
  right: 0;
  color: #8f9aa5 !important;
}

.connection-card dl {
  margin: 15px 0 10px;
}

.connection-card dl div {
  display: grid;
  grid-template-columns: 160px 1fr;
  min-height: 29px;
}

.connection-actions {
  display: flex;
  gap: 18px;
  align-items: center;
}

.connection-actions :deep(.ant-btn) {
  width: 250px;
  color: #6d7781;
  background: rgb(3 8 13 / 72%);
  border-color: #495361;
}

.connection-actions span {
  font-size: 12px;
  color: #88939e;
}

.registration-actions {
  display: flex;
  gap: 16px;
  align-items: center;
  margin-top: 22px;
}

.registration-actions :deep(.ant-btn) {
  min-width: 206px;
  height: 48px;
  color: #dce3e8;
  background: rgb(3 8 13 / 78%);
  border-color: #465667;
}

.registration-actions :deep(.ant-btn[disabled]) {
  color: #68727e;
  background: rgb(53 59 68 / 55%);
}

.registration-actions :deep(.ant-tag) {
  padding: 4px 10px;
  margin-left: auto;
  background: transparent;
}

.registration-page > footer {
  display: flex;
  gap: 12px;
  align-items: center;
  margin-top: 19px;
  color: #9aa5af;
}
</style>
