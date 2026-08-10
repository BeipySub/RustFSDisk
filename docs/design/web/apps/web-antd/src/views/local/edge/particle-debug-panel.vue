<!--
  @file particle-debug-panel.vue
  @description 粒子效果开发调节面板，用于预览状态、速度、发光强度和颜色覆盖。
  @usage 仅开发环境加载，不修改接口数据、页面业务状态或生产构建行为。
  @scope 调节结果只传递给 DataFlowField 渲染器。
-->
<script setup lang="ts">
import type {
  ParticleDebugSettings,
  ParticleDebugState,
} from './particle-debug';

import { computed, ref } from 'vue';

import { defaultParticleDebugSettings } from './particle-debug';

const props = defineProps<{
  modelValue: ParticleDebugSettings;
}>();

const emit = defineEmits<{
  'update:modelValue': [settings: ParticleDebugSettings];
}>();

const open = ref(false);

const stateOptions: Array<{
  label: string;
  value: null | ParticleDebugState;
}> = [
  { label: '跟随页面', value: null },
  { label: '汇聚', value: 'loading' },
  { label: '运行', value: 'running' },
  { label: '暂停', value: 'paused' },
  { label: '空闲', value: 'idle' },
  { label: '完成', value: 'complete' },
  { label: '异常', value: 'error' },
  { label: '无权限', value: 'denied' },
];

const paletteOptions = [
  { color: null, label: '状态色' },
  { color: '#008eff', label: '电光蓝' },
  { color: '#00bedc', label: '冰青' },
  { color: '#23d296', label: '翠绿' },
  { color: '#ff9f36', label: '琥珀' },
  { color: '#975cff', label: '紫罗兰' },
] as const;

const colorPickerValue = computed(() => props.modelValue.color ?? '#008eff');

function update(patch: Partial<ParticleDebugSettings>) {
  emit('update:modelValue', { ...props.modelValue, ...patch });
}

function selectState(state: null | ParticleDebugState) {
  update({ state });
}

function selectColor(color: null | string) {
  update({ color });
}

function readRange(event: Event) {
  return Number((event.target as HTMLInputElement).value);
}

function readColor(event: Event) {
  return (event.target as HTMLInputElement).value;
}

function reset() {
  emit('update:modelValue', defaultParticleDebugSettings());
}
</script>

<template>
  <aside
    aria-label="粒子开发调试"
    class="particle-debug"
    :class="{ 'is-open': open }"
    data-particle-debug-panel
  >
    <header class="debug-header">
      <button
        :aria-expanded="open"
        class="debug-toggle"
        type="button"
        @click="open = !open"
      >
        <span><i aria-hidden="true"></i>粒子调节</span>
        <em>{{ open ? '收起' : '展开' }}</em>
      </button>
      <button v-if="open" class="debug-reset" type="button" @click="reset">
        恢复默认
      </button>
    </header>

    <div v-if="open" class="debug-body">
      <p class="debug-notice">开发调试 · 仅覆盖粒子，不修改页面状态</p>

      <section class="debug-section">
        <span class="debug-label">粒子状态</span>
        <div class="state-options">
          <button
            v-for="option in stateOptions"
            :key="option.label"
            :aria-pressed="modelValue.state === option.value"
            :class="{ 'is-active': modelValue.state === option.value }"
            type="button"
            @click="selectState(option.value)"
          >
            {{ option.label }}
          </button>
        </div>
      </section>

      <section class="debug-section range-control">
        <label for="particle-debug-speed">
          <span>传输速度</span>
          <output>{{ modelValue.speedMultiplier.toFixed(2) }}×</output>
        </label>
        <input
          id="particle-debug-speed"
          max="2.5"
          min="0.25"
          :value="modelValue.speedMultiplier"
          step="0.05"
          type="range"
          @input="update({ speedMultiplier: readRange($event) })"
        />
      </section>

      <section class="debug-section range-control">
        <label for="particle-debug-glow">
          <span>外发光</span>
          <output>{{ Math.round(modelValue.glowStrength * 100) }}%</output>
        </label>
        <input
          id="particle-debug-glow"
          max="2"
          min="0.25"
          :value="modelValue.glowStrength"
          step="0.05"
          type="range"
          @input="update({ glowStrength: readRange($event) })"
        />
      </section>

      <section class="debug-section">
        <span class="debug-label">颜色</span>
        <div class="palette-options">
          <button
            v-for="option in paletteOptions"
            :key="option.label"
            :aria-pressed="modelValue.color === option.color"
            :class="{ 'is-active': modelValue.color === option.color }"
            type="button"
            @click="selectColor(option.color)"
          >
            <i
              aria-hidden="true"
              :class="{ semantic: option.color === null }"
              :style="
                option.color ? { backgroundColor: option.color } : undefined
              "
            ></i>
            {{ option.label }}
          </button>
        </div>
        <label class="custom-color" for="particle-debug-color">
          <span>自定义</span>
          <input
            id="particle-debug-color"
            aria-label="选择自定义粒子颜色"
            :value="colorPickerValue"
            type="color"
            @input="selectColor(readColor($event))"
          />
          <output>{{ modelValue.color ?? '跟随状态' }}</output>
        </label>
      </section>
    </div>
  </aside>
</template>

