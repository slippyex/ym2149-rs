// ============================================================================
// UI Elements - DOM element cache
// ============================================================================

// ============================================================================
// DOM Element Cache
// ============================================================================

export const elements = {
    searchInput: null,
    searchClear: null,
    searchCount: null,
    collectionTabs: null,
    totalTracks: null,
    filteredCount: null,
    trackList: null,
    trackListInner: null,
    fileInput: null,
    songTitle: null,
    playerFavBtn: null,
    playerFavIcon: null,
    songAuthor: null,
    songFormat: null,
    songFrames: null,
    songChannels: null,
    subsongSelect: null,
    envelopeShape: null,
    lmc1992Panel: null,
    lmcMasterVol: null,
    lmcLeftVol: null,
    lmcRightVol: null,
    lmcBass: null,
    lmcTreble: null,
    similarPanel: null,
    similarAuthor: null,
    similarTracks: null,
    waveformScrubber: null,
    waveformOverview: null,
    waveformPlayhead: null,
    waveformCurrentTime: null,
    waveformTotalTime: null,
    waveformLoopA: null,
    waveformLoopB: null,
    waveformLoopRegion: null,
    progressContainer: null,
    progressBar: null,
    currentTime: null,
    totalTime: null,
    playBtn: null,
    playIcon: null,
    pauseIcon: null,
    stopBtn: null,
    restartBtn: null,
    nextBtn: null,
    shuffleBtn: null,
    autoPlayBtn: null,
    loopABtn: null,
    loopBBtn: null,
    loopClearBtn: null,
    loopIndicator: null,
    loopMarkerA: null,
    loopMarkerB: null,
    loopRegion: null,
    speedSelect: null,
    sidebar: null,
    sidebarToggle: null,
    sidebarBackdrop: null,
    mobileMenuBtn: null,
    hideSidebarBtn: null,
    toast: null,
    toastMessage: null,
    shareBtn: null,
    volumeSlider: null,
    exportBtn: null,
    exportModal: null,
    exportDuration: null,
    exportMode: null,
    exportStemOptions: null,
    exportChannelCheckboxes: null,
    exportSampleRate: null,
    exportProgress: null,
    exportProgressBar: null,
    exportCancel: null,
    exportStart: null,
    dropZone: null,
    vizModeOsc: null,
    vizModeSpec: null,
    oscView: null,
    specView: null,
    oscChannels: null,
    specChannels: null,
    channelMutes: null,
};

// ============================================================================
// Canvas references
// ============================================================================

export const canvases = {
    oscMono: null,
    specCombined: null,
};

export const contexts = {};

// Dynamic channel UI references
export const channelCanvases = { osc: [], spec: [] };
export const channelContexts = { osc: [], spec: [] };
export const channelNotes = [];
export const channelMuteButtons = [];

// ============================================================================
// Initialize Elements
// ============================================================================

