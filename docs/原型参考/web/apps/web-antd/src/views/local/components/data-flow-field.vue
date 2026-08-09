<!--
  @file data-flow-field.vue
  @description A 工厂 Canvas 粒子数据流渲染器，表达汇聚、传输、暂停、完成和异常状态。
  @usage 由 runtime-panel.vue 注入页面状态，可接收开发态粒子调节覆盖。
  @scope 仅渲染视觉状态，不修改任务、介质或同步业务数据。
-->
<script setup lang="ts">
import type { ParticleDebugState } from '../edge/particle-debug';

import type { RuntimeState } from '#/api/local-views';

import {
  computed,
  nextTick,
  onBeforeUnmount,
  onMounted,
  ref,
  watch,
} from 'vue';

type ParticleColor = readonly [number, number, number];
type SceneState =
  | 'complete'
  | 'denied'
  | 'error'
  | 'idle'
  | 'loading'
  | 'paused'
  | 'running';
type FlowPoint = Readonly<{ x: number; y: number }>;
type FlowPath = Readonly<{
  control1: FlowPoint;
  control2: FlowPoint;
  end: FlowPoint;
  spread?: number;
  start: FlowPoint;
}>;

const props = defineProps<{
  customColor?: null | string;
  debugState?: null | ParticleDebugState;
  glowStrength?: number;
  path?: FlowPath;
  paused?: boolean;
  speedMultiplier?: number;
  state: RuntimeState;
}>();

const canvasRef = ref<HTMLCanvasElement>();
let cleanup: (() => void) | undefined;
const defaultPath: FlowPath = {
  control1: { x: 0.4, y: 0.45 },
  control2: { x: 0.52, y: 0.73 },
  end: { x: 0.66, y: 0.7 },
  spread: 0.072,
  start: { x: 0.3, y: 0.45 },
};
const resolvedPath = computed(() => props.path ?? defaultPath);
const formatPoint = (point: FlowPoint) =>
  `${point.x.toFixed(3)},${point.y.toFixed(3)}`;

const sceneState = computed<SceneState>(() => {
  const states: Record<RuntimeState, SceneState> = {
    COMPLETED: 'complete',
    IDLE: 'idle',
    LOADING: 'loading',
    NO_MEDIA: 'idle',
    PAUSED: 'paused',
    PERMISSION_DENIED: 'denied',
    RISK_LOCKED: 'error',
    RUNNING: 'running',
  };
  return props.debugState ?? states[props.state];
});

const particleThemes: Record<
  SceneState,
  { accent: ParticleColor; base: ParticleColor; glow: ParticleColor }
> = {
  complete: {
    accent: [190, 255, 229],
    base: [57, 223, 166],
    glow: [20, 157, 116],
  },
  denied: {
    accent: [220, 194, 255],
    base: [166, 107, 255],
    glow: [112, 63, 207],
  },
  error: {
    accent: [255, 175, 184],
    base: [255, 77, 97],
    glow: [196, 31, 55],
  },
  idle: {
    accent: [170, 185, 202],
    base: [101, 120, 142],
    glow: [62, 78, 98],
  },
  loading: {
    accent: [205, 245, 255],
    base: [72, 164, 255],
    glow: [34, 126, 232],
  },
  paused: {
    accent: [255, 221, 155],
    base: [255, 177, 74],
    glow: [214, 123, 31],
  },
  running: {
    accent: [74, 202, 255],
    base: [0, 142, 255],
    glow: [0, 116, 232],
  },
};

function blendColor(
  from: ParticleColor,
  to: ParticleColor,
  amount: number,
): ParticleColor {
  return [
    Math.round(from[0] + (to[0] - from[0]) * amount),
    Math.round(from[1] + (to[1] - from[1]) * amount),
    Math.round(from[2] + (to[2] - from[2]) * amount),
  ];
}

