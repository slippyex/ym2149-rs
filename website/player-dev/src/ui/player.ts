// ============================================================================
// UI Player - Player UI controls and metadata display
// ============================================================================

import { CHANNEL_COLORS, CHANNEL_NAMES, SNDH_CHANNEL_NAMES, SNDH_CHANNEL_COLORS, RECENT_DISPLAY_LIMIT } from '../config.ts';
import * as state from '../state.ts';
import { elements } from './elements.ts';
import { escapeHtml } from '../search.ts';
import { getRecentlyPlayedCount, getValidFavoritesCount } from '../storage.ts';
import { findSimilarByFingerprint } from '../fingerprint.ts';
import type { Track, CollectionId, SimilarTrackClickHandler } from '../types/index.ts';

// ============================================================================
// Channel Helpers
// ============================================================================

export function getChannelName(index: number): string {
  if (state.currentFormat === 'SNDH' && index < SNDH_CHANNEL_NAMES.length) {
    return SNDH_CHANNEL_NAMES[index] ?? `${index + 1}`;
  }
  return CHANNEL_NAMES[index] ?? `${index + 1}`;
}

export function getChannelColor(index: number): string {
  if (state.currentFormat === 'SNDH' && index < SNDH_CHANNEL_COLORS.length) {
    return SNDH_CHANNEL_COLORS[index] ?? CHANNEL_COLORS[0] ?? '#8b5cf6';
  }
  return CHANNEL_COLORS[index % CHANNEL_COLORS.length] ?? '#8b5cf6';
}

export function getChannelNames(channelCount: number): string[] {
  const format = state.currentFormat.toUpperCase();
  if (format === 'SNDH' && channelCount === 5) {
    return ['CH A', 'CH B', 'CH C', 'DAC L', 'DAC R'];
  } else if (channelCount === 6) {
    return ['CH A', 'CH B', 'CH C', 'CH D', 'CH E', 'CH F'];
  } else {
    return Array.from({ length: channelCount }, (_, i) => `CH ${String.fromCharCode(65 + i)}`);
  }
}

// ============================================================================
// Time Formatting
// ============================================================================

export function formatTime(seconds: number): string {
  const m = Math.floor(seconds / 60);
  const s = Math.floor(seconds % 60);
  return `${m}:${s.toString().padStart(2, '0')}`;
}

// ============================================================================
// Metadata UI
// ============================================================================

export function updateMetadataUI(track: Track | null): void {
  if (!state.wasmPlayer) return;
  const meta = state.wasmPlayer.metadata;
  const channelCount = state.wasmPlayer.channelCount ? state.wasmPlayer.channelCount() : 3;

  if (elements.songTitle) elements.songTitle.textContent = track?.title || meta.title || 'Unknown';
  if (elements.songAuthor) elements.songAuthor.textContent = track?.author || meta.author || 'Unknown';
  if (elements.songFormat) elements.songFormat.textContent = track?.format || meta.format || '-';
  if (elements.songFrames) elements.songFrames.textContent = `${meta.frame_count} frames`;

  // Show channel count if more than 3
  if (elements.songChannels) {
    if (channelCount > 3) {
      elements.songChannels.textContent = `${channelCount} channels`;
      elements.songChannels.classList.remove('hidden');
    } else {
      elements.songChannels.classList.add('hidden');
    }
  }

  updateSubsongSelector();
  if (elements.totalTime) elements.totalTime.textContent = formatTime(meta.duration_seconds);
  if (elements.progressBar) {
    (elements.progressBar as HTMLInputElement).value = '0';
  }
  if (elements.currentTime) elements.currentTime.textContent = '0:00';
}

