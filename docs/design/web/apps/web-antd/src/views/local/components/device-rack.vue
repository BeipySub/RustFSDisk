<!--
  @file device-rack.vue
  @description 轻量机柜视觉组件，用 CSS 构造可访问的标准或紧凑机柜示意。
  @usage 用于无需基线设备位图的辅助状态场景。
  @scope 不表示真实盘位、容量或硬件健康事实。
-->
<script setup lang="ts">
withDefaults(
  defineProps<{
    compact?: boolean;
    label: string;
  }>(),
  { compact: false },
);
</script>

<template>
  <figure class="rack" :class="{ compact }" :aria-label="label">
    <div class="rack-edge"></div>
    <div class="rack-slots" aria-hidden="true">
      <span v-for="slot in 9" :key="slot"></span>
    </div>
    <div class="rack-glow" aria-hidden="true"></div>
    <figcaption>{{ label }}</figcaption>
  </figure>
</template>

<style scoped>
.rack {
  position: relative;
  width: 210px;
  height: 320px;
  margin: 0;
  background:
    linear-gradient(
      90deg,
      #111925 0 12%,
      #293543 13% 16%,
      #0a1018 17% 84%,
      #1d2b3b 85% 91%,
      #070a0f 92%
    ),
    #0a0d12;
  border: 1px solid #3b536d;
  border-radius: 18px 13px 15px 20px;
  box-shadow:
    inset 0 0 28px rgb(0 0 0 / 85%),
    0 0 52px rgb(16 132 255 / 12%);
  filter: drop-shadow(0 28px 30px rgb(0 0 0 / 55%));
  transform: perspective(620px) rotateY(6deg);
}

.rack.compact {
  width: 168px;
  height: 240px;
  transform: perspective(620px) rotateY(-7deg);
}

.rack-edge {
  position: absolute;
  inset: 8% auto 9% 11%;
  width: 3px;
  background: linear-gradient(transparent, #168dff 15% 86%, transparent);
  box-shadow: 0 0 13px #168dff;
}

.rack-slots {
  position: absolute;
  inset: 12% 16% 10% 25%;
  display: grid;
  gap: 5px;
  padding: 8px;
  background: #04070b;
  border: 1px solid #26384b;
}

.rack-slots span {
  position: relative;
  display: block;
  min-height: 12px;
  background:
    repeating-linear-gradient(90deg, #1a2633 0 3px, #080b10 3px 5px), #111923;
  border: 1px solid #26394d;
  box-shadow: inset 0 0 8px #000;
}

.rack-slots span::after {
  position: absolute;
  top: 50%;
  right: 4px;
  width: 3px;
  height: 3px;
  content: '';
  background: #53dfff;
  box-shadow: 0 0 7px #53dfff;
  transform: translateY(-50%);
}

.rack-glow {
  position: absolute;
  right: 8%;
  bottom: 8%;
  width: 3px;
  height: 70%;
  background: linear-gradient(transparent, #117eea 25% 78%, transparent);
  box-shadow: 0 0 17px #117eea;
}

figcaption {
  position: absolute;
  right: 0;
  bottom: -30px;
  left: 0;
  font-size: 12px;
  color: var(--fd-text-muted);
  text-align: center;
}

@media (max-width: 1400px) {
  .rack {
    width: 160px;
    height: 250px;
  }

  .rack.compact {
    width: 130px;
    height: 196px;
  }
}
</style>
