// ============================================================================
// Events - Event handlers, keyboard shortcuts, drag & drop
// ============================================================================

import * as state from './state.ts';
import { elements } from './ui/elements.ts';
import {
  filterAndGroupTracks,
  updateVisibleRows,
  collapseAllAuthors,
  expandAllAuthors,
  toggleAuthor,
  attachTrackListHandlers,
  updateStickyAuthorHeader,
  scrollToCurrentTrack,
  renderTrackList,
} from './ui/trackList.ts';
import {
  showToast,
  toggleSidebar,
  updateProgressUI,
  renderCollections,
  updateFavoritesCount,
  updatePlayerFavoriteButton,
} from './ui/player.ts';
import {
  toggleFavoriteStorage,
  togglePinAuthorStorage,
  clearPlayStats,
  clearOwnFilesStorage,
  saveOwnFile,
  saveAuthorSortMode,
} from './storage.ts';
import { ensureAudioContext } from './audio/context.ts';
import {
  togglePlayPause,
  stop,
  restart,
  playNext,
  playPrev,
  playTrack,
  toggleShuffle,
  toggleAutoPlay,
  setLoopA,
  setLoopB,
  clearLoop,
  setPlaybackSpeed,
  changeSubsong,
  prevSubsong,
  nextSubsong,
  shareCurrentTrack,
  loadFromFile,
  downloadCurrentTrack,
} from './audio/playback.ts';
import { updateWaveformPlayhead, startScrubbing, continueScrubbing, stopScrubbing, drawWaveformOverview } from './visualization/waveform.ts';
import { setupAllCanvases } from './visualization/core.ts';
import { showExportModal, hideExportModal, exportWav } from './export.ts';
import type { CollectionId } from './types/index.ts';

// ============================================================================
// Keyboard Shortcuts
// ============================================================================

export function handleKeyboardShortcuts(e: KeyboardEvent): void {
  // Don't trigger if typing in an input
  const target = e.target as HTMLElement;
  if (target.tagName === 'INPUT' || target.tagName === 'TEXTAREA' || target.tagName === 'SELECT') return;

  // Handle space key
  if (e.code === 'Space' || e.key === ' ') {
    e.preventDefault();
    togglePlayPause();
    return;
  }

  // Calculate seek amount
  const duration = state.wasmPlayer?.metadata?.duration_seconds || 180;
  const seekAmount = 5 / duration;

  switch (e.key.toLowerCase()) {
    case 'arrowleft':
      if (e.shiftKey && state.wasmPlayer) {
        e.preventDefault();
        state.wasmPlayer.seek_to_percentage(Math.max(0, state.wasmPlayer.position_percentage() - seekAmount));
        updateProgressUI();
        updateWaveformPlayhead();
      } else if (!e.shiftKey) {
        playPrev();
        setTimeout(scrollToCurrentTrack, 100);
      }
      break;
    case 'arrowright':
      if (e.shiftKey && state.wasmPlayer) {
        e.preventDefault();
        state.wasmPlayer.seek_to_percentage(Math.min(1, state.wasmPlayer.position_percentage() + seekAmount));
        updateProgressUI();
        updateWaveformPlayhead();
      } else if (!e.shiftKey) {
        playNext();
        setTimeout(scrollToCurrentTrack, 100);
      }
      break;
    case 'arrowup':
      e.preventDefault();
      if (elements.volumeSlider) {
        elements.volumeSlider.value = String(Math.min(100, parseInt(elements.volumeSlider.value) + 10));
        elements.volumeSlider.dispatchEvent(new Event('input'));
      }
      break;
    case 'arrowdown':
      e.preventDefault();
      if (elements.volumeSlider) {
        elements.volumeSlider.value = String(Math.max(0, parseInt(elements.volumeSlider.value) - 10));
        elements.volumeSlider.dispatchEvent(new Event('input'));
      }
      break;
    case 'm':
      if (elements.volumeSlider) {
        elements.volumeSlider.value = parseInt(elements.volumeSlider.value) > 0 ? '0' : '100';
        elements.volumeSlider.dispatchEvent(new Event('input'));
      }
      break;
    case 's':
      toggleShuffle();
      break;
    case 'n':
      toggleAutoPlay();
      break;
    case 'a':
      if (!e.ctrlKey && !e.metaKey) setLoopA();
      break;
    case 'b':
      setLoopB();
      break;
    case 'c':
      if (e.shiftKey) clearLoop();
      break;
    case 'r':
      restart();
      break;
    case 'f':
      if (!e.ctrlKey && !e.metaKey && state.currentTrackIndex >= 0 && state.filteredTracks[state.currentTrackIndex]) {
        toggleFavorite(state.filteredTracks[state.currentTrackIndex]?.path ?? '');
      }
      break;
    case 'tab':
      e.preventDefault();
      toggleSidebar();
      break;
    case 'escape':
      hideKeyboardHelp();
      if (state.sidebarVisible) toggleSidebar();
      break;
    case '[':
      prevSubsong();
      break;
    case ']':
      nextSubsong();
      break;
    case '?':
      showKeyboardHelp();
      break;
  }
}

