// ============================================================================
// Init - Bootstrap module that connects everything
// ============================================================================

import * as state from './state.js';
import { initElements } from './ui/elements.js';
import {
    loadFavorites,
    loadPlayStats,
    loadPinnedAuthors,
    loadOwnFilesMeta,
} from './storage.js';
import { loadFingerprints } from './fingerprint.js';
import { loadCatalogCached } from './catalog.js';
import {
    filterAndGroupTracks,
    renderTrackList,
    updateVisibleRows,
    attachTrackListHandlers,
} from './ui/trackList.js';
import {
    toggleSidebar,
    showToast,
    renderCollections,
    updateSubsongSelector,
    updateProgressUI,
} from './ui/player.js';
import { setupChannelUI, setupAllCanvases } from './visualization/core.js';
import { setupEventHandlers, getCollectionCallbacks, setupTrackListInteraction } from './events.js';
import { playTrack } from './audio/playback.js';

// ============================================================================
// Initialize Player
// ============================================================================

async function initPlayer(Ym2149PlayerClass) {
    state.setYm2149Player(Ym2149PlayerClass);

    // Initialize DOM elements cache
    initElements();

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

    // Load catalog (with caching and gzip support)
    try {
        const catalog = await loadCatalogCached();
        state.setCatalog(catalog);

        // Sort catalog: collection, author, title
        catalog.tracks.sort((a, b) => {
            const colOrder = ["sndh", "ym", "ay", "arkos"];
            const colCmp = colOrder.indexOf(a.collection) - colOrder.indexOf(b.collection);
            if (colCmp !== 0) return colCmp;
            const authorCmp = (a.author || "").localeCompare(b.author || "");
            if (authorCmp !== 0) return authorCmp;
            return (a.title || "").localeCompare(b.title || "");
        });

        renderCollections(getCollectionCallbacks());
        filterAndGroupTracks();

        // Setup track list interaction handlers after initial render
        setupTrackListInteractionDeferred();

        // Check URL params
        handleUrlParams(catalog);
    } catch (err) {
        console.error("Failed to load catalog:", err);
    }
}

// ============================================================================
// Deferred Setup (needs track list to be rendered)
// ============================================================================

function setupTrackListInteractionDeferred() {
    // Observer to attach handlers when track list updates
    const observer = new MutationObserver(() => {
        setupTrackListInteraction();
    });
    const trackListInner = document.getElementById("trackListInner");
    if (trackListInner) {
        observer.observe(trackListInner, { childList: true });
        setupTrackListInteraction(); // Initial setup
    }
}

// ============================================================================
// URL Parameter Handling
// ============================================================================

function handleUrlParams(catalog) {
    const params = new URLSearchParams(window.location.search);
    const file = params.get("file") || params.get("track");
    const subsong = parseInt(params.get("sub")) || 0;
    const startTime = parseInt(params.get("t")) || 0;

    if (file) {
        const decodedFile = decodeURIComponent(file);

        // Try multiple matching strategies
        const track = catalog.tracks.find(
            (t) =>
                t.path === file ||
                t.path === decodedFile ||
                t.path.endsWith(file) ||
                t.path.endsWith(decodedFile) ||
                file.endsWith(t.path) ||
                decodedFile.endsWith(t.path),
        );

        if (track) {
            // Switch to 'all' collection
            state.setCurrentCollection("all");
            renderCollections(getCollectionCallbacks());
            filterAndGroupTracks();

            const idx = state.filteredTracks.findIndex((t) => t.path === track.path);
            if (idx >= 0) {
                showToast(`Loading: ${track.title || track.path}`);
                playTrack(idx).then(() => {
                    // Switch to subsong if specified
                    if (subsong > 1 && state.wasmPlayer && state.wasmPlayer.setSubsong) {
                        state.wasmPlayer.setSubsong(subsong);
                        updateSubsongSelector();
                        document.getElementById("subsongSelect").value = subsong;
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
                showToast("Track not found");
            }
        } else {
            showToast("Shared track not found");
        }
    }
}

// ============================================================================
// WASM Module Import and Init
// ============================================================================

import("../pkg/ym2149_wasm.js").then((module) => {
    module.default().then(() => {
        initPlayer(module.Ym2149Player);
    });
});
