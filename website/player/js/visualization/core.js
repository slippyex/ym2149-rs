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
} from '../config.js';
import * as state from '../state.js';
import { elements, canvases, contexts, channelCanvases, channelContexts, channelNotes, channelMuteButtons } from '../ui/elements.js';
import { getChannelName, getChannelColor, updateProgressUI, updateLmc1992Display } from '../ui/player.js';
import { initChannelArrays } from '../audio/context.js';
import { updateWaveformPlayhead } from './waveform.js';
import { checkLoopBoundary } from '../audio/playback.js';

// ============================================================================
// Visualization Control
// ============================================================================

export function startVisualization() {
    if (state.animationId) return;
    state.setAnimationId(requestAnimationFrame(visualizationLoop));
}

export function stopVisualization() {
    if (state.animationId) {
        cancelAnimationFrame(state.animationId);
        state.setAnimationId(null);
    }
}

export function resetVisualization() {
    // Reset UI elements but let waveforms decay naturally
    for (const arr of state.channelSpectrums) arr.fill(0);
    state.combinedSpectrum.fill(0);
    for (let i = 0; i < state.channelPhases.length; i++) state.channelPhases[i] = 0;
    for (const noteEl of channelNotes) {
        if (noteEl) noteEl.textContent = "---";
    }
    elements.envelopeShape.textContent = "-";
    drawAllVisualization();
}

export function clearAllWaveforms() {
    for (const arr of state.channelWaveforms) arr.fill(0);
    state.monoWaveform.fill(0);
    for (const arr of state.channelSpectrums) arr.fill(0);
    state.combinedSpectrum.fill(0);
    for (const buf of state.channelSampleBuffers) buf.fill(0);
    state.audioSampleBuffer.fill(0);
    for (const history of state.noteHistories) history.length = 0;
    state.setNoteScrollOffset(0);
}

function decayWaveforms() {
    const decayFactor = 0.92;
    for (const arr of state.channelWaveforms) {
        for (let i = 0; i < arr.length; i++) {
            arr[i] *= decayFactor;
        }
    }
    for (let i = 0; i < state.monoWaveform.length; i++) {
        state.monoWaveform[i] *= decayFactor;
    }
}

// ============================================================================
// Main Visualization Loop
// ============================================================================

