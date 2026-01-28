// ============================================================================
// State Types - Application state interfaces
// ============================================================================

import type { Track, Catalog, GroupedItem, PlayStatsMap, OwnFileMetadata, CollectionId, Fingerprint } from './track.ts';
import type { Ym2149Player, Ym2149PlayerConstructor } from './player.ts';

/** Note history entry for tracker-style display */
export interface NoteHistoryEntry {
  note: string;
  amp: number;
  noise: boolean;
  env: boolean;
  envShape: string;
  envName: string;
  y: number;
}

/** Complete application state */
export interface AppState {
  // WASM and catalog
  Ym2149Player: Ym2149PlayerConstructor | null;
  catalog: Catalog | null;

  // Track state
  filteredTracks: Track[];
  groupedTracks: GroupedItem[];
  currentTrackIndex: number;
  loadedFileData: Uint8Array | null;
  loadedFileName: string | null;

  // Player state
  wasmPlayer: Ym2149Player | null;
  audioContext: AudioContext | null;
  scriptProcessor: ScriptProcessorNode | null;
  isPlaying: boolean;
  animationId: number | null;

  // Playback options
  shuffleEnabled: boolean;
  autoPlayEnabled: boolean;
  playbackSpeed: number;

  // A-B Loop
  loopA: number | null;
  loopB: number | null;

  // Waveform
  waveformOverviewData: Float32Array | null;

  // Collection/filter state
  currentCollection: CollectionId;
  searchQuery: string;
  visibleStart: number;
  visibleEnd: number;
  collapsedAuthors: Set<string>;
  allCollapsed: boolean;

  // User data (localStorage)
  favorites: Set<string>;
  ownFiles: OwnFileMetadata[];
  playStats: PlayStatsMap;
  audioFingerprints: Record<string, Fingerprint>;
  pinnedAuthors: Set<string>;

  // Visualization arrays
  channelWaveforms: Float32Array[];
  monoWaveform: Float32Array;
  channelPhases: number[];
  channelSpectrums: Float32Array[];
  spectrumTargets: Float32Array[];
  combinedSpectrum: Float32Array;

  // Per-channel sample buffers for real audio visualization
  channelSampleBuffers: Float32Array[];
  channelSampleWritePos: number;
  audioSampleBuffer: Float32Array;
  audioSampleWritePos: number;

  // Amplitude history for SID visualization
  amplitudeHistory: number[][];
  sidModeDetected: boolean[];

  // Peak hold for spectrum analyzer
  channelSpectrumPeaks: Float32Array[];
  combinedSpectrumPeaks: Float32Array | null;

  // Note history for tracker-style display
  noteHistories: NoteHistoryEntry[][];
  noteScrollOffset: number;

  // Current format for channel naming
  currentFormat: string;

  // Current channel count
  currentChannelCount: number;

  // Speed resampling
  speedResampleBuffer: Float32Array;
  speedResamplePos: number;

  // Audio unlocked flag (for iOS)
  audioUnlocked: boolean;

  // Media stream (for iOS/mobile)
  mediaStreamDest: MediaStreamAudioDestinationNode | null;
  audioElement: HTMLAudioElement | null;

  // Scrubbing state
  isScrubbing: boolean;

  // Toast timeout
  toastTimeout: ReturnType<typeof setTimeout> | null;

  // Sidebar state
  sidebarVisible: boolean;

  // Waveform DB reference
  waveformDb: IDBDatabase | null;
}

/** State setter function type */
export type StateSetter<T> = (value: T) => void;

/** Track list event callbacks */
export interface TrackListCallbacks {
  onTrackClick: (index: number) => void;
  onFavoriteClick: (path: string, event: MouseEvent) => void;
  onPinClick: (collection: string, author: string, event: MouseEvent) => void;
  onAuthorClick: (collection: string, author: string) => void;
}

/** Similar track click handler */
export type SimilarTrackClickHandler = (path: string, index: number) => void;
