// ============================================================================
// Catalog - Catalog fetch, cache, and decompression
// ============================================================================

import { CATALOG_DB_NAME, CATALOG_DB_VERSION, CATALOG_STORE } from './config.ts';
import type { Catalog } from './types/index.ts';

// ============================================================================
// IndexedDB for Catalog Caching
// ============================================================================

interface CachedCatalogData {
  catalog: Catalog;
  etag: string;
  timestamp: number;
}

async function openCatalogDB(): Promise<IDBDatabase> {
  return new Promise((resolve, reject) => {
    const request = indexedDB.open(CATALOG_DB_NAME, CATALOG_DB_VERSION);
    request.onerror = () => reject(request.error);
    request.onsuccess = () => resolve(request.result);
    request.onupgradeneeded = (e) => {
      const db = (e.target as IDBOpenDBRequest).result;
      if (!db.objectStoreNames.contains(CATALOG_STORE)) {
        db.createObjectStore(CATALOG_STORE);
      }
    };
  });
}

async function getCachedCatalog(): Promise<CachedCatalogData | null> {
  try {
    const db = await openCatalogDB();
    return new Promise((resolve) => {
      const tx = db.transaction(CATALOG_STORE, 'readonly');
      const store = tx.objectStore(CATALOG_STORE);
      const request = store.get('data');
      request.onsuccess = () => {
        db.close();
        resolve(request.result as CachedCatalogData | null);
      };
      request.onerror = () => {
        db.close();
        resolve(null);
      };
    });
  } catch {
    return null;
  }
}

async function setCachedCatalog(data: Catalog, etag: string): Promise<void> {
  try {
    const db = await openCatalogDB();
    const tx = db.transaction(CATALOG_STORE, 'readwrite');
    const store = tx.objectStore(CATALOG_STORE);
    store.put({ catalog: data, etag, timestamp: Date.now() }, 'data');
    db.close();
  } catch (e) {
    console.warn('Failed to cache catalog:', e);
  }
}

// ============================================================================
// Fetch and Decompress
// ============================================================================

interface FetchResult {
  data: Catalog;
  etag: string;
}

async function fetchAndDecompress(url: string): Promise<FetchResult> {
  const response = await fetch(url);
  if (!response.ok) throw new Error(`HTTP ${response.status}`);
  const etag =
    response.headers.get('ETag') ||
    response.headers.get('Last-Modified') ||
    '';

  // Use DecompressionStream if available (modern browsers)
  if (url.endsWith('.gz') && typeof DecompressionStream !== 'undefined') {
    const ds = new DecompressionStream('gzip');
    const decompressed = response.body!.pipeThrough(ds);
    const text = await new Response(decompressed).text();
    return { data: JSON.parse(text) as Catalog, etag };
  }

  return { data: (await response.json()) as Catalog, etag };
}

// ============================================================================
// Load Catalog (with caching)
// ============================================================================

export async function loadCatalogCached(): Promise<Catalog> {
  // Try cache first
  const cached = await getCachedCatalog();

  // Try gzip version first, then fallbacks
  const urls = [
    'catalog.json.gz',
    'catalog.min.json',
    'catalog.json',
  ];

  for (const url of urls) {
    try {
      // If cached and same ETag, use cache
      const headResp = await fetch(url, { method: 'HEAD' }).catch(() => null);
      const serverEtag =
        headResp?.headers.get('ETag') ||
        headResp?.headers.get('Last-Modified') ||
        '';

      if (cached && serverEtag && cached.etag === serverEtag) {
        console.log('Using cached catalog');
        return cached.catalog;
      }

      // Fetch fresh
      console.log(`Loading catalog from ${url}...`);
      const { data, etag } = await fetchAndDecompress(url);
      await setCachedCatalog(data, etag || String(Date.now()));
      console.log('Catalog loaded and cached');
      return data;
    } catch (e) {
      console.warn(`Failed to load ${url}:`, (e as Error).message);
      continue;
    }
  }

  // Last resort: use cache even if potentially stale
  if (cached) {
    console.log('Using stale cache as fallback');
    return cached.catalog;
  }

  throw new Error('Failed to load catalog');
}
