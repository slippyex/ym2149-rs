// ============================================================================
// Storage - localStorage and IndexedDB functions
// ============================================================================

import {
  STORAGE_KEY_FAVORITES,
  STORAGE_KEY_STATS,
  STORAGE_KEY_PINNED,
  STORAGE_KEY_OWN_FILES,
  STORAGE_KEY_AUTHOR_SORT,
  OWN_FILES_DB_NAME,
  OWN_FILES_DB_VERSION,
  OWN_FILES_STORE,
} from './config.ts';

import * as state from './state.ts';
import type { Track, PlayStatsMap, OwnFileMetadata } from './types/index.ts';
import type { AuthorSortMode } from './state.ts';

// ============================================================================
// Favorites (localStorage)
// ============================================================================

export function loadFavorites(): void {
  try {
    const stored = localStorage.getItem(STORAGE_KEY_FAVORITES);
    if (stored) state.setFavorites(new Set(JSON.parse(stored) as string[]));
  } catch (e) {
    console.warn('Failed to load favorites:', e);
  }
}

export function saveFavorites(): void {
  try {
    localStorage.setItem(
      STORAGE_KEY_FAVORITES,
      JSON.stringify([...state.favorites]),
    );
  } catch (e) {
    console.warn('Failed to save favorites:', e);
  }
}

export function isFavorite(path: string): boolean {
  return state.favorites.has(path);
}

