// ============================================================================
// Audio Playback - Play, pause, stop, seek, and loop control
// ============================================================================

import * as state from '../state.ts';
import { elements } from '../ui/elements.ts';
import {
  updateMetadataUI,
  updatePlayButton,
  updateLoopUI,
  updatePlayerFavoriteButton,
  updateSimilarTracks,
  enableControls,
  updateLmc1992Display,
  closeSidebarOnMobile,
  showToast,
  formatTime,
} from '../ui/player.ts';
import { updateVisibleRows } from '../ui/trackList.ts';
import { recordPlay, loadOwnFileData } from '../storage.ts';
import { ensureAudioContext, startAudioProcessing, stopAudioProcessing } from './context.ts';
import { startVisualization, resetVisualization, clearAllWaveforms, drawAllVisualization, setupChannelUI } from '../visualization/core.ts';
import { loadPrerenderedWaveform, loadOrGenerateWaveform, updateWaveformPlayhead } from '../visualization/waveform.ts';
import type { SimilarTrackClickHandler } from '../types/index.ts';

// ============================================================================
// Track Loading
// ============================================================================

export async function loadTrack(path: string): Promise<Uint8Array> {
  // Check if this is an own file (stored in IndexedDB)
  if (path.startsWith('own_')) {
    const data = await loadOwnFileData(path);
    if (!data) throw new Error('Own file not found');
    return data;
  }

  const response = await fetch(path);
  if (!response.ok) throw new Error(`HTTP ${response.status}`);
  return new Uint8Array(await response.arrayBuffer());
}

// ============================================================================
// Play Track
// ============================================================================

export async function playTrack(index: number): Promise<void> {
  if (index < 0 || index >= state.filteredTracks.length) return;
  closeSidebarOnMobile();

  const track = state.filteredTracks[index];
  if (!track) return;
  state.setCurrentTrackIndex(index);
  state.setLoadedFileData(null);
  state.setLoadedFileName(null);

  // Record play for statistics
  recordPlay(track.path);

  // Clear previous waveform, show fallback progress bar
  state.setWaveformOverviewData(null);
  elements.waveformScrubber?.classList.add('hidden');
  elements.progressContainer?.classList.remove('hidden');

  try {
    // Load track data
    const data = await loadTrack(track.path);
    if (!state.Ym2149Player) throw new Error('Player not initialized');
    const player = new state.Ym2149Player(data);
    const volumeValue = elements.volumeSlider ? parseFloat(elements.volumeSlider.value) / 100 : 1;
    player.set_volume(volumeValue);
    state.setWasmPlayer(player);

    // Set format for channel naming
    state.setCurrentFormat(player.metadata.format || track.format || '');

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
      elements.waveformScrubber?.classList.add('hidden');
      elements.progressContainer?.classList.remove('hidden');
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
      console.warn('Audio autoplay blocked:', audioErr);
      state.setIsPlaying(false);
      updatePlayButton();
    }
  } catch (err) {
    console.error('Playback error:', err);
    if (elements.songTitle) elements.songTitle.textContent = 'Error loading';
    if (elements.songAuthor) elements.songAuthor.textContent = (err as Error).message;
  }
}

const handleSimilarTrackClick: SimilarTrackClickHandler = (_path, idx) => {
  // Update filtered tracks to include all
  const origFiltered = state.filteredTracks;
  if (state.catalog) {
    state.setFilteredTracks(state.catalog.tracks);
  }
  playTrack(idx);
  state.setFilteredTracks(origFiltered);
};

// ============================================================================
// Play Controls
// ============================================================================

