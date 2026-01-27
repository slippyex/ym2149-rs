// ============================================================================
// State - Global state variables for the YM2149 Web Player
// ============================================================================

import { WAVEFORM_SIZE, SPECTRUM_BINS, AUDIO_VIS_BUFFER_SIZE } from './config.ts';
import type {
  Track,
  Catalog,
  GroupedItem,
  PlayStatsMap,
  OwnFileMetadata,
  CollectionId,
  Fingerprint,
  Ym2149Player as Ym2149PlayerInstance,
  Ym2149PlayerConstructor,
  NoteHistoryEntry,
} from './types/index.ts';

// WASM and catalog
export let Ym2149Player: Ym2149PlayerConstructor | null = null;
export let catalog: Catalog | null = null;

// Track state
export let filteredTracks: Track[] = [];
export let groupedTracks: GroupedItem[] = [];
export let currentTrackIndex = -1;
export let loadedFileData: Uint8Array | null = null;
export let loadedFileName: string | null = null;

// Player state
export let wasmPlayer: Ym2149PlayerInstance | null = null;
export let audioContext: AudioContext | null = null;
export let scriptProcessor: ScriptProcessorNode | null = null;
export let isPlaying = false;
export let animationId: number | null = null;

// Playback options
export let shuffleEnabled = false;
export let autoPlayEnabled = false;
export let playbackSpeed = 1.0;

// A-B Loop
export let loopA: number | null = null;
export let loopB: number | null = null;

// Waveform
export let waveformOverviewData: Float32Array | null = null;

// Collection/filter state
export let currentCollection: CollectionId = 'all';
export let searchQuery = '';
export let visibleStart = 0;
export let visibleEnd = 0;
export let collapsedAuthors: Set<string> = new Set();
export let allCollapsed = true;

// User data (localStorage)
export let favorites: Set<string> = new Set();
export let ownFiles: OwnFileMetadata[] = [];
export let playStats: PlayStatsMap = {};
export let audioFingerprints: Record<string, Fingerprint> = {};
export let pinnedAuthors: Set<string> = new Set();

// Visualization arrays
export let channelWaveforms: Float32Array[] = [];
export let monoWaveform: Float32Array = new Float32Array(WAVEFORM_SIZE);
export let channelPhases: number[] = [];
export let channelSpectrums: Float32Array[] = [];
export let spectrumTargets: Float32Array[] = [];
export let combinedSpectrum: Float32Array = new Float32Array(SPECTRUM_BINS);

// Per-channel sample buffers for real audio visualization
export let channelSampleBuffers: Float32Array[] = [];
export let channelSampleWritePos = 0;
export let audioSampleBuffer: Float32Array = new Float32Array(AUDIO_VIS_BUFFER_SIZE);
export let audioSampleWritePos = 0;

// Amplitude history for SID visualization
export let amplitudeHistory: number[][] = [];
export let sidModeDetected: boolean[] = [];

// Peak hold for spectrum analyzer
export let channelSpectrumPeaks: Float32Array[] = [];
export let combinedSpectrumPeaks: Float32Array | null = null;

// Note history for tracker-style display
export let noteHistories: NoteHistoryEntry[][] = [];
export let noteScrollOffset = 0;

// Current format for channel naming
export let currentFormat = '';

// Current channel count
export let currentChannelCount = 3;

// Speed resampling
export let speedResampleBuffer: Float32Array = new Float32Array(0);
export let speedResamplePos = 0;

// Audio unlocked flag (for iOS)
export let audioUnlocked = false;

// Media stream (for iOS/mobile)
export let mediaStreamDest: MediaStreamAudioDestinationNode | null = null;
export let audioElement: HTMLAudioElement | null = null;

// Scrubbing state
export let isScrubbing = false;

// Toast timeout
export let toastTimeout: ReturnType<typeof setTimeout> | null = null;

// Sidebar state
export let sidebarVisible = true;

// Author sort mode
export type AuthorSortMode = 'alpha' | 'count';
export let authorSortMode: AuthorSortMode = 'alpha';

// Waveform DB reference
export let waveformDb: IDBDatabase | null = null;

// ============================================================================
// Setters - Use these to modify state from other modules
// ============================================================================

