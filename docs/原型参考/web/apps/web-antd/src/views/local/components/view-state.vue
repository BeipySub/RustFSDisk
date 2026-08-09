<!--
  @file view-state.vue
  @description 本机视图通用加载与失败关闭状态，统一提供可访问提示和重试入口。
  @usage 用于 EDGE、CONTROL 页面数据尚未就绪或本机数据源不可用的场景。
  @scope 不承载或伪造业务数据。
-->
<script setup lang="ts">
defineProps<{
  kind: 'error' | 'loading';
  message: string;
}>();

defineEmits<{ retry: [] }>();
</script>

<template>
  <section
    class="view-state"
    :aria-live="kind === 'loading' ? 'polite' : 'assertive'"
    :role="kind === 'error' ? 'alert' : 'status'"
  >
    <div
      aria-hidden="true"
      class="state-orbit"
      :class="{ still: kind === 'error' }"
    ></div>
    <p class="eyebrow">
      {{ kind === 'loading' ? 'LOCAL VIEW' : 'FAIL CLOSED' }}
    </p>
    <h1>{{ kind === 'loading' ? '正在读取本机状态' : '本机视图不可用' }}</h1>
    <p>{{ message }}</p>
    <button v-if="kind === 'error'" type="button" @click="$emit('retry')">
      重新读取
    </button>
  </section>
</template>

<style scoped>
.view-state {
  display: grid;
  place-items: center;
  min-height: 70vh;
  padding: 48px;
  color: var(--fd-text-secondary);
  text-align: center;
}

.view-state h1 {
  margin: 5px 0 10px;
  font-size: clamp(26px, 4vw, 42px);
  color: var(--fd-text-primary);
}

.view-state p {
  max-width: 660px;
}

.view-state button {
  min-height: 42px;
  padding: 0 22px;
  margin-top: 18px;
  color: #dff6ff;
  cursor: pointer;
  background: #087dd0;
  border: 1px solid #58dcff;
  border-radius: 6px;
}

.state-orbit {
  width: 56px;
  height: 56px;
  border: 2px solid rgb(88 220 255 / 18%);
  border-top-color: #58dcff;
  border-radius: 50%;
  animation: orbit 1.1s linear infinite;
}

.state-orbit.still {
  border-color: #ff4d61;
  animation: none;
}

@keyframes orbit {
  to {
    transform: rotate(360deg);
  }
}
</style>
