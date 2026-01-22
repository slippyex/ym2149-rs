// ============================================================================
// Audio Playback - Play, pause, stop, seek, and loop control
// ============================================================================

import * as state from '../state.js';
import { elements } from '../ui/elements.js';
import {
    updateMetadataUI,
    updatePlayButton,
    updateLoopUI,
    updatePlayerFavoriteButton,
    updateSimilarTracks,
    updateProgressUI,
    enableControls,
    updateLmc1992Display,
    closeSidebarOnMobile,
    showToast,
    formatTime,
} from '../ui/player.js';
import { updateVisibleRows } from '../ui/trackList.js';
import { recordPlay, loadOwnFileData } from '../storage.js';
import { ensureAudioContext, startAudioProcessing, stopAudioProcessing } from './context.js';
import { startVisualization, resetVisualization, clearAllWaveforms, drawAllVisualization } from '../visualization/core.js';
import { loadPrerenderedWaveform, loadOrGenerateWaveform, updateWaveformPlayhead } from '../visualization/waveform.js';
import { setupChannelUI } from '../visualization/core.js';

// ============================================================================
// Track Loading
// ============================================================================

export async function loadTrack(path) {
    // Check if this is an own file (stored in IndexedDB)
    if (path.startsWith("own_")) {
        const data = await loadOwnFileData(path);
        if (!data) throw new Error("Own file not found");
        return data;
    }

    const response = await fetch(path);
    if (!response.ok) throw new Error(`HTTP ${response.status}`);
    return new Uint8Array(await response.arrayBuffer());
}

// ============================================================================
// Play Track
// ============================================================================

export async function playTrack(index) {
    if (index < 0 || index >= state.filteredTracks.length) return;
    closeSidebarOnMobile();

    const track = state.filteredTracks[index];
    state.setCurrentTrackIndex(index);
    state.setLoadedFileData(null);
    state.setLoadedFileName(null);

    // Record play for statistics
    recordPlay(track.path);

    // Clear previous waveform, show fallback progress bar
    state.setWaveformOverviewData(null);
    elements.waveformScrubber.classList.add("hidden");
    elements.progressContainer.classList.remove("hidden");

    try {
        // Load track data
        const data = await loadTrack(track.path);
        const player = new state.Ym2149Player(data);
        player.set_volume(elements.volumeSlider.value / 100);
        state.setWasmPlayer(player);

        // Set format for channel naming
        state.setCurrentFormat(player.metadata.format || track.format || "");

        // Setup channel UI for multi-chip support
        const channelCount = player.channelCount ? player.channelCount() : 3;
        setupChannelUI(channelCount);

        // Clear all oscilloscope waveforms
        clearAllWaveforms();
        drawAllVisualization();

        updateMetadataUI(track);
        enableControls();
        resetVisualization();
        updateLmc1992Display();
        updateVisibleRows(true);
        updatePlayerFavoriteButton();
        updateSimilarTracks(track, handleSimilarTrackClick);

        // Use pre-rendered waveform from catalog if available
        if (track.w) {
            loadPrerenderedWaveform(track.w);
        } else if (track.isOwnFile) {
            const duration = player.metadata?.duration_seconds || 0;
            loadOrGenerateWaveform(data, duration);
        } else {
            elements.waveformScrubber.classList.add("hidden");
            elements.progressContainer.classList.remove("hidden");
        }

        // Clear loop
        clearLoop();

        // Try to start audio
        try {
            await ensureAudioContext();
            player.play();
            state.setIsPlaying(true);
            startAudioProcessing();
            startVisualization();
            updatePlayButton();
            updateVisibleRows(true);
        } catch (audioErr) {
            console.warn("Audio autoplay blocked:", audioErr);
            state.setIsPlaying(false);
            updatePlayButton();
        }
    } catch (err) {
        console.error("Playback error:", err);
        elements.songTitle.textContent = "Error loading";
        elements.songAuthor.textContent = err.message;
    }
}

function handleSimilarTrackClick(path, idx) {
    // Update filtered tracks to include all
    const origFiltered = state.filteredTracks;
    state.setFilteredTracks(state.catalog.tracks);
    playTrack(idx);
    state.setFilteredTracks(origFiltered);
}

// ============================================================================
// Play Controls
// ============================================================================

