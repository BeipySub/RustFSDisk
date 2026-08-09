<script setup lang="ts">
import { computed } from 'vue';

import { VbenCountToAnimator } from '@vben/common-ui';

import { usePreferredReducedMotion } from '@vueuse/core';

defineOptions({ name: 'FustfsProductRuntimeHeader' });

const props = withDefaults(
  defineProps<{
    activityLabel: string;
    decimals?: number;
    label: string;
    speed?: null | number;
    unit?: string;
    unknownLabel?: string;
  }>(),
  {
    decimals: 0,
    speed: null,
    unit: '',
    unknownLabel: '速率未知',
  },
);

const preferredReducedMotion = usePreferredReducedMotion();
const animationDuration = computed(() =>
  preferredReducedMotion.value === 'reduce' ? 0 : 1200,
);
const hasSpeed = computed(
  () => props.speed !== null && Number.isFinite(props.speed),
);
</script>

<template>
  <div class="runtime-header-activity" :aria-label="activityLabel">
    <span class="runtime-header-label">{{ label }}</span>
    <i aria-hidden="true" data-runtime-heartbeat>
      <svg role="presentation" viewBox="0 0 44 18">
        <path
          class="heartbeat-track"
          d="M1 9h8l3-6 5 13 5-11 4 7 3-3h14"
          pathLength="100"
        />
        <path
          class="heartbeat-pulse"
          d="M1 9h8l3-6 5 13 5-11 4 7 3-3h14"
          pathLength="100"
        />
      </svg>
    </i>
    <template v-if="hasSpeed">
      <VbenCountToAnimator
        class="runtime-header-speed"
        :decimals="decimals"
        :duration="animationDuration"
        :end-val="speed ?? 0"
        transition="easeOutCubic"
      />
      <em>{{ unit }}</em>
    </template>
    <strong v-else>{{ unknownLabel }}</strong>
  </div>
</template>
