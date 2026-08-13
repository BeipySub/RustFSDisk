<script setup lang="ts">
type TelemetryTone = "ok" | "quiet" | "warning";

interface Props {
  httpTone: TelemetryTone;
  localTone: TelemetryTone;
  wsTone: TelemetryTone;
  refreshLabel: string;
  refreshDisabled?: boolean;
  showStatusPills?: boolean;
}

withDefaults(defineProps<Props>(), {
  showStatusPills: true,
});

const emit = defineEmits<{
  refresh: [];
}>();
</script>

<template>
  <section class="top-telemetry" aria-label="Edge 连接状态">
    <template v-if="showStatusPills">
      <span :class="['status-pill', httpTone]"><i></i> HTTP</span>
      <span :class="['status-pill', localTone]"><i></i> 本机服务</span>
      <span :class="['status-pill', wsTone]"><i></i> WebSocket</span>
    </template>
    <button
      :aria-label="refreshLabel"
      class="icon-refresh"
      :disabled="refreshDisabled"
      type="button"
      @click="emit('refresh')"
    >
      ↻
    </button>
  </section>
</template>
