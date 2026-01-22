// ============================================================================
// Storage - localStorage and IndexedDB functions
// ============================================================================

import {
    STORAGE_KEY_FAVORITES,
    STORAGE_KEY_STATS,
    STORAGE_KEY_PINNED,
    STORAGE_KEY_OWN_FILES,
    OWN_FILES_DB_NAME,
    OWN_FILES_DB_VERSION,
    OWN_FILES_STORE,
} from './config.js';

import * as state from './state.js';

// ============================================================================
// Favorites (localStorage)
// ============================================================================

export function loadFavorites() {
    try {
        const stored = localStorage.getItem(STORAGE_KEY_FAVORITES);
        if (stored) state.setFavorites(new Set(JSON.parse(stored)));
    } catch (e) {
        console.warn("Failed to load favorites:", e);
    }
}

export function saveFavorites() {
    try {
        localStorage.setItem(
            STORAGE_KEY_FAVORITES,
            JSON.stringify([...state.favorites]),
        );
    } catch (e) {
        console.warn("Failed to save favorites:", e);
    }
}

export function isFavorite(path) {
    return state.favorites.has(path);
}

export function toggleFavoriteStorage(path) {
    if (state.favorites.has(path)) {
        state.favorites.delete(path);
    } else {
        state.favorites.add(path);
    }
    saveFavorites();
    return state.favorites.has(path);
}

// ============================================================================
// Pinned Authors (localStorage)
// ============================================================================

export function loadPinnedAuthors() {
    try {
        const stored = localStorage.getItem(STORAGE_KEY_PINNED);
        if (stored) state.setPinnedAuthors(new Set(JSON.parse(stored)));
    } catch (e) {
        console.warn("Failed to load pinned authors:", e);
    }
}

export function savePinnedAuthors() {
    try {
        localStorage.setItem(
            STORAGE_KEY_PINNED,
            JSON.stringify([...state.pinnedAuthors]),
        );
    } catch (e) {
        console.warn("Failed to save pinned authors:", e);
    }
}

export function getAuthorKey(collection, author) {
    return `${collection}:${author}`;
}

export function isAuthorPinned(collection, author) {
    return state.pinnedAuthors.has(getAuthorKey(collection, author));
}

export function togglePinAuthorStorage(collection, author) {
    const key = getAuthorKey(collection, author);
    if (state.pinnedAuthors.has(key)) {
        state.pinnedAuthors.delete(key);
    } else {
        state.pinnedAuthors.add(key);
    }
    savePinnedAuthors();
    return state.pinnedAuthors.has(key);
}

// ============================================================================
// Play Stats (localStorage)
// ============================================================================

export function loadPlayStats() {
    try {
        const stored = localStorage.getItem(STORAGE_KEY_STATS);
        if (stored) state.setPlayStats(JSON.parse(stored));
    } catch (e) {
        console.warn("Failed to load stats:", e);
    }
}

export function savePlayStats() {
    try {
        localStorage.setItem(
            STORAGE_KEY_STATS,
            JSON.stringify(state.playStats),
        );
    } catch (e) {
        console.warn("Failed to save stats:", e);
    }
}

export function recordPlay(path) {
    if (!state.playStats[path]) {
        state.playStats[path] = { playCount: 0, lastPlayed: 0 };
    }
    state.playStats[path].playCount++;
    state.playStats[path].lastPlayed = Date.now();
    savePlayStats();
}

export function getRecentlyPlayedCount() {
    return Object.keys(state.playStats).filter(
        (path) => state.playStats[path].lastPlayed > 0,
    ).length;
}

export function clearPlayStats() {
    state.setPlayStats({});
    savePlayStats();
}

// ============================================================================
// Own Files (IndexedDB + localStorage metadata)
// ============================================================================

