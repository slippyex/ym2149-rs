// ============================================================================
// Visualization Core - Oscilloscope, Spectrum, and main visualization loop
// ============================================================================

import {
  WAVEFORM_SIZE,
  SPECTRUM_BINS,
  SPECTRUM_DECAY,
  SPECTRUM_ATTACK,
  SPECTRUM_BASE_FREQ,
  BINS_PER_OCTAVE,
  AUDIO_VIS_BUFFER_SIZE,
  NOTE_HISTORY_SIZE,
  NOTE_SCROLL_SPEED,
  COLORS,
} from '../config.ts';
import * as state from '../state.ts';
import { elements, canvases, contexts, channelCanvases, channelContexts, channelNotes, channelMuteButtons } from '../ui/elements.ts';
import { getChannelName, getChannelColor, updateProgressUI, updateLmc1992Display } from '../ui/player.ts';
import { initChannelArrays } from '../audio/context.ts';
import { updateWaveformPlayhead } from './waveform.ts';
import { checkLoopBoundary } from '../audio/playback.ts';
import type { RgbColor, NoteHistoryEntry, ChannelStatesResult, ChannelState } from '../types/index.ts';

// ============================================================================
// Visualization Control
// ============================================================================

export function startVisualization(): void {
  if (state.animationId) return;
  state.setAnimationId(requestAnimationFrame(visualizationLoop));
}

export function stopVisualization(): void {
  if (state.animationId) {
    cancelAnimationFrame(state.animationId);
    state.setAnimationId(null);
  }
}

export function resetVisualization(): void {
  // Reset UI elements but let waveforms decay naturally
  for (const arr of state.channelSpectrums) arr.fill(0);
  state.combinedSpectrum.fill(0);
  for (let i = 0; i < state.channelPhases.length; i++) state.channelPhases[i] = 0;
  for (const noteEl of channelNotes) {
    if (noteEl) noteEl.textContent = '---';
  }
  if (elements.envelopeShape) elements.envelopeShape.textContent = '-';
  drawAllVisualization();
}

export function clearAllWaveforms(): void {
  for (const arr of state.channelWaveforms) arr.fill(0);
  state.monoWaveform.fill(0);
  for (const arr of state.channelSpectrums) arr.fill(0);
  state.combinedSpectrum.fill(0);
  for (const buf of state.channelSampleBuffers) buf.fill(0);
  state.audioSampleBuffer.fill(0);
  for (const history of state.noteHistories) history.length = 0;
  state.setNoteScrollOffset(0);
}

function decayWaveforms(): void {
  const decayFactor = 0.92;
  for (const arr of state.channelWaveforms) {
    for (let i = 0; i < arr.length; i++) {
      const val = arr[i];
      if (val !== undefined) arr[i] = val * decayFactor;
    }
  }
  for (let i = 0; i < state.monoWaveform.length; i++) {
    const val = state.monoWaveform[i];
    if (val !== undefined) state.monoWaveform[i] = val * decayFactor;
  }
}

// ============================================================================
// Main Visualization Loop
// ============================================================================

function visualizationLoop(): void {
  if (state.wasmPlayer && state.isPlaying) {
    updateVisualizationData();
    updateProgressUI();
    updateWaveformPlayhead();
    checkLoopBoundary();
  } else {
    decayWaveforms();
  }
  drawAllVisualization();
  state.setAnimationId(requestAnimationFrame(visualizationLoop));
}

// ============================================================================
// Canvas Setup
// ============================================================================

export function setupCanvas(canvas: HTMLCanvasElement | null): CanvasRenderingContext2D | null {
  if (!canvas) return null;
  const dpr = window.devicePixelRatio || 1;
  const rect = canvas.getBoundingClientRect();
  canvas.width = rect.width * dpr;
  canvas.height = rect.height * dpr;
  const ctx = canvas.getContext('2d');
  if (ctx) ctx.scale(dpr, dpr);
  return ctx;
}

