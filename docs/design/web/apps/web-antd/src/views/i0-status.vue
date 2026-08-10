<script lang="ts" setup>
import { Alert, Card, Tag } from 'ant-design-vue';
</script>

<template>
  <main class="i0-shell">
    <Card class="i0-card" :bordered="false">
      <div class="i0-heading">
        <div>
          <p class="eyebrow">FustFsDisk</p>
          <h1>实施工程基线</h1>
        </div>
        <Tag color="blue">I0</Tag>
      </div>
      <Alert
        description="当前仅完成 Vue/Vben 构建基线。产品页面将在页面契约确认后依据冻结基线图实现。"
        message="工程已建立，业务页面尚未启用"
        show-icon
        type="info"
      />
    </Card>
  </main>
</template>

<style scoped>
.i0-shell {
  display: grid;
  place-items: center;
  min-height: 100vh;
  padding: 32px;
  background: #0a1020;
}

.i0-card {
  width: min(680px, 100%);
}

.i0-heading {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  margin-bottom: 24px;
}

.eyebrow {
  margin: 0 0 6px;
  font-size: 12px;
  font-weight: 700;
  color: #1677ff;
  text-transform: uppercase;
  letter-spacing: 0.14em;
}

h1 {
  margin: 0;
  font-size: 28px;
}

@media (prefers-reduced-motion: reduce) {
  *,
  *::before,
  *::after {
    scroll-behavior: auto !important;
    transition-duration: 0.01ms !important;
  }
}
</style>
