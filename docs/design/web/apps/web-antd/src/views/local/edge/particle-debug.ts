export type ParticleDebugState =
  | 'complete'
  | 'denied'
  | 'error'
  | 'idle'
  | 'loading'
  | 'paused'
  | 'running';

export interface ParticleDebugSettings {
  color: null | string;
  glowStrength: number;
  speedMultiplier: number;
  state: null | ParticleDebugState;
}

export const defaultParticleDebugSettings = (): ParticleDebugSettings => ({
  color: null,
  glowStrength: 1,
  speedMultiplier: 1,
  state: null,
});
