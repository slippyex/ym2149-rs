// ============================================================================
// UI Track List - Track filtering, grouping, and rendering
// ============================================================================

import { ROW_HEIGHT, BUFFER_ROWS, RECENT_DISPLAY_LIMIT } from '../config.js';
import * as state from '../state.js';
import { elements } from './elements.js';
import { escapeHtml, highlightMatches, fuzzyMatch } from '../search.js';
import {
    isFavorite,
    isAuthorPinned,
    getAuthorKey,
    getOwnFileTracks,
} from '../storage.js';

// ============================================================================
// Author Collapse/Expand
// ============================================================================

export function isAuthorCollapsed(collection, author) {
    return state.collapsedAuthors.has(getAuthorKey(collection, author));
}

export function toggleAuthor(collection, author) {
    const key = getAuthorKey(collection, author);
    if (state.collapsedAuthors.has(key)) {
        state.collapsedAuthors.delete(key);
        state.setAllCollapsed(false);
        updateCollapseButtonText();
    } else {
        state.collapsedAuthors.add(key);
    }
    rebuildGroupedTracks();
    renderTrackList();
}

export function updateCollapseButtonText() {
    const btn = document.getElementById("toggleCollapseBtn");
    if (btn) {
        btn.textContent = state.allCollapsed ? "Expand All" : "Collapse All";
    }
}

export function expandAllAuthors() {
    state.collapsedAuthors.clear();
    state.setAllCollapsed(false);
    rebuildGroupedTracks();
    renderTrackList();
}

export function collapseAllAuthors() {
    if (!state.catalog) return;
    for (const track of state.catalog.tracks) {
        state.collapsedAuthors.add(
            getAuthorKey(track.collection, track.author || "Unknown"),
        );
    }
    state.setAllCollapsed(true);
    rebuildGroupedTracks();
    renderTrackList();
}

// ============================================================================
// Track Filtering & Grouping
// ============================================================================

export function filterAndGroupTracks() {
    if (!state.catalog) return;

    let tracks = state.catalog.tracks;

    // Filter by collection
    if (state.currentCollection === "favorites") {
        tracks = tracks.filter((t) => state.favorites.has(t.path));
        // Also include favorited own files
        const favOwnTracks = getOwnFileTracks().filter((t) => state.favorites.has(t.path));
        tracks = [...tracks, ...favOwnTracks];
    } else if (state.currentCollection === "recent") {
        // Get recently played tracks, sorted by lastPlayed descending
        const recentPaths = Object.entries(state.playStats)
            .filter(([, stats]) => stats.lastPlayed > 0)
            .sort((a, b) => b[1].lastPlayed - a[1].lastPlayed)
            .slice(0, RECENT_DISPLAY_LIMIT)
            .map(([path]) => path);
        // Include both catalog tracks and own files
        const ownTracks = getOwnFileTracks();
        const allTracks = [...tracks, ...ownTracks];
        tracks = recentPaths
            .map((path) => allTracks.find((t) => t.path === path))
            .filter(Boolean);
    } else if (state.currentCollection === "own") {
        tracks = getOwnFileTracks();
    } else if (state.currentCollection !== "all") {
        tracks = tracks.filter((t) => t.collection === state.currentCollection);
    }

    // Filter by search
    if (state.searchQuery) {
        const results = [];
        for (const track of tracks) {
            const titleMatch = fuzzyMatch(state.searchQuery, track.title || "");
            const authorMatch = fuzzyMatch(state.searchQuery, track.author || "");
            if (titleMatch.match || authorMatch.match) {
                results.push({
                    ...track,
                    searchScore: Math.max(titleMatch.score, authorMatch.score),
                    titleIndices: titleMatch.indices,
                    authorIndices: authorMatch.indices,
                });
            }
        }
        results.sort((a, b) => b.searchScore - a.searchScore);
        tracks = results;
    }

    state.setFilteredTracks(tracks);

    // Initialize all authors as collapsed on first load
    if (state.allCollapsed && state.collapsedAuthors.size === 0) {
        for (const track of state.filteredTracks) {
            state.collapsedAuthors.add(
                getAuthorKey(track.collection, track.author || "Unknown"),
            );
        }
    }

    rebuildGroupedTracks();

    elements.searchCount.textContent = `${state.filteredTracks.length}`;
    elements.searchCount.classList.toggle("hidden", !state.searchQuery);
    elements.filteredCount.textContent = state.searchQuery
        ? ""
        : `${state.filteredTracks.length} shown`;

    renderTrackList();
}