export function updateSubsongSelector(): void {
  if (!state.wasmPlayer || !elements.subsongSelect) {
    elements.subsongSelect?.classList.add('hidden');
    return;
  }

  const subsongCount = state.wasmPlayer.subsongCount ? state.wasmPlayer.subsongCount() : 1;
  const currentSubsong = state.wasmPlayer.currentSubsong ? state.wasmPlayer.currentSubsong() : 1;

  if (subsongCount <= 1) {
    elements.subsongSelect.classList.add('hidden');
    return;
  }

  let html = '';
  for (let i = 1; i <= subsongCount; i++) {
    html += `<option value="${i}" ${i === currentSubsong ? 'selected' : ''}>Track ${i}/${subsongCount}</option>`;
  }
  elements.subsongSelect.innerHTML = html;
  elements.subsongSelect.classList.remove('hidden');
}

// ============================================================================
// Play Button
// ============================================================================

export function updatePlayButton(): void {
  elements.playIcon?.classList.toggle('hidden', state.isPlaying);
  elements.pauseIcon?.classList.toggle('hidden', !state.isPlaying);
  elements.playBtn?.classList.toggle('playing', state.isPlaying);
}

// ============================================================================
// Favorite Button
// ============================================================================

export function updatePlayerFavoriteButton(): void {
  if (state.currentTrackIndex < 0 || !state.filteredTracks[state.currentTrackIndex]) {
    elements.playerFavBtn?.classList.add('hidden');
    return;
  }
  const path = state.filteredTracks[state.currentTrackIndex]?.path;
  if (!path) return;
  const isFav = state.favorites.has(path);
  elements.playerFavBtn?.classList.remove('hidden');
  elements.playerFavBtn?.classList.toggle('text-red-500', isFav);
  elements.playerFavBtn?.classList.toggle('text-gray-600', !isFav);
  elements.playerFavIcon?.setAttribute('fill', isFav ? 'currentColor' : 'none');
  if (elements.playerFavBtn) {
    elements.playerFavBtn.title = isFav ? 'Remove from favorites' : 'Add to favorites';
  }
}

export function updateFavoritesCount(): void {
  const tab = document.querySelector('[data-collection="favorites"]');
  if (tab) tab.textContent = `♥ (${getValidFavoritesCount()})`;
}

// ============================================================================
// Loop UI
// ============================================================================

export function updateLoopUI(): void {
  const hasLoop = state.loopA !== null && state.loopB !== null;

  elements.loopABtn?.classList.toggle('bg-chip-cyan', state.loopA !== null);
  elements.loopABtn?.classList.toggle('text-black', state.loopA !== null);
  elements.loopABtn?.classList.toggle('active', state.loopA !== null);
  elements.loopBBtn?.classList.toggle('bg-chip-cyan', state.loopB !== null);
  elements.loopBBtn?.classList.toggle('text-black', state.loopB !== null);
  elements.loopBBtn?.classList.toggle('active', state.loopB !== null);
  elements.loopClearBtn?.classList.toggle('hidden', !hasLoop);

  // Update progress bar markers
  if (state.loopA !== null && elements.loopMarkerA) {
    elements.loopMarkerA.style.left = `${state.loopA * 100}%`;
    elements.loopMarkerA.classList.remove('hidden');
  } else {
    elements.loopMarkerA?.classList.add('hidden');
  }

  if (state.loopB !== null && elements.loopMarkerB) {
    elements.loopMarkerB.style.left = `${state.loopB * 100}%`;
    elements.loopMarkerB.classList.remove('hidden');
  } else {
    elements.loopMarkerB?.classList.add('hidden');
  }

  if (hasLoop && state.loopA !== null && state.loopB !== null) {
    const duration = state.wasmPlayer?.metadata?.duration_seconds || 0;
    const startTime = formatTime(state.loopA * duration);
    const endTime = formatTime(state.loopB * duration);
    if (elements.loopIndicator) {
      elements.loopIndicator.textContent = `${startTime}-${endTime}`;
      elements.loopIndicator.classList.remove('hidden');
    }
    if (elements.loopRegion) {
      elements.loopRegion.style.left = `${state.loopA * 100}%`;
      elements.loopRegion.style.width = `${(state.loopB - state.loopA) * 100}%`;
      elements.loopRegion.classList.remove('hidden');
    }
  } else {
    elements.loopIndicator?.classList.add('hidden');
    elements.loopRegion?.classList.add('hidden');
  }
}

