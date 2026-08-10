<!-- A-07-registration：EDGE 首次注册向导。当前 I4 仅展示已验证装包状态，写操作失败关闭。 -->
<script setup lang="ts">
import { computed, ref } from 'vue';

import {
  exportIsolatedTrialRegistrationRequest,
  getIsolatedTrialRegistrationView,
  importIsolatedTrialRegistrationResponse,
} from '#/api/local-views';

import ProductShell from '../components/product-shell.vue';
import ViewState from '../components/view-state.vue';
import { formatTimestamp } from '../model';
import { useLocalView } from '../use-local-view';

import { isIsolatedReadOnlyRegistration } from './registration-state';

const { data, error, loading, reload } = useLocalView(
  getIsolatedTrialRegistrationView,
);
const actionError = ref('');
const actionPending = ref(false);
const requestExported = ref(false);

const isolatedReadOnlyAvailable = computed(() =>
  isIsolatedReadOnlyRegistration(data.value),
);

const currentStep = computed(() => {
  if (data.value?.phase === 'COMPLETE') return 4;
  if (data.value?.phase === 'CERTIFICATE') return 3;
  if (data.value?.phase === 'CONFIRM') return 2;
  return 1;
});

const packageTone = computed(() => {
  if (!data.value) return 'muted';
  return data.value.package.state === 'VALID' ? 'success' : 'danger';
});

const canOperateIsolatedTrial = computed(
  () =>
    isolatedReadOnlyAvailable.value &&
    data.value?.phase !== 'COMPLETE' &&
    !actionPending.value,
);

const showImportResponse = computed(
  () => requestExported.value || data.value?.phase === 'CERTIFICATE',
);

async function exportRegistrationRequest() {
  if (!canOperateIsolatedTrial.value) return;
  actionPending.value = true;
  actionError.value = '';
  try {
    await exportIsolatedTrialRegistrationRequest();
    requestExported.value = true;
    await reload();
  } catch {
    actionError.value =
      '导出请求未完成。请确认本机 Agent 仍处于隔离试运行模式后重试。';
  } finally {
    actionPending.value = false;
  }
}

async function importRegistrationResponse() {
  if (!canOperateIsolatedTrial.value || !showImportResponse.value) return;
  actionPending.value = true;
  actionError.value = '';
  try {
    await importIsolatedTrialRegistrationResponse();
    await reload();
  } catch {
    actionError.value =
      '导入响应未完成。请由 B 端操作员完成签发并放入固定交换目录后重试。';
  } finally {
    actionPending.value = false;
  }
}
</script>