export async function togglePlayPause(): Promise<void> {
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

export function stop(): void {
  if (!state.wasmPlayer) return;
  state.wasmPlayer.stop();
  state.wasmPlayer.seek_to_percentage(0);
  state.setIsPlaying(false);
  stopAudioProcessing();
  updatePlayButton();
  if (elements.progressBar) (elements.progressBar as HTMLInputElement).value = '0';
  if (elements.currentTime) elements.currentTime.textContent = '0:00';
  updateWaveformPlayhead();
  resetVisualization();
  updateVisibleRows(true);
}

export function restart(): void {
  if (!state.wasmPlayer) return;
  state.wasmPlayer.seek_to_percentage(0);
  if (elements.progressBar) (elements.progressBar as HTMLInputElement).value = '0';
  if (elements.currentTime) elements.currentTime.textContent = '0:00';
  updateWaveformPlayhead();
  if (!state.isPlaying) {
    togglePlayPause();
  }
}

export function playNext(): void {
  if (state.shuffleEnabled) {
    playRandomTrack();
  } else if (state.currentTrackIndex < state.filteredTracks.length - 1) {
    playTrack(state.currentTrackIndex + 1);
  }
}

export function playPrev(): void {
  if (state.shuffleEnabled) {
    playRandomTrack();
  } else if (state.currentTrackIndex > 0) {
    playTrack(state.currentTrackIndex - 1);
  }
}

export function playRandomTrack(): void {
  if (state.filteredTracks.length <= 1) return;
  let randomIndex: number;
  do {
    randomIndex = Math.floor(Math.random() * state.filteredTracks.length);
  } while (randomIndex === state.currentTrackIndex);
  playTrack(randomIndex);
}

// ============================================================================
// Shuffle & Auto-Play
// ============================================================================

export function toggleShuffle(): void {
  state.setShuffleEnabled(!state.shuffleEnabled);
  elements.shuffleBtn?.classList.toggle('bg-chip-purple', state.shuffleEnabled);
  elements.shuffleBtn?.classList.toggle('text-white', state.shuffleEnabled);
  elements.shuffleBtn?.classList.toggle('bg-gray-800', !state.shuffleEnabled);
  elements.shuffleBtn?.classList.toggle('active', state.shuffleEnabled);
  showToast(state.shuffleEnabled ? 'Shuffle ON' : 'Shuffle OFF');
}

export function toggleAutoPlay(): void {
  state.setAutoPlayEnabled(!state.autoPlayEnabled);
  elements.autoPlayBtn?.classList.toggle('bg-chip-purple', state.autoPlayEnabled);
  elements.autoPlayBtn?.classList.toggle('text-white', state.autoPlayEnabled);
  elements.autoPlayBtn?.classList.toggle('bg-gray-800', !state.autoPlayEnabled);
  elements.autoPlayBtn?.classList.toggle('active', state.autoPlayEnabled);
  showToast(state.autoPlayEnabled ? 'Auto-Play ON' : 'Auto-Play OFF');
}

// ============================================================================
// A-B Loop
// ============================================================================

export function setLoopA(): void {
  if (!state.wasmPlayer) return;
  state.setLoopA(state.wasmPlayer.position_percentage());
  updateLoopUI();
}

export function setLoopB(): void {
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

export function clearLoop(): void {
  state.setLoopA(null);
  state.setLoopB(null);
  updateLoopUI();
}

export function checkLoopBoundary(): void {
  if (!state.wasmPlayer || state.loopA === null || state.loopB === null) return;
  const position = state.wasmPlayer.position_percentage();
  if (position >= state.loopB) {
    state.wasmPlayer.seek_to_percentage(state.loopA);
  }
}

// ============================================================================
// Playback Speed
// ============================================================================

export function setPlaybackSpeed(speed: number): void {
  state.setPlaybackSpeed(speed);
}

// ============================================================================
// Subsong
// ============================================================================

export function changeSubsong(index: number): void {
  if (!state.wasmPlayer || !state.wasmPlayer.setSubsong) return;

  const wasPlaying = state.isPlaying;
  if (wasPlaying) {
    state.wasmPlayer.pause();
  }

  const success = state.wasmPlayer.setSubsong(index);
  if (success) {
    const meta = state.wasmPlayer.metadata;
    if (elements.totalTime) elements.totalTime.textContent = formatTime(meta.duration_seconds);
    if (elements.progressBar) (elements.progressBar as HTMLInputElement).value = '0';
    if (elements.currentTime) elements.currentTime.textContent = '0:00';
    showToast(`Track ${index}`);
  }

  if (wasPlaying) {
    state.wasmPlayer.play();
  }
}

export function prevSubsong(): void {
  if (!state.wasmPlayer || !state.wasmPlayer.subsongCount || state.wasmPlayer.subsongCount() <= 1) return;
  const current = state.wasmPlayer.currentSubsong?.() ?? 1;
  if (current > 1) {
    changeSubsong(current - 1);
    if (elements.subsongSelect) elements.subsongSelect.value = String(current - 1);
  }
}

export function nextSubsong(): void {
  if (!state.wasmPlayer || !state.wasmPlayer.subsongCount || state.wasmPlayer.subsongCount() <= 1) return;
  const current = state.wasmPlayer.currentSubsong?.() ?? 1;
  const count = state.wasmPlayer.subsongCount();
  if (current < count) {
    changeSubsong(current + 1);
    if (elements.subsongSelect) elements.subsongSelect.value = String(current + 1);
  }
}

// ============================================================================
// Share
// ============================================================================

export function shareCurrentTrack(): void {
  if (state.currentTrackIndex < 0 || !state.filteredTracks[state.currentTrackIndex]) {
    showToast('No track selected');
    return;
  }
  const track = state.filteredTracks[state.currentTrackIndex];
  if (!track) return;
  const position = state.wasmPlayer ? state.wasmPlayer.position_percentage() : 0;

  const url = new URL(window.location.origin + window.location.pathname);
  url.searchParams.set('track', track.path);

  if (state.wasmPlayer && state.wasmPlayer.subsongCount && state.wasmPlayer.subsongCount() > 1) {
    const currentSub = state.wasmPlayer.currentSubsong?.() ?? 1;
    if (currentSub > 1) {
      url.searchParams.set('sub', String(currentSub));
    }
  }

  if (position > 0.01 && state.wasmPlayer) {
    url.searchParams.set('t', String(Math.floor(position * (state.wasmPlayer.metadata?.duration_seconds || 0))));
  }

  const shareUrl = url.toString();

  if (navigator.clipboard && navigator.clipboard.writeText) {
    navigator.clipboard.writeText(shareUrl)
      .then(() => showToast('Link copied!'))
      .catch(() => fallbackCopyToClipboard(shareUrl));
  } else {
    fallbackCopyToClipboard(shareUrl);
  }
}

function fallbackCopyToClipboard(text: string): void {
  const textArea = document.createElement('textarea');
  textArea.value = text;
  textArea.style.position = 'fixed';
  textArea.style.left = '-999999px';
  document.body.appendChild(textArea);
  textArea.select();
  try {
    document.execCommand('copy');
    showToast('Link copied to clipboard!');
  } catch {
    showToast('Failed to copy link');
  }
  document.body.removeChild(textArea);
}

// ============================================================================
// Load From File (Drag & Drop / File Input)
// ============================================================================

// ============================================================================
// Download Original Track
// ============================================================================

function getFileExtension(format: string): string {
  const map: Record<string, string> = {
    'SNDH': 'sndh',
    'YM': 'ym',
    'AY': 'ay',
    'AKS': 'aks',
  };
  return map[format] || format.toLowerCase();
}

function sanitizeFilename(name: string): string {
  return name.replace(/[<>:"/\\|?*]/g, '_');
}

export async function downloadCurrentTrack(): Promise<void> {
  const track = state.filteredTracks[state.currentTrackIndex];
  if (!track) {
    showToast('No track selected');
    return;
  }

  try {
    const data = await loadTrack(track.path);
    const ext = getFileExtension(track.format);
    const filename = sanitizeFilename(track.title || 'track') + '.' + ext;

    const arrayBuffer = data.buffer.slice(data.byteOffset, data.byteOffset + data.byteLength) as ArrayBuffer;
    const blob = new Blob([arrayBuffer], { type: 'application/octet-stream' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = filename;
    a.click();
    URL.revokeObjectURL(url);

    showToast(`Downloaded ${filename}`);
  } catch (err) {
    console.error('Download error:', err);
    showToast('Download failed');
  }
}

// ============================================================================
// Load From File (Drag & Drop / File Input)
// ============================================================================

export async function loadFromFile(file: File): Promise<void> {
  try {
    await ensureAudioContext();
    const buffer = await file.arrayBuffer();
    const data = new Uint8Array(buffer);

    if (!state.Ym2149Player) throw new Error('Player not initialized');
    const player = new state.Ym2149Player(data);
    const volumeValue = elements.volumeSlider ? parseFloat(elements.volumeSlider.value) / 100 : 1;
    player.set_volume(volumeValue);
    state.setWasmPlayer(player);
    state.setCurrentTrackIndex(-1);
    state.setLoadedFileData(data);
    state.setLoadedFileName(file.name);

    // Reset waveform
    state.setWaveformOverviewData(null);
    elements.waveformScrubber?.classList.add('hidden');
    elements.progressContainer?.classList.remove('hidden');

    const meta = player.metadata;
    state.setCurrentFormat(meta.format || '');

    // Setup channel UI
    const channelCount = player.channelCount ? player.channelCount() : 3;
    setupChannelUI(channelCount);

    clearAllWaveforms();
    drawAllVisualization();

    if (elements.songTitle) elements.songTitle.textContent = file.name;
    if (elements.songAuthor) elements.songAuthor.textContent = meta.author || 'Dropped file';
    if (elements.songFormat) elements.songFormat.textContent = meta.format || '-';
    if (elements.songFrames) elements.songFrames.textContent = `${meta.frame_count} frames`;
    if (elements.totalTime) elements.totalTime.textContent = formatTime(meta.duration_seconds);

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
    console.error('Load error:', err);
  }
}
