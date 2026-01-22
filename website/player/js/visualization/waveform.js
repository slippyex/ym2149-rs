// ============================================================================
// Visualization Waveform - Waveform scrubber and generation
// ============================================================================

import { WAVEFORM_DB_NAME, WAVEFORM_STORE_NAME } from '../config.js';
import * as state from '../state.js';
import { elements } from '../ui/elements.js';
import { formatTime, updateLoopUI } from '../ui/player.js';

// ============================================================================
// Waveform DB (IndexedDB)
// ============================================================================

async function openWaveformDb() {
    if (state.waveformDb) return state.waveformDb;
    return new Promise((resolve, reject) => {
        const request = indexedDB.open(WAVEFORM_DB_NAME, 1);
        request.onerror = () => reject(request.error);
        request.onsuccess = () => {
            state.setWaveformDb(request.result);
            resolve(request.result);
        };
        request.onupgradeneeded = (e) => {
            const db = e.target.result;
            if (!db.objectStoreNames.contains(WAVEFORM_STORE_NAME)) {
                db.createObjectStore(WAVEFORM_STORE_NAME);
            }
        };
    });
}

async function getCachedWaveform(fingerprint) {
    try {
        const db = await openWaveformDb();
        return new Promise((resolve) => {
            const tx = db.transaction(WAVEFORM_STORE_NAME, "readonly");
            const store = tx.objectStore(WAVEFORM_STORE_NAME);
            const request = store.get(fingerprint);
            request.onsuccess = () => resolve(request.result || null);
            request.onerror = () => resolve(null);
        });
    } catch {
        return null;
    }
}

async function cacheWaveform(fingerprint, peaks) {
    try {
        const db = await openWaveformDb();
        const tx = db.transaction(WAVEFORM_STORE_NAME, "readwrite");
        const store = tx.objectStore(WAVEFORM_STORE_NAME);
        store.put(Array.from(peaks), fingerprint);
    } catch (err) {
        console.warn("Failed to cache waveform:", err);
    }
}

// ============================================================================
// File Fingerprint
// ============================================================================

async function computeFileFingerprint(data) {
    const hashBuffer = await crypto.subtle.digest("SHA-256", data);
    const hashArray = Array.from(new Uint8Array(hashBuffer));
    return hashArray.map(b => b.toString(16).padStart(2, "0")).join("");
}

// ============================================================================
// Waveform Generation
// ============================================================================

async function generateWaveformOverview(data, durationSeconds) {
    const WAVEFORM_BARS = 200;
    const sampleRate = 44100;
    const totalSamples = Math.ceil(durationSeconds * sampleRate);
    const samplesPerBar = Math.ceil(totalSamples / WAVEFORM_BARS);

    // Create a temporary player
    const tempPlayer = new state.Ym2149Player(data);
    tempPlayer.play();

    const peaks = new Float32Array(WAVEFORM_BARS);
    const chunkSize = 4096;

    let sampleIndex = 0;
    let barIndex = 0;
    let barMax = 0;
    let barSampleCount = 0;

    while (barIndex < WAVEFORM_BARS && sampleIndex < totalSamples) {
        const samplesToGenerate = Math.min(chunkSize, totalSamples - sampleIndex);
        const samples = tempPlayer.generateSamples(samplesToGenerate);

        for (let i = 0; i < samples.length && barIndex < WAVEFORM_BARS; i++) {
            const abs = Math.abs(samples[i]);
            if (abs > barMax) barMax = abs;
            barSampleCount++;

            if (barSampleCount >= samplesPerBar) {
                peaks[barIndex] = barMax;
                barIndex++;
                barMax = 0;
                barSampleCount = 0;
            }
        }

        sampleIndex += samplesToGenerate;

        // Yield to UI
        if (sampleIndex % (chunkSize * 10) === 0) {
            await new Promise(r => setTimeout(r, 0));
        }
    }

    // Handle last partial bar
    if (barSampleCount > 0 && barIndex < WAVEFORM_BARS) {
        peaks[barIndex] = barMax;
    }

    return peaks;
}