export function setupAllCanvases(): void {
  for (const [key, canvas] of Object.entries(canvases) as [keyof typeof canvases, HTMLCanvasElement | null][]) {
    if (canvas) (contexts as unknown as Record<string, CanvasRenderingContext2D | null>)[key] = setupCanvas(canvas);
  }
  // Setup dynamic channel canvases
  for (let i = 0; i < channelCanvases.osc.length; i++) {
    const oscCanvas = channelCanvases.osc[i];
    const specCanvas = channelCanvases.spec[i];
    channelContexts.osc[i] = oscCanvas ? setupCanvas(oscCanvas) : null;
    channelContexts.spec[i] = specCanvas ? setupCanvas(specCanvas) : null;
  }
}

// ============================================================================
// Channel UI Setup
// ============================================================================

export function setupChannelUI(count: number): void {
  if (count === state.currentChannelCount && channelCanvases.osc.length === count) {
    return;
  }
  state.setCurrentChannelCount(count);

  // Initialize data arrays
  initChannelArrays(count);

  // Clear existing UI
  channelCanvases.osc.length = 0;
  channelCanvases.spec.length = 0;
  channelContexts.osc.length = 0;
  channelContexts.spec.length = 0;
  channelNotes.length = 0;
  channelMuteButtons.length = 0;

  // Determine grid columns
  const cols = count <= 3 ? count : count <= 6 ? 3 : 4;

  // Create oscilloscope channels
  if (elements.oscChannels) {
    elements.oscChannels.innerHTML = '';
    elements.oscChannels.style.gridTemplateColumns = `repeat(${cols}, 1fr)`;
    for (let i = 0; i < count; i++) {
      const color = getChannelColor(i);
      const name = getChannelName(i);
      const div = document.createElement('div');
      div.className = 'relative';
      div.innerHTML = `
        <div class="absolute top-1 left-2 text-xs font-mono z-10" style="color: ${color}99;">CH ${name}</div>
        <div class="absolute top-1 right-2 text-xs font-mono text-gray-500 z-10" id="note${i}">---</div>
        <canvas class="viz-canvas w-full h-20 bg-chip-darker rounded-lg" id="osc${i}"></canvas>
      `;
      elements.oscChannels.appendChild(div);
      channelCanvases.osc.push(div.querySelector('canvas') as HTMLCanvasElement);
      channelNotes.push(div.querySelector(`#note${i}`));
    }
  }

  // Create spectrum channels
  if (elements.specChannels) {
    elements.specChannels.innerHTML = '';
    elements.specChannels.style.gridTemplateColumns = `repeat(${cols}, 1fr)`;
    for (let i = 0; i < count; i++) {
      const color = getChannelColor(i);
      const name = getChannelName(i);
      const div = document.createElement('div');
      div.className = 'relative';
      div.innerHTML = `
        <div class="absolute top-1 left-2 text-xs font-mono z-10" style="color: ${color}99;">CH ${name}</div>
        <canvas class="viz-canvas w-full h-20 bg-chip-darker rounded-lg" id="spec${i}"></canvas>
      `;
      elements.specChannels.appendChild(div);
      channelCanvases.spec.push(div.querySelector('canvas') as HTMLCanvasElement);
    }
  }

  // Create channel mute buttons
  if (elements.channelMutes) {
    const existingLabel = elements.channelMutes.querySelector('span');
    elements.channelMutes.innerHTML = '';
    if (existingLabel) {
      elements.channelMutes.appendChild(existingLabel);
    } else {
      const label = document.createElement('span');
      label.className = 'text-xs text-gray-500 mr-1';
      label.textContent = 'CH:';
      elements.channelMutes.appendChild(label);
    }

    for (let i = 0; i < count; i++) {
      const color = getChannelColor(i);
      const name = getChannelName(i);
      const btn = document.createElement('button');
      btn.className = 'channel-btn px-2 py-1 text-xs font-mono rounded transition-all';
      btn.dataset.channel = String(i);
      btn.style.background = `${color}33`;
      btn.style.color = color;
      btn.textContent = name;
      btn.addEventListener('click', () => {
        if (!state.wasmPlayer) return;
        const muted = state.wasmPlayer.isChannelMuted(i);
        state.wasmPlayer.setChannelMute(i, !muted);
        btn.style.opacity = muted ? '1' : '0.3';
      });
      elements.channelMutes.appendChild(btn);
      channelMuteButtons.push(btn);
    }
  }

  // Setup canvases
  requestAnimationFrame(setupAllCanvases);
}

