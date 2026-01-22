// ============================================================================
// Export - WAV export functionality
// ============================================================================

import * as state from './state.js';
import { elements } from './ui/elements.js';
import { getChannelNames } from './ui/player.js';
import { loadTrack } from './audio/playback.js';

// ============================================================================
// Export Modal
// ============================================================================

export function showExportModal() {
    if (!state.wasmPlayer) return;

    // Set default duration to song length
    const duration = Math.ceil(state.wasmPlayer.metadata.duration_seconds);
    elements.exportDuration.value = duration > 0 ? duration : 180;
    elements.exportMode.value = "mixed";
    elements.exportStemOptions.classList.add("hidden");

    // Populate channel checkboxes
    const channelCount = state.wasmPlayer.channelCount ? state.wasmPlayer.channelCount() : 3;
    let checkboxHtml = "";
    const channelNames = getChannelNames(channelCount);
    for (let i = 0; i < channelCount; i++) {
        checkboxHtml += `
            <label class="flex items-center gap-1 bg-gray-800 rounded px-2 py-1 cursor-pointer">
                <input type="checkbox" class="export-channel-cb" value="${i}" checked />
                <span class="text-xs">${channelNames[i]}</span>
            </label>`;
    }
    elements.exportChannelCheckboxes.innerHTML = checkboxHtml;

    elements.exportModal.classList.remove("hidden");
    elements.exportProgress.classList.add("hidden");
}

export function hideExportModal() {
    elements.exportModal.classList.add("hidden");
}

// ============================================================================
// WAV Export
// ============================================================================

export async function exportWav() {
    if (!state.wasmPlayer) return;
    // Need either a catalog track or user-loaded file data
    if (state.currentTrackIndex < 0 && !state.loadedFileData) return;

    const duration = parseInt(elements.exportDuration.value) || 180;
    const exportMode = elements.exportMode.value;
    const sampleRate = 44100;
    const numSamples = duration * sampleRate;

    elements.exportProgress.classList.remove("hidden");
    elements.exportStart.disabled = true;

    try {
        // Get data from catalog track or user-loaded file
        let data;
        let trackTitle;
        if (state.currentTrackIndex >= 0 && state.filteredTracks[state.currentTrackIndex]) {
            const track = state.filteredTracks[state.currentTrackIndex];
            data = await loadTrack(track.path);
            trackTitle = track.title || "track";
        } else if (state.loadedFileData) {
            data = state.loadedFileData;
            trackTitle = state.loadedFileName ? state.loadedFileName.replace(/\.[^/.]+$/, "") : "track";
        } else {
            throw new Error("No track loaded");
        }

        if (exportMode === "stems") {
            await exportStems(data, trackTitle, numSamples, sampleRate);
        } else {
            await exportMixed(data, trackTitle, numSamples, sampleRate);
        }
    } catch (err) {
        console.error("Export error:", err);
        alert("Export failed: " + err.message);
    } finally {
        hideExportModal();
        elements.exportStart.disabled = false;
    }
}

async function exportStems(data, trackTitle, numSamples, sampleRate) {
    // Get selected channels
    const selectedChannels = [];
    elements.exportChannelCheckboxes
        .querySelectorAll(".export-channel-cb:checked")
        .forEach((cb) => {
            selectedChannels.push(parseInt(cb.value));
        });

    if (selectedChannels.length === 0) {
        alert("Please select at least one channel to export");
        return;
    }

    const channelNames = getChannelNames(state.wasmPlayer.channelCount ? state.wasmPlayer.channelCount() : 3);

    // Export each selected channel as a separate file
    for (let i = 0; i < selectedChannels.length; i++) {
        const ch = selectedChannels[i];
        const channelCount = state.wasmPlayer.channelCount ? state.wasmPlayer.channelCount() : 3;

        // Create a fresh player for each channel
        const exportPlayer = new state.Ym2149Player(data);
        exportPlayer.play();

        // Mute all channels except the current one
        for (let c = 0; c < channelCount; c++) {
            exportPlayer.setChannelMute(c, c !== ch);
        }

        const chunkSize = sampleRate;
        const samples = new Float32Array(numSamples);
        let offset = 0;

        while (offset < numSamples) {
            const remaining = Math.min(chunkSize, numSamples - offset);
            const chunk = exportPlayer.generateSamples(remaining);
            samples.set(chunk, offset);
            offset += remaining;

            // Update progress for all channels combined
            const totalProgress = (i + offset / numSamples) / selectedChannels.length;
            elements.exportProgressBar.style.width = `${totalProgress * 100}%`;
            await new Promise((r) => setTimeout(r, 0));
        }

        const wavBuffer = createWavFile(samples, sampleRate);
        const blob = new Blob([wavBuffer], { type: "audio/wav" });
        const url = URL.createObjectURL(blob);

        const a = document.createElement("a");
        a.href = url;
        const chName = channelNames[ch].replace(/\s+/g, "_");
        a.download = `${trackTitle}_${chName}.wav`;
        a.click();
        URL.revokeObjectURL(url);

        // Small delay between downloads
        await new Promise((r) => setTimeout(r, 200));
    }
}

async function exportMixed(data, trackTitle, numSamples, sampleRate) {
    const exportPlayer = new state.Ym2149Player(data);
    exportPlayer.play();

    const chunkSize = sampleRate;
    const samples = new Float32Array(numSamples);
    let offset = 0;

    while (offset < numSamples) {
        const remaining = Math.min(chunkSize, numSamples - offset);
        const chunk = exportPlayer.generateSamples(remaining);
        samples.set(chunk, offset);
        offset += remaining;
        elements.exportProgressBar.style.width = `${(offset / numSamples) * 100}%`;
        await new Promise((r) => setTimeout(r, 0));
    }

    const wavBuffer = createWavFile(samples, sampleRate);
    const blob = new Blob([wavBuffer], { type: "audio/wav" });
    const url = URL.createObjectURL(blob);

    const a = document.createElement("a");
    a.href = url;
    a.download = `${trackTitle}.wav`;
    a.click();
    URL.revokeObjectURL(url);
}

// ============================================================================
// WAV File Creation
// ============================================================================

function createWavFile(samples, sampleRate) {
    const numChannels = 1;
    const bitsPerSample = 16;
    const bytesPerSample = bitsPerSample / 8;
    const blockAlign = numChannels * bytesPerSample;
    const byteRate = sampleRate * blockAlign;
    const dataSize = samples.length * bytesPerSample;
    const buffer = new ArrayBuffer(44 + dataSize);
    const view = new DataView(buffer);

    const writeString = (offset, string) => {
        for (let i = 0; i < string.length; i++) {
            view.setUint8(offset + i, string.charCodeAt(i));
        }
    };

    writeString(0, "RIFF");
    view.setUint32(4, 36 + dataSize, true);
    writeString(8, "WAVE");
    writeString(12, "fmt ");
    view.setUint32(16, 16, true);
    view.setUint16(20, 1, true);
    view.setUint16(22, numChannels, true);
    view.setUint32(24, sampleRate, true);
    view.setUint32(28, byteRate, true);
    view.setUint16(32, blockAlign, true);
    view.setUint16(34, bitsPerSample, true);
    writeString(36, "data");
    view.setUint32(40, dataSize, true);

    let offset = 44;
    for (let i = 0; i < samples.length; i++) {
        const sample = Math.max(-1, Math.min(1, samples[i]));
        view.setInt16(offset, sample * 32767, true);
        offset += 2;
    }

    return buffer;
}