function visualizationLoop() {
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

export function setupCanvas(canvas) {
    if (!canvas) return;
    const dpr = window.devicePixelRatio || 1;
    const rect = canvas.getBoundingClientRect();
    canvas.width = rect.width * dpr;
    canvas.height = rect.height * dpr;
    const ctx = canvas.getContext("2d");
    ctx.scale(dpr, dpr);
    return ctx;
}

export function setupAllCanvases() {
    for (const [key, canvas] of Object.entries(canvases)) {
        if (canvas) contexts[key] = setupCanvas(canvas);
    }
    // Setup dynamic channel canvases
    for (let i = 0; i < channelCanvases.osc.length; i++) {
        channelContexts.osc[i] = setupCanvas(channelCanvases.osc[i]);
        channelContexts.spec[i] = setupCanvas(channelCanvases.spec[i]);
    }
}

// ============================================================================
// Channel UI Setup
// ============================================================================

export function setupChannelUI(count) {
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
    elements.oscChannels.innerHTML = "";
    elements.oscChannels.style.gridTemplateColumns = `repeat(${cols}, 1fr)`;
    for (let i = 0; i < count; i++) {
        const color = getChannelColor(i);
        const name = getChannelName(i);
        const div = document.createElement("div");
        div.className = "relative";
        div.innerHTML = `
            <div class="absolute top-1 left-2 text-xs font-mono z-10" style="color: ${color}99;">CH ${name}</div>
            <div class="absolute top-1 right-2 text-xs font-mono text-gray-500 z-10" id="note${i}">---</div>
            <canvas class="viz-canvas w-full h-20 bg-chip-darker rounded-lg" id="osc${i}"></canvas>
        `;
        elements.oscChannels.appendChild(div);
        channelCanvases.osc.push(div.querySelector("canvas"));
        channelNotes.push(div.querySelector(`#note${i}`));
    }

    // Create spectrum channels
    elements.specChannels.innerHTML = "";
    elements.specChannels.style.gridTemplateColumns = `repeat(${cols}, 1fr)`;
    for (let i = 0; i < count; i++) {
        const color = getChannelColor(i);
        const name = getChannelName(i);
        const div = document.createElement("div");
        div.className = "relative";
        div.innerHTML = `
            <div class="absolute top-1 left-2 text-xs font-mono z-10" style="color: ${color}99;">CH ${name}</div>
            <canvas class="viz-canvas w-full h-20 bg-chip-darker rounded-lg" id="spec${i}"></canvas>
        `;
        elements.specChannels.appendChild(div);
        channelCanvases.spec.push(div.querySelector("canvas"));
    }

    // Create channel mute buttons
    const existingLabel = elements.channelMutes.querySelector("span");
    elements.channelMutes.innerHTML = "";
    if (existingLabel) {
        elements.channelMutes.appendChild(existingLabel);
    } else {
        const label = document.createElement("span");
        label.className = "text-xs text-gray-500 mr-1";
        label.textContent = "CH:";
        elements.channelMutes.appendChild(label);
    }

    for (let i = 0; i < count; i++) {
        const color = getChannelColor(i);
        const name = getChannelName(i);
        const btn = document.createElement("button");
        btn.className = "channel-btn px-2 py-1 text-xs font-mono rounded transition-all";
        btn.dataset.channel = i;
        btn.style.background = `${color}33`;
        btn.style.color = color;
        btn.textContent = name;
        btn.addEventListener("click", () => {
            if (!state.wasmPlayer) return;
            const muted = state.wasmPlayer.isChannelMuted(i);
            state.wasmPlayer.setChannelMute(i, !muted);
            btn.style.opacity = muted ? "1" : "0.3";
        });
        elements.channelMutes.appendChild(btn);
        channelMuteButtons.push(btn);
    }

    // Setup canvases
    requestAnimationFrame(setupAllCanvases);
}

// ============================================================================
// Visualization Data Update
// ============================================================================

function updateVisualizationData() {
    if (!state.wasmPlayer) return;

    const states = state.wasmPlayer.getChannelStates();
    const channelCount = states.channels?.length || 3;

    // Update note displays and history
    for (let i = 0; i < channelCount && i < channelNotes.length; i++) {
        const note = states.channels[i]?.note || "---";
        if (channelNotes[i]) {
            channelNotes[i].textContent = note;
        }
        // Record note to history
        if (i < state.noteHistories.length) {
            const history = state.noteHistories[i];
            const chState = states.channels[i];
            const amp = chState?.amplitude || 0;
            const noise = chState?.noiseEnabled || false;
            const env = chState?.envelopeEnabled || false;
            const envShape = env ? (states.envelope?.shape ?? "") : "";
            const envName = env ? (states.envelope?.shapeName || "") : "";

            const entrySpacing = 12;
            const lastEntry = history.length > 0 ? history[history.length - 1] : null;
            const currentNote = note !== "---" ? note : "";
            const lastNote = lastEntry?.note || "";
            const noteChanged = currentNote !== lastNote && currentNote !== "";

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

    if (states.envelope) {
        elements.envelopeShape.textContent = states.envelope.shapeName || "-";
    }

    updateLmc1992Display();
    synthesizeWaveforms(states);
    updateSpectrum(states);
}

// ============================================================================
// Waveform Synthesis
// ============================================================================

function findTriggerPoint(buffer, writePos, bufferSize, waveformSize) {
    const searchLen = Math.min(waveformSize / 4, 64);
    const startPos = (writePos - waveformSize + bufferSize) % bufferSize;

    for (let i = 1; i < searchLen; i++) {
        const pos = (startPos + i) % bufferSize;
        const prevPos = (startPos + i - 1) % bufferSize;
        const curr = buffer[pos];
        const prev = buffer[prevPos];
        if (prev <= 0 && curr > 0) {
            return i;
        }
    }
    return 0;
}

function synthesizeWaveforms(states) {
    const channelCount = states.channels?.length || 0;

    // Use real per-channel samples from ring buffers
    for (let ch = 0; ch < channelCount && ch < state.channelWaveforms.length; ch++) {
        const waveform = state.channelWaveforms[ch];
        if (!waveform) continue;

        if (ch < state.channelSampleBuffers.length && state.channelSampleBuffers[ch]) {
            const srcBuffer = state.channelSampleBuffers[ch];
            const triggerOffset = findTriggerPoint(srcBuffer, state.channelSampleWritePos, AUDIO_VIS_BUFFER_SIZE, WAVEFORM_SIZE);
            const startPos = (state.channelSampleWritePos - WAVEFORM_SIZE + triggerOffset + AUDIO_VIS_BUFFER_SIZE) % AUDIO_VIS_BUFFER_SIZE;

            for (let i = 0; i < WAVEFORM_SIZE; i++) {
                const readPos = (startPos + i) % AUDIO_VIS_BUFFER_SIZE;
                waveform[i] = srcBuffer[readPos];
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
            sum += state.audioSampleBuffer[readPos];
        }
        state.monoWaveform[i] = sum / monoOversample;
    }
}

// ============================================================================
// Spectrum Update
// ============================================================================

function freqToBin(freq) {
    if (freq <= 0) return 0;
    const octavesAboveC1 = Math.log2(freq / SPECTRUM_BASE_FREQ);
    const bin = Math.round(octavesAboveC1 * BINS_PER_OCTAVE);
    return Math.max(0, Math.min(SPECTRUM_BINS - 1, bin));
}

function updateSpectrum(states) {
    const channelCount = states.channels?.length || 0;

    // Helper: Apply attack/decay smoothing to a spectrum bin
    function smoothValue(current, target) {
        if (target > current) {
            // Attack: gradually rise toward target (resistance when going up)
            return current + (target - current) * SPECTRUM_ATTACK;
        } else {
            // Decay: gradually fall
            return current * SPECTRUM_DECAY;
        }
    }

    // Build target values for each channel
    const targets = state.channelSpectrums.map(() => new Float32Array(SPECTRUM_BINS));

    for (let ch = 0; ch < channelCount && ch < state.channelSpectrums.length; ch++) {
        const chState = states.channels[ch];
        if (!chState) continue;

        const amplitude = chState.amplitude || 0;
        if (amplitude <= 0) continue;

        const toneEnabled = chState.toneEnabled;
        const noiseEnabled = chState.noiseEnabled;
        const envEnabled = chState.envelopeEnabled;
        const frequency = chState.frequency || 0;

        // DAC channels: spread across mid frequencies
        if (chState.isDac) {
            for (let bin = 8; bin <= 24; bin++) {
                const falloff = 1.0 - Math.abs(bin - 16) / 16;
                targets[ch][bin] = Math.max(targets[ch][bin], amplitude * falloff * 0.8);
            }
            continue;
        }

        // Tone: energy at fundamental frequency + harmonics
        if (toneEnabled && frequency > 0) {
            const baseBin = freqToBin(frequency);
            // Fundamental
            targets[ch][baseBin] = Math.max(targets[ch][baseBin], amplitude);
            // Add subtle harmonics (2nd, 3rd) for richer display
            const harm2 = Math.min(SPECTRUM_BINS - 1, baseBin + BINS_PER_OCTAVE);
            const harm3 = Math.min(SPECTRUM_BINS - 1, baseBin + Math.floor(BINS_PER_OCTAVE * 1.58));
            targets[ch][harm2] = Math.max(targets[ch][harm2], amplitude * 0.3);
            targets[ch][harm3] = Math.max(targets[ch][harm3], amplitude * 0.15);
        }

        // Noise: broadband energy in upper frequencies
        if (noiseEnabled) {
            // Noise spreads across mid-high frequencies (bins 20-55 for 64 bins)
            const noiseAmp = amplitude * (toneEnabled ? 0.6 : 0.9); // Less if mixed with tone
            for (let bin = 20; bin < 56; bin++) {
                // Noise has slight emphasis in mid-highs
                const emphasis = 1.0 - Math.abs(bin - 38) / 36;
                const noiseLevel = noiseAmp * emphasis * (0.5 + Math.random() * 0.5);
                targets[ch][bin] = Math.max(targets[ch][bin], noiseLevel);
            }
        }

        // Envelope (buzz/sync effects): adds harmonic richness
        if (envEnabled && !noiseEnabled) {
            // Envelope creates sawtooth-like harmonics
            if (frequency > 0) {
                const baseBin = freqToBin(frequency);
                // Envelope adds many harmonics (buzz effect)
                for (let h = 2; h <= 8; h++) {
                    const harmBin = Math.min(SPECTRUM_BINS - 1, baseBin + Math.floor(BINS_PER_OCTAVE * Math.log2(h)));
                    const harmAmp = amplitude / h * 0.7;
                    targets[ch][harmBin] = Math.max(targets[ch][harmBin], harmAmp);
                }
            } else {
                // Envelope without tone: spread low-mid frequencies
                for (let bin = 4; bin <= 32; bin++) {
                    const falloff = 1.0 - Math.abs(bin - 16) / 28;
                    targets[ch][bin] = Math.max(targets[ch][bin], amplitude * falloff * 0.7);
                }
            }
        }

        // Fallback: amplitude but nothing specific enabled
        if (!toneEnabled && !noiseEnabled && !envEnabled) {
            for (let bin = 8; bin <= 28; bin++) {
                const falloff = 1.0 - Math.abs(bin - 18) / 20;
                targets[ch][bin] = Math.max(targets[ch][bin], amplitude * falloff * 0.8);
            }
        }
    }

    // Apply smoothing to each channel's spectrum
    for (let ch = 0; ch < state.channelSpectrums.length; ch++) {
        const spectrum = state.channelSpectrums[ch];
        const target = targets[ch];
        if (!spectrum || !target) continue;

        for (let i = 0; i < SPECTRUM_BINS; i++) {
            spectrum[i] = smoothValue(spectrum[i], target[i]);
        }
    }

    // Combined spectrum with smoothing
    for (let i = 0; i < SPECTRUM_BINS; i++) {
        let maxTarget = 0;
        for (let ch = 0; ch < state.channelSpectrums.length; ch++) {
            maxTarget = Math.max(maxTarget, state.channelSpectrums[ch][i]);
        }
        state.combinedSpectrum[i] = smoothValue(state.combinedSpectrum[i], maxTarget);
    }
}

// ============================================================================
// Drawing
// ============================================================================

export function drawAllVisualization() {
    // Continuously scroll note entries upward and clean up old ones
    for (const history of state.noteHistories) {
        for (const entry of history) {
            entry.y += NOTE_SCROLL_SPEED;
        }
        while (history.length > 0 && history[0].y > 150) {
            history.shift();
        }
    }

    // Draw each channel's oscilloscope
    for (let ch = 0; ch < state.channelWaveforms.length; ch++) {
        const ctx = channelContexts.osc[ch];
        const color = getChannelColor(ch);
        const history = ch < state.noteHistories.length ? state.noteHistories[ch] : null;
        drawOscilloscope(ctx, state.channelWaveforms[ch], color, history);
    }
    // Draw mono combined oscilloscope
    drawOscilloscope(contexts.oscMono, state.monoWaveform, COLORS.green, null);

    // Draw each channel's spectrum
    for (let ch = 0; ch < state.channelSpectrums.length; ch++) {
        const ctx = channelContexts.spec[ch];
        const color = getChannelColor(ch);
        drawSpectrum(ctx, state.channelSpectrums[ch], color, ch);
    }
    // Draw combined spectrum
    drawSpectrum(contexts.specCombined, state.combinedSpectrum, COLORS.green, -1);
}

function drawOscilloscope(ctx, data, color, noteHistory = null) {
    if (!ctx) return;
    const canvas = ctx.canvas;
    const dpr = window.devicePixelRatio || 1;
    const w = canvas.width / dpr;
    const h = canvas.height / dpr;
    if (w === 0 || h === 0) return;

    ctx.fillStyle = "rgba(10, 10, 15, 1)";
    ctx.fillRect(0, 0, w, h);

    // Grid
    ctx.strokeStyle = "rgba(139, 92, 246, 0.08)";
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
    ctx.strokeStyle = "rgba(139, 92, 246, 0.15)";
    ctx.beginPath();
    ctx.moveTo(0, h / 2);
    ctx.lineTo(w, h / 2);
    ctx.stroke();

    // Waveform
    ctx.beginPath();
    const step = w / data.length;
    for (let i = 0; i < data.length; i++) {
        const x = i * step;
        const y = h / 2 - data[i] * (h / 2) * 0.9;
        if (i === 0) ctx.moveTo(x, y);
        else ctx.lineTo(x, y);
    }

    ctx.strokeStyle = color;
    ctx.lineWidth = 1;
    ctx.stroke();

    // Draw scrolling note history (tracker style) - centered with vignette, muted colors
    if (noteHistory && noteHistory.length > 0) {
        const rgb = hexToRgb(color);
        // Muted color (55% brightness)
        const dimR = Math.round(rgb.r * 0.55);
        const dimG = Math.round(rgb.g * 0.55);
        const dimB = Math.round(rgb.b * 0.55);

        ctx.textAlign = "center";
        ctx.font = "bold 9px monospace";
        const centerX = w / 2;

        for (const entry of noteHistory) {
            const yPos = h - 8 - entry.y;
            if (yPos < -10 || yPos > h + 10) continue;

            // Fade based on y position
            const age = entry.y / 120;
            const alpha = Math.max(0.1, 0.75 - age * 0.6);

            if (entry.note) {
                // Build display: NOTE  N E  SHAPE
                const noteText = entry.note;
                const noiseInd = entry.noise ? "N" : "-";
                const envInd = entry.env ? "E" : "-";
                const shapeText = entry.env && entry.envName ? entry.envName : "";
                const fullText = `${noteText} ${noiseInd} ${envInd}${shapeText ? " " + shapeText : ""}`;

                // Subtle vignette shadow behind text
                ctx.shadowColor = "rgba(0, 0, 0, 0.9)";
                ctx.shadowBlur = 10;
                ctx.shadowOffsetX = 0;
                ctx.shadowOffsetY = 0;

                // Draw text with muted color
                ctx.fillStyle = `rgba(${dimR}, ${dimG}, ${dimB}, ${alpha})`;
                ctx.fillText(fullText, centerX, yPos);

                // Reset shadow
                ctx.shadowBlur = 0;
            }
        }
    }
}

// Peak hold state (managed per-call for simplicity)
const peakHoldState = new Map();
const PEAK_HOLD_TIME = 30; // frames to hold peak
const PEAK_FALL_SPEED = 0.02; // how fast peaks fall

function drawSpectrum(ctx, data, color, channelIndex = -1) {
    if (!ctx) return;
    const canvas = ctx.canvas;
    const dpr = window.devicePixelRatio || 1;
    const w = canvas.width / dpr;
    const h = canvas.height / dpr;
    if (w === 0 || h === 0) return;

    // Get or create peak hold state for this canvas
    const canvasId = canvas.id || `canvas_${channelIndex}`;
    if (!peakHoldState.has(canvasId)) {
        peakHoldState.set(canvasId, {
            peaks: new Float32Array(SPECTRUM_BINS),
            holdTimers: new Uint8Array(SPECTRUM_BINS),
        });
    }
    const peakState = peakHoldState.get(canvasId);

    // Background with subtle gradient
    const bgGrad = ctx.createLinearGradient(0, 0, 0, h);
    bgGrad.addColorStop(0, "#0a0a0f");
    bgGrad.addColorStop(1, "#0d0d15");
    ctx.fillStyle = bgGrad;
    ctx.fillRect(0, 0, w, h);

    // Grid lines (subtle)
    ctx.strokeStyle = "rgba(139, 92, 246, 0.06)";
    ctx.lineWidth = 1;
    ctx.beginPath();
    // Vertical grid lines (every 8 bins = 1 octave with BINS_PER_OCTAVE=8)
    for (let i = BINS_PER_OCTAVE; i < SPECTRUM_BINS; i += BINS_PER_OCTAVE) {
        const x = (i / SPECTRUM_BINS) * w;
        ctx.moveTo(x, 0);
        ctx.lineTo(x, h);
    }
    // Horizontal grid lines (dB levels)
    for (let i = 1; i <= 4; i++) {
        const y = (i / 5) * h;
        ctx.moveTo(0, y);
        ctx.lineTo(w, y);
    }
    ctx.stroke();

    // Parse color to RGB for manipulations
    const rgb = hexToRgb(color);
    // Noise tint: shift toward white/cyan for high frequency bins
    const noiseRgb = { r: Math.min(255, rgb.r + 60), g: Math.min(255, rgb.g + 80), b: Math.min(255, rgb.b + 40) };
    const barGap = Math.max(1, Math.floor(w / SPECTRUM_BINS * 0.15)); // Dynamic gap
    const totalBarWidth = w / SPECTRUM_BINS;
    const barWidth = Math.max(1.5, totalBarWidth - barGap);
    const maxHeight = h * 0.85;
    const cornerRadius = Math.min(barWidth / 2, 2);

    // Draw bars with glow
    for (let i = 0; i < SPECTRUM_BINS; i++) {
        const value = data[i];
        const barHeight = Math.max(0, value * maxHeight);
        const x = i * totalBarWidth + barGap / 2;
        const y = h - barHeight;

        // Update peak hold
        if (value >= peakState.peaks[i]) {
            peakState.peaks[i] = value;
            peakState.holdTimers[i] = PEAK_HOLD_TIME;
        } else if (peakState.holdTimers[i] > 0) {
            peakState.holdTimers[i]--;
        } else {
            peakState.peaks[i] = Math.max(0, peakState.peaks[i] - PEAK_FALL_SPEED);
        }

        // Blend toward noise color for high frequency bins (noise region: 20-56)
        const noiseBlend = i < 20 ? 0 : Math.min(1, (i - 20) / 24);
        const barR = Math.round(rgb.r + (noiseRgb.r - rgb.r) * noiseBlend);
        const barG = Math.round(rgb.g + (noiseRgb.g - rgb.g) * noiseBlend);
        const barB = Math.round(rgb.b + (noiseRgb.b - rgb.b) * noiseBlend);
        const barColor = `rgb(${barR}, ${barG}, ${barB})`;

        if (barHeight > 1) {
            // Outer glow (scaled to bar width)
            ctx.shadowColor = barColor;
            ctx.shadowBlur = Math.min(8, barWidth * 2);
            ctx.shadowOffsetX = 0;
            ctx.shadowOffsetY = 0;

            // Main bar gradient (bottom to top, dark to bright)
            const barGrad = ctx.createLinearGradient(x, h, x, y);
            barGrad.addColorStop(0, `rgba(${barR}, ${barG}, ${barB}, 0.2)`);
            barGrad.addColorStop(0.4, `rgba(${barR}, ${barG}, ${barB}, 0.7)`);
            barGrad.addColorStop(0.8, barColor);
            barGrad.addColorStop(1, lightenColor(barColor, 40));

            ctx.fillStyle = barGrad;

            // Draw rounded bar
            ctx.beginPath();
            ctx.roundRect(x, y, barWidth, barHeight, [cornerRadius, cornerRadius, 0, 0]);
            ctx.fill();

            // Reset shadow for inner details
            ctx.shadowBlur = 0;

            // Inner highlight (left edge) - only for wider bars
            if (barWidth > 3) {
                const highlightGrad = ctx.createLinearGradient(x, 0, x + barWidth * 0.3, 0);
                highlightGrad.addColorStop(0, "rgba(255, 255, 255, 0.12)");
                highlightGrad.addColorStop(1, "rgba(255, 255, 255, 0)");
                ctx.fillStyle = highlightGrad;
                ctx.beginPath();
                ctx.roundRect(x, y, barWidth * 0.4, barHeight, [cornerRadius, 0, 0, 0]);
                ctx.fill();
            }

            // Top cap glow
            const capGrad = ctx.createLinearGradient(x, y, x, y + 3);
            capGrad.addColorStop(0, lightenColor(color, 60));
            capGrad.addColorStop(1, "transparent");
            ctx.fillStyle = capGrad;
            ctx.beginPath();
            ctx.roundRect(x, y, barWidth, Math.min(3, barHeight), [cornerRadius, cornerRadius, 0, 0]);
            ctx.fill();
        }

        // Draw peak indicator
        const peakY = h - peakState.peaks[i] * maxHeight;
        if (peakState.peaks[i] > 0.02 && peakY < h - 3) {
            // Peak line with glow (use blended color)
            ctx.shadowColor = lightenColor(barColor, 40);
            ctx.shadowBlur = Math.min(4, barWidth);
            ctx.fillStyle = lightenColor(barColor, 70);
            const peakHeight = Math.max(1, Math.min(2, barWidth * 0.5));
            ctx.fillRect(x, peakY - peakHeight, barWidth, peakHeight);
            ctx.shadowBlur = 0;
        }
    }

    // Reflection effect at bottom
    ctx.globalAlpha = 0.15;
    ctx.scale(1, -1);
    ctx.translate(0, -h * 2);

    for (let i = 0; i < SPECTRUM_BINS; i++) {
        const value = data[i];
        const barHeight = Math.min(value * maxHeight * 0.3, h * 0.15);
        const x = i * totalBarWidth + barGap / 2;

        if (barHeight > 1) {
            const reflGrad = ctx.createLinearGradient(x, h, x, h - barHeight);
            reflGrad.addColorStop(0, color);
            reflGrad.addColorStop(1, "transparent");
            ctx.fillStyle = reflGrad;
            ctx.fillRect(x, h - barHeight, barWidth, barHeight);
        }
    }

    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.globalAlpha = 1;

    // Bottom line accent
    const lineGrad = ctx.createLinearGradient(0, h - 1, w, h - 1);
    lineGrad.addColorStop(0, "transparent");
    lineGrad.addColorStop(0.2, `rgba(${rgb.r}, ${rgb.g}, ${rgb.b}, 0.3)`);
    lineGrad.addColorStop(0.5, `rgba(${rgb.r}, ${rgb.g}, ${rgb.b}, 0.5)`);
    lineGrad.addColorStop(0.8, `rgba(${rgb.r}, ${rgb.g}, ${rgb.b}, 0.3)`);
    lineGrad.addColorStop(1, "transparent");
    ctx.strokeStyle = lineGrad;
    ctx.lineWidth = 1;
    ctx.beginPath();
    ctx.moveTo(0, h - 0.5);
    ctx.lineTo(w, h - 0.5);
    ctx.stroke();
}

// Helper: Convert hex to RGB
function hexToRgb(hex) {
    const result = /^#?([a-f\d]{2})([a-f\d]{2})([a-f\d]{2})$/i.exec(hex);
    return result ? {
        r: parseInt(result[1], 16),
        g: parseInt(result[2], 16),
        b: parseInt(result[3], 16)
    } : { r: 139, g: 92, b: 246 }; // fallback purple
}

// Helper: Lighten a hex color
function lightenColor(hex, percent) {
    const rgb = hexToRgb(hex);
    const r = Math.min(255, rgb.r + (255 - rgb.r) * percent / 100);
    const g = Math.min(255, rgb.g + (255 - rgb.g) * percent / 100);
    const b = Math.min(255, rgb.b + (255 - rgb.b) * percent / 100);
    return `rgb(${Math.round(r)}, ${Math.round(g)}, ${Math.round(b)})`;
}