export function rebuildGroupedTracks() {
    const grouped = [];

    if (
        state.searchQuery ||
        state.currentCollection === "favorites" ||
        state.currentCollection === "recent"
    ) {
        // Flat list for search, favorites, and recent
        for (const track of state.filteredTracks) {
            grouped.push({
                type: "track",
                track,
                index: state.filteredTracks.indexOf(track),
            });
        }
    } else {
        // Group by collection -> author
        const collections =
            state.currentCollection === "all"
                ? ["sndh", "ym", "ay", "arkos"]
                : [state.currentCollection];

        for (const colId of collections) {
            const colTracks = state.filteredTracks.filter((t) => t.collection === colId);
            if (colTracks.length === 0) continue;

            // Group by author
            const byAuthor = {};
            for (const track of colTracks) {
                const author = track.author || "Unknown";
                if (!byAuthor[author]) byAuthor[author] = [];
                byAuthor[author].push(track);
            }

            // Sort authors (pinned first, then alphabetically)
            const authors = Object.keys(byAuthor).sort((a, b) => {
                const aPinned = isAuthorPinned(colId, a);
                const bPinned = isAuthorPinned(colId, b);
                if (aPinned && !bPinned) return -1;
                if (!aPinned && bPinned) return 1;
                return a.localeCompare(b);
            });

            // Check if we need a separator
            const hasPinned = authors.some((a) => isAuthorPinned(colId, a));
            const hasUnpinned = authors.some((a) => !isAuthorPinned(colId, a));
            const needsSeparator = hasPinned && hasUnpinned;
            let separatorAdded = false;

            for (const author of authors) {
                const isCollapsed = isAuthorCollapsed(colId, author);
                const isPinned = isAuthorPinned(colId, author);

                // Add separator before first unpinned author
                if (needsSeparator && !isPinned && !separatorAdded) {
                    grouped.push({ type: "separator", collection: colId });
                    separatorAdded = true;
                }

                grouped.push({
                    type: "author",
                    author,
                    collection: colId,
                    count: byAuthor[author].length,
                    collapsed: isCollapsed,
                    pinned: isPinned,
                });

                // Only add tracks if not collapsed
                if (!isCollapsed) {
                    byAuthor[author].sort((a, b) =>
                        (a.title || "").localeCompare(b.title || ""),
                    );
                    for (const track of byAuthor[author]) {
                        grouped.push({
                            type: "track",
                            track,
                            index: state.filteredTracks.indexOf(track),
                        });
                    }
                }
            }
        }
    }

    state.setGroupedTracks(grouped);
}

// ============================================================================
// Virtual Scroll Rendering
// ============================================================================

export function renderTrackList() {
    const totalHeight = state.groupedTracks.length * ROW_HEIGHT;
    elements.trackListInner.style.height = `${totalHeight}px`;
    state.setVisibleStart(-1);
    state.setVisibleEnd(-1);
    updateVisibleRows();
    updateStickyAuthorHeader();
}