<template>
  <ProductShell
    baseline-canvas
    :display-name="data?.package.site_display_name ?? 'EDGE 首次注册'"
    hide-navigation
    role="EDGE"
  >
    <ViewState
      v-if="loading"
      kind="loading"
      message="正在验证离线注册包、站点身份和可信管控端。"
    />
    <ViewState
      v-else-if="error || !data || !isolatedReadOnlyAvailable"
      kind="error"
      :message="
        error ||
        '本机 Agent 未明确启用隔离试运行只读注册视图；页面保持失败关闭。'
      "
      @retry="reload"
    />
    <section v-else aria-labelledby="registration-title" class="registration">
      <header class="registration-heading">
        <p>EDGE 站点初始化</p>
        <h1 id="registration-title">首次注册</h1>
        <span>使用已签名离线注册包建立本机身份</span>
      </header>

      <ol class="registration-steps" aria-label="注册进度">
        <li :class="{ active: currentStep >= 1 }">
          <i>1</i><strong>导入注册包</strong><span>校验签名与有效期</span>
        </li>
        <li :class="{ active: currentStep >= 2 }">
          <i>2</i><strong>确认站点信息</strong><span>绑定唯一站点身份</span>
        </li>
        <li :class="{ active: currentStep >= 3 }">
          <i>3</i><strong>生成本机证书</strong><span>建立设备密钥边界</span>
        </li>
        <li :class="{ active: currentStep >= 4 }">
          <i>4</i><strong>完成注册</strong><span>进入运行首页</span>
        </li>
      </ol>

      <div class="registration-body">
        <article class="package-card">
          <header>
            <div>
              <span>离线注册包</span>
              <strong>{{ data.package.package_id }}</strong>
            </div>
            <em :class="`tone-${packageTone}`">
              <i aria-hidden="true"></i>
              {{ data.package.signature_valid ? '签名有效' : '签名无效' }}
            </em>
          </header>
          <dl>
            <div>
              <dt>站点名称</dt>
              <dd>{{ data.package.site_display_name }}</dd>
            </div>
            <div>
              <dt>站点 ID</dt>
              <dd>{{ data.package.site_id }}</dd>
            </div>
            <div>
              <dt>安装角色</dt>
              <dd>EDGE 边缘站点</dd>
            </div>
            <div>
              <dt>可信管控端</dt>
              <dd>{{ data.package.control_label }}</dd>
            </div>
            <div>
              <dt>注册包有效期</dt>
              <dd>{{ formatTimestamp(data.package.expires_at) }}</dd>
            </div>
            <div>
              <dt>包状态</dt>
              <dd :class="`tone-${packageTone}`">
                {{
                  data.package.state === 'VALID'
                    ? '可用于本机注册'
                    : data.package.state
                }}
              </dd>
            </div>
          </dl>
        </article>

        <aside class="trust-card">
          <h2>注册前检查</h2>
          <ul>
            <li
              v-for="capability in data.capabilities"
              :key="capability.label"
              :class="{
                danger:
                  capability.state === 'ERROR' ||
                  capability.state === 'PERMISSION_DENIED',
                warning:
                  capability.state === 'UNKNOWN' ||
                  capability.state === 'WARNING',
              }"
            >
              <i aria-hidden="true">
                {{
                  capability.state === 'READY'
                    ? '✓'
                    : capability.state === 'ERROR' ||
                        capability.state === 'PERMISSION_DENIED'
                      ? '×'
                      : '!'
                }}
              </i>
              <span>
                <strong>{{ capability.label }}</strong>
                {{ capability.detail }}
              </span>
            </li>
          </ul>
          <p>
            注册包必须与本机预期站点一致。签名、有效期或站点身份任一校验失败时，
            系统不会生成本机证书。
          </p>
        </aside>
      </div>

      <footer class="registration-actions">
        <span>
          <i aria-hidden="true">⌾</i>
          注册过程不会连接公网，也不会导出本机私钥
        </span>
        <RouterLink
        v-if="data.phase === 'COMPLETE' && isolatedReadOnlyAvailable"
          class="primary-action"
          to="/edge"
        >
          进入运行首页
        </RouterLink>
        <button
          v-else-if="!showImportResponse"
          aria-describedby="registration-action-status"
          :disabled="!canOperateIsolatedTrial"
          type="button"
          @click="exportRegistrationRequest"
        >
          生成本机证书
        </button>
        <button
          v-else
          :disabled="!canOperateIsolatedTrial"
          type="button"
          @click="importRegistrationResponse"
        >
          {{ actionPending ? '正在导入响应…' : '导入已签发响应' }}
        </button>
      </footer>
      <p
        v-if="data.phase !== 'COMPLETE'"
        id="registration-action-status"
        class="action-status"
        role="status"
      >
        当前仅显示由本机 Agent 明确启用的隔离试运行只读状态；安全写入接口装配前不会执行证书生成。
      </p>
    </section>
  </ProductShell>
</template>

<style scoped>
.registration {
  position: relative;
  width: 100%;
  height: 100%;
  padding: 52px 94px 26px;
  overflow: hidden;
  color: #d9e0e7;
  background:
    radial-gradient(circle at 52% 62%, rgb(16 105 159 / 13%), transparent 41%),
    linear-gradient(180deg, rgb(2 8 13 / 20%), rgb(2 8 13 / 82%));
}

.registration::before {
  position: absolute;
  right: 0;
  bottom: -80px;
  left: 0;
  height: 330px;
  pointer-events: none;
  content: '';
  background:
    linear-gradient(rgb(29 95 132 / 11%) 1px, transparent 1px),
    linear-gradient(90deg, rgb(29 95 132 / 11%) 1px, transparent 1px);
  background-size: 48px 48px;
  mask-image: linear-gradient(transparent, black);
  transform: perspective(450px) rotateX(63deg);
  transform-origin: bottom;
}

.registration-heading {
  position: relative;
  z-index: 1;
  text-align: center;
}

.registration-heading p {
  margin: 0 0 8px;
  font-size: 14px;
  color: #159fe9;
  letter-spacing: 0.24em;
}

.registration-heading h1 {
  margin: 0;
  font-size: 32px;
  font-weight: 500;
  letter-spacing: 0.06em;
}

.registration-heading > span {
  display: block;
  margin-top: 7px;
  color: #7f8b97;
}

.registration-steps {
  position: relative;
  z-index: 1;
  display: grid;
  grid-template-columns: repeat(4, 1fr);
  width: 940px;
  padding: 0;
  margin: 30px auto 27px;
  list-style: none;
}

.registration-steps::before {
  position: absolute;
  top: 17px;
  right: 11%;
  left: 11%;
  height: 1px;
  content: '';
  background: #263847;
}

.registration-steps li {
  z-index: 1;
  display: grid;
  justify-items: center;
  color: #62707c;
}