// ============================================================================
// Visualization Data Update
// ============================================================================

function updateVisualizationData(): void {
  if (!state.wasmPlayer) return;

  const states = state.wasmPlayer.getChannelStates();
  const channelCount = states.channels?.length || 3;

  // Update note displays and history
  for (let i = 0; i < channelCount && i < channelNotes.length; i++) {
    const chState = states.channels[i];
    const note = chState?.note || '---';
    const noteEl = channelNotes[i];
    if (noteEl) {
      noteEl.textContent = note;
    }
    // Record note to history
    if (i < state.noteHistories.length) {
      const history = state.noteHistories[i];
      if (!history) continue;
      const amp = chState?.amplitude || 0;
      const noise = chState?.noiseEnabled || false;
      const env = chState?.envelopeEnabled || false;
      const envShape = env ? String(states.envelope?.shape ?? '') : '';
      const envName = env ? (states.envelope?.shapeName || '') : '';

      const entrySpacing = 12;
      const lastEntry = history.length > 0 ? history[history.length - 1] : null;
      const currentNote = note !== '---' ? note : '';
      const lastNote = lastEntry?.note || '';
      const noteChanged = currentNote !== lastNote && currentNote !== '';

      if (noteChanged) {
        for (const entry of history) {
          entry.y += entrySpacing;
        }
        history.push({ note: currentNote, amp, noise, env, envShape, envName, y: 0 });
        if (history.length > NOTE_HISTORY_SIZE) {
          history.shift();
        }
      }
    }
  }

  if (states.envelope && elements.envelopeShape) {
    elements.envelopeShape.textContent = states.envelope.shapeName || '-';
  }

  updateLmc1992Display();
  synthesizeWaveforms(states);
  updateSpectrum(states);
}

// ============================================================================
// Waveform Synthesis
// ============================================================================

function findTriggerPoint(buffer: Float32Array, writePos: number, bufferSize: number, waveformSize: number): number {
  const searchLen = Math.min(waveformSize / 4, 64);
  const startPos = (writePos - waveformSize + bufferSize) % bufferSize;

  for (let i = 1; i < searchLen; i++) {
    const pos = (startPos + i) % bufferSize;
    const prevPos = (startPos + i - 1) % bufferSize;
    const curr = buffer[pos] ?? 0;
    const prev = buffer[prevPos] ?? 0;
    if (prev <= 0 && curr > 0) {
      return i;
    }
  }
  return 0;
}