export function updateVisibleRows(force = false) {
    const scrollTop = elements.trackList.scrollTop;
    const containerHeight = elements.trackList.clientHeight;

    const start = Math.max(0, Math.floor(scrollTop / ROW_HEIGHT) - BUFFER_ROWS);
    const end = Math.min(
        state.groupedTracks.length,
        Math.ceil((scrollTop + containerHeight) / ROW_HEIGHT) + BUFFER_ROWS,
    );

    if (!force && start === state.visibleStart && end === state.visibleEnd) return;
    state.setVisibleStart(start);
    state.setVisibleEnd(end);

    let html = "";
    for (let i = start; i < end; i++) {
        const item = state.groupedTracks[i];

        if (item.type === "separator") {
            html += `
                <div class="px-2 flex items-center"
                     style="position: absolute; top: ${i * ROW_HEIGHT}px; left: 0; right: 0; height: ${ROW_HEIGHT}px;">
                    <div class="flex-1 h-px bg-gradient-to-r from-chip-cyan/50 via-chip-purple/30 to-transparent"></div>
                </div>
            `;
        } else if (item.type === "author") {
            const colLabel = item.collection.toUpperCase();
            const chevronClass = item.collapsed ? "collapsed" : "";
            const pinClass = item.pinned ? "text-chip-cyan" : "text-gray-600 hover:text-chip-cyan";
            const pinFill = item.pinned ? "currentColor" : "none";
            html += `
                <div class="author-header px-2 py-1 text-xs font-medium text-gray-300"
                     style="position: absolute; top: ${i * ROW_HEIGHT}px; left: 0; right: 0; height: ${ROW_HEIGHT}px; display: flex; align-items: center;"
                     data-collection="${item.collection}" data-author="${escapeHtml(item.author)}">
                    <svg class="collapse-chevron w-3 h-3 mr-1 text-gray-500 ${chevronClass}" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 9l-7 7-7-7"/>
                    </svg>
                    <span class="text-chip-purple/60 mr-2">${colLabel}</span>
                    <span class="truncate flex-1">${escapeHtml(item.author)}</span>
                    <span class="text-gray-500 mr-2">${item.count}</span>
                    <button class="pin-btn ${pinClass} p-1 rounded hover:bg-gray-700/50 transition-colors" title="${item.pinned ? "Unpin author" : "Pin author"}">
                        <svg class="w-3 h-3" fill="${pinFill}" stroke="currentColor" viewBox="0 0 24 24">
                            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 5a2 2 0 012-2h10a2 2 0 012 2v16l-7-3.5L5 21V5z"/>
                        </svg>
                    </button>
                </div>
            `;
        } else {
            const track = item.track;
            const trackIdx = item.index;
            const isActive = trackIdx === state.currentTrackIndex;
            const isCurrentPlaying = isActive && state.isPlaying;
            const isFav = isFavorite(track.path);

            const titleHtml = track.titleIndices
                ? highlightMatches(track.title, track.titleIndices)
                : escapeHtml(track.title);

            const channelBadge =
                track.channels && track.channels > 3
                    ? `<span class="text-xs px-1 py-0.5 rounded bg-chip-purple/30 text-chip-purple ml-1">${track.channels}ch</span>`
                    : "";

            const heartClass = isFav ? "text-red-500" : "text-gray-600 hover:text-red-400";
            const heartFill = isFav ? "currentColor" : "none";

            html += `
                <div class="track-row flex items-center px-2 py-1 cursor-pointer ${isActive ? "active" : ""} ${isCurrentPlaying ? "playing" : ""}"
                     style="position: absolute; top: ${i * ROW_HEIGHT}px; left: 0; right: 0; height: ${ROW_HEIGHT}px;"
                     data-index="${trackIdx}" data-path="${escapeHtml(track.path)}">
                    <div class="w-5 text-center text-gray-500 text-xs shrink-0">${isCurrentPlaying ? "▶" : ""}</div>
                    <button class="fav-btn w-5 shrink-0 ${heartClass}" data-path="${escapeHtml(track.path)}">
                        <svg class="w-3.5 h-3.5" fill="${heartFill}" stroke="currentColor" viewBox="0 0 24 24">
                            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4.318 6.318a4.5 4.5 0 000 6.364L12 20.364l7.682-7.682a4.5 4.5 0 00-6.364-6.364L12 7.636l-1.318-1.318a4.5 4.5 0 00-6.364 0z"/>
                        </svg>
                    </button>
                    <div class="flex-1 min-w-0 px-1">
                        <div class="truncate text-sm">${titleHtml}</div>
                    </div>
                    <div class="text-right flex items-center justify-end gap-1">
                        <span class="text-xs px-1 py-0.5 rounded bg-gray-800/50 text-gray-500">${track.format}</span>${channelBadge}
                    </div>
                </div>
            `;
        }
    }
    elements.trackListInner.innerHTML = html;
}