.registration-steps i {
  display: grid;
  place-items: center;
  width: 35px;
  height: 35px;
  font-style: normal;
  background: #08131c;
  border: 1px solid #344756;
  border-radius: 50%;
}

.registration-steps strong {
  margin-top: 9px;
  font-size: 15px;
  font-weight: 450;
}

.registration-steps span {
  margin-top: 2px;
  font-size: 12px;
}

.registration-steps li.active {
  color: #18a8f5;
}

.registration-steps li.active i {
  background: #092b41;
  border-color: #17a5ef;
  box-shadow: 0 0 16px rgb(15 162 239 / 22%);
}

.registration-body {
  position: relative;
  z-index: 1;
  display: grid;
  grid-template-columns: minmax(0, 1.45fr) minmax(330px, 0.75fr);
  gap: 20px;
  width: 1110px;
  margin: 0 auto;
}

.package-card,
.trust-card {
  min-height: 365px;
  padding: 25px 30px;
  background: linear-gradient(145deg, rgb(11 27 39 / 88%), rgb(4 12 18 / 91%));
  border: 1px solid rgb(87 123 151 / 30%);
  border-radius: 8px;
  box-shadow: 0 22px 70px rgb(0 0 0 / 25%);
}

.package-card > header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding-bottom: 20px;
  border-bottom: 1px solid rgb(91 124 149 / 25%);
}

.package-card header > div {
  display: grid;
  gap: 5px;
}

.package-card header span {
  color: #7f8c97;
}

.package-card header strong {
  font-size: 21px;
  font-weight: 450;
}

.package-card header em {
  padding: 7px 11px;
  font-style: normal;
  border: 1px solid currentcolor;
  border-radius: 5px;
}

.package-card header em i {
  display: inline-block;
  width: 8px;
  height: 8px;
  margin-right: 7px;
  background: currentcolor;
  border-radius: 50%;
  box-shadow: 0 0 10px currentcolor;
}

.package-card dl {
  display: grid;
  grid-template-columns: repeat(2, 1fr);
  gap: 0 44px;
  margin: 12px 0 0;
}

.package-card dl > div {
  min-height: 82px;
  padding: 15px 0;
  border-bottom: 1px solid rgb(91 124 149 / 18%);
}

.package-card dt {
  margin-bottom: 8px;
  color: #74818d;
}

.package-card dd {
  margin: 0;
  font-size: 16px;
  color: #c7cfd7;
}

.trust-card h2 {
  margin: 0 0 20px;
  font-size: 20px;
  font-weight: 500;
}

.trust-card ul {
  padding: 0;
  margin: 0;
  list-style: none;
}

.trust-card li {
  display: grid;
  grid-template-columns: 28px 1fr;
  gap: 11px;
  min-height: 70px;
  padding: 12px 0;
  color: var(--fd-success);
  border-bottom: 1px solid rgb(91 124 149 / 18%);
}

.trust-card li > i {
  display: grid;
  place-items: center;
  width: 24px;
  height: 24px;
  font-style: normal;
  border: 1px solid currentcolor;
  border-radius: 50%;
}

.trust-card li span {
  display: grid;
  color: #7f8b96;
}

.trust-card li strong {
  font-weight: 500;
  color: #c7cfd6;
}

.trust-card li.warning {
  color: var(--fd-warning);
}

.trust-card li.danger {
  color: var(--fd-danger);
}

.trust-card > p {
  margin: 21px 0 0;
  font-size: 13px;
  line-height: 1.7;
  color: #707d89;
}

.registration-actions {
  position: relative;
  z-index: 1;
  display: flex;
  align-items: center;
  justify-content: space-between;
  width: 1110px;
  margin: 17px auto 0;
}

.registration-actions > span {
  color: #72808b;
}

.registration-actions > span i {
  margin-right: 8px;
  font-style: normal;
  color: #1aa9f4;
}

.registration-actions button,
.primary-action {
  display: inline-grid;
  place-items: center;
  min-width: 184px;
  height: 44px;
  color: #d5e9f5;
  text-decoration: none;
  background: linear-gradient(90deg, #0b86d6, #19b8ee);
  border: 0;
  border-radius: 5px;
}

.registration-actions button:disabled {
  color: #7b8994;
  cursor: not-allowed;
  background: #152631;
  border: 1px solid #304655;
}

.action-status {
  position: relative;
  z-index: 1;
  width: 1110px;
  margin: 8px auto 0;
  font-size: 12px;
  color: #778591;
  text-align: right;
}

.tone-success {
  color: var(--fd-success) !important;
}

.tone-danger {
  color: var(--fd-danger) !important;
}

.tone-muted {
  color: #7c8791 !important;
}
</style>
