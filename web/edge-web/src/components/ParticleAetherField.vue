<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref, watch } from "vue";

type ParticlePalette = "semantic" | "electric" | "cyan" | "emerald" | "amber" | "violet";
type ParticleColor = readonly [number, number, number];

const props = withDefaults(
  defineProps<{
    active?: boolean;
    speed?: number;
    glow?: number;
    palette?: ParticlePalette;
    startX?: number;
    startY?: number;
    endX?: number;
    endY?: number;
  }>(),
  {
    active: true,
    speed: 1,
    glow: 1,
    palette: "semantic",
    startX: 0.3,
    startY: 0.49,
    endX: 0.66,
    endY: 0.73,
  },
);

const canvasRef = ref<HTMLCanvasElement | null>(null);

const particleThemes: Record<ParticlePalette, { base: ParticleColor; accent: ParticleColor; glow: ParticleColor }> = {
  semantic: { base: [32, 140, 255], accent: [54, 216, 255], glow: [22, 119, 255] },
  electric: { base: [22, 142, 255], accent: [203, 244, 255], glow: [18, 125, 255] },
  cyan: { base: [48, 205, 231], accent: [178, 250, 255], glow: [35, 187, 239] },
  emerald: { base: [42, 218, 157], accent: [174, 255, 217], glow: [34, 183, 134] },
  amber: { base: [255, 174, 66], accent: [255, 226, 154], glow: [255, 151, 48] },
  violet: { base: [164, 109, 255], accent: [226, 205, 255], glow: [128, 94, 255] },
};

const speedRef = ref(props.speed);
const themeRef = ref(particleThemes[props.palette]);
const glowRef = ref(props.glow);
const activeRef = ref(props.active);

watch(() => props.speed, (value) => {
  speedRef.value = value;
});
watch(() => props.palette, () => {
  themeRef.value = particleThemes[props.palette];
});
watch(() => props.glow, (value) => {
  glowRef.value = value;
});
watch(() => props.active, (value) => {
  activeRef.value = value;
});

