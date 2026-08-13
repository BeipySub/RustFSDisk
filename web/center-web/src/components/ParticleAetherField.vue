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
const activeRef = ref(props.active);
const speedRef = ref(props.speed);
const glowRef = ref(props.glow);
const paletteRef = ref(props.palette);

const paletteColors: Record<ParticlePalette, { base: ParticleColor; accent: ParticleColor }> = {
  semantic: { base: [32, 140, 255], accent: [54, 216, 255] },
  electric: { base: [22, 142, 255], accent: [203, 244, 255] },
  cyan: { base: [48, 205, 231], accent: [178, 250, 255] },
  emerald: { base: [42, 218, 157], accent: [174, 255, 217] },
  amber: { base: [255, 174, 66], accent: [255, 226, 154] },
  violet: { base: [164, 109, 255], accent: [226, 205, 255] },
};

watch(() => props.active, (value) => {
  activeRef.value = value;
});
watch(() => props.speed, (value) => {
  speedRef.value = value;
});
watch(() => props.glow, (value) => {
  glowRef.value = value;
});
watch(() => props.palette, (value) => {
  paletteRef.value = value;
});

onMounted(() => {
  const canvas = canvasRef.value;
  const context = canvas?.getContext("2d", { alpha: true, desynchronized: true });
  if (!canvas || !context) return;

  let width = 0;
  let height = 0;
  let raf = 0;
  let previous = performance.now();
  const reduced = window.matchMedia("(prefers-reduced-motion: reduce)");
  const particleCount = 360;
  const particles = Array.from({ length: particleCount }, (_, index) => ({
    progress: (index / particleCount + Math.random() * 0.08) % 1,
    offset: (Math.random() * 2 - 1) * 0.9,
    size: 0.7 + Math.random() * 1.8,
    speed: 0.00007 + Math.random() * 0.00016,
    phase: Math.random() * Math.PI * 2,
  }));

  const rgba = (color: ParticleColor, alpha: number) => `rgba(${color[0]}, ${color[1]}, ${color[2]}, ${alpha})`;

  const pointAt = (progress: number, offset: number, now: number) => {
    const t = Math.max(0, Math.min(1, progress));
    const inverse = 1 - t;
    const p0 = { x: width * props.startX, y: height * props.startY };
    const p3 = { x: width * props.endX, y: height * props.endY };
    const p1 = { x: p0.x + (p3.x - p0.x) * 0.28, y: p0.y + (p3.y - p0.y) * 0.02 };
    const p2 = { x: p0.x + (p3.x - p0.x) * 0.68, y: p0.y + (p3.y - p0.y) * 0.96 };
    const x = inverse ** 3 * p0.x + 3 * inverse ** 2 * t * p1.x + 3 * inverse * t ** 2 * p2.x + t ** 3 * p3.x;
    const y = inverse ** 3 * p0.y + 3 * inverse ** 2 * t * p1.y + 3 * inverse * t ** 2 * p2.y + t ** 3 * p3.y;
    const wave = Math.sin(t * Math.PI * 7 + offset * 2.8 + now * 0.002) * 0.018;
    const spread = Math.sin(t * Math.PI) * height * 0.08 * (offset + wave);
    return { x, y: y + spread };
  };

  const resize = () => {
    const rect = canvas.getBoundingClientRect();
    width = Math.max(1, rect.width);
    height = Math.max(1, rect.height);
    canvas.width = Math.round(width);
    canvas.height = Math.round(height);
  };

  const draw = (now: number) => {
    raf = requestAnimationFrame(draw);
    if (document.hidden || width <= 1 || height <= 1) return;
    const delta = Math.min(34, now - previous);
    previous = now;
    context.globalCompositeOperation = "source-over";
    context.clearRect(0, 0, width, height);

    const theme = paletteColors[paletteRef.value];
    const running = activeRef.value && !reduced.matches;
    context.globalCompositeOperation = "lighter";
    context.lineCap = "round";
    context.lineJoin = "round";

    for (let band = 0; band < 10; band += 1) {
      context.beginPath();
      for (let step = 0; step <= 80; step += 1) {
        const progress = step / 80;
        const offset = (band / 9 - 0.5) * 0.65;
        const point = pointAt(progress, offset, now * 0.34);
        if (step === 0) context.moveTo(point.x, point.y);
        else context.lineTo(point.x, point.y);
      }
      context.strokeStyle = rgba(theme.base, 0.035 + band * 0.004);
      context.lineWidth = 0.4;
      context.stroke();
    }

    for (const particle of particles) {
      if (running) {
        particle.progress += particle.speed * delta * speedRef.value;
        if (particle.progress > 1.04) particle.progress = -Math.random() * 0.08;
      }
      const point = pointAt(particle.progress, particle.offset, now + particle.phase * 1000);
      const alpha = (0.22 + Math.sin(Math.PI * Math.max(0, Math.min(1, particle.progress))) * 0.58) * glowRef.value;
      const radius = particle.size * (particle.progress > 0.86 ? 1.25 : 1);
      context.fillStyle = rgba(particle.progress > 0.9 ? theme.accent : theme.base, Math.min(0.9, alpha));
      context.beginPath();
      context.arc(point.x, point.y, radius, 0, Math.PI * 2);
      context.fill();
    }
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
  <canvas
    ref="canvasRef"
    class="particle-canvas particle-aether-canvas"
    data-particle-renderer="path-aether"
    aria-label="路径约束的数据粒子正在导入运输盘"
  />
</template>