// ============================================================================
// Progress UI
// ============================================================================

export function updateProgressUI(): void {
  if (!state.wasmPlayer) return;
  const position = state.wasmPlayer.position_percentage();
  const duration = state.wasmPlayer.metadata.duration_seconds;
  if (elements.progressBar) {
    (elements.progressBar as HTMLInputElement).value = String(position * 100);
  }
  if (elements.currentTime) {
    elements.currentTime.textContent = formatTime(position * duration);
  }
}

// ============================================================================
// LMC1992 Display (SNDH only)
// ============================================================================

interface Lmc1992StateExtended {
  masterVolume: number;
  masterVolumeRaw: number;
  leftVolume: number;
  leftVolumeRaw: number;
  rightVolume: number;
  rightVolumeRaw: number;
  bass: number;
  bassRaw: number;
  treble: number;
  trebleRaw: number;
}

interface WasmPlayerWithLmc {
  getLmc1992State?: () => Lmc1992StateExtended | null;
}

export function updateLmc1992Display(): void {
  if (!state.wasmPlayer) {
    elements.lmc1992Panel?.classList.add('hidden');
    return;
  }

  const player = state.wasmPlayer as unknown as WasmPlayerWithLmc;
  if (typeof player.getLmc1992State !== 'function') {
    elements.lmc1992Panel?.classList.add('hidden');
    return;
  }

  const lmc = player.getLmc1992State();

  if (!lmc || lmc.masterVolume === undefined) {
    elements.lmc1992Panel?.classList.add('hidden');
    return;
  }

  elements.lmc1992Panel?.classList.remove('hidden');
  if (elements.lmcMasterVol) elements.lmcMasterVol.textContent = `${lmc.masterVolume} (${lmc.masterVolumeRaw})`;
  if (elements.lmcLeftVol) elements.lmcLeftVol.textContent = `${lmc.leftVolume} (${lmc.leftVolumeRaw})`;
  if (elements.lmcRightVol) elements.lmcRightVol.textContent = `${lmc.rightVolume} (${lmc.rightVolumeRaw})`;
  const bassDb = lmc.bass >= 0 ? `+${lmc.bass}` : String(lmc.bass);
  if (elements.lmcBass) elements.lmcBass.textContent = `${bassDb} (${lmc.bassRaw})`;
  const trebleDb = lmc.treble >= 0 ? `+${lmc.treble}` : String(lmc.treble);
  if (elements.lmcTreble) elements.lmcTreble.textContent = `${trebleDb} (${lmc.trebleRaw})`;
}

// ============================================================================
// Similar Tracks
// ============================================================================