// ============================================================================
// Attach Track List Event Handlers
// ============================================================================

export function attachTrackListHandlers(callbacks) {
    const { onTrackClick, onFavoriteClick, onPinClick, onAuthorClick } = callbacks;

    // Click handlers for tracks
    elements.trackListInner.querySelectorAll(".track-row").forEach((row) => {
        row.addEventListener("click", (e) => {
            if (e.target.closest(".fav-btn")) return;
            onTrackClick(parseInt(row.dataset.index));
        });
    });

    // Click handlers for favorite buttons
    elements.trackListInner.querySelectorAll(".fav-btn").forEach((btn) => {
        btn.addEventListener("click", (e) => {
            e.stopPropagation();
            onFavoriteClick(btn.dataset.path, e);
        });
    });

    // Click handlers for pin buttons
    elements.trackListInner.querySelectorAll(".pin-btn").forEach((btn) => {
        btn.addEventListener("click", (e) => {
            e.stopPropagation();
            const header = btn.closest(".author-header");
            if (header) {
                onPinClick(header.dataset.collection, header.dataset.author, e);
            }
        });
    });

    // Click handlers for author headers
    elements.trackListInner.querySelectorAll(".author-header").forEach((header) => {
        header.addEventListener("click", () => {
            onAuthorClick(header.dataset.collection, header.dataset.author);
        });
    });
}

// ============================================================================
// Sticky Author Header
// ============================================================================

export function updateStickyAuthorHeader() {
    if (!elements.stickyAuthorHeader) return;

    const scrollTop = elements.trackList.scrollTop;

    // Find the author for the first visible row
    const firstVisibleIndex = Math.floor(scrollTop / ROW_HEIGHT);

    // Search backwards from first visible to find the author
    let currentAuthor = null;
    let currentCollection = null;

    for (let i = firstVisibleIndex; i >= 0; i--) {
        const item = state.groupedTracks[i];
        if (item && item.type === "author") {
            currentAuthor = item.author;
            currentCollection = item.collection;
            break;
        }
    }

    // Show/hide sticky header
    if (currentAuthor && scrollTop > ROW_HEIGHT) {
        elements.stickyAuthorHeader.classList.remove("hidden");
        elements.stickyAuthorCollection.textContent = currentCollection?.toUpperCase() || "";
        elements.stickyAuthorName.textContent = currentAuthor;
    } else {
        elements.stickyAuthorHeader.classList.add("hidden");
    }
}

// ============================================================================
// Scroll to Current Track
// ============================================================================

export function scrollToCurrentTrack() {
    if (state.currentTrackIndex < 0) return;

    // Find the track in groupedTracks
    for (let i = 0; i < state.groupedTracks.length; i++) {
        const item = state.groupedTracks[i];
        if (item.type === "track" && item.index === state.currentTrackIndex) {
            const targetScrollTop = i * ROW_HEIGHT - elements.trackList.clientHeight / 2 + ROW_HEIGHT / 2;
            elements.trackList.scrollTo({
                top: Math.max(0, targetScrollTop),
                behavior: "smooth"
            });
            break;
        }
    }
}