// ============================================================================
// Load/Generate Waveform
// ============================================================================

export async function loadOrGenerateWaveform(data, durationSeconds) {
    const fingerprint = await computeFileFingerprint(data);

    // Try cache first
    const cached = await getCachedWaveform(fingerprint);
    if (cached) {
        console.log("Waveform loaded from cache");
        state.setWaveformOverviewData(new Float32Array(cached));
        showWaveformScrubber();
        return;
    }

    // Generate in background
    console.log("Generating waveform overview...");
    try {
        const peaks = await generateWaveformOverview(data, durationSeconds);
        state.setWaveformOverviewData(peaks);
        showWaveformScrubber();

        // Cache for next time
        await cacheWaveform(fingerprint, peaks);
        console.log("Waveform cached");
    } catch (err) {
        console.error("Waveform generation failed:", err);
    }
}

export function loadPrerenderedWaveform(base64Data) {
    try {
        // Decode base64 waveform data
        const binaryStr = atob(base64Data);
        const peaks = new Float32Array(binaryStr.length);
        for (let i = 0; i < binaryStr.length; i++) {
            peaks[i] = binaryStr.charCodeAt(i) / 255; // Normalize 0-255 to 0-1
        }

        state.setWaveformOverviewData(peaks);

        // Show waveform, hide fallback progress bar
        elements.waveformScrubber.classList.remove("hidden");
        elements.progressContainer.classList.add("hidden");

        // Draw immediately
        requestAnimationFrame(() => {
            drawWaveformOverview();
            const duration = state.wasmPlayer?.metadata?.duration_seconds || 0;
            elements.waveformTotalTime.textContent = formatTime(duration);
            elements.waveformCurrentTime.textContent = "0:00";
        });
    } catch (err) {
        console.error("Waveform decode error:", err);
        elements.waveformScrubber.classList.add("hidden");
        elements.progressContainer.classList.remove("hidden");
    }
}

function showWaveformScrubber() {
    elements.waveformScrubber.classList.remove("hidden");
    elements.progressContainer.classList.add("hidden");
    requestAnimationFrame(() => {
        drawWaveformOverview();
        const duration = state.wasmPlayer?.metadata?.duration_seconds || 0;
        elements.waveformTotalTime.textContent = formatTime(duration);
        elements.waveformCurrentTime.textContent = "0:00";
    });
}

// ============================================================================
// Waveform Drawing
// ============================================================================

export function drawWaveformOverview() {
    if (!state.waveformOverviewData) return;

    const canvas = elements.waveformOverview;
    const ctx = canvas.getContext("2d");

    const rect = canvas.getBoundingClientRect();
    if (rect.width === 0 || rect.height === 0) return;

    canvas.width = rect.width * window.devicePixelRatio;
    canvas.height = rect.height * window.devicePixelRatio;
    ctx.scale(window.devicePixelRatio, window.devicePixelRatio);

    const width = rect.width;
    const height = rect.height;
    const barCount = state.waveformOverviewData.length;
    const barWidth = width / barCount;
    const centerY = height / 2;
    const maxBarHeight = (height - 16) / 2;

    // Clear
    ctx.fillStyle = "#0d0d14";
    ctx.fillRect(0, 0, width, height);

    // Find max peak for normalization
    let maxPeak = 0;
    for (let i = 0; i < barCount; i++) {
        if (state.waveformOverviewData[i] > maxPeak) maxPeak = state.waveformOverviewData[i];
    }
    const normalizer = maxPeak > 0 ? 1 / maxPeak : 1;

    // Gradient for bars
    const gradient = ctx.createLinearGradient(0, centerY - maxBarHeight, 0, centerY + maxBarHeight);
    gradient.addColorStop(0, "#8b5cf6");
    gradient.addColorStop(0.35, "#06b6d4");
    gradient.addColorStop(0.5, "#22d3ee");
    gradient.addColorStop(0.65, "#06b6d4");
    gradient.addColorStop(1, "#8b5cf6");

    ctx.fillStyle = gradient;

    // Draw bars
    const gap = Math.max(0.5, barWidth * 0.15);
    const actualBarWidth = barWidth - gap;

    for (let i = 0; i < barCount; i++) {
        const peak = state.waveformOverviewData[i] * normalizer;
        const barHeight = Math.max(2, peak * maxBarHeight);
        const x = i * barWidth + gap / 2;

        ctx.beginPath();
        ctx.roundRect(x, centerY - barHeight, actualBarWidth, barHeight * 2, 1);
        ctx.fill();
    }

    // Glow overlay
    ctx.globalCompositeOperation = "lighter";
    ctx.fillStyle = "rgba(6, 182, 212, 0.05)";
    ctx.fillRect(0, 0, width, height);
    ctx.globalCompositeOperation = "source-over";

    // Center line
    ctx.strokeStyle = "rgba(255, 255, 255, 0.08)";
    ctx.lineWidth = 1;
    ctx.beginPath();
    ctx.moveTo(0, centerY);
    ctx.lineTo(width, centerY);
    ctx.stroke();
}