export function setYm2149Player(player: Ym2149PlayerConstructor | null): void {
  Ym2149Player = player;
}
export function setCatalog(data: Catalog | null): void {
  catalog = data;
}
export function setFilteredTracks(tracks: Track[]): void {
  filteredTracks = tracks;
}
export function setGroupedTracks(tracks: GroupedItem[]): void {
  groupedTracks = tracks;
}
export function setCurrentTrackIndex(index: number): void {
  currentTrackIndex = index;
}
export function setLoadedFileData(data: Uint8Array | null): void {
  loadedFileData = data;
}
export function setLoadedFileName(name: string | null): void {
  loadedFileName = name;
}
export function setWasmPlayer(player: Ym2149PlayerInstance | null): void {
  wasmPlayer = player;
}
export function setAudioContext(ctx: AudioContext | null): void {
  audioContext = ctx;
}
export function setScriptProcessor(proc: ScriptProcessorNode | null): void {
  scriptProcessor = proc;
}
export function setIsPlaying(playing: boolean): void {
  isPlaying = playing;
}
export function setAnimationId(id: number | null): void {
  animationId = id;
}
export function setShuffleEnabled(enabled: boolean): void {
  shuffleEnabled = enabled;
}
export function setAutoPlayEnabled(enabled: boolean): void {
  autoPlayEnabled = enabled;
}
export function setPlaybackSpeed(speed: number): void {
  playbackSpeed = speed;
}
export function setLoopA(val: number | null): void {
  loopA = val;
}
export function setLoopB(val: number | null): void {
  loopB = val;
}
export function setWaveformOverviewData(data: Float32Array | null): void {
  waveformOverviewData = data;
}
export function setCurrentCollection(col: CollectionId): void {
  currentCollection = col;
}
export function setSearchQuery(query: string): void {
  searchQuery = query;
}
export function setVisibleStart(val: number): void {
  visibleStart = val;
}
export function setVisibleEnd(val: number): void {
  visibleEnd = val;
}
export function setAllCollapsed(val: boolean): void {
  allCollapsed = val;
}
export function setFavorites(favs: Set<string>): void {
  favorites = favs;
}
export function setOwnFiles(files: OwnFileMetadata[]): void {
  ownFiles = files;
}
export function setPlayStats(stats: PlayStatsMap): void {
  playStats = stats;
}
export function setAudioFingerprints(fps: Record<string, Fingerprint>): void {
  audioFingerprints = fps;
}
export function setPinnedAuthors(authors: Set<string>): void {
  pinnedAuthors = authors;
}
export function setChannelWaveforms(wfs: Float32Array[]): void {
  channelWaveforms = wfs;
}
export function setMonoWaveform(wf: Float32Array): void {
  monoWaveform = wf;
}
export function setChannelPhases(phases: number[]): void {
  channelPhases = phases;
}
export function setChannelSpectrums(specs: Float32Array[]): void {
  channelSpectrums = specs;
}
export function setSpectrumTargets(targets: Float32Array[]): void {
  spectrumTargets = targets;
}
export function setCombinedSpectrum(spec: Float32Array): void {
  combinedSpectrum = spec;
}
export function setChannelSampleBuffers(bufs: Float32Array[]): void {
  channelSampleBuffers = bufs;
}
export function setChannelSampleWritePos(pos: number): void {
  channelSampleWritePos = pos;
}
export function setAudioSampleBuffer(buf: Float32Array): void {
  audioSampleBuffer = buf;
}
export function setAudioSampleWritePos(pos: number): void {
  audioSampleWritePos = pos;
}
export function setAmplitudeHistory(hist: number[][]): void {
  amplitudeHistory = hist;
}
export function setSidModeDetected(detected: boolean[]): void {
  sidModeDetected = detected;
}
export function setChannelSpectrumPeaks(peaks: Float32Array[]): void {
  channelSpectrumPeaks = peaks;
}
export function setCombinedSpectrumPeaks(peaks: Float32Array | null): void {
  combinedSpectrumPeaks = peaks;
}
export function setNoteHistories(hist: NoteHistoryEntry[][]): void {
  noteHistories = hist;
}
export function setNoteScrollOffset(offset: number): void {
  noteScrollOffset = offset;
}
export function setCurrentFormat(fmt: string): void {
  currentFormat = fmt;
}
export function setCurrentChannelCount(count: number): void {
  currentChannelCount = count;
}
export function setSpeedResampleBuffer(buf: Float32Array): void {
  speedResampleBuffer = buf;
}
export function setSpeedResamplePos(pos: number): void {
  speedResamplePos = pos;
}
export function setAudioUnlocked(unlocked: boolean): void {
  audioUnlocked = unlocked;
}
export function setMediaStreamDest(dest: MediaStreamAudioDestinationNode | null): void {
  mediaStreamDest = dest;
}
export function setAudioElement(elem: HTMLAudioElement | null): void {
  audioElement = elem;
}
export function setIsScrubbing(scrubbing: boolean): void {
  isScrubbing = scrubbing;
}
export function setToastTimeout(timeout: ReturnType<typeof setTimeout> | null): void {
  toastTimeout = timeout;
}
export function setSidebarVisible(visible: boolean): void {
  sidebarVisible = visible;
}
export function setWaveformDb(db: IDBDatabase | null): void {
  waveformDb = db;
}
export function setAuthorSortMode(mode: AuthorSortMode): void {
  authorSortMode = mode;
}
