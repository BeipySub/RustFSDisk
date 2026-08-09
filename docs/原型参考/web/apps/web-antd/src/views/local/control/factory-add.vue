<!-- B-08-add-factory · frozen 1672×941 baseline fixture -->
<script setup lang="ts">
import { reactive, ref } from 'vue';

import { Button, Form, FormItem, Input, Select } from 'ant-design-vue';

import ControlStageShell from './control-stage-shell.vue';

const form = reactive({
  collectionMode: 'SCHEDULED_ON_DEMAND',
  displayName: '子工厂 A-007',
  endpoint: 'https://a-007.mgmt.local:9443',
  expiryHours: '36',
  siteId: 'factory-a-007',
  window: '01:30 — 04:30',
});
const notice = ref('');
const collectionOptions = [
  { label: '每日错峰 + 按需', value: 'SCHEDULED_ON_DEMAND' },
  { label: '仅每日错峰', value: 'SCHEDULED' },
  { label: '仅按需', value: 'ON_DEMAND' },
];
const expiryOptions = [
  { label: '24 小时', value: '24' },
  { label: '36 小时', value: '36' },
  { label: '48 小时', value: '48' },
];

function explainFixtureOnly() {
  notice.value = '冻结基线夹具未连接管理员注册服务，没有生成或下载注册包。';
}
</script>

<template>
  <ControlStageShell
    active-tab="factories"
    admin-enabled
    baseline-key="B-08-add-factory"
    close-label="关闭新增子工厂并返回入库总览"
  >
    <article
      class="factory-add-page"
      aria-labelledby="factory-add-title"
      data-required-role="CONTROL_ADMIN"
    >
      <p class="breadcrumb">
        中控配置&nbsp;&nbsp;/&nbsp;&nbsp;子工厂管理&nbsp;&nbsp;/&nbsp;&nbsp;新增
      </p>
      <header class="add-heading">
        <span aria-hidden="true">◇</span>
        <div>
          <h1 id="factory-add-title">新增子工厂</h1>
          <p>登记站点并生成一次性注册包</p>
        </div>
      </header>

      <ol class="registration-steps" aria-label="注册流程">
        <li class="active"><b>1</b><strong>基本信息</strong></li>
        <li><b>2</b><strong>注册与证书</strong></li>
        <li><b>3</b><strong>连接验证</strong></li>
      </ol>

      <div class="add-workspace">
        <Form
          class="factory-form"
          :label-col="{ span: 6 }"
          :model="form"
          :wrapper-col="{ span: 18 }"
          @finish="explainFixtureOnly"
        >
          <h2>基本信息</h2>
          <FormItem
            label="工厂显示名称"
            name="displayName"
            :rules="[{ required: true, message: '请输入 1～64 个字符' }]"
          >
            <Input v-model:value="form.displayName" :maxlength="64" />
          </FormItem>
          <FormItem
            label="站点 ID"
            name="siteId"
            :rules="[
              {
                pattern: /^[a-z0-9][a-z0-9-]{2,62}$/,
                required: true,
                message: '站点 ID 格式无效或已存在',
              },
            ]"
          >
            <Input v-model:value="form.siteId" />
            <small>创建后不可修改</small>
          </FormItem>
          <FormItem
            label="管理网地址"
            name="endpoint"
            :rules="[
              {
                pattern: /^https:\/\/[^@\s/]+(?::\d+)?(?:\/.*)?$/,
                required: true,
                message: '请输入可解析的 HTTPS 管理地址',
              },
            ]"
          >
            <Input v-model:value="form.endpoint" />
          </FormItem>
          <FormItem label="采集方式" name="collectionMode" required>
            <Select
              v-model:value="form.collectionMode"
              :options="collectionOptions"
            />
          </FormItem>
          <FormItem
            label="采集窗口"
            name="window"
            :rules="[{ required: true, message: '请选择未冲突的采集窗口' }]"
          >
            <Input v-model:value="form.window" />
          </FormItem>
          <FormItem label="数据过期阈值" name="expiryHours" required>
            <Select v-model:value="form.expiryHours" :options="expiryOptions" />
          </FormItem>

          <aside class="key-note" role="note">
            <span aria-hidden="true">◇</span>
            站点私钥将在 A 本机生成，不进入注册包。
          </aside>

          <div class="form-actions">
            <RouterLink to="/control/settings/factories">
              <Button>取消</Button>
            </RouterLink>
            <Button html-type="submit" type="primary">生成注册包</Button>
          </div>
          <p v-if="notice" class="fixture-notice" role="status">{{ notice }}</p>
        </Form>

        <aside class="capability-card">
          <h2><span aria-hidden="true">◇</span>注册后能力</h2>
          <dl>
            <div>
              <dt>站点角色</dt>
              <dd>EDGE 子工厂</dd>
            </div>
            <div>
              <dt>远程接口</dt>
              <dd>只读状态快照</dd>
            </div>
            <div>
              <dt>业务数据</dt>
              <dd>仍通过运输盘离线摆渡</dd>
            </div>
            <div>
              <dt>中心权限</dt>
              <dd>不允许远程写入或设备操作</dd>
            </div>
          </dl>
        </aside>
      </div>
    </article>
  </ControlStageShell>