async function openOwnFilesDB() {
    return new Promise((resolve, reject) => {
        const request = indexedDB.open(OWN_FILES_DB_NAME, OWN_FILES_DB_VERSION);
        request.onerror = () => reject(request.error);
        request.onsuccess = () => resolve(request.result);
        request.onupgradeneeded = (event) => {
            const db = event.target.result;
            if (!db.objectStoreNames.contains(OWN_FILES_STORE)) {
                db.createObjectStore(OWN_FILES_STORE, { keyPath: "id" });
            }
        };
    });
}

export function loadOwnFilesMeta() {
    try {
        const stored = localStorage.getItem(STORAGE_KEY_OWN_FILES);
        if (stored) state.setOwnFiles(JSON.parse(stored));
    } catch (e) {
        console.warn("Failed to load own files:", e);
    }
}

export function saveOwnFilesMeta() {
    try {
        localStorage.setItem(STORAGE_KEY_OWN_FILES, JSON.stringify(state.ownFiles));
    } catch (e) {
        console.warn("Failed to save own files:", e);
    }
}

export async function saveOwnFile(file) {
    const id = `own_${Date.now()}_${Math.random().toString(36).substr(2, 9)}`;
    const arrayBuffer = await file.arrayBuffer();
    const data = new Uint8Array(arrayBuffer);

    const db = await openOwnFilesDB();
    const tx = db.transaction(OWN_FILES_STORE, "readwrite");
    const store = tx.objectStore(OWN_FILES_STORE);
    await new Promise((resolve, reject) => {
        const request = store.put({ id, data, name: file.name });
        request.onsuccess = resolve;
        request.onerror = () => reject(request.error);
    });
    db.close();

    state.ownFiles.push({ id, name: file.name, size: file.size, addedAt: Date.now() });
    saveOwnFilesMeta();
    return id;
}

export async function loadOwnFileData(id) {
    const db = await openOwnFilesDB();
    const tx = db.transaction(OWN_FILES_STORE, "readonly");
    const store = tx.objectStore(OWN_FILES_STORE);
    return new Promise((resolve, reject) => {
        const request = store.get(id);
        request.onsuccess = () => {
            db.close();
            resolve(request.result?.data || null);
        };
        request.onerror = () => {
            db.close();
            reject(request.error);
        };
    });
}

export async function deleteOwnFile(id) {
    const db = await openOwnFilesDB();
    const tx = db.transaction(OWN_FILES_STORE, "readwrite");
    const store = tx.objectStore(OWN_FILES_STORE);
    await new Promise((resolve, reject) => {
        const request = store.delete(id);
        request.onsuccess = resolve;
        request.onerror = () => reject(request.error);
    });
    db.close();

    state.setOwnFiles(state.ownFiles.filter(f => f.id !== id));
    saveOwnFilesMeta();
}

export async function clearOwnFilesStorage() {
    const db = await openOwnFilesDB();
    const tx = db.transaction(OWN_FILES_STORE, "readwrite");
    const store = tx.objectStore(OWN_FILES_STORE);
    await new Promise((resolve, reject) => {
        const request = store.clear();
        request.onsuccess = resolve;
        request.onerror = () => reject(request.error);
    });
    db.close();

    state.setOwnFiles([]);
    saveOwnFilesMeta();
}

export function getOwnFileTracks() {
    return state.ownFiles.map(f => ({
        path: f.id,
        title: f.name.replace(/\.[^.]+$/, ""),
        author: "Local",
        collection: "own",
        format: f.name.split(".").pop()?.toUpperCase() || "?",
        isOwnFile: true
    }));
}

export function getValidFavoritesCount() {
    if (!state.catalog) return state.favorites.size;
    const catalogPaths = new Set(state.catalog.tracks.map((t) => t.path));
    const ownPaths = new Set(state.ownFiles.map((f) => f.id));
    let count = 0;
    for (const path of state.favorites) {
        if (catalogPaths.has(path) || ownPaths.has(path)) count++;
    }
    return count;
}