export function updateSimilarTracks(currentTrack: Track | null, onTrackClick: SimilarTrackClickHandler | null): void {
  if (!state.catalog || !currentTrack) {
    elements.similarPanel?.classList.add('hidden');
    return;
  }

  const author = currentTrack.author;
  const format = currentTrack.format;

  // Calculate how many similar tracks to show based on screen width
  const containerWidth = elements.similarTracks?.parentElement?.offsetWidth ?? window.innerWidth;
  const trackButtonWidth = 160;
  const headerWidth = 120;
  const availableWidth = containerWidth - headerWidth;
  const maxTracks = Math.max(2, Math.min(8, Math.floor(availableWidth / trackButtonWidth)));

  let similar: Track[] = [];
  let similarityMode = 'metadata';

  // Try fingerprint-based similarity first
  if (currentTrack.fp) {
    const fpSimilar = findSimilarByFingerprint(currentTrack.path, maxTracks + 2);
    if (fpSimilar.length >= 2) {
      similar = fpSimilar;
      similarityMode = 'audio';
    }
  }

  // Fallback to metadata-based similarity
  if (similar.length < 2) {
    similar = state.catalog.tracks.filter(
      (t) => t.path !== currentTrack.path && (t.author === author || t.format === format),
    );
    similar.sort((a, b) => {
      const aScore = (a.author === author ? 2 : 0) + (a.format === format ? 1 : 0);
      const bScore = (b.author === author ? 2 : 0) + (b.format === format ? 1 : 0);
      return bScore - aScore;
    });
    similarityMode = 'metadata';
  }

  similar = similar.slice(0, maxTracks);

  if (similar.length < 2) {
    elements.similarPanel?.classList.add('hidden');
    return;
  }

  if (elements.similarAuthor) {
    if (similarityMode === 'audio') {
      elements.similarAuthor.textContent = 'similar sound';
    } else {
      elements.similarAuthor.textContent = author || 'similar artists';
    }
  }
  elements.similarPanel?.classList.remove('hidden');

  let html = '';
  for (const track of similar) {
    const isSameAuthor = track.author === author;
    html += `
      <button class="similar-track shrink-0 px-2 py-1 bg-gray-800 hover:bg-gray-700 rounded text-xs text-left max-w-[150px] truncate"
              data-path="${escapeHtml(track.path)}" title="${escapeHtml(track.title)} by ${escapeHtml(track.author || 'Unknown')}">
        <div class="truncate ${isSameAuthor ? 'text-white' : 'text-gray-300'}">${escapeHtml(track.title)}</div>
        ${!isSameAuthor ? `<div class="truncate text-gray-500 text-[10px]">${escapeHtml(track.author || 'Unknown')}</div>` : ''}
      </button>`;
  }
  if (elements.similarTracks) elements.similarTracks.innerHTML = html;

  // Add click handlers
  elements.similarTracks?.querySelectorAll('.similar-track').forEach((btn) => {
    btn.addEventListener('click', () => {
      const path = (btn as HTMLElement).dataset.path;
      if (!path || !state.catalog) return;
      const idx = state.catalog.tracks.findIndex((t) => t.path === path);
      if (idx >= 0 && onTrackClick) {
        onTrackClick(path, idx);
      }
    });
  });
}

// ============================================================================
// Collection Tabs
// ============================================================================

interface CollectionCallbacks {
  onCollectionClick: (collection: CollectionId) => void;
  onClearRecent: () => void;
  onClearOwn: () => void;
}

interface CatalogCollection {
  id: CollectionId;
  format: string;
  trackCount: number;
}

interface CatalogWithCollections {
  tracks: Track[];
  collections: CatalogCollection[];
}

