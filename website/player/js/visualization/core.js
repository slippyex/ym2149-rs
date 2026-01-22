// ============================================================================
// Visualization Core - Oscilloscope, Spectrum, and main visualization loop
// ============================================================================

import {
    WAVEFORM_SIZE,
    SPECTRUM_BINS,
    SPECTRUM_DECAY,
    SPECTRUM_BASE_FREQ,
    BINS_PER_OCTAVE,
    AUDIO_VIS_BUFFER_SIZE,
    NOTE_HISTORY_SIZE,
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

    // Apply decay
    for (const spectrum of state.channelSpectrums) {
        for (let i = 0; i < SPECTRUM_BINS; i++) {
            spectrum[i] *= SPECTRUM_DECAY;
        }
    }
    for (let i = 0; i < SPECTRUM_BINS; i++) {
        state.combinedSpectrum[i] *= SPECTRUM_DECAY;
    }

    // Update each channel's spectrum
    for (let ch = 0; ch < channelCount && ch < state.channelSpectrums.length; ch++) {
        const chState = states.channels[ch];
        const spectrum = state.channelSpectrums[ch];
        if (!chState || !spectrum) continue;

        const hasOutput = chState.toneEnabled || chState.noiseEnabled || chState.envelopeEnabled;
        const amplitude = chState.amplitude || 0;

        if (chState.isDac && amplitude > 0) {
            for (let bin = 4; bin <= 12; bin++) {
                const falloff = 1.0 - Math.abs(bin - 8) / 8;
                spectrum[bin] = Math.max(spectrum[bin], amplitude * falloff * 0.8);
            }
        } else if (!hasOutput && amplitude > 0) {
            for (let bin = 4; bin <= 14; bin++) {
                const falloff = 1.0 - Math.abs(bin - 9) / 10;
                spectrum[bin] = Math.max(spectrum[bin], amplitude * falloff * 0.9);
            }
        } else if (hasOutput && amplitude > 0 && chState.frequency > 0) {
            const bin = freqToBin(chState.frequency);
            spectrum[bin] = Math.max(spectrum[bin], amplitude);
        }
    }

    // Combined spectrum
    for (let i = 0; i < SPECTRUM_BINS; i++) {
        let max = 0;
        for (let ch = 0; ch < state.channelSpectrums.length; ch++) {
            max = Math.max(max, state.channelSpectrums[ch][i]);
        }
        state.combinedSpectrum[i] = max;
    }
}

// ============================================================================
// Drawing
// ============================================================================

export function drawAllVisualization() {
    // Clean up old note entries
    for (const history of state.noteHistories) {
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
        drawSpectrum(ctx, state.channelSpectrums[ch], color);
    }
    // Draw combined spectrum
    drawSpectrum(contexts.specCombined, state.combinedSpectrum, COLORS.green);
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
}

function drawSpectrum(ctx, data, color) {
    if (!ctx) return;
    const canvas = ctx.canvas;
    const dpr = window.devicePixelRatio || 1;
    const w = canvas.width / dpr;
    const h = canvas.height / dpr;
    if (w === 0 || h === 0) return;

    ctx.fillStyle = "#0a0a0f";
    ctx.fillRect(0, 0, w, h);

    const barWidth = w / SPECTRUM_BINS - 1;
    for (let i = 0; i < SPECTRUM_BINS; i++) {
        const x = i * (barWidth + 1);
        const barHeight = data[i] * h * 0.9;
        const y = h - barHeight;
        const gradient = ctx.createLinearGradient(x, h, x, y);
        gradient.addColorStop(0, color + "30");
        gradient.addColorStop(1, color);
        ctx.fillStyle = gradient;
        ctx.fillRect(x, y, barWidth, barHeight);
    }
}