</template>

<style scoped>
.factory-add-page {
  width: 1170px;
  padding: 21px 0 0 6px;
}

.breadcrumb {
  margin: 0 0 23px;
  color: #99a3ad;
}

.add-heading {
  display: flex;
  gap: 22px;
  align-items: center;
}

.add-heading > span {
  display: grid;
  place-items: center;
  width: 60px;
  height: 68px;
  font-size: 32px;
  color: #44baff;
  border: 2px solid #1399e4;
  clip-path: polygon(50% 0, 92% 21%, 92% 72%, 50% 100%, 8% 72%, 8% 21%);
}

.add-heading h1 {
  margin: 0;
  font-size: 38px;
  font-weight: 500;
  color: #eff3f7;
}

.add-heading p {
  margin: 6px 0 0;
  color: #a0a9b3;
}

.registration-steps {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  width: 840px;
  padding: 0;
  margin: 28px 0 22px 78px;
  list-style: none;
}

.registration-steps li {
  position: relative;
  display: flex;
  gap: 14px;
  align-items: center;
  color: #8e98a3;
}

.registration-steps li:not(:last-child)::after {
  position: absolute;
  top: 21px;
  right: 20px;
  width: 120px;
  height: 1px;
  content: '';
  background: #5d6268;
}

.registration-steps b {
  display: grid;
  place-items: center;
  width: 42px;
  height: 42px;
  font-size: 20px;
  font-weight: 400;
  border: 2px solid #777f87;
  border-radius: 50%;
}

.registration-steps .active {
  color: #1fc7ff;
}

.registration-steps .active b {
  border-color: #1fc7ff;
  box-shadow: 0 0 15px rgb(31 199 255 / 22%);
}

.add-workspace {
  display: grid;
  grid-template-columns: 740px 375px;
  gap: 20px;
}

.factory-form {
  position: relative;
  padding: 18px 0 0;
  border-top: 1px solid rgb(106 122 139 / 36%);
}

.factory-form h2 {
  margin: 0 0 22px;
  font-size: 21px;
  font-weight: 500;
}

.factory-form :deep(.ant-form-item) {
  margin-bottom: 17px;
}

.factory-form :deep(.ant-form-item-label > label) {
  height: 43px;
  font-size: 16px;
  color: #d2dae1;
}

.factory-form :deep(.ant-form-item-required::before) {
  color: #ff5b62 !important;
}

.factory-form :deep(.ant-input),
.factory-form :deep(.ant-select-selector) {
  min-height: 43px;
  color: #e2e7eb;
  background: rgb(2 7 12 / 78%);
  border-color: #35485a;
}

.factory-form :deep(.ant-select-selection-item) {
  line-height: 41px;
  color: #e2e7eb;
}

.factory-form small {
  position: absolute;
  top: 11px;
  right: -125px;
  color: #7f8994;
}

.key-note {
  display: flex;
  gap: 14px;
  align-items: center;
  height: 50px;
  padding: 0 15px;
  color: #aab4bd;
  border: 1px solid rgb(92 116 139 / 42%);
}

.form-actions {
  display: flex;
  gap: 16px;
  justify-content: flex-end;
  padding-top: 18px;
  margin-top: 14px;
  border-top: 1px solid rgb(106 122 139 / 36%);
}

.form-actions :deep(.ant-btn) {
  min-width: 160px;
  height: 48px;
}

.form-actions :deep(.ant-btn-default) {
  color: #e0e5ea;
  background: rgb(3 8 13 / 72%);
  border-color: #3c4d5f;
}

.form-actions :deep(.ant-btn-primary) {
  background: linear-gradient(#1aaaf0, #078bd5);
  border-color: #30bfff;
}

.fixture-notice {
  margin: 10px 0 0;
  color: var(--fd-warning);
  text-align: right;
}

.capability-card {
  height: 452px;
  padding: 25px 26px;
  background: rgb(7 15 23 / 74%);
  border: 1px solid rgb(97 120 143 / 48%);
}

.capability-card h2 {
  display: flex;
  gap: 15px;
  align-items: center;
  margin: 0 0 25px;
  font-size: 21px;
  font-weight: 500;
}

.capability-card h2 span {
  color: #25b6ff;
}

.capability-card dl {
  margin: 0;
}

.capability-card dl div {
  display: grid;
  grid-template-columns: 145px 1fr;
  align-items: center;
  min-height: 68px;
  border-bottom: 1px solid rgb(86 108 130 / 34%);
}

.capability-card dt {
  color: #929ca6;
}

.capability-card dd {
  margin: 0;
  color: #d8dfe5;
}
</style>