<style scoped>
.particle-debug {
  position: fixed;
  top: 82px;
  right: 28px;
  z-index: 100;
  width: 146px;
  overflow: hidden;
  font-family: 'Microsoft YaHei UI', 'Microsoft YaHei', sans-serif;
  color: #d2deeb;
  background: linear-gradient(145deg, rgb(7 15 25 / 92%), rgb(5 10 17 / 82%));
  border: 1px solid rgb(73 151 226 / 22%);
  border-radius: 12px;
  box-shadow:
    0 18px 48px rgb(0 5 12 / 34%),
    inset 0 1px rgb(153 218 255 / 6%);
  backdrop-filter: blur(18px) saturate(125%);
  transition:
    width 250ms ease,
    border-color 250ms ease;
}

.particle-debug.is-open {
  width: 286px;
}

.particle-debug:hover,
.particle-debug:focus-within {
  border-color: rgb(72 182 255 / 38%);
}

.debug-header {
  display: flex;
  gap: 8px;
  align-items: center;
  justify-content: space-between;
  min-height: 43px;
  padding: 7px 8px 7px 12px;
  border-bottom: 1px solid transparent;
}

.is-open .debug-header {
  border-bottom-color: rgb(94 150 205 / 10%);
}

.debug-toggle,
.debug-reset,
.state-options button,
.palette-options button {
  padding: 0;
  font: inherit;
  cursor: pointer;
  background: transparent;
  border: 0;
}

.debug-toggle {
  display: flex;
  flex: 1;
  gap: 8px;
  align-items: center;
  justify-content: space-between;
  min-width: 0;
  padding: 4px 0;
  text-align: left;
}

.debug-toggle span {
  display: flex;
  gap: 7px;
  align-items: center;
  font-size: 14px;
  color: #d2deeb;
}

.debug-toggle span i {
  width: 7px;
  height: 7px;
  background: #28b8ff;
  border-radius: 50%;
  box-shadow: 0 0 10px rgb(40 184 255 / 90%);
}

.debug-toggle em {
  font-size: 12px;
  font-style: normal;
  color: #627188;
}

.debug-reset {
  flex: 0 0 auto;
  padding: 5px 7px;
  font-size: 12px;
  color: #718299;
  border-radius: 5px;
}

.debug-reset:hover {
  color: #a9c7e4;
  background: rgb(47 136 218 / 12%);
}

.debug-body {
  padding: 4px 12px 12px;
}

.debug-notice {
  padding: 7px 8px;
  margin: 5px 0 0;
  font-size: 12px;
  line-height: 1.5;
  color: #678199;
  background: rgb(28 137 222 / 7%);
  border-left: 2px solid rgb(40 184 255 / 50%);
}

.debug-section {
  padding: 9px 0;
  border-bottom: 1px solid rgb(89 126 165 / 9%);
}

.debug-section:last-child {
  padding-bottom: 2px;
  border-bottom: 0;
}

.debug-label,
.range-control label {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 7px;
  font-size: 12px;
  color: #728198;
  letter-spacing: 0.08em;
}

.range-control output {
  font-size: 12px;
  color: #58cfff;
  letter-spacing: 0;
}

.state-options,
.palette-options {
  display: grid;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  gap: 5px;
}

.state-options button,
.palette-options button {
  min-height: 30px;
  font-size: 12px;
  color: #718198;
  background: rgb(30 52 74 / 32%);
  border: 1px solid rgb(81 124 166 / 14%);
  border-radius: 5px;
}

.state-options button:hover,
.palette-options button:hover {
  color: #b9d7ed;
  border-color: rgb(72 182 255 / 32%);
}

.state-options button.is-active,
.palette-options button.is-active {
  color: #c8efff;
  background: rgb(19 133 220 / 18%);
  border-color: rgb(63 187 255 / 48%);
  box-shadow: inset 0 0 12px rgb(40 184 255 / 8%);
}

.range-control input[type='range'] {
  display: block;
  width: 100%;
  height: 3px;
  margin: 0;
  appearance: none;
  cursor: pointer;
  background: linear-gradient(
    90deg,
    rgb(26 150 238 / 90%),
    rgb(54 208 255 / 80%)
  );
  border-radius: 99px;
}

.range-control input[type='range']::-webkit-slider-thumb {
  width: 12px;
  height: 12px;
  appearance: none;
  background: #198fe8;
  border: 2px solid #bdeeff;
  border-radius: 50%;
  box-shadow: 0 0 10px rgb(31 171 255 / 74%);
}

.palette-options {
  grid-template-columns: repeat(3, minmax(0, 1fr));
}

.palette-options button {
  display: flex;
  gap: 5px;
  align-items: center;
  justify-content: center;
}

.palette-options button i {
  width: 7px;
  height: 7px;
  border-radius: 50%;
  box-shadow: 0 0 7px currentcolor;
}

.palette-options button i.semantic {
  background: conic-gradient(#39dfa6, #ffb14a, #ff4d61, #a66bff, #208cff);
}

.custom-color {
  display: grid;
  grid-template-columns: auto 31px 1fr;
  gap: 8px;
  align-items: center;
  margin-top: 8px;
  font-size: 12px;
  color: #728198;
}

.custom-color input {
  width: 31px;
  height: 23px;
  padding: 1px;
  cursor: pointer;
  background: #0b1119;
  border: 1px solid rgb(81 124 166 / 28%);
  border-radius: 4px;
}

.custom-color output {
  overflow: hidden;
  text-overflow: ellipsis;
  font-size: 12px;
  color: #58cfff;
  white-space: nowrap;
}

@media (max-width: 1279px) {
  .particle-debug {
    right: 18px;
  }
}

@media (prefers-reduced-motion: reduce) {
  .particle-debug {
    transition-duration: 0.01ms;
  }
}
</style>
