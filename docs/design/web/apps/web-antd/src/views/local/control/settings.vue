<!-- B-07-control-config · frozen 1672×941 baseline fixture -->
<script setup lang="ts">
import { Tag } from 'ant-design-vue';

import ControlStageShell from './control-stage-shell.vue';

withDefaults(defineProps<{ embedded?: boolean }>(), { embedded: false });

const sections = [
  {
    icon: '◇',
    rows: [
      ['站点角色', 'CONTROL 中控'],
      ['站点标识', 'B-CONTROL'],
      ['策略来源', '中心安装策略'],
      ['访问权限', '中心归档写入 · 子工厂只读采集'],
    ],
    title: '1. 中心身份',
  },
  {
    icon: '▱',
    rows: [
      ['归档规则', 'fustfs-archive / <source_site_id> / <batch_date> / …'],
      ['目标 Versioning', '已启用'],
      ['冲突策略', '禁止覆盖 · 对象锁定'],
      ['完成凭证', '整盘完成并校验后签发'],
    ],
    title: '2. 入库与归档',
  },
  {
    icon: '⌁',
    rows: [
      ['定时采集', '每日错峰 01:30–04:30'],
      ['按需采集', '仅管理员可触发异步任务'],
      ['快照策略', '保留最近完整快照并标记新鲜度'],
      ['远程边界', '只读状态接口'],
    ],
    title: '3. 状态采集',
  },
  {
    icon: '▣',
    rows: [
      ['双向认证', 'mTLS 已启用'],
      ['证书校验', '有效期 · 吊销 · 站点匹配'],
      ['解密密钥', '受托管 · 状态正常'],
      ['签名密钥', '独立用途 · 状态正常'],
    ],
    title: '4. 安全与密钥',
  },
] as const;
</script>

<template>
  <component
    :is="embedded ? 'section' : ControlStageShell"
    :class="{ 'settings-embedded-stage': embedded }"
    v-bind="
      embedded
        ? {
            'data-baseline-key': 'B-07-control-config',
            'data-view-source': 'frozen-baseline-fixture',
          }
        : {
            activeTab: 'settings',
            baselineKey: 'B-07-control-config',
            closeLabel: '关闭中控配置并返回入库总览',
          }
    "
  >
    <article class="settings-page" aria-labelledby="settings-title">
      <header class="settings-heading">
        <div>
          <h1 id="settings-title">中控配置</h1>
          <p>由中心安装策略托管 · 只读</p>
        </div>
        <div class="settings-status">
          <Tag color="success">中控策略已生效</Tag>
        </div>
      </header>

      <section
        v-for="section in sections"
        :key="section.title"
        class="settings-section"
      >
        <h2>
          <span aria-hidden="true">{{ section.icon }}</span>
          <span class="section-title">{{ section.title }}</span>
        </h2>
        <dl>
          <div v-for="row in section.rows" :key="row[0]">
            <dt>{{ row[0] }}</dt>
            <dd>{{ row[1] }}</dd>
          </div>
        </dl>
      </section>

      <footer>
        <span aria-hidden="true">◇</span>
        页面不显示 Endpoint、凭据、私钥或密钥材料；普通人员无需配置。
      </footer>
    </article>
  </component>
</template>

<style scoped>
.settings-embedded-stage {
  position: absolute;
  inset: 0;
  overflow: hidden;
  background:
    radial-gradient(circle at 73% 44%, rgb(26 109 180 / 18%), transparent 31%),
    url('/assets/fustfs-baseline/factory-environment-v4.webp') center / cover
      no-repeat,
    linear-gradient(135deg, #020407, #07111b 64%, #020507);
}

.settings-embedded-stage .settings-page {
  position: absolute;
  top: 58px;
  left: 0;
  padding-top: 0;
}

.settings-embedded-stage .settings-heading {
  margin-bottom: 0;
}

.settings-embedded-stage .settings-section {
  height: 145px;
  padding-top: 14px;
  padding-bottom: 14px;
}

.settings-embedded-stage .settings-section:first-of-type {
  padding-top: 18px;
}

.settings-embedded-stage .settings-section:first-of-type dl {
  transform: none;
}

.settings-embedded-stage .settings-page footer {
  padding-top: 12px;
}

.settings-page {
  width: 1030px;
  padding: 20px 0 0 72px;
}

.settings-heading {
  display: flex;
  align-items: flex-start;
  margin-bottom: 7px;
}

.settings-heading h1 {
  margin: 0;
  font-size: 38px;
  font-weight: 500;
  color: #edf2f7;
}

.settings-heading p {
  margin: 8px 0 0;
  font-size: 17px;
  color: #a1aab5;
}

.settings-status {
  position: fixed;
  top: 116px;
  right: 69px;
}

.settings-status :deep(.ant-tag) {
  display: flex;
  gap: 14px;
  align-items: center;
  padding: 0;
  margin: 0;
  font-size: 20px;
  color: #dce5eb;
  background: transparent;
  border: 0;
}

.settings-status :deep(.ant-tag)::after {
  width: 12px;
  height: 12px;
  content: '';
  background: #10d59a;
  border-radius: 50%;
  box-shadow: 0 0 14px rgb(16 213 154 / 42%);
}

.settings-section {
  box-sizing: border-box;
  display: grid;
  grid-template-columns: 295px 1fr;
  height: 166px;
  padding: 22px 0;
  border-bottom: 1px solid rgb(109 127 145 / 28%);
}

.settings-section:first-of-type {
  padding-top: 31px;
}

.settings-section:first-of-type dl {
  transform: translateY(-7px);
}

.settings-section h2 {
  display: flex;
  gap: 22px;
  align-items: flex-start;
  margin: 0;
  font-size: 22px;
  font-weight: 500;
  color: #e5ebf0;
}

.settings-section h2 > span:first-child {
  display: grid;
  place-items: center;
  width: 48px;
  height: 48px;
  font-size: 24px;
  color: #49bfff;
  background: rgb(16 110 174 / 16%);
  border: 1px solid #168ed1;
  clip-path: polygon(50% 0, 94% 22%, 94% 72%, 50% 100%, 6% 72%, 6% 22%);
}

.settings-section .section-title {
  padding-top: 9px;
}

.settings-section dl {
  display: grid;
  gap: 8px;
  margin: 0;
}

.settings-section dl div {
  display: grid;
  grid-template-columns: 197px 1fr;
  align-items: center;
  height: 25px;
  line-height: 25px;
}

.settings-section dt {
  font-size: 17px;
  color: #9fa9b3;
}

.settings-section dd {
  margin: 0;
  font-size: 18px;
  color: #e0e6eb;
}

.settings-page footer {
  display: flex;
  gap: 12px;
  align-items: center;
  padding-top: 18px;
  color: #9ca6b0;
}

.settings-page footer span {
  color: #b5c1cc;
}
</style>