export function initElements() {
    elements.searchInput = document.getElementById("searchInput");
    elements.searchClear = document.getElementById("searchClear");
    elements.searchCount = document.getElementById("searchCount");
    elements.collectionTabs = document.getElementById("collectionTabs");
    elements.totalTracks = document.getElementById("totalTracks");
    elements.filteredCount = document.getElementById("filteredCount");
    elements.trackList = document.getElementById("trackList");
    elements.trackListInner = document.getElementById("trackListInner");
    elements.fileInput = document.getElementById("fileInput");
    elements.songTitle = document.getElementById("songTitle");
    elements.playerFavBtn = document.getElementById("playerFavBtn");
    elements.playerFavIcon = document.getElementById("playerFavIcon");
    elements.songAuthor = document.getElementById("songAuthor");
    elements.songFormat = document.getElementById("songFormat");
    elements.songFrames = document.getElementById("songFrames");
    elements.songChannels = document.getElementById("songChannels");
    elements.subsongSelect = document.getElementById("subsongSelect");
    elements.envelopeShape = document.getElementById("envelopeShape");
    elements.lmc1992Panel = document.getElementById("lmc1992Panel");
    elements.lmcMasterVol = document.getElementById("lmcMasterVol");
    elements.lmcLeftVol = document.getElementById("lmcLeftVol");
    elements.lmcRightVol = document.getElementById("lmcRightVol");
    elements.lmcBass = document.getElementById("lmcBass");
    elements.lmcTreble = document.getElementById("lmcTreble");
    elements.similarPanel = document.getElementById("similarPanel");
    elements.similarAuthor = document.getElementById("similarAuthor");
    elements.similarTracks = document.getElementById("similarTracks");
    elements.waveformScrubber = document.getElementById("waveformScrubber");
    elements.waveformOverview = document.getElementById("waveformOverview");
    elements.waveformPlayhead = document.getElementById("waveformPlayhead");
    elements.waveformCurrentTime = document.getElementById("waveformCurrentTime");
    elements.waveformTotalTime = document.getElementById("waveformTotalTime");
    elements.waveformLoopA = document.getElementById("waveformLoopA");
    elements.waveformLoopB = document.getElementById("waveformLoopB");
    elements.waveformLoopRegion = document.getElementById("waveformLoopRegion");
    elements.progressContainer = document.getElementById("progressContainer");
    elements.progressBar = document.getElementById("progressBar");
    elements.currentTime = document.getElementById("currentTime");
    elements.totalTime = document.getElementById("totalTime");
    elements.playBtn = document.getElementById("playBtn");
    elements.playIcon = document.getElementById("playIcon");
    elements.pauseIcon = document.getElementById("pauseIcon");
    elements.stopBtn = document.getElementById("stopBtn");
    elements.restartBtn = document.getElementById("restartBtn");
    elements.nextBtn = document.getElementById("nextBtn");
    elements.shuffleBtn = document.getElementById("shuffleBtn");
    elements.autoPlayBtn = document.getElementById("autoPlayBtn");
    elements.loopABtn = document.getElementById("loopABtn");
    elements.loopBBtn = document.getElementById("loopBBtn");
    elements.loopClearBtn = document.getElementById("loopClearBtn");
    elements.loopIndicator = document.getElementById("loopIndicator");
    elements.loopMarkerA = document.getElementById("loopMarkerA");
    elements.loopMarkerB = document.getElementById("loopMarkerB");
    elements.loopRegion = document.getElementById("loopRegion");
    elements.speedSelect = document.getElementById("speedSelect");
    elements.sidebar = document.getElementById("sidebar");
    elements.sidebarToggle = document.getElementById("sidebarToggle");
    elements.sidebarBackdrop = document.getElementById("sidebarBackdrop");
    elements.mobileMenuBtn = document.getElementById("mobileMenuBtn");
    elements.hideSidebarBtn = document.getElementById("hideSidebarBtn");
    elements.toast = document.getElementById("toast");
    elements.toastMessage = document.getElementById("toastMessage");
    elements.shareBtn = document.getElementById("shareBtn");
    elements.volumeSlider = document.getElementById("volumeSlider");
    elements.exportBtn = document.getElementById("exportBtn");
    elements.exportModal = document.getElementById("exportModal");
    elements.exportDuration = document.getElementById("exportDuration");
    elements.exportMode = document.getElementById("exportMode");
    elements.exportStemOptions = document.getElementById("exportStemOptions");
    elements.exportChannelCheckboxes = document.getElementById("exportChannelCheckboxes");
    elements.exportSampleRate = document.getElementById("exportSampleRate");
    elements.exportProgress = document.getElementById("exportProgress");
    elements.exportProgressBar = document.getElementById("exportProgressBar");
    elements.exportCancel = document.getElementById("exportCancel");
    elements.exportStart = document.getElementById("exportStart");
    elements.dropZone = document.getElementById("dropZone");
    elements.vizModeOsc = document.getElementById("vizModeOsc");
    elements.vizModeSpec = document.getElementById("vizModeSpec");
    elements.oscView = document.getElementById("oscView");
    elements.specView = document.getElementById("specView");
    elements.oscChannels = document.getElementById("oscChannels");
    elements.specChannels = document.getElementById("specChannels");
    elements.channelMutes = document.getElementById("channelMutes");

    // Static canvases
    canvases.oscMono = document.getElementById("oscMono");
    canvases.specCombined = document.getElementById("specCombined");
}