export async function togglePlayPause() {
    if (!state.wasmPlayer) return;
    if (state.isPlaying) {
        state.wasmPlayer.pause();
        state.setIsPlaying(false);
    } else {
        await ensureAudioContext();
        state.wasmPlayer.play();
        state.setIsPlaying(true);
        startAudioProcessing();
        startVisualization();
    }
    updatePlayButton();
    updateVisibleRows(true);
}

export function stop() {
    if (!state.wasmPlayer) return;
    state.wasmPlayer.stop();
    state.wasmPlayer.seek_to_percentage(0);
    state.setIsPlaying(false);
    stopAudioProcessing();
    updatePlayButton();
    elements.progressBar.value = 0;
    elements.currentTime.textContent = "0:00";
    updateWaveformPlayhead();
    resetVisualization();
    updateVisibleRows(true);
}

export function restart() {
    if (!state.wasmPlayer) return;
    state.wasmPlayer.seek_to_percentage(0);
    elements.progressBar.value = 0;
    elements.currentTime.textContent = "0:00";
    updateWaveformPlayhead();
    if (!state.isPlaying) {
        togglePlayPause();
    }
}

export function playNext() {
    if (state.shuffleEnabled) {
        playRandomTrack();
    } else if (state.currentTrackIndex < state.filteredTracks.length - 1) {
        playTrack(state.currentTrackIndex + 1);
    }
}

export function playRandomTrack() {
    if (state.filteredTracks.length <= 1) return;
    let randomIndex;
    do {
        randomIndex = Math.floor(Math.random() * state.filteredTracks.length);
    } while (randomIndex === state.currentTrackIndex);
    playTrack(randomIndex);
}

// ============================================================================
// Shuffle & Auto-Play
// ============================================================================

export function toggleShuffle() {
    state.setShuffleEnabled(!state.shuffleEnabled);
    elements.shuffleBtn.classList.toggle("bg-chip-purple", state.shuffleEnabled);
    elements.shuffleBtn.classList.toggle("text-white", state.shuffleEnabled);
    elements.shuffleBtn.classList.toggle("bg-gray-800", !state.shuffleEnabled);
    elements.shuffleBtn.classList.toggle("active", state.shuffleEnabled);
    showToast(state.shuffleEnabled ? "Shuffle ON" : "Shuffle OFF");
}

export function toggleAutoPlay() {
    state.setAutoPlayEnabled(!state.autoPlayEnabled);
    elements.autoPlayBtn.classList.toggle("bg-chip-purple", state.autoPlayEnabled);
    elements.autoPlayBtn.classList.toggle("text-white", state.autoPlayEnabled);
    elements.autoPlayBtn.classList.toggle("bg-gray-800", !state.autoPlayEnabled);
    elements.autoPlayBtn.classList.toggle("active", state.autoPlayEnabled);
    showToast(state.autoPlayEnabled ? "Auto-Play ON" : "Auto-Play OFF");
}

// ============================================================================
// A-B Loop
// ============================================================================

export function setLoopA() {
    if (!state.wasmPlayer) return;
    state.setLoopA(state.wasmPlayer.position_percentage());
    updateLoopUI();
}

export function setLoopB() {
    if (!state.wasmPlayer) return;
    let loopB = state.wasmPlayer.position_percentage();
    let loopA = state.loopA;
    // Ensure A < B
    if (loopA !== null && loopB < loopA) {
        [loopA, loopB] = [loopB, loopA];
        state.setLoopA(loopA);
    }
    state.setLoopB(loopB);
    updateLoopUI();
}

export function clearLoop() {
    state.setLoopA(null);
    state.setLoopB(null);
    updateLoopUI();
}

export function checkLoopBoundary() {
    if (!state.wasmPlayer || state.loopA === null || state.loopB === null) return;
    const position = state.wasmPlayer.position_percentage();
    if (position >= state.loopB) {
        state.wasmPlayer.seek_to_percentage(state.loopA);
    }
}

// ============================================================================
// Playback Speed
// ============================================================================

export function setPlaybackSpeed(speed) {
    state.setPlaybackSpeed(speed);
}

// ============================================================================
// Subsong
// ============================================================================

export function changeSubsong(index) {
    if (!state.wasmPlayer || !state.wasmPlayer.setSubsong) return;

    const wasPlaying = state.isPlaying;
    if (wasPlaying) {
        state.wasmPlayer.pause();
    }

    const success = state.wasmPlayer.setSubsong(index);
    if (success) {
        const meta = state.wasmPlayer.metadata;
        elements.totalTime.textContent = formatTime(meta.duration_seconds);
        elements.progressBar.value = 0;
        elements.currentTime.textContent = "0:00";
        showToast(`Track ${index}`);
    }

    if (wasPlaying) {
        state.wasmPlayer.play();
    }
}