function parseHexColor(value: null | string | undefined): null | ParticleColor {
  if (!value || !/^#[\da-f]{6}$/i.test(value)) return null;
  return [
    Number.parseInt(value.slice(1, 3), 16),
    Number.parseInt(value.slice(3, 5), 16),
    Number.parseInt(value.slice(5, 7), 16),
  ];
}

function resolveTheme(state: SceneState) {
  const customColor = parseHexColor(props.customColor);
  if (!customColor) return particleThemes[state];
  return {
    accent: blendColor(customColor, [255, 255, 255], 0.56),
    base: customColor,
    glow: blendColor(customColor, [0, 0, 0], 0.24),
  };
}

function startParticleField() {
  cleanup?.();

  const canvas = canvasRef.value;
  if (!canvas) return;

  const context = canvas.getContext('2d', {
    alpha: true,
    desynchronized: true,
  });
  if (!context) return;

  const bufferCanvas = document.createElement('canvas');
  const buffer = bufferCanvas.getContext('2d', {
    alpha: true,
    desynchronized: true,
  });
  if (!buffer) return;

  const currentState = sceneState.value;
  const theme = resolveTheme(currentState);
  const flowPath = resolvedPath.value;
  const reduced = window.matchMedia('(prefers-reduced-motion: reduce)');
  const particleCount = 1800;
  const particleSpread = 12;
  const particles = new Float32Array(
    particleCount * particleSpread,
  ) as unknown as Record<number, number>;
  const runningIce = [214, 248, 255] as const;
  let width = 0;
  let height = 0;
  let animationFrame = 0;
  let previous = performance.now();
  let formationStartedAt = previous;
  let staticFrameDrawn = false;
  let seed = 1_779_033_703;

  const random = () => {
    seed = (seed * 1_664_525 + 1_013_904_223) >>> 0;
    return seed / 4_294_967_296;
  };
  const readParticle = (base: number, offset = 0) =>
    particles[base + offset] ?? 0;

  const rgba = (color: ParticleColor, alpha: number) =>
    `rgba(${color[0]}, ${color[1]}, ${color[2]}, ${alpha})`;

  const pathPoint = (
    progress: number,
    offset: number,
    phase: number,
    now: number,
  ) => {
    const time = Math.max(0, Math.min(1, progress));
    const inverse = 1 - time;
    const point0 = {
      x: width * flowPath.start.x,
      y: height * flowPath.start.y,
    };
    const point1 = {
      x: width * flowPath.control1.x,
      y: height * flowPath.control1.y,
    };
    const point2 = {
      x: width * flowPath.control2.x,
      y: height * flowPath.control2.y,
    };
    const point3 = {
      x: width * flowPath.end.x,
      y: height * flowPath.end.y,
    };
    const x =
      inverse ** 3 * point0.x +
      3 * inverse ** 2 * time * point1.x +
      3 * inverse * time ** 2 * point2.x +
      time ** 3 * point3.x;
    const y =
      inverse ** 3 * point0.y +
      3 * inverse ** 2 * time * point1.y +
      3 * inverse * time ** 2 * point2.y +
      time ** 3 * point3.y;
    const dx =
      3 * inverse ** 2 * (point1.x - point0.x) +
      6 * inverse * time * (point2.x - point1.x) +
      3 * time ** 2 * (point3.x - point2.x);
    const dy =
      3 * inverse ** 2 * (point1.y - point0.y) +
      6 * inverse * time * (point2.y - point1.y) +
      3 * time ** 2 * (point3.y - point2.y);
    const tangentLength = Math.hypot(dx, dy) || 1;
    const leftFan = (1 - time) ** 2.1 * 0.7;
    const rightFan = time ** 3 * 0.12;
    const envelope =
      0.18 +
      leftFan +
      rightFan +
      Math.sin(time * Math.PI) ** 0.82 * (0.74 - time * 0.22);
    const primaryNoise = Math.sin(
      progress * Math.PI * (4.6 + (phase % 1.8)) + phase + now * 0.000_34,
    );
    const detailNoise = Math.sin(
      progress * Math.PI * 13.7 - phase * 1.7 - now * 0.000_23,
    );
    const noiseOffset = primaryNoise * 0.05 + detailNoise * 0.016;
    const asymmetricOffset = offset < 0 ? offset * 1.16 : offset * 0.9;
    const displacement =
      height *
      (flowPath.spread ?? defaultPath.spread ?? 0.072) *
      envelope *
      (asymmetricOffset + noiseOffset);

    return {
      dx,
      dy,
      x: x - (dy / tangentLength) * displacement,
      y: y + (dx / tangentLength) * displacement,
    };
  };

  const resetParticle = (index: number, distributed: boolean) => {
    const base = index * particleSpread;
    const rawOffset = random() * 2 - 1;
    const offset =
      index % 11 === 0
        ? rawOffset
        : Math.sign(rawOffset) * Math.abs(rawOffset) ** 1.8;
    const progress = distributed ? random() : -random() * 0.055;
    const phase = random() * Math.PI * 2;
    const speed = 0.000_055 + random() ** 1.45 * 0.000_14;
    const pulseCycle = 1100 + random() * 2200;
    const point = pathPoint(progress, offset, phase, performance.now());

    particles[base] = point.x;
    particles[base + 1] = point.y;
    particles[base + 2] = progress;
    particles[base + 3] = offset;
    particles[base + 4] = speed;
    particles[base + 5] = 0.42 + random() ** 2.4 * 1.65;
    particles[base + 6] = 0.2 + random() ** 1.8 * 1.2;
    particles[base + 7] = distributed ? random() * pulseCycle : 0;
    particles[base + 8] = pulseCycle;
    particles[base + 9] = phase;
    particles[base + 10] = random() ** 2.2;
    particles[base + 11] = random() * 0.72;
  };

  const resetAllParticles = (distributed = true) => {
    for (let index = 0; index < particleCount; index += 1) {
      resetParticle(index, distributed);
    }
  };

  const convergenceStartPoint = (index: number) => {
    const base = index * particleSpread;
    const side = index % 4;
    const phase = readParticle(base, 9);
    const spread = 1.05 + (phase / (Math.PI * 2)) * 0.72;
    let progress = readParticle(base, 2);
    let offset = readParticle(base, 3);

    if (side === 0) offset -= spread;
    else if (side === 2) offset += spread;
    else {
      const direction = side === 1 ? 1 : -1;
      progress += direction * (0.065 + spread * 0.035);
      offset += Math.sin(phase * 1.7) * 0.58;
    }

    return pathPoint(progress, offset, phase, formationStartedAt);
  };

  const prepareConvergenceStarts = () => {
    for (let index = 0; index < particleCount; index += 1) {
      const base = index * particleSpread;
      const start = convergenceStartPoint(index);
      particles[base] = start.x;
      particles[base + 1] = start.y;
    }
  };

  const resize = () => {
    const rect = canvas.getBoundingClientRect();
    width = Math.max(1, rect.width);
    height = Math.max(1, rect.height);
    canvas.width = Math.round(width);
    canvas.height = Math.round(height);
    bufferCanvas.width = Math.round(width);
    bufferCanvas.height = Math.round(height);
    context.setTransform(1, 0, 0, 1, 0, 0);
    buffer.setTransform(1, 0, 0, 1, 0, 0);
    context.clearRect(0, 0, width, height);
    buffer.clearRect(0, 0, width, height);
    formationStartedAt = performance.now();
    staticFrameDrawn = false;
    resetAllParticles(true);
    if (currentState === 'loading') prepareConvergenceStarts();
  };

  const fadeTrails = (amount: number) => {
    context.save();
    context.globalCompositeOperation = 'destination-out';
    context.fillStyle = `rgba(0, 0, 0, ${amount})`;
    context.fillRect(0, 0, width, height);
    context.restore();
  };

  const drawParticleSegment = (
    fromX: number,
    fromY: number,
    toX: number,
    toY: number,
    widthValue: number,
    alpha: number,
    tone: number,
  ) => {
    const color =
      currentState === 'running' && tone > 0.94
        ? blendColor(theme.accent, runningIce, (tone - 0.94) / 0.06)
        : blendColor(theme.base, theme.accent, tone);
    buffer.strokeStyle = rgba(color, alpha);
    buffer.lineWidth = widthValue;
    buffer.lineCap = 'round';
    buffer.beginPath();
    buffer.moveTo(fromX, fromY);
    buffer.lineTo(toX, toY);
    buffer.stroke();
  };

  const drawParticleHead = (
    x: number,
    y: number,
    radius: number,
    alpha: number,
    tone: number,
  ) => {
    const color =
      currentState === 'running' && tone > 0.92
        ? blendColor(theme.accent, runningIce, (tone - 0.92) / 0.08)
        : blendColor(theme.base, theme.accent, tone);
    buffer.fillStyle = rgba(color, alpha);
    buffer.beginPath();
    buffer.arc(x, y, radius, 0, Math.PI * 2);
    buffer.fill();
  };

  const drawParticleFlare = (
    x: number,
    y: number,
    radius: number,
    alpha: number,
  ) => {
    buffer.strokeStyle = rgba(runningIce, alpha);
    buffer.lineWidth = 0.42;
    buffer.lineCap = 'round';
    buffer.beginPath();
    buffer.moveTo(x - radius, y);
    buffer.lineTo(x + radius, y);
    buffer.moveTo(x, y - radius);
    buffer.lineTo(x, y + radius);
    buffer.stroke();
  };

  const drawGuideFilaments = (now: number) => {
    const filamentCount = 16;
    for (let index = 0; index < filamentCount; index += 1) {
      const normalized = (((index + 0.5) / filamentCount) * 2 - 1) * 0.6;
      const phase = index * 0.731;
      const tone = ((index * 17) % filamentCount) / (filamentCount - 1);
      const color =
        tone > 0.95
          ? blendColor(theme.accent, runningIce, (tone - 0.95) / 0.05)
          : blendColor(theme.base, theme.accent, tone);
      buffer.strokeStyle = rgba(color, 0.045 + tone * 0.1);
      buffer.lineWidth = 0.18 + (index % 4) * 0.045;
      buffer.lineCap = 'round';
      buffer.lineJoin = 'round';
      buffer.beginPath();
      for (let step = 0; step <= 72; step += 1) {
        const progress = step / 72;
        const offset =
          normalized + Math.sin(progress * Math.PI * 2 + phase) * 0.018;
        const point = pathPoint(progress, offset, phase, now * 0.22);
        if (step === 0) buffer.moveTo(point.x, point.y);
        else buffer.lineTo(point.x, point.y);
      }
      buffer.stroke();
    }
  };

  const drawEndpointBloom = (now: number) => {
    const point = pathPoint(1, 0, 0, now);
    const radius = height * 0.048;
    const gradient = buffer.createRadialGradient(
      point.x,
      point.y,
      0,
      point.x,
      point.y,
      radius,
    );
    gradient.addColorStop(0, rgba(runningIce, 0.14));
    gradient.addColorStop(0.24, rgba(theme.accent, 0.055));
    gradient.addColorStop(0.58, rgba(theme.glow, 0.038));
    gradient.addColorStop(1, rgba(theme.glow, 0));
    buffer.fillStyle = gradient;
    buffer.fillRect(point.x - radius, point.y - radius, radius * 2, radius * 2);
  };

  const drawRunningParticles = (now: number, delta: number) => {
    for (let index = 0; index < particleCount; index += 1) {
      const base = index * particleSpread;
      const oldX = readParticle(base);
      const oldY = readParticle(base, 1);
      const speedMultiplier = Math.max(
        0.1,
        Math.min(3, props.speedMultiplier ?? 1),
      );
      const progress =
        readParticle(base, 2) + readParticle(base, 4) * delta * speedMultiplier;
      const pulseCycle = readParticle(base, 8);
      const life = (readParticle(base, 7) + delta) % pulseCycle;
      if (progress > 1.04) {
        resetParticle(index, false);
        continue;
      }

      const point = pathPoint(
        progress,
        readParticle(base, 3),
        readParticle(base, 9),
        now,
      );
      const follow = Math.min(1, delta * 0.038);
      const nextX = oldX + (point.x - oldX) * follow;
      const nextY = oldY + (point.y - oldY) * follow;
      const progressFade =
        0.52 +
        Math.max(0, Math.sin(Math.PI * Math.min(1, Math.max(0, progress)))) *
          0.48;
      const breathing =
        0.68 +
        Math.max(
          0,
          Math.sin((life / pulseCycle) * Math.PI * 2 + readParticle(base, 9)),
        ) *
          0.32;
      const startFade = 0.3 + Math.min(1, Math.max(0, progress) * 18) * 0.7;
      const tone = readParticle(base, 10);
      const alpha = (0.22 + tone * 0.56) * progressFade * breathing * startFade;
      const velocityX = nextX - oldX;
      const velocityY = nextY - oldY;
      const velocityLength = Math.hypot(velocityX, velocityY) || 1;
      const trailLength = Math.min(
        18,
        Math.max(3.8, velocityLength * (3.4 + readParticle(base, 5) * 2.2)),
      );
      const tailX = nextX - (velocityX / velocityLength) * trailLength;
      const tailY = nextY - (velocityY / velocityLength) * trailLength;
      const shapeRoll = ((index * 37) % 100) / 100;
      const isFlare = index % 67 === 0;

      if (shapeRoll >= 0.7) {
        drawParticleSegment(
          tailX,
          tailY,
          nextX,
          nextY,
          0.28 + readParticle(base, 6) * progressFade * 0.34,
          alpha * 0.76,
          tone,
        );
      }

      const headRadius =
        0.42 + readParticle(base, 5) * (tone > 0.92 ? 0.62 : 0.38);
      drawParticleHead(
        nextX,
        nextY,
        headRadius,
        Math.min(0.96, alpha * (1 + tone * 0.55)),
        tone,
      );
      if (isFlare) {
        drawParticleFlare(
          nextX,
          nextY,
          2.6 + tone * 2.4,
          Math.min(0.46, alpha * 0.78),
        );
      }
      particles[base] = nextX;
      particles[base + 1] = nextY;
      particles[base + 2] = progress;
      particles[base + 7] = life;
    }
  };

  const drawConvergence = (now: number) => {
    const speedMultiplier = Math.max(
      0.1,
      Math.min(3, props.speedMultiplier ?? 1),
    );
    const formation = reduced.matches
      ? 1
      : Math.min(1, (now - formationStartedAt) / (3600 / speedMultiplier));
    for (let index = 0; index < particleCount; index += 1) {
      const base = index * particleSpread;
      const delay = readParticle(base, 11);
      const local = Math.max(0, Math.min(1, (formation - delay) / (1 - delay)));
      if (local <= 0) continue;

      const eased = local * local * (3 - 2 * local);
      const start = convergenceStartPoint(index);
      const target = pathPoint(
        readParticle(base, 2),
        readParticle(base, 3),
        readParticle(base, 9),
        now,
      );
      const nextX = start.x + (target.x - start.x) * eased;
      const nextY = start.y + (target.y - start.y) * eased;
      const oldX = readParticle(base);
      const oldY = readParticle(base, 1);
      const revealProgress = Math.min(1, local / 0.38);
      const reveal = revealProgress * revealProgress * (3 - 2 * revealProgress);
      const globalReveal = Math.min(1, formation / 0.5);
      drawParticleSegment(
        oldX,
        oldY,
        nextX,
        nextY,
        0.45 + readParticle(base, 6),
        (readParticle(base, 10) * 0.46 + 0.1) * reveal * globalReveal,
        readParticle(base, 10),
      );
      particles[base] = nextX;
      particles[base + 1] = nextY;
    }
    return formation;
  };

  const drawStaticPath = (now: number) => {
    const step = currentState === 'running' ? 1 : 5;
    for (let index = 0; index < particleCount; index += step) {
      const base = index * particleSpread;
      const point = pathPoint(
        readParticle(base, 2),
        readParticle(base, 3),
        readParticle(base, 9),
        now,
      );
      const tangentLength = Math.hypot(point.dx, point.dy) || 1;
      const segment = 0.8 + readParticle(base, 5) * 0.9;
      drawParticleSegment(
        point.x - (point.dx / tangentLength) * segment,
        point.y - (point.dy / tangentLength) * segment,
        point.x + (point.dx / tangentLength) * segment,
        point.y + (point.dy / tangentLength) * segment,
        0.42 + readParticle(base, 6) * 0.45,
        currentState === 'running' ? 0.68 : 0.08,
        readParticle(base, 10),
      );
    }
  };

  const compositeBuffer = (mode: 'loading' | 'running' | 'static') => {
    const glowStrength = Math.max(0.1, Math.min(2, props.glowStrength ?? 1));
    const outerAlpha =
      { loading: 0.1, running: 0.18, static: 0.12 }[mode] * glowStrength;
    const middleAlpha =
      { loading: 0.34, running: 0.27, static: 0.28 }[mode] * glowStrength;
    const detailAlpha = { loading: 0.6, running: 0.76, static: 0.52 }[mode];

    context.save();
    context.globalCompositeOperation = 'lighter';
    context.filter =
      mode === 'running' ? 'blur(12px) saturate(170%)' : 'blur(9px)';
    context.globalAlpha = outerAlpha;
    context.drawImage(bufferCanvas, 0, 0);
    context.restore();

    context.save();
    context.globalCompositeOperation = 'lighter';
    context.filter = 'blur(3px)';
    context.globalAlpha = middleAlpha;
    context.drawImage(bufferCanvas, 0, 0);
    context.restore();

    context.save();
    context.globalCompositeOperation = 'lighter';
    context.globalAlpha = detailAlpha;
    context.drawImage(bufferCanvas, 0, 0);
    context.restore();
  };

  const draw = (now: number) => {
    animationFrame = requestAnimationFrame(draw);
    if (document.hidden || width <= 1 || height <= 1) return;

    const delta = Math.min(32, now - previous);
    previous = now;
    const running =
      currentState === 'running' && !props.paused && !reduced.matches;
    const loading =
      currentState === 'loading' && !props.paused && !reduced.matches;
    if (!running && !loading && staticFrameDrawn) return;

    let mode: 'loading' | 'running' | 'static' = 'static';
    if (running) mode = 'running';
    else if (loading) mode = 'loading';

    fadeTrails({ loading: 0.27, running: 0.14, static: 1 }[mode]);
    buffer.clearRect(0, 0, width, height);
    if (running) {
      drawGuideFilaments(now);
      drawRunningParticles(now, delta);
      drawEndpointBloom(now);
    } else if (loading) {
      const formed = drawConvergence(now);
      if (formed >= 1) drawStaticPath(now);
    } else if (currentState === 'running') {
      drawGuideFilaments(0);
      drawStaticPath(0);
      drawEndpointBloom(0);
    } else {
      drawStaticPath(now);
    }
    compositeBuffer(mode);
    staticFrameDrawn = !running && !loading;
  };

  resize();
  const observer = new ResizeObserver(resize);
  observer.observe(canvas);
  animationFrame = requestAnimationFrame(draw);
  cleanup = () => {
    observer.disconnect();
    cancelAnimationFrame(animationFrame);
  };
}

onMounted(startParticleField);

watch([sceneState, () => props.customColor, () => props.path], async () => {
  await nextTick();
  startParticleField();
});

onBeforeUnmount(() => cleanup?.());
</script>

<template>
  <canvas
    ref="canvasRef"
    :aria-label="
      sceneState === 'running'
        ? '路径约束的蓝色数据粒子正在写入运输 NAS'
        : `${sceneState} 粒子状态`
    "
    class="data-flow-canvas"
    :data-particle-color="customColor ?? 'semantic'"
    :data-particle-path-end="formatPoint(resolvedPath.end)"
    :data-particle-path-start="formatPoint(resolvedPath.start)"
    data-particle-renderer="path-aether"
    :data-particle-state="sceneState"
  ></canvas>
</template>

<style scoped>
.data-flow-canvas {
  position: absolute;
  inset: 0;
  z-index: 9;
  width: 100%;
  height: 100%;
  pointer-events: none;
  mix-blend-mode: screen;
}

@media (prefers-reduced-motion: reduce) {
  .data-flow-canvas {
    opacity: 0.78;
  }
}
</style>