export function renderCollections(callbacks: CollectionCallbacks): void {
  if (!state.catalog) return;

  const { onCollectionClick, onClearRecent, onClearOwn } = callbacks;
  const catalogWithCollections = state.catalog as unknown as CatalogWithCollections;

  let html = `<button class="collection-tab px-2 py-1 rounded text-xs border border-transparent ${state.currentCollection === 'all' ? 'active' : 'hover:bg-gray-800'}" data-collection="all">All</button>`;

  // Favorites tab
  html += `<button class="collection-tab px-2 py-1 rounded text-xs border border-transparent ${state.currentCollection === 'favorites' ? 'active' : 'hover:bg-gray-800'}" data-collection="favorites">♥ (${getValidFavoritesCount()})</button>`;

  // Recently Played tab
  const recentCount = getRecentlyPlayedCount();
  const recentDisplayCount = Math.min(recentCount, RECENT_DISPLAY_LIMIT);
  html += `<span class="inline-flex flex-nowrap"><button class="collection-tab px-2 py-1 rounded-l text-xs border border-transparent ${state.currentCollection === 'recent' ? 'active' : 'hover:bg-gray-800'}" data-collection="recent">Recent (${recentDisplayCount})</button>`;
  if (recentCount > 0) {
    html += `<button class="collection-tab px-1 py-1 rounded-r text-xs border border-transparent hover:bg-red-900/50 text-gray-500 hover:text-red-400" data-clear="recent" title="Clear recent history">×</button>`;
  }
  html += `</span>`;

  // Own files tab
  const ownCount = state.ownFiles.length;
  if (ownCount > 0 || state.currentCollection === 'own') {
    html += `<span class="inline-flex flex-nowrap"><button class="collection-tab px-2 py-1 rounded-l text-xs border border-transparent ${state.currentCollection === 'own' ? 'active' : 'hover:bg-gray-800'}" data-collection="own">Own (${ownCount})</button>`;
    if (ownCount > 0) {
      html += `<button class="collection-tab px-1 py-1 rounded-r text-xs border border-transparent hover:bg-red-900/50 text-gray-500 hover:text-red-400" data-clear="own" title="Clear own files">×</button>`;
    }
    html += `</span>`;
  }

  if (catalogWithCollections.collections) {
    for (const col of catalogWithCollections.collections) {
      if (col.trackCount === 0) continue;
      html += `<button class="collection-tab px-2 py-1 rounded text-xs border border-transparent ${state.currentCollection === col.id ? 'active' : 'hover:bg-gray-800'}" data-collection="${col.id}">${col.format} (${col.trackCount})</button>`;
    }
  }

  if (elements.collectionTabs) elements.collectionTabs.innerHTML = html;
  if (elements.totalTracks) elements.totalTracks.textContent = state.catalog.tracks.length.toLocaleString();

  // Attach handlers
  elements.collectionTabs?.querySelectorAll('.collection-tab[data-collection]').forEach((tab) => {
    tab.addEventListener('click', () => onCollectionClick((tab as HTMLElement).dataset.collection as CollectionId));
  });

  elements.collectionTabs?.querySelectorAll('[data-clear]').forEach((btn) => {
    btn.addEventListener('click', (e) => {
      e.stopPropagation();
      const clearType = (btn as HTMLElement).dataset.clear;
      if (clearType === 'recent') {
        onClearRecent();
      } else if (clearType === 'own') {
        onClearOwn();
      }
    });
  });
}

// ============================================================================
// Toast Notification
// ============================================================================

export function showToast(message: string, duration = 2000): void {
  if (elements.toastMessage) elements.toastMessage.textContent = message;
  elements.toast?.classList.remove('opacity-0', 'translate-y-4');
  elements.toast?.classList.add('opacity-100', 'translate-y-0');

  if (state.toastTimeout) clearTimeout(state.toastTimeout);
  state.setToastTimeout(setTimeout(() => {
    elements.toast?.classList.add('opacity-0', 'translate-y-4');
    elements.toast?.classList.remove('opacity-100', 'translate-y-0');
  }, duration));
}

// ============================================================================
// Sidebar
// ============================================================================

export function toggleSidebar(): void {
  state.setSidebarVisible(!state.sidebarVisible);
  if (state.sidebarVisible) {
    elements.sidebar?.classList.remove('w-0', 'overflow-hidden');
    elements.sidebar?.classList.add('w-72', 'lg:w-80');
    elements.sidebarToggle?.classList.add('hidden');
    if (window.innerWidth < 1024) {
      elements.sidebarBackdrop?.classList.remove('hidden');
    }
  } else {
    elements.sidebar?.classList.add('w-0', 'overflow-hidden');
    elements.sidebar?.classList.remove('w-72', 'lg:w-80');
    elements.sidebarToggle?.classList.remove('hidden');
    elements.sidebarBackdrop?.classList.add('hidden');
  }
}

export function closeSidebarOnMobile(): void {
  if (window.innerWidth < 1024 && state.sidebarVisible) {
    toggleSidebar();
  }
}

// ============================================================================
// Controls Enable
// ============================================================================

export function enableControls(): void {
  if (elements.playBtn) elements.playBtn.disabled = false;
  if (elements.stopBtn) elements.stopBtn.disabled = false;
  if (elements.restartBtn) elements.restartBtn.disabled = false;
  if (elements.nextBtn) elements.nextBtn.disabled = false;
  if (elements.progressBar) (elements.progressBar as HTMLInputElement).disabled = false;
  if (elements.loopABtn) elements.loopABtn.disabled = false;
  if (elements.loopBBtn) elements.loopBBtn.disabled = false;
}
