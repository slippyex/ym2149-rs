// ============================================================================
// State - Global state variables for the YM2149 Web Player
// ============================================================================

import { WAVEFORM_SIZE, SPECTRUM_BINS, AUDIO_VIS_BUFFER_SIZE } from './config.js';

// WASM and catalog
export let Ym2149Player = null;
export let catalog = null;

// Track state
export let filteredTracks = [];
export let groupedTracks = []; // { type: 'author'|'track', ... }
export let currentTrackIndex = -1;
export let loadedFileData = null; // Stored file data for user-loaded files (for export)
export let loadedFileName = null; // Stored file name for user-loaded files

// Player state
export let wasmPlayer = null;
export let audioContext = null;
export let scriptProcessor = null;
export let isPlaying = false;
export let animationId = null;

// Playback options
export let shuffleEnabled = false;
export let autoPlayEnabled = false;
export let playbackSpeed = 1.0;

// A-B Loop
export let loopA = null;
export let loopB = null;

// Waveform
export let waveformOverviewData = null;

// Collection/filter state
export let currentCollection = "all";
export let searchQuery = "";
export let visibleStart = 0;
export let visibleEnd = 0;
export let collapsedAuthors = new Set();
export let allCollapsed = true;

// User data (localStorage)
export let favorites = new Set();
export let ownFiles = [];
export let playStats = {};
export let audioFingerprints = {};
export let pinnedAuthors = new Set();

// Visualization arrays
export let channelWaveforms = [];
export let monoWaveform = new Float32Array(WAVEFORM_SIZE);
export let channelPhases = [];
export let channelSpectrums = [];
export let combinedSpectrum = new Float32Array(SPECTRUM_BINS);

// Per-channel sample buffers for real audio visualization
export let channelSampleBuffers = [];
export let channelSampleWritePos = 0;
export let audioSampleBuffer = new Float32Array(AUDIO_VIS_BUFFER_SIZE);
export let audioSampleWritePos = 0;

// Amplitude history for SID visualization
export let amplitudeHistory = [];
export let sidModeDetected = [];

// Note history for tracker-style display
export let noteHistories = [];
export let noteScrollOffset = 0;

// Current format for channel naming
export let currentFormat = "";

// Current channel count
export let currentChannelCount = 3;

// Speed resampling
export let speedResampleBuffer = new Float32Array(0);
export let speedResamplePos = 0;

// Audio unlocked flag (for iOS)
export let audioUnlocked = false;

// Media stream (for iOS/mobile)
export let mediaStreamDest = null;
export let audioElement = null;

// Scrubbing state
export let isScrubbing = false;

// Toast timeout
export let toastTimeout = null;

// Sidebar state
export let sidebarVisible = true;

// Waveform DB reference
export let waveformDb = null;

// ============================================================================
// Setters - Use these to modify state from other modules
// ============================================================================

export function setYm2149Player(player) { Ym2149Player = player; }
export function setCatalog(data) { catalog = data; }
export function setFilteredTracks(tracks) { filteredTracks = tracks; }
export function setGroupedTracks(tracks) { groupedTracks = tracks; }
export function setCurrentTrackIndex(index) { currentTrackIndex = index; }
export function setLoadedFileData(data) { loadedFileData = data; }
export function setLoadedFileName(name) { loadedFileName = name; }
export function setWasmPlayer(player) { wasmPlayer = player; }
export function setAudioContext(ctx) { audioContext = ctx; }
export function setScriptProcessor(proc) { scriptProcessor = proc; }
export function setIsPlaying(playing) { isPlaying = playing; }
export function setAnimationId(id) { animationId = id; }
export function setShuffleEnabled(enabled) { shuffleEnabled = enabled; }
export function setAutoPlayEnabled(enabled) { autoPlayEnabled = enabled; }
export function setPlaybackSpeed(speed) { playbackSpeed = speed; }
export function setLoopA(val) { loopA = val; }
export function setLoopB(val) { loopB = val; }
export function setWaveformOverviewData(data) { waveformOverviewData = data; }
export function setCurrentCollection(col) { currentCollection = col; }
export function setSearchQuery(query) { searchQuery = query; }
export function setVisibleStart(val) { visibleStart = val; }
export function setVisibleEnd(val) { visibleEnd = val; }
export function setAllCollapsed(val) { allCollapsed = val; }
export function setFavorites(favs) { favorites = favs; }
export function setOwnFiles(files) { ownFiles = files; }
export function setPlayStats(stats) { playStats = stats; }
export function setAudioFingerprints(fps) { audioFingerprints = fps; }
export function setPinnedAuthors(authors) { pinnedAuthors = authors; }
export function setChannelWaveforms(wfs) { channelWaveforms = wfs; }
export function setMonoWaveform(wf) { monoWaveform = wf; }
export function setChannelPhases(phases) { channelPhases = phases; }
export function setChannelSpectrums(specs) { channelSpectrums = specs; }
export function setCombinedSpectrum(spec) { combinedSpectrum = spec; }
export function setChannelSampleBuffers(bufs) { channelSampleBuffers = bufs; }
export function setChannelSampleWritePos(pos) { channelSampleWritePos = pos; }
export function setAudioSampleBuffer(buf) { audioSampleBuffer = buf; }
export function setAudioSampleWritePos(pos) { audioSampleWritePos = pos; }
export function setAmplitudeHistory(hist) { amplitudeHistory = hist; }
export function setSidModeDetected(detected) { sidModeDetected = detected; }
export function setNoteHistories(hist) { noteHistories = hist; }
export function setNoteScrollOffset(offset) { noteScrollOffset = offset; }
export function setCurrentFormat(fmt) { currentFormat = fmt; }
export function setCurrentChannelCount(count) { currentChannelCount = count; }
export function setSpeedResampleBuffer(buf) { speedResampleBuffer = buf; }
export function setSpeedResamplePos(pos) { speedResamplePos = pos; }
export function setAudioUnlocked(unlocked) { audioUnlocked = unlocked; }
export function setMediaStreamDest(dest) { mediaStreamDest = dest; }
export function setAudioElement(elem) { audioElement = elem; }
export function setIsScrubbing(scrubbing) { isScrubbing = scrubbing; }
export function setToastTimeout(timeout) { toastTimeout = timeout; }
export function setSidebarVisible(visible) { sidebarVisible = visible; }
export function setWaveformDb(db) { waveformDb = db; }