function synthesizeWaveforms(states: ChannelStatesResult): void {
  const channelCount = states.channels?.length || 0;

  // Use real per-channel samples from ring buffers
  for (let ch = 0; ch < channelCount && ch < state.channelWaveforms.length; ch++) {
    const waveform = state.channelWaveforms[ch];
    if (!waveform) continue;

    const srcBuffer = state.channelSampleBuffers[ch];
    if (srcBuffer) {
      const triggerOffset = findTriggerPoint(srcBuffer, state.channelSampleWritePos, AUDIO_VIS_BUFFER_SIZE, WAVEFORM_SIZE);
      const startPos = (state.channelSampleWritePos - WAVEFORM_SIZE + triggerOffset + AUDIO_VIS_BUFFER_SIZE) % AUDIO_VIS_BUFFER_SIZE;

      for (let i = 0; i < WAVEFORM_SIZE; i++) {
        const readPos = (startPos + i) % AUDIO_VIS_BUFFER_SIZE;
        waveform[i] = srcBuffer[readPos] ?? 0;
      }
    }
  }

  // Mono waveform
  const monoOversample = 4;
  const monoReadSize = WAVEFORM_SIZE * monoOversample;
  const monoTriggerOffset = findTriggerPoint(state.audioSampleBuffer, state.audioSampleWritePos, AUDIO_VIS_BUFFER_SIZE, monoReadSize);
  const monoStartPos = (state.audioSampleWritePos - monoReadSize + monoTriggerOffset + AUDIO_VIS_BUFFER_SIZE) % AUDIO_VIS_BUFFER_SIZE;

  for (let i = 0; i < WAVEFORM_SIZE; i++) {
    let sum = 0;
    for (let j = 0; j < monoOversample; j++) {
      const readPos = (monoStartPos + i * monoOversample + j) % AUDIO_VIS_BUFFER_SIZE;
      sum += state.audioSampleBuffer[readPos] ?? 0;
    }
    state.monoWaveform[i] = sum / monoOversample;
  }
}

// ============================================================================
// Spectrum Update
// ============================================================================

function freqToBin(freq: number): number {
  if (freq <= 0) return 0;
  const octavesAboveC1 = Math.log2(freq / SPECTRUM_BASE_FREQ);
  const bin = Math.round(octavesAboveC1 * BINS_PER_OCTAVE);
  return Math.max(0, Math.min(SPECTRUM_BINS - 1, bin));
}

// Pre-computed noise pattern
const NOISE_PATTERN = new Float32Array(36);
for (let i = 0; i < 36; i++) {
  NOISE_PATTERN[i] = 0.5 + ((i * 7 + 3) % 11) / 22;
}
let noisePatternOffset = 0;

function smoothValue(current: number, target: number): number {
  if (target > current) {
    return current + (target - current) * SPECTRUM_ATTACK;
  } else {
    return current * SPECTRUM_DECAY;
  }
}