function showKeyboardHelp(): void {
  document.getElementById('helpModal')?.classList.remove('hidden');
}

function hideKeyboardHelp(): void {
  document.getElementById('helpModal')?.classList.add('hidden');
}

// ============================================================================
// Favorites
// ============================================================================

function toggleFavorite(path: string, e: MouseEvent | null = null): void {
  if (!path) return;
  const isFav = toggleFavoriteStorage(path);
  if (e) {
    const btn = (e.target as HTMLElement).closest('.fav-btn');
    if (btn) {
      btn.classList.toggle('text-red-500', isFav);
      btn.classList.toggle('text-gray-600', !isFav);
      const svg = btn.querySelector('svg');
      if (svg) svg.setAttribute('fill', isFav ? 'currentColor' : 'none');
    }
  }
  updateFavoritesCount();
  showToast(isFav ? 'Added to favorites' : 'Removed from favorites');

  // Re-render if in favorites collection
  if (state.currentCollection === 'favorites') {
    filterAndGroupTracks();
  }

  updatePlayerFavoriteButton();
}

// ============================================================================
// Pin Authors
// ============================================================================

function togglePinAuthor(collection: string, author: string, _e: MouseEvent | null = null): void {
  const isPinned = togglePinAuthorStorage(collection, author);
  showToast(isPinned ? `Pinned ${author}` : `Unpinned ${author}`);
  filterAndGroupTracks();
}

// ============================================================================
// Drag & Drop
// ============================================================================

export async function handleFileDrop(e: DragEvent): Promise<void> {
  if (!e.dataTransfer) return;
  const files = [...e.dataTransfer.files].filter(f =>
    /\.(ym|sndh|ay|vtx|psg|stc|pt3|sqt|asc|fxm|fc|aks)$/i.test(f.name)
  );

  if (files.length === 0) {
    showToast('No supported audio files found');
    return;
  }

  for (const file of files) {
    await saveOwnFile(file);
  }
  showToast(`Added ${files.length} file${files.length > 1 ? 's' : ''}`);

  renderCollections(getCollectionCallbacks());

  // Switch to Own tab and play first dropped file
  state.setCurrentCollection('own');
  filterAndGroupTracks();
  const lastAdded = state.ownFiles[state.ownFiles.length - 1];
  if (lastAdded) {
    const idx = state.filteredTracks.findIndex(t => t.path === lastAdded.path);
    if (idx >= 0) playTrack(idx);
  }
}

function setupDragAndDrop(): void {
  const overlay = document.getElementById('dropOverlay');
  let dragCounter = 0;

  document.body.addEventListener('dragenter', (e) => {
    e.preventDefault();
    dragCounter++;
    overlay?.classList.add('visible');
  });

  document.body.addEventListener('dragleave', (e) => {
    e.preventDefault();
    dragCounter--;
    if (dragCounter <= 0) {
      dragCounter = 0;
      overlay?.classList.remove('visible');
    }
  });

  document.body.addEventListener('dragover', (e) => {
    e.preventDefault();
  });

  document.body.addEventListener('drop', (e) => {
    e.preventDefault();
    dragCounter = 0;
    overlay?.classList.remove('visible');
    handleFileDrop(e);
  });
}

function setupDragDrop(): void {
  document.addEventListener('dragover', (e) => {
    e.preventDefault();
    elements.dropZone?.classList.remove('hidden');
  });

  document.addEventListener('dragleave', (e) => {
    if ((e as DragEvent).relatedTarget === null) {
      elements.dropZone?.classList.add('hidden');
    }
  });

  document.addEventListener('drop', async (e) => {
    e.preventDefault();
    elements.dropZone?.classList.add('hidden');

    const file = (e as DragEvent).dataTransfer?.files[0];
    if (!file) return;

    await loadFromFile(file);
  });
}

// ============================================================================
// Collection Callbacks
// ============================================================================

