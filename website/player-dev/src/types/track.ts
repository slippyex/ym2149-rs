// ============================================================================
// Track Types - Track, Fingerprint, Collection, and GroupedItem types
// ============================================================================

/** Collection identifiers for music libraries */
export type CollectionId = 'sndh' | 'ym' | 'ay' | 'arkos' | 'own' | 'all' | 'favorites' | 'recent';

/** Format identifiers for audio file formats */
export type FormatId = 'SNDH' | 'YM' | 'AY' | 'AKS' | 'VTXLIB' | '';

/** Audio fingerprint for similarity matching */
export interface Fingerprint {
  /** Average amplitude */
  amp: number;
  /** Note density */
  density: number;
  /** Amplitude variance */
  variance?: number;
  /** Punchiness */
  punch?: number;
  /** Spectral brightness */
  brightness?: number;
  /** Energy distribution histogram (8 bins) */
  hist?: number[];
  /** Song structure sections (4 quarters) */
  sections?: number[];
  /** BPM-like tempo value */
  tempo?: number;
  /** Spectral centroid (0-1) */
  centroid?: number;
  /** Spectral flatness: tonal vs noise (0-1) */
  flatness?: number;
  /** Spectral bands: bass/low-mid/high-mid/treble (4 bins, 0-255) */
  bands?: number[];
  /** Pitch class histogram (12 bins, 0-255) */
  chroma?: number[];
  /** Rhythm regularity */
  rhythm_reg?: number;
  /** Rhythm strength */
  rhythm_str?: number;
  /** MFCCs - 13 coefficients */
  mfcc?: number[];
  /** MFCC Deltas */
  mfcc_d?: number[];
  /** MFCC Delta-Deltas */
  mfcc_dd?: number[];
  /** Chromagram - 96 values (12 pitch classes x 8 time segments) */
  chromagram?: number[];
}

/** Track metadata and properties */
export interface Track {
  /** File path (can be URL or IndexedDB key like "own_xxx") */
  path: string;
  /** Track title */
  title: string;
  /** Track author/composer */
  author: string;
  /** Collection this track belongs to */
  collection: CollectionId;
  /** Audio format */
  format: FormatId;
  /** Duration in frames */
  frames: number;
  /** Duration in seconds */
  duration: number;
  /** Number of audio channels */
  channels: number;
  /** Base64-encoded pre-rendered waveform data */
  w?: string;
  /** Audio fingerprint for similarity matching */
  fp?: Fingerprint;
  /** Whether this is a user-uploaded file */
  isOwnFile?: boolean;
  /** Search score (added during filtering) */
  searchScore?: number;
  /** Matched title character indices for highlighting */
  titleIndices?: number[];
  /** Matched author character indices for highlighting */
  authorIndices?: number[];
}

/** Catalog containing all tracks */
export interface Catalog {
  tracks: Track[];
  version?: string;
  generated?: string;
}

/** Author group header in grouped track list */
export interface AuthorGroupItem {
  type: 'author';
  author: string;
  collection: string;
  count: number;
  collapsed: boolean;
  pinned: boolean;
}

/** Single track item in grouped track list */
export interface TrackItem {
  type: 'track';
  track: Track;
  index: number;
}

/** Separator between pinned and unpinned authors */
export interface SeparatorItem {
  type: 'separator';
  collection: string;
}

/** Union type for all items in the grouped track list */
export type GroupedItem = AuthorGroupItem | TrackItem | SeparatorItem;

/** Play statistics for a track */
export interface PlayStats {
  playCount: number;
  lastPlayed: number;
}

/** All play statistics keyed by track path */
export type PlayStatsMap = Record<string, PlayStats>;

/** Own file metadata stored in localStorage */
export interface OwnFileMetadata {
  path: string;
  name: string;
  title: string;
  author: string;
  format: FormatId;
  frames: number;
  duration: number;
  channels: number;
  addedAt: number;
}