export function toggleFavoriteStorage(path: string): boolean {
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

export function loadPinnedAuthors(): void {
  try {
    const stored = localStorage.getItem(STORAGE_KEY_PINNED);
    if (stored) state.setPinnedAuthors(new Set(JSON.parse(stored) as string[]));
  } catch (e) {
    console.warn('Failed to load pinned authors:', e);
  }
}

export function savePinnedAuthors(): void {
  try {
    localStorage.setItem(
      STORAGE_KEY_PINNED,
      JSON.stringify([...state.pinnedAuthors]),
    );
  } catch (e) {
    console.warn('Failed to save pinned authors:', e);
  }
}

export function getAuthorKey(collection: string, author: string): string {
  return `${collection}:${author}`;
}

export function isAuthorPinned(collection: string, author: string): boolean {
  return state.pinnedAuthors.has(getAuthorKey(collection, author));
}

export function togglePinAuthorStorage(collection: string, author: string): boolean {
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
// Author Sort Mode (localStorage)
// ============================================================================

export function loadAuthorSortMode(): void {
  try {
    const stored = localStorage.getItem(STORAGE_KEY_AUTHOR_SORT);
    if (stored === 'alpha' || stored === 'count') {
      state.setAuthorSortMode(stored as AuthorSortMode);
    }
  } catch (e) {
    console.warn('Failed to load author sort mode:', e);
  }
}

export function saveAuthorSortMode(): void {
  try {
    localStorage.setItem(STORAGE_KEY_AUTHOR_SORT, state.authorSortMode);
  } catch (e) {
    console.warn('Failed to save author sort mode:', e);
  }
}

// ============================================================================
// Play Stats (localStorage)
// ============================================================================

export function loadPlayStats(): void {
  try {
    const stored = localStorage.getItem(STORAGE_KEY_STATS);
    if (stored) state.setPlayStats(JSON.parse(stored) as PlayStatsMap);
  } catch (e) {
    console.warn('Failed to load stats:', e);
  }
}

export function savePlayStats(): void {
  try {
    localStorage.setItem(
      STORAGE_KEY_STATS,
      JSON.stringify(state.playStats),
    );
  } catch (e) {
    console.warn('Failed to save stats:', e);
  }
}

export function recordPlay(path: string): void {
  if (!state.playStats[path]) {
    state.playStats[path] = { playCount: 0, lastPlayed: 0 };
  }
  const stats = state.playStats[path];
  if (stats) {
    stats.playCount++;
    stats.lastPlayed = Date.now();
  }
  savePlayStats();
}

export function getRecentlyPlayedCount(): number {
  return Object.keys(state.playStats).filter(
    (path) => (state.playStats[path]?.lastPlayed ?? 0) > 0,
  ).length;
}

export function clearPlayStats(): void {
  state.setPlayStats({});
  savePlayStats();
}

// ============================================================================
// Own Files (IndexedDB + localStorage metadata)
// ============================================================================

interface OwnFileRecord {
  id: string;
  data: Uint8Array;
  name: string;
}

async function openOwnFilesDB(): Promise<IDBDatabase> {
  return new Promise((resolve, reject) => {
    const request = indexedDB.open(OWN_FILES_DB_NAME, OWN_FILES_DB_VERSION);
    request.onerror = () => reject(request.error);
    request.onsuccess = () => resolve(request.result);
    request.onupgradeneeded = (event) => {
      const db = (event.target as IDBOpenDBRequest).result;
      if (!db.objectStoreNames.contains(OWN_FILES_STORE)) {
        db.createObjectStore(OWN_FILES_STORE, { keyPath: 'id' });
      }
    };
  });
}

export function loadOwnFilesMeta(): void {
  try {
    const stored = localStorage.getItem(STORAGE_KEY_OWN_FILES);
    if (stored) state.setOwnFiles(JSON.parse(stored) as OwnFileMetadata[]);
  } catch (e) {
    console.warn('Failed to load own files:', e);
  }
}

export function saveOwnFilesMeta(): void {
  try {
    localStorage.setItem(STORAGE_KEY_OWN_FILES, JSON.stringify(state.ownFiles));
  } catch (e) {
    console.warn('Failed to save own files:', e);
  }
}

export async function saveOwnFile(file: File): Promise<string> {
  const id = `own_${Date.now()}_${Math.random().toString(36).substr(2, 9)}`;
  const arrayBuffer = await file.arrayBuffer();
  const data = new Uint8Array(arrayBuffer);

  const db = await openOwnFilesDB();
  const tx = db.transaction(OWN_FILES_STORE, 'readwrite');
  const store = tx.objectStore(OWN_FILES_STORE);
  await new Promise<void>((resolve, reject) => {
    const request = store.put({ id, data, name: file.name });
    request.onsuccess = () => resolve();
    request.onerror = () => reject(request.error);
  });
  db.close();

  const meta: OwnFileMetadata = {
    path: id,
    name: file.name,
    title: file.name.replace(/\.[^.]+$/, ''),
    author: 'Local',
    format: (file.name.split('.').pop()?.toUpperCase() || '?') as OwnFileMetadata['format'],
    frames: 0,
    duration: 0,
    channels: 3,
    addedAt: Date.now(),
  };
  state.ownFiles.push(meta);
  saveOwnFilesMeta();
  return id;
}

export async function loadOwnFileData(id: string): Promise<Uint8Array | null> {
  const db = await openOwnFilesDB();
  const tx = db.transaction(OWN_FILES_STORE, 'readonly');
  const store = tx.objectStore(OWN_FILES_STORE);
  return new Promise((resolve, reject) => {
    const request = store.get(id);
    request.onsuccess = () => {
      db.close();
      const result = request.result as OwnFileRecord | undefined;
      resolve(result?.data ?? null);
    };
    request.onerror = () => {
      db.close();
      reject(request.error);
    };
  });
}

export async function deleteOwnFile(id: string): Promise<void> {
  const db = await openOwnFilesDB();
  const tx = db.transaction(OWN_FILES_STORE, 'readwrite');
  const store = tx.objectStore(OWN_FILES_STORE);
  await new Promise<void>((resolve, reject) => {
    const request = store.delete(id);
    request.onsuccess = () => resolve();
    request.onerror = () => reject(request.error);
  });
  db.close();

  state.setOwnFiles(state.ownFiles.filter(f => f.path !== id));
  saveOwnFilesMeta();
}

export async function clearOwnFilesStorage(): Promise<void> {
  const db = await openOwnFilesDB();
  const tx = db.transaction(OWN_FILES_STORE, 'readwrite');
  const store = tx.objectStore(OWN_FILES_STORE);
  await new Promise<void>((resolve, reject) => {
    const request = store.clear();
    request.onsuccess = () => resolve();
    request.onerror = () => reject(request.error);
  });
  db.close();

  state.setOwnFiles([]);
  saveOwnFilesMeta();
}

export function getOwnFileTracks(): Track[] {
  return state.ownFiles.map(f => ({
    path: f.path,
    title: f.name.replace(/\.[^.]+$/, ''),
    author: 'Local',
    collection: 'own' as const,
    format: (f.name.split('.').pop()?.toUpperCase() || '?') as Track['format'],
    frames: f.frames,
    duration: f.duration,
    channels: f.channels,
    isOwnFile: true,
  }));
}

export function getValidFavoritesCount(): number {
  if (!state.catalog) return state.favorites.size;
  const catalogPaths = new Set(state.catalog.tracks.map((t) => t.path));
  const ownPaths = new Set(state.ownFiles.map((f) => f.path));
  let count = 0;
  for (const path of state.favorites) {
    if (catalogPaths.has(path) || ownPaths.has(path)) count++;
  }
  return count;
}