onMounted(() => {
  const canvas = canvasRef.value;
  if (!canvas) return;
  const context = canvas.getContext("2d", { alpha: true, desynchronized: true });
  if (!context) return;
  const bufferCanvas = document.createElement("canvas");
  const buffer = bufferCanvas.getContext("2d", { alpha: true, desynchronized: true });
  if (!buffer) return;

  const reduced = window.matchMedia("(prefers-reduced-motion: reduce)");
  const particleCount = 1800;
  const particleSpread = 12;
  const particles = new Float32Array(particleCount * particleSpread);
  const runningIce: ParticleColor = [214, 248, 255];
  let width = 0;
  let height = 0;
  let raf = 0;
  let previous = performance.now();
  let staticFrameDrawn = false;
  let seed = 0x6a09e667;

  const random = () => {
    seed = (seed * 1664525 + 1013904223) >>> 0;
    return seed / 4294967296;
  };
  const blendColor = (from: ParticleColor, to: ParticleColor, amount: number): ParticleColor => [
    Math.round(from[0] + (to[0] - from[0]) * amount),
    Math.round(from[1] + (to[1] - from[1]) * amount),
    Math.round(from[2] + (to[2] - from[2]) * amount),
  ];
  const rgba = (color: ParticleColor, alpha: number) => `rgba(${color[0]}, ${color[1]}, ${color[2]}, ${alpha})`;

  const pathPoint = (progress: number, offset: number, phase: number, now: number) => {
    const t = Math.max(0, Math.min(1, progress));
    const inverse = 1 - t;
    const p0 = { x: width * props.startX, y: height * props.startY };
    const p3 = { x: width * props.endX, y: height * props.endY };
    const spanX = p3.x - p0.x;
    const spanY = p3.y - p0.y;
    const p1 = { x: p0.x + spanX * 0.28, y: p0.y + spanY * 0.02 };
    const p2 = { x: p0.x + spanX * 0.68, y: p0.y + spanY * 0.96 };
    const x = inverse ** 3 * p0.x + 3 * inverse ** 2 * t * p1.x + 3 * inverse * t ** 2 * p2.x + t ** 3 * p3.x;
    const y = inverse ** 3 * p0.y + 3 * inverse ** 2 * t * p1.y + 3 * inverse * t ** 2 * p2.y + t ** 3 * p3.y;
    const dx = 3 * inverse ** 2 * (p1.x - p0.x) + 6 * inverse * t * (p2.x - p1.x) + 3 * t ** 2 * (p3.x - p2.x);
    const dy = 3 * inverse ** 2 * (p1.y - p0.y) + 6 * inverse * t * (p2.y - p1.y) + 3 * t ** 2 * (p3.y - p2.y);
    const tangentLength = Math.hypot(dx, dy) || 1;
    const leftFan = Math.pow(1 - t, 2.1) * 0.6;
    const endTaper = 1 - Math.max(0, (t - 0.84) / 0.16) * 0.48;
    const envelope = (0.16 + leftFan + Math.pow(Math.sin(t * Math.PI), 0.82) * (0.72 - t * 0.2)) * endTaper;
    const primaryNoise = Math.sin(progress * Math.PI * (4.6 + (phase % 1.8)) + phase + now * 0.00034);
    const detailNoise = Math.sin(progress * Math.PI * 13.7 - phase * 1.7 - now * 0.00023);
    const noiseOffset = (primaryNoise * 0.048 + detailNoise * 0.015) * endTaper;
    const asymmetricOffset = offset < 0 ? offset * 1.18 : offset * 0.92;
    const displacement = height * 0.07 * envelope * (asymmetricOffset + noiseOffset);
    return {
      x: x - (dy / tangentLength) * displacement,
      y: y + (dx / tangentLength) * displacement,
      dx,
      dy,
    };
  };

  const resetParticle = (index: number, distributed: boolean) => {
    const base = index * particleSpread;
    const rawOffset = random() * 2 - 1;
    const offset = index % 11 === 0 ? rawOffset : Math.sign(rawOffset) * Math.pow(Math.abs(rawOffset), 1.8);
    const progress = distributed ? random() : -random() * 0.055;
    const phase = random() * Math.PI * 2;
    const speed = 0.000055 + Math.pow(random(), 1.45) * 0.00014;
    const pulseCycle = 1100 + random() * 2200;
    const point = pathPoint(progress, offset, phase, performance.now());
    particles[base] = point.x;
    particles[base + 1] = point.y;
    particles[base + 2] = progress;
    particles[base + 3] = offset;
    particles[base + 4] = speed;
    particles[base + 5] = 0.42 + Math.pow(random(), 2.4) * 1.65;
    particles[base + 6] = 0.2 + Math.pow(random(), 1.8) * 1.2;
    particles[base + 7] = distributed ? random() * pulseCycle : 0;
    particles[base + 8] = pulseCycle;
    particles[base + 9] = phase;
    particles[base + 10] = Math.pow(random(), 2.2);
    particles[base + 11] = random() * 0.72;
  };

  const resetAllParticles = (distributed = true) => {
    for (let index = 0; index < particleCount; index += 1) resetParticle(index, distributed);
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
    staticFrameDrawn = false;
    resetAllParticles(true);
  };

  const fadeTrails = (amount: number) => {
    context.save();
    context.globalCompositeOperation = "destination-out";
    context.fillStyle = `rgba(0, 0, 0, ${amount})`;
    context.fillRect(0, 0, width, height);
    context.restore();
  };

  const drawParticleSegment = (fromX: number, fromY: number, toX: number, toY: number, widthValue: number, alpha: number, tone: number) => {
    const theme = themeRef.value;
    const color = tone > 0.94 ? blendColor(theme.accent, runningIce, (tone - 0.94) / 0.06) : blendColor(theme.base, theme.accent, tone);
    buffer.strokeStyle = rgba(color, alpha);
    buffer.lineWidth = widthValue;
    buffer.lineCap = "round";
    buffer.beginPath();
    buffer.moveTo(fromX, fromY);
    buffer.lineTo(toX, toY);
    buffer.stroke();
  };

  const drawParticleHead = (x: number, y: number, radius: number, alpha: number, tone: number) => {
    const theme = themeRef.value;
    const color = tone > 0.92 ? blendColor(theme.accent, runningIce, (tone - 0.92) / 0.08) : blendColor(theme.base, theme.accent, tone);
    buffer.fillStyle = rgba(color, alpha);
    buffer.beginPath();
    buffer.arc(x, y, radius, 0, Math.PI * 2);
    buffer.fill();
  };

  const drawGuideFilaments = (now: number) => {
    const theme = themeRef.value;
    const filamentCount = 16;
    for (let index = 0; index < filamentCount; index += 1) {
      const normalized = ((index + 0.5) / filamentCount * 2 - 1) * 0.6;
      const phase = index * 0.731;
      const tone = ((index * 17) % filamentCount) / (filamentCount - 1);
      const color = tone > 0.95 ? blendColor(theme.accent, runningIce, (tone - 0.95) / 0.05) : blendColor(theme.base, theme.accent, tone);
      buffer.strokeStyle = rgba(color, 0.045 + tone * 0.1);
      buffer.lineWidth = 0.18 + (index % 4) * 0.045;
      buffer.lineCap = "round";
      buffer.lineJoin = "round";
      buffer.beginPath();
      for (let step = 0; step <= 72; step += 1) {
        const progress = step / 72;
        const offset = normalized + Math.sin(progress * Math.PI * 2 + phase) * 0.018;
        const point = pathPoint(progress, offset, phase, now * 0.22);
        if (step === 0) buffer.moveTo(point.x, point.y);
        else buffer.lineTo(point.x, point.y);
      }
      buffer.stroke();
    }
  };

  const drawEndpointBloom = (now: number) => {
    const theme = themeRef.value;
    const point = pathPoint(1, 0, 0, now);
    const radius = height * 0.028;
    const gradient = buffer.createRadialGradient(point.x, point.y, 0, point.x, point.y, radius);
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
      const oldX = particles[base];
      const oldY = particles[base + 1];
      const progress = particles[base + 2] + particles[base + 4] * delta * speedRef.value;
      const pulseCycle = particles[base + 8];
      const life = (particles[base + 7] + delta * speedRef.value) % pulseCycle;
      if (progress > 1.04) {
        resetParticle(index, false);
        continue;
      }
      const point = pathPoint(progress, particles[base + 3], particles[base + 9], now);
      const follow = Math.min(1, delta * 0.038);
      const nextX = oldX + (point.x - oldX) * follow;
      const nextY = oldY + (point.y - oldY) * follow;
      const progressFade = 0.52 + Math.max(0, Math.sin(Math.PI * Math.min(1, Math.max(0, progress)))) * 0.48;
      const breathing = 0.68 + Math.max(0, Math.sin(life / pulseCycle * Math.PI * 2 + particles[base + 9])) * 0.32;
      const startFade = 0.3 + Math.min(1, Math.max(0, progress) * 18) * 0.7;
      const tone = particles[base + 10];
      const alpha = (0.22 + tone * 0.56) * progressFade * breathing * startFade;
      const velocityX = nextX - oldX;
      const velocityY = nextY - oldY;
      const velocityLength = Math.hypot(velocityX, velocityY) || 1;
      const trailLength = Math.min(18, Math.max(3.8, velocityLength * (3.4 + particles[base + 5] * 2.2)));
      const tailX = nextX - (velocityX / velocityLength) * trailLength;
      const tailY = nextY - (velocityY / velocityLength) * trailLength;
      const shapeRoll = (index * 37 % 100) / 100;
      if (shapeRoll >= 0.7) {
        drawParticleSegment(tailX, tailY, nextX, nextY, 0.28 + particles[base + 6] * progressFade * 0.34, alpha * 0.76, tone);
      }
      drawParticleHead(nextX, nextY, 0.42 + particles[base + 5] * (tone > 0.92 ? 0.62 : 0.38), Math.min(0.96, alpha * (1 + tone * 0.55)), tone);
      particles[base] = nextX;
      particles[base + 1] = nextY;
      particles[base + 2] = progress;
      particles[base + 7] = life;
    }
  };

  const drawStaticPath = (now: number) => {
    for (let index = 0; index < particleCount; index += 5) {
      const base = index * particleSpread;
      const point = pathPoint(particles[base + 2], particles[base + 3], particles[base + 9], now);
      const tangentLength = Math.hypot(point.dx, point.dy) || 1;
      const segment = 0.8 + particles[base + 5] * 0.9;
      drawParticleSegment(
        point.x - (point.dx / tangentLength) * segment,
        point.y - (point.dy / tangentLength) * segment,
        point.x + (point.dx / tangentLength) * segment,
        point.y + (point.dy / tangentLength) * segment,
        0.42 + particles[base + 6] * 0.45,
        activeRef.value ? 0.16 + particles[base + 10] * 0.18 : 0.08,
        particles[base + 10],
      );
    }
  };

  const compositeBuffer = (mode: "running" | "static") => {
    context.save();
    context.globalCompositeOperation = "lighter";
    context.filter = mode === "running" ? "blur(12px) saturate(170%)" : "blur(9px)";
    context.globalAlpha = mode === "running" ? Math.min(0.48, 0.24 * glowRef.value) : 0.12;
    context.drawImage(bufferCanvas, 0, 0);
    context.restore();
    context.save();
    context.globalCompositeOperation = "lighter";
    context.filter = "blur(3px)";
    context.globalAlpha = mode === "running" ? 0.34 : 0.28;
    context.drawImage(bufferCanvas, 0, 0);
    context.restore();
    context.save();
    context.globalCompositeOperation = "lighter";
    context.globalAlpha = mode === "running" ? 0.9 : 0.52;
    context.drawImage(bufferCanvas, 0, 0);
    context.restore();
  };

  const draw = (now: number) => {
    raf = requestAnimationFrame(draw);
    if (document.hidden || width <= 1 || height <= 1) return;
    const delta = Math.min(32, now - previous);
    previous = now;
    const running = activeRef.value && !reduced.matches;
    if (!running && staticFrameDrawn) return;
    const startX = Math.min(width * props.startX, width * props.endX);
    const endX = Math.max(width * props.startX, width * props.endX);
    const clipPadding = Math.max(58, height * 0.16);
    fadeTrails(running ? 0.14 : 1);
    buffer.clearRect(0, 0, width, height);
    buffer.save();
    buffer.beginPath();
    buffer.rect(Math.max(0, startX - clipPadding), 0, Math.min(width, endX + clipPadding) - Math.max(0, startX - clipPadding), height);
    buffer.clip();
    if (running) {
      drawGuideFilaments(now);
      drawRunningParticles(now, delta);
      drawEndpointBloom(now);
    } else {
      drawStaticPath(now);
    }
    buffer.restore();
    compositeBuffer(running ? "running" : "static");
    staticFrameDrawn = !running;
  };

  resize();
  const observer = new ResizeObserver(resize);
  observer.observe(canvas);
  raf = requestAnimationFrame(draw);

  onBeforeUnmount(() => {
    observer.disconnect();
    cancelAnimationFrame(raf);
  });
});
</script>

<template>
  <canvas ref="canvasRef" class="particle-canvas particle-aether-canvas" data-particle-renderer="path-aether" aria-label="路径约束的数据粒子正在写入运输 NAS" />
</template>