interface CollectionCallbacks {
  onCollectionClick: (collection: CollectionId) => void;
  onClearRecent: () => void;
  onClearOwn: () => void;
}

function getCollectionCallbacks(): CollectionCallbacks {
  return {
    onCollectionClick: (collection: CollectionId) => {
      state.setCurrentCollection(collection);
      filterAndGroupTracks();
      renderCollections(getCollectionCallbacks());
    },
    onClearRecent: () => {
      clearPlayStats();
      filterAndGroupTracks();
      renderCollections(getCollectionCallbacks());
    },
    onClearOwn: () => {
      clearOwnFilesStorage();
      filterAndGroupTracks();
      renderCollections(getCollectionCallbacks());
    },
  };
}

export { getCollectionCallbacks };

// ============================================================================
// Setup All Event Handlers
// ============================================================================

export function setupEventHandlers(): void {
  // Setup drag and drop
  setupDragAndDrop();
  setupDragDrop();

  // Help modal
  document.getElementById('helpBtn')?.addEventListener('click', showKeyboardHelp);
  document.getElementById('helpModal')?.addEventListener('click', (e) => {
    if ((e.target as HTMLElement).id === 'helpModal') hideKeyboardHelp();
  });
  document.getElementById('helpCloseBtn')?.addEventListener('click', hideKeyboardHelp);

  // iOS audio unlock
  const unlockAudio = async () => {
    await ensureAudioContext();
    document.removeEventListener('touchstart', unlockAudio);
    document.removeEventListener('touchend', unlockAudio);
    document.removeEventListener('click', unlockAudio);
  };
  document.addEventListener('touchstart', unlockAudio, { once: true });
  document.addEventListener('touchend', unlockAudio, { once: true });
  document.addEventListener('click', unlockAudio, { once: true });

  // Search
  let searchTimeout: ReturnType<typeof setTimeout>;
  elements.searchInput?.addEventListener('input', () => {
    clearTimeout(searchTimeout);
    elements.searchClear?.classList.toggle('hidden', !elements.searchInput?.value);
    searchTimeout = setTimeout(() => {
      state.setSearchQuery(elements.searchInput?.value.trim() ?? '');
      filterAndGroupTracks();
    }, 150);
  });

  elements.searchClear?.addEventListener('click', () => {
    if (elements.searchInput) elements.searchInput.value = '';
    elements.searchClear?.classList.add('hidden');
    state.setSearchQuery('');
    filterAndGroupTracks();
    elements.searchInput?.focus();
  });

  // Virtual scroll
  elements.trackList?.addEventListener('scroll', () => {
    requestAnimationFrame(() => {
      updateVisibleRows();
      updateStickyAuthorHeader();
    });
  });

  // Toggle collapse all button
  const toggleCollapseBtn = document.getElementById('toggleCollapseBtn');
  toggleCollapseBtn?.addEventListener('click', () => {
    if (state.allCollapsed) {
      expandAllAuthors();
      if (toggleCollapseBtn) toggleCollapseBtn.textContent = 'Collapse All';
    } else {
      collapseAllAuthors();
      if (toggleCollapseBtn) toggleCollapseBtn.textContent = 'Expand All';
    }
  });

  // Sort mode button
  const sortModeBtn = document.getElementById('sortModeBtn');
  sortModeBtn?.addEventListener('click', () => {
    const newMode = state.authorSortMode === 'alpha' ? 'count' : 'alpha';
    state.setAuthorSortMode(newMode);
    saveAuthorSortMode();
    if (sortModeBtn) sortModeBtn.textContent = newMode === 'alpha' ? 'A-Z' : '#';
    filterAndGroupTracks();
  });

  // File input
  elements.fileInput?.addEventListener('change', async (e) => {
    const file = (e.target as HTMLInputElement).files?.[0];
    if (file) await loadFromFile(file);
  });

  // Player controls
  elements.playBtn?.addEventListener('click', togglePlayPause);
  elements.stopBtn?.addEventListener('click', stop);
  elements.restartBtn?.addEventListener('click', restart);
  elements.nextBtn?.addEventListener('click', playNext);
  elements.shuffleBtn?.addEventListener('click', toggleShuffle);
  elements.autoPlayBtn?.addEventListener('click', toggleAutoPlay);
  elements.loopABtn?.addEventListener('click', setLoopA);
  elements.loopBBtn?.addEventListener('click', setLoopB);
  elements.loopClearBtn?.addEventListener('click', clearLoop);
  elements.speedSelect?.addEventListener('change', (e) => setPlaybackSpeed(parseFloat((e.target as HTMLSelectElement).value)));
  elements.subsongSelect?.addEventListener('change', (e) => changeSubsong(parseInt((e.target as HTMLSelectElement).value)));
  elements.shareBtn?.addEventListener('click', shareCurrentTrack);
  elements.downloadBtn?.addEventListener('click', downloadCurrentTrack);
  elements.sidebarToggle?.addEventListener('click', toggleSidebar);
  elements.mobileMenuBtn?.addEventListener('click', toggleSidebar);
  elements.hideSidebarBtn?.addEventListener('click', toggleSidebar);
  elements.sidebarBackdrop?.addEventListener('click', toggleSidebar);

  // Keyboard shortcuts
  document.addEventListener('keydown', handleKeyboardShortcuts);

  // Player favorite button
  elements.playerFavBtn?.addEventListener('click', () => {
    if (state.currentTrackIndex >= 0 && state.filteredTracks[state.currentTrackIndex]) {
      toggleFavorite(state.filteredTracks[state.currentTrackIndex]?.path ?? '');
    }
  });

  // Progress seek
  elements.progressBar?.addEventListener('input', () => {
    if (!state.wasmPlayer || !elements.progressBar) return;
    state.wasmPlayer.seek_to_percentage(parseFloat(elements.progressBar.value) / 100);
    updateProgressUI();
  });

  // Volume
  elements.volumeSlider?.addEventListener('input', () => {
    if (state.wasmPlayer && elements.volumeSlider) {
      state.wasmPlayer.set_volume(parseFloat(elements.volumeSlider.value) / 100);
    }
  });

  // Visualization mode
  elements.vizModeOsc?.addEventListener('click', () => {
    elements.oscView?.classList.remove('hidden');
    elements.specView?.classList.add('hidden');
    elements.vizModeOsc?.classList.remove('bg-gray-800', 'text-gray-400');
    elements.vizModeOsc?.classList.add('bg-chip-purple/20', 'text-chip-purple', 'active');
    elements.vizModeSpec?.classList.remove('bg-chip-purple/20', 'text-chip-purple', 'active');
    elements.vizModeSpec?.classList.add('bg-gray-800', 'text-gray-400');
    requestAnimationFrame(setupAllCanvases);
  });

  elements.vizModeSpec?.addEventListener('click', () => {
    elements.specView?.classList.remove('hidden');
    elements.oscView?.classList.add('hidden');
    elements.vizModeSpec?.classList.remove('bg-gray-800', 'text-gray-400');
    elements.vizModeSpec?.classList.add('bg-chip-purple/20', 'text-chip-purple', 'active');
    elements.vizModeOsc?.classList.remove('bg-chip-purple/20', 'text-chip-purple', 'active');
    elements.vizModeOsc?.classList.add('bg-gray-800', 'text-gray-400');
    requestAnimationFrame(setupAllCanvases);
  });

  // Export
  elements.exportBtn?.addEventListener('click', showExportModal);
  elements.exportCancel?.addEventListener('click', hideExportModal);
  elements.exportStart?.addEventListener('click', exportWav);
  elements.exportMode?.addEventListener('change', (e) => {
    elements.exportStemOptions?.classList.toggle('hidden', (e.target as HTMLSelectElement).value !== 'stems');
  });

  // Waveform scrubber
  elements.waveformScrubber?.addEventListener('mousedown', startScrubbing);
  document.addEventListener('mousemove', continueScrubbing);
  document.addEventListener('mouseup', stopScrubbing);
  elements.waveformScrubber?.addEventListener('touchstart', startScrubbing, { passive: false });
  document.addEventListener('touchmove', continueScrubbing, { passive: false });
  document.addEventListener('touchend', stopScrubbing);

  // Resize
  window.addEventListener('resize', () => {
    setupAllCanvases();
    renderTrackList();
    if (state.waveformOverviewData) drawWaveformOverview();
  });
}

// ============================================================================
// Track List Handler Registration
// ============================================================================

export function setupTrackListInteraction(): void {
  attachTrackListHandlers({
    onTrackClick: (index: number) => playTrack(index),
    onFavoriteClick: (path: string, e: MouseEvent) => toggleFavorite(path, e),
    onPinClick: (collection: string, author: string, e: MouseEvent) => togglePinAuthor(collection, author, e),
    onAuthorClick: (collection: string, author: string) => toggleAuthor(collection, author),
  });
}