export function prevSubsong() {
    if (!state.wasmPlayer || !state.wasmPlayer.subsongCount || state.wasmPlayer.subsongCount() <= 1) return;
    const current = state.wasmPlayer.currentSubsong();
    if (current > 1) {
        changeSubsong(current - 1);
        elements.subsongSelect.value = current - 1;
    }
}

export function nextSubsong() {
    if (!state.wasmPlayer || !state.wasmPlayer.subsongCount || state.wasmPlayer.subsongCount() <= 1) return;
    const current = state.wasmPlayer.currentSubsong();
    const count = state.wasmPlayer.subsongCount();
    if (current < count) {
        changeSubsong(current + 1);
        elements.subsongSelect.value = current + 1;
    }
}

// ============================================================================
// Share
// ============================================================================

export function shareCurrentTrack() {
    if (state.currentTrackIndex < 0 || !state.filteredTracks[state.currentTrackIndex]) {
        showToast("No track selected");
        return;
    }
    const track = state.filteredTracks[state.currentTrackIndex];
    const position = state.wasmPlayer ? state.wasmPlayer.position_percentage() : 0;

    const url = new URL(window.location.origin + window.location.pathname);
    url.searchParams.set("track", track.path);

    if (state.wasmPlayer && state.wasmPlayer.subsongCount && state.wasmPlayer.subsongCount() > 1) {
        const currentSub = state.wasmPlayer.currentSubsong();
        if (currentSub > 1) {
            url.searchParams.set("sub", currentSub);
        }
    }

    if (position > 0.01 && state.wasmPlayer) {
        url.searchParams.set("t", Math.floor(position * (state.wasmPlayer.metadata?.duration_seconds || 0)));
    }

    const shareUrl = url.toString();

    if (navigator.clipboard && navigator.clipboard.writeText) {
        navigator.clipboard.writeText(shareUrl)
            .then(() => showToast("Link copied!"))
            .catch(() => fallbackCopyToClipboard(shareUrl));
    } else {
        fallbackCopyToClipboard(shareUrl);
    }
}

function fallbackCopyToClipboard(text) {
    const textArea = document.createElement("textarea");
    textArea.value = text;
    textArea.style.position = "fixed";
    textArea.style.left = "-999999px";
    document.body.appendChild(textArea);
    textArea.select();
    try {
        document.execCommand("copy");
        showToast("Link copied to clipboard!");
    } catch (err) {
        showToast("Failed to copy link");
    }
    document.body.removeChild(textArea);
}

// ============================================================================
// Load From File (Drag & Drop / File Input)
// ============================================================================

export async function loadFromFile(file) {
    try {
        await ensureAudioContext();
        const buffer = await file.arrayBuffer();
        const data = new Uint8Array(buffer);

        const player = new state.Ym2149Player(data);
        player.set_volume(elements.volumeSlider.value / 100);
        state.setWasmPlayer(player);
        state.setCurrentTrackIndex(-1);
        state.setLoadedFileData(data);
        state.setLoadedFileName(file.name);

        // Reset waveform
        state.setWaveformOverviewData(null);
        elements.waveformScrubber.classList.add("hidden");
        elements.progressContainer.classList.remove("hidden");

        const meta = player.metadata;
        state.setCurrentFormat(meta.format || "");

        // Setup channel UI
        const channelCount = player.channelCount ? player.channelCount() : 3;
        setupChannelUI(channelCount);

        clearAllWaveforms();
        drawAllVisualization();

        elements.songTitle.textContent = file.name;
        elements.songAuthor.textContent = meta.author || "Dropped file";
        elements.songFormat.textContent = meta.format || "-";
        elements.songFrames.textContent = `${meta.frame_count} frames`;
        elements.totalTime.textContent = formatTime(meta.duration_seconds);

        enableControls();
        resetVisualization();
        updateLmc1992Display();

        player.play();
        state.setIsPlaying(true);
        startAudioProcessing();
        startVisualization();
        updatePlayButton();

        // Generate waveform overview in background
        loadOrGenerateWaveform(data, meta.duration_seconds);
    } catch (err) {
        console.error("Load error:", err);
    }
}