function updateSpectrum(states: ChannelStatesResult): void {
  const channelCount = states.channels?.length || 0;

  // Ensure pre-allocated target arrays exist
  if (state.spectrumTargets.length !== state.channelSpectrums.length) {
    state.setSpectrumTargets(
      state.channelSpectrums.map(() => new Float32Array(SPECTRUM_BINS))
    );
  }

  // Clear target arrays
  for (const target of state.spectrumTargets) {
    target.fill(0);
  }

  noisePatternOffset = (noisePatternOffset + 1) % 36;

  for (let ch = 0; ch < channelCount && ch < state.channelSpectrums.length; ch++) {
    const chState = states.channels[ch];
    if (!chState) continue;

    const amplitude = chState.amplitude || 0;
    if (amplitude <= 0) continue;

    const target = state.spectrumTargets[ch];
    if (!target) continue;

    const toneEnabled = chState.toneEnabled;
    const noiseEnabled = chState.noiseEnabled;
    const envEnabled = chState.envelopeEnabled;
    const frequency = chState.frequency || 0;

    // DAC channels
    if ((chState as ChannelState & { isDac?: boolean }).isDac) {
      for (let bin = 8; bin <= 24; bin++) {
        const falloff = 1.0 - Math.abs(bin - 16) / 16;
        target[bin] = Math.max(target[bin] ?? 0, amplitude * falloff * 0.8);
      }
      continue;
    }

    // Tone
    if (toneEnabled && frequency > 0) {
      const baseBin = freqToBin(frequency);
      target[baseBin] = Math.max(target[baseBin] ?? 0, amplitude);
      const harm2 = Math.min(SPECTRUM_BINS - 1, baseBin + BINS_PER_OCTAVE);
      const harm3 = Math.min(SPECTRUM_BINS - 1, baseBin + Math.floor(BINS_PER_OCTAVE * 1.58));
      target[harm2] = Math.max(target[harm2] ?? 0, amplitude * 0.3);
      target[harm3] = Math.max(target[harm3] ?? 0, amplitude * 0.15);
    }

    // Noise
    if (noiseEnabled) {
      const noiseAmp = amplitude * (toneEnabled ? 0.6 : 0.9);
      for (let bin = 20; bin < 56; bin++) {
        const emphasis = 1.0 - Math.abs(bin - 38) / 36;
        const patternIdx = (bin - 20 + noisePatternOffset + ch * 7) % 36;
        const noiseLevel = noiseAmp * emphasis * (NOISE_PATTERN[patternIdx] ?? 0.5);
        target[bin] = Math.max(target[bin] ?? 0, noiseLevel);
      }
    }

    // Envelope
    if (envEnabled && !noiseEnabled) {
      if (frequency > 0) {
        const baseBin = freqToBin(frequency);
        for (let h = 2; h <= 8; h++) {
          const harmBin = Math.min(SPECTRUM_BINS - 1, baseBin + Math.floor(BINS_PER_OCTAVE * Math.log2(h)));
          const harmAmp = amplitude / h * 0.7;
          target[harmBin] = Math.max(target[harmBin] ?? 0, harmAmp);
        }
      } else {
        for (let bin = 4; bin <= 32; bin++) {
          const falloff = 1.0 - Math.abs(bin - 16) / 28;
          target[bin] = Math.max(target[bin] ?? 0, amplitude * falloff * 0.7);
        }
      }
    }

    // Fallback
    if (!toneEnabled && !noiseEnabled && !envEnabled) {
      for (let bin = 8; bin <= 28; bin++) {
        const falloff = 1.0 - Math.abs(bin - 18) / 20;
        target[bin] = Math.max(target[bin] ?? 0, amplitude * falloff * 0.8);
      }
    }
  }

  // Apply smoothing
  for (let ch = 0; ch < state.channelSpectrums.length; ch++) {
    const spectrum = state.channelSpectrums[ch];
    const target = state.spectrumTargets[ch];
    if (!spectrum || !target) continue;

    for (let i = 0; i < SPECTRUM_BINS; i++) {
      spectrum[i] = smoothValue(spectrum[i] ?? 0, target[i] ?? 0);
    }
  }

  // Combined spectrum
  for (let i = 0; i < SPECTRUM_BINS; i++) {
    let maxTarget = 0;
    for (let ch = 0; ch < state.channelSpectrums.length; ch++) {
      maxTarget = Math.max(maxTarget, state.channelSpectrums[ch]?.[i] ?? 0);
    }
    state.combinedSpectrum[i] = smoothValue(state.combinedSpectrum[i] ?? 0, maxTarget);
  }
}

// ============================================================================
// Drawing
// ============================================================================

export function drawAllVisualization(): void {
  // Scroll note entries
  for (const history of state.noteHistories) {
    for (const entry of history) {
      entry.y += NOTE_SCROLL_SPEED;
    }
    while (history.length > 0 && (history[0]?.y ?? 0) > 150) {
      history.shift();
    }
  }

  // Draw each channel's oscilloscope
  for (let ch = 0; ch < state.channelWaveforms.length; ch++) {
    const ctx = channelContexts.osc[ch];
    const color = getChannelColor(ch);
    const history = ch < state.noteHistories.length ? state.noteHistories[ch] : null;
    const waveform = state.channelWaveforms[ch];
    if (waveform) drawOscilloscope(ctx ?? null, waveform, color, history ?? null);
  }
  // Draw mono combined oscilloscope
  drawOscilloscope(contexts.oscMono, state.monoWaveform, COLORS.green, null);

  // Draw each channel's spectrum
  for (let ch = 0; ch < state.channelSpectrums.length; ch++) {
    const ctx = channelContexts.spec[ch];
    const color = getChannelColor(ch);
    const spectrum = state.channelSpectrums[ch];
    if (spectrum) drawSpectrum(ctx ?? null, spectrum, color, ch);
  }
  // Draw combined spectrum
  drawSpectrum(contexts.specCombined, state.combinedSpectrum, COLORS.green, -1);
}