// ============================================================================
// Waveform Playhead
// ============================================================================

export function updateWaveformPlayhead() {
    if (!state.wasmPlayer) return;
    const position = state.wasmPlayer.position_percentage();
    elements.waveformPlayhead.style.left = `${position * 100}%`;
    const duration = state.wasmPlayer.metadata?.duration_seconds || 0;
    elements.waveformCurrentTime.textContent = formatTime(position * duration);
}

export function updateWaveformLoopMarkers() {
    if (state.loopA !== null) {
        elements.waveformLoopA.style.left = `${state.loopA * 100}%`;
        elements.waveformLoopA.classList.remove("hidden");
    } else {
        elements.waveformLoopA.classList.add("hidden");
    }

    if (state.loopB !== null) {
        elements.waveformLoopB.style.left = `${state.loopB * 100}%`;
        elements.waveformLoopB.classList.remove("hidden");
    } else {
        elements.waveformLoopB.classList.add("hidden");
    }

    if (state.loopA !== null && state.loopB !== null) {
        const left = Math.min(state.loopA, state.loopB) * 100;
        const right = Math.max(state.loopA, state.loopB) * 100;
        elements.waveformLoopRegion.style.left = `${left}%`;
        elements.waveformLoopRegion.style.width = `${right - left}%`;
        elements.waveformLoopRegion.classList.remove("hidden");
    } else {
        elements.waveformLoopRegion.classList.add("hidden");
    }
}

// ============================================================================
// Scrubbing
// ============================================================================

function getEventX(e) {
    if (e.touches && e.touches.length > 0) {
        return e.touches[0].clientX;
    }
    if (e.changedTouches && e.changedTouches.length > 0) {
        return e.changedTouches[0].clientX;
    }
    return e.clientX;
}

function handleWaveformScrub(e) {
    if (!state.wasmPlayer) return;

    const rect = elements.waveformScrubber.getBoundingClientRect();
    const x = getEventX(e) - rect.left;
    const position = Math.max(0, Math.min(1, x / rect.width));

    state.wasmPlayer.seek_to_percentage(position);
    // Import at runtime to avoid circular dependency
    import('../ui/player.js').then(({ updateProgressUI }) => {
        updateProgressUI();
    });
    updateWaveformPlayhead();
}

export function startScrubbing(e) {
    if (!state.wasmPlayer) return;
    e.preventDefault();
    state.setIsScrubbing(true);
    handleWaveformScrub(e);
}

export function continueScrubbing(e) {
    if (!state.isScrubbing) return;
    e.preventDefault();
    handleWaveformScrub(e);
}

export function stopScrubbing(e) {
    if (!state.isScrubbing) return;
    state.setIsScrubbing(false);
}
