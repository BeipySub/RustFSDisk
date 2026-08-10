<!--
  @file status-chip.vue
  @description 本机视图语义状态标签，以统一形状和颜色表达正常、警告、危险与未知状态。
  @usage 供 CONTROL 列表和详情面板复用。
  @scope 只负责视觉表达，不推导或改变业务状态。
-->
<script setup lang="ts">
import type { SemanticTone } from '../model';

withDefaults(
  defineProps<{
    label: string;
    tone?: SemanticTone;
  }>(),
  { tone: 'muted' },
);
</script>

<template>
  <span class="status-chip" :class="`tone-${tone}`">
    <span aria-hidden="true" class="status-shape"></span>
    {{ label }}
  </span>
</template>

<style scoped>
.status-chip {
  display: inline-flex;
  gap: 8px;
  align-items: center;
  min-height: 28px;
  padding: 4px 10px;
  font-size: 13px;
  color: var(--fd-text-secondary);
  white-space: nowrap;
  background: rgb(255 255 255 / 4%);
  border: 1px solid var(--tone-border);
  border-radius: 999px;
}

.status-shape {
  width: 8px;
  height: 8px;
  background: var(--tone);
  border-radius: 2px;
  box-shadow: 0 0 12px color-mix(in srgb, var(--tone), transparent 30%);
}
</style>