function hexToRgb(hex: string): RgbColor {
  const result = /^#?([a-f\d]{2})([a-f\d]{2})([a-f\d]{2})$/i.exec(hex);
  return result ? {
    r: parseInt(result[1] ?? 'ff', 16),
    g: parseInt(result[2] ?? 'ff', 16),
    b: parseInt(result[3] ?? 'ff', 16)
  } : { r: 139, g: 92, b: 246 };
}

function lightenColor(hex: string, percent: number): string {
  const rgb = hexToRgb(hex);
  const r = Math.min(255, rgb.r + (255 - rgb.r) * percent / 100);
  const g = Math.min(255, rgb.g + (255 - rgb.g) * percent / 100);
  const b = Math.min(255, rgb.b + (255 - rgb.b) * percent / 100);
  return `rgb(${Math.round(r)}, ${Math.round(g)}, ${Math.round(b)})`;
}

function drawOscilloscope(ctx: CanvasRenderingContext2D | null, data: Float32Array, color: string, noteHistory: NoteHistoryEntry[] | null): void {
  if (!ctx) return;
  const canvas = ctx.canvas;
  const dpr = window.devicePixelRatio || 1;
  const w = canvas.width / dpr;
  const h = canvas.height / dpr;
  if (w === 0 || h === 0) return;

  ctx.fillStyle = 'rgba(10, 10, 15, 1)';
  ctx.fillRect(0, 0, w, h);

  // Grid
  ctx.strokeStyle = 'rgba(139, 92, 246, 0.08)';
  ctx.lineWidth = 1;
  ctx.beginPath();
  for (let i = 1; i < 8; i++) {
    const x = (w / 8) * i;
    ctx.moveTo(x, 0);
    ctx.lineTo(x, h);
  }
  for (let i = 1; i < 4; i++) {
    const y = (h / 4) * i;
    ctx.moveTo(0, y);
    ctx.lineTo(w, y);
  }
  ctx.stroke();

  // Center line
  ctx.strokeStyle = 'rgba(139, 92, 246, 0.15)';
  ctx.beginPath();
  ctx.moveTo(0, h / 2);
  ctx.lineTo(w, h / 2);
  ctx.stroke();

  // Waveform
  ctx.beginPath();
  const step = w / data.length;
  for (let i = 0; i < data.length; i++) {
    const x = i * step;
    const y = h / 2 - (data[i] ?? 0) * (h / 2) * 0.9;
    if (i === 0) ctx.moveTo(x, y);
    else ctx.lineTo(x, y);
  }

  ctx.strokeStyle = color;
  ctx.lineWidth = 1;
  ctx.stroke();

  // Draw note history
  if (noteHistory && noteHistory.length > 0) {
    const rgb = hexToRgb(color);
    const dimR = Math.round(rgb.r * 0.55);
    const dimG = Math.round(rgb.g * 0.55);
    const dimB = Math.round(rgb.b * 0.55);

    ctx.textAlign = 'center';
    ctx.font = 'bold 9px monospace';
    const centerX = w / 2;

    for (const entry of noteHistory) {
      const yPos = h - 8 - entry.y;
      if (yPos < -10 || yPos > h + 10) continue;

      const age = entry.y / 120;
      const alpha = Math.max(0.1, 0.75 - age * 0.6);

      if (entry.note) {
        const noiseInd = entry.noise ? 'N' : '-';
        const envInd = entry.env ? 'E' : '-';
        const shapeText = entry.env && entry.envName ? entry.envName : '';
        const fullText = `${entry.note} ${noiseInd} ${envInd}${shapeText ? ' ' + shapeText : ''}`;

        ctx.shadowColor = 'rgba(0, 0, 0, 0.9)';
        ctx.shadowBlur = 10;

        ctx.fillStyle = `rgba(${dimR}, ${dimG}, ${dimB}, ${alpha})`;
        ctx.fillText(fullText, centerX, yPos);

        ctx.shadowBlur = 0;
      }
    }
  }
}

