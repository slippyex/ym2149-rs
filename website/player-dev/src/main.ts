// ============================================================================
// Main - Bootstrap module that connects everything
// ============================================================================

import './styles/main.css';
import * as state from './state.ts';
import { initElements } from './ui/elements.ts';
import {
  loadFavorites,
  loadPlayStats,
  loadPinnedAuthors,
  loadOwnFilesMeta,
  loadAuthorSortMode,
} from './storage.ts';
import { loadFingerprints } from './fingerprint.ts';
import { loadCatalogCached } from './catalog.ts';
import {
  filterAndGroupTracks,
} from './ui/trackList.ts';
import {
  toggleSidebar,
  showToast,
  renderCollections,
  updateSubsongSelector,
  updateProgressUI,
} from './ui/player.ts';
import { setupChannelUI, setupAllCanvases } from './visualization/core.ts';
import { setupEventHandlers, getCollectionCallbacks, setupTrackListInteraction } from './events.ts';
import { playTrack } from './audio/playback.ts';
import type { Catalog, Ym2149PlayerConstructor, Track, CollectionId } from './types/index.ts';

// ============================================================================
// Initialize Player
// ============================================================================

async function initPlayer(Ym2149PlayerClass: Ym2149PlayerConstructor): Promise<void> {
  console.log('initPlayer called');
  state.setYm2149Player(Ym2149PlayerClass);

  // Initialize DOM elements cache
  initElements();
  console.log('DOM elements initialized');

  // Initialize default 3-channel UI
  setupChannelUI(3);
  setupAllCanvases();
  setupEventHandlers();

  // Hide sidebar by default on mobile
  if (window.innerWidth < 768) {
    state.setSidebarVisible(true);
    toggleSidebar();
  }

  // Load user data from localStorage
  loadFavorites();
  loadPlayStats();
  loadFingerprints();
  loadPinnedAuthors();
  loadOwnFilesMeta();
  loadAuthorSortMode();

  // Update sort mode button text based on loaded state
  const sortModeBtn = document.getElementById('sortModeBtn');
  if (sortModeBtn) {
    sortModeBtn.textContent = state.authorSortMode === 'alpha' ? 'A-Z' : '#';
  }

  // Load catalog (with caching and gzip support)
  console.log('Loading catalog...');
  try {
    const catalog = await loadCatalogCached();
    console.log('Catalog loaded, tracks:', catalog.tracks?.length);
    state.setCatalog(catalog);

    // Sort catalog: collection, author, title
    const collectionOrder: CollectionId[] = ['sndh', 'ym', 'ay', 'arkos'];
    catalog.tracks.sort((a: Track, b: Track) => {
      const colCmp = collectionOrder.indexOf(a.collection as CollectionId) - collectionOrder.indexOf(b.collection as CollectionId);
      if (colCmp !== 0) return colCmp;
      const authorCmp = (a.author || '').localeCompare(b.author || '');
      if (authorCmp !== 0) return authorCmp;
      return (a.title || '').localeCompare(b.title || '');
    });

    console.log('Rendering collections...');
    renderCollections(getCollectionCallbacks());
    console.log('Filtering and grouping tracks...');
    filterAndGroupTracks();
    console.log('Setting up track list interaction...');

    // Setup track list interaction handlers after initial render
    setupTrackListInteractionDeferred();
    console.log('Initialization complete');

    // Check URL params
    handleUrlParams(catalog);
  } catch (err) {
    console.error('Failed to load catalog:', err);
  }
}

// ============================================================================
// Deferred Setup (needs track list to be rendered)
// ============================================================================

function setupTrackListInteractionDeferred(): void {
  // Observer to attach handlers when track list updates
  const observer = new MutationObserver(() => {
    setupTrackListInteraction();
  });
  const trackListInner = document.getElementById('trackListInner');
  if (trackListInner) {
    observer.observe(trackListInner, { childList: true });
    setupTrackListInteraction(); // Initial setup
  }
}

// ============================================================================
// URL Parameter Handling
// ============================================================================

function handleUrlParams(catalog: Catalog): void {
  const params = new URLSearchParams(window.location.search);
  const file = params.get('file') || params.get('track');
  const subsong = parseInt(params.get('sub') ?? '0') || 0;
  const startTime = parseInt(params.get('t') ?? '0') || 0;

  if (file) {
    const decodedFile = decodeURIComponent(file);

    // Try multiple matching strategies
    const track = catalog.tracks.find(
      (t: Track) =>
        t.path === file ||
        t.path === decodedFile ||
        t.path.endsWith(file) ||
        t.path.endsWith(decodedFile) ||
        file.endsWith(t.path) ||
        decodedFile.endsWith(t.path),
    );

    if (track) {
      // Switch to 'all' collection
      state.setCurrentCollection('all');
      renderCollections(getCollectionCallbacks());
      filterAndGroupTracks();

      const idx = state.filteredTracks.findIndex((t: Track) => t.path === track.path);
      if (idx >= 0) {
        showToast(`Loading: ${track.title || track.path}`);
        playTrack(idx).then(() => {
          // Switch to subsong if specified
          if (subsong > 1 && state.wasmPlayer && state.wasmPlayer.setSubsong) {
            state.wasmPlayer.setSubsong(subsong);
            updateSubsongSelector();
            const subsongSelect = document.getElementById('subsongSelect') as HTMLSelectElement | null;
            if (subsongSelect) subsongSelect.value = String(subsong);
          }
          // Seek to time position if specified
          if (startTime > 0 && state.wasmPlayer) {
            const duration = state.wasmPlayer.metadata.duration_seconds || 0;
            if (duration > 0) {
              state.wasmPlayer.seek_to_percentage(startTime / duration);
              updateProgressUI();
            }
          }
        });
      } else {
        showToast('Track not found');
      }
    } else {
      showToast('Shared track not found');
    }
  }
}

// ============================================================================
// WASM Module Import and Init
// ============================================================================

interface WasmModule {
  default: () => Promise<void>;
  Ym2149Player: Ym2149PlayerConstructor;
}

import('../pkg/ym2149_wasm.js')
  .then((module: unknown) => {
    const wasmModule = module as WasmModule;
    return wasmModule.default().then(() => {
      console.log('WASM module initialized');
      initPlayer(wasmModule.Ym2149Player);
    });
  })
  .catch((err) => {
    console.error('Failed to load WASM module:', err);
  });