// Peak hold state
const peakHoldState = new Map<string, { peaks: Float32Array; holdTimers: Uint8Array }>();
const PEAK_HOLD_TIME = 30;
const PEAK_FALL_SPEED = 0.02;

function drawSpectrum(ctx: CanvasRenderingContext2D | null, data: Float32Array, color: string, channelIndex: number): void {
  if (!ctx) return;
  const canvas = ctx.canvas;
  const dpr = window.devicePixelRatio || 1;
  const w = canvas.width / dpr;
  const h = canvas.height / dpr;
  if (w === 0 || h === 0) return;

  const canvasId = canvas.id || `canvas_${channelIndex}`;
  if (!peakHoldState.has(canvasId)) {
    peakHoldState.set(canvasId, {
      peaks: new Float32Array(SPECTRUM_BINS),
      holdTimers: new Uint8Array(SPECTRUM_BINS),
    });
  }
  const peakState = peakHoldState.get(canvasId)!;

  // Background
  const bgGrad = ctx.createLinearGradient(0, 0, 0, h);
  bgGrad.addColorStop(0, '#0a0a0f');
  bgGrad.addColorStop(1, '#0d0d15');
  ctx.fillStyle = bgGrad;
  ctx.fillRect(0, 0, w, h);

  // Grid
  ctx.strokeStyle = 'rgba(139, 92, 246, 0.06)';
  ctx.lineWidth = 1;
  ctx.beginPath();
  for (let i = BINS_PER_OCTAVE; i < SPECTRUM_BINS; i += BINS_PER_OCTAVE) {
    const x = (i / SPECTRUM_BINS) * w;
    ctx.moveTo(x, 0);
    ctx.lineTo(x, h);
  }
  for (let i = 1; i <= 4; i++) {
    const y = (i / 5) * h;
    ctx.moveTo(0, y);
    ctx.lineTo(w, y);
  }
  ctx.stroke();

  const rgb = hexToRgb(color);
  const noiseRgb = { r: Math.min(255, rgb.r + 60), g: Math.min(255, rgb.g + 80), b: Math.min(255, rgb.b + 40) };
  const totalBarWidth = w / SPECTRUM_BINS;
  const barGap = Math.max(1, Math.floor(totalBarWidth * 0.15));
  const barWidth = Math.max(1.5, totalBarWidth - barGap);
  const maxHeight = h * 0.85;
  const cornerRadius = Math.min(barWidth / 2, 2);

  for (let i = 0; i < SPECTRUM_BINS; i++) {
    const value = data[i] ?? 0;
    const barHeight = Math.max(0, value * maxHeight);
    const x = i * totalBarWidth + barGap / 2;
    const y = h - barHeight;

    // Peak hold
    if (value >= (peakState.peaks[i] ?? 0)) {
      peakState.peaks[i] = value;
      peakState.holdTimers[i] = PEAK_HOLD_TIME;
    } else if ((peakState.holdTimers[i] ?? 0) > 0) {
      const currentTimer = peakState.holdTimers[i];
      if (currentTimer !== undefined) peakState.holdTimers[i] = currentTimer - 1;
    } else {
      peakState.peaks[i] = Math.max(0, (peakState.peaks[i] ?? 0) - PEAK_FALL_SPEED);
    }

    const noiseBlend = i < 20 ? 0 : Math.min(1, (i - 20) / 24);
    const barR = Math.round(rgb.r + (noiseRgb.r - rgb.r) * noiseBlend);
    const barG = Math.round(rgb.g + (noiseRgb.g - rgb.g) * noiseBlend);
    const barB = Math.round(rgb.b + (noiseRgb.b - rgb.b) * noiseBlend);
    const barColor = `rgb(${barR}, ${barG}, ${barB})`;

    if (barHeight > 1) {
      ctx.shadowColor = barColor;
      ctx.shadowBlur = Math.min(8, barWidth * 2);

      const barGrad = ctx.createLinearGradient(x, h, x, y);
      barGrad.addColorStop(0, `rgba(${barR}, ${barG}, ${barB}, 0.2)`);
      barGrad.addColorStop(0.4, `rgba(${barR}, ${barG}, ${barB}, 0.7)`);
      barGrad.addColorStop(0.8, barColor);
      barGrad.addColorStop(1, lightenColor(barColor, 40));

      ctx.fillStyle = barGrad;
      ctx.beginPath();
      ctx.roundRect(x, y, barWidth, barHeight, [cornerRadius, cornerRadius, 0, 0]);
      ctx.fill();

      ctx.shadowBlur = 0;

      // Top cap
      const capGrad = ctx.createLinearGradient(x, y, x, y + 3);
      capGrad.addColorStop(0, lightenColor(color, 60));
      capGrad.addColorStop(1, 'transparent');
      ctx.fillStyle = capGrad;
      ctx.beginPath();
      ctx.roundRect(x, y, barWidth, Math.min(3, barHeight), [cornerRadius, cornerRadius, 0, 0]);
      ctx.fill();
    }

    // Peak indicator
    const peakY = h - (peakState.peaks[i] ?? 0) * maxHeight;
    if ((peakState.peaks[i] ?? 0) > 0.02 && peakY < h - 3) {
      ctx.shadowColor = lightenColor(barColor, 40);
      ctx.shadowBlur = Math.min(4, barWidth);
      ctx.fillStyle = lightenColor(barColor, 70);
      const peakHeight = Math.max(1, Math.min(2, barWidth * 0.5));
      ctx.fillRect(x, peakY - peakHeight, barWidth, peakHeight);
      ctx.shadowBlur = 0;
    }
  }

  // Reflection
  ctx.globalAlpha = 0.15;
  ctx.scale(1, -1);
  ctx.translate(0, -h * 2);

  for (let i = 0; i < SPECTRUM_BINS; i++) {
    const value = data[i] ?? 0;
    const barHeight = Math.min(value * maxHeight * 0.3, h * 0.15);
    const x = i * totalBarWidth + barGap / 2;

    if (barHeight > 1) {
      const reflGrad = ctx.createLinearGradient(x, h, x, h - barHeight);
      reflGrad.addColorStop(0, color);
      reflGrad.addColorStop(1, 'transparent');
      ctx.fillStyle = reflGrad;
      ctx.fillRect(x, h - barHeight, barWidth, barHeight);
    }
  }

  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  ctx.globalAlpha = 1;

  // Bottom line
  const lineGrad = ctx.createLinearGradient(0, h - 1, w, h - 1);
  lineGrad.addColorStop(0, 'transparent');
  lineGrad.addColorStop(0.2, `rgba(${rgb.r}, ${rgb.g}, ${rgb.b}, 0.3)`);
  lineGrad.addColorStop(0.5, `rgba(${rgb.r}, ${rgb.g}, ${rgb.b}, 0.5)`);
  lineGrad.addColorStop(0.8, `rgba(${rgb.r}, ${rgb.g}, ${rgb.b}, 0.3)`);
  lineGrad.addColorStop(1, 'transparent');
  ctx.strokeStyle = lineGrad;
  ctx.lineWidth = 1;
  ctx.beginPath();
  ctx.moveTo(0, h - 0.5);
  ctx.lineTo(w, h - 0.5);
  ctx.stroke();
}
