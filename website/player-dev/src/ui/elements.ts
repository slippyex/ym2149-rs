// ============================================================================
// UI Elements - DOM element cache
// ============================================================================

import type { CanvasContextMap, CanvasElementMap, ChannelCanvases, ChannelContexts } from '../types/index.ts';

// ============================================================================
// DOM Element Cache
// ============================================================================

export interface DOMElements {
  searchInput: HTMLInputElement | null;
  searchClear: HTMLButtonElement | null;
  searchCount: HTMLElement | null;
  collectionTabs: HTMLElement | null;
  totalTracks: HTMLElement | null;
  filteredCount: HTMLElement | null;
  trackList: HTMLElement | null;
  trackListInner: HTMLElement | null;
  stickyAuthorHeader: HTMLElement | null;
  stickyAuthorCollection: HTMLElement | null;
  stickyAuthorName: HTMLElement | null;
  fileInput: HTMLInputElement | null;
  songTitle: HTMLElement | null;
  playerFavBtn: HTMLButtonElement | null;
  playerFavIcon: SVGElement | null;
  songAuthor: HTMLElement | null;
  songFormat: HTMLElement | null;
  songFrames: HTMLElement | null;
  songChannels: HTMLElement | null;
  subsongSelect: HTMLSelectElement | null;
  envelopeShape: HTMLElement | null;
  lmc1992Panel: HTMLElement | null;
  lmcMasterVol: HTMLElement | null;
  lmcLeftVol: HTMLElement | null;
  lmcRightVol: HTMLElement | null;
  lmcBass: HTMLElement | null;
  lmcTreble: HTMLElement | null;
  similarPanel: HTMLElement | null;
  similarAuthor: HTMLElement | null;
  similarTracks: HTMLElement | null;
  waveformScrubber: HTMLElement | null;
  waveformOverview: HTMLCanvasElement | null;
  waveformPlayhead: HTMLElement | null;
  waveformCurrentTime: HTMLElement | null;
  waveformTotalTime: HTMLElement | null;
  waveformLoopA: HTMLElement | null;
  waveformLoopB: HTMLElement | null;
  waveformLoopRegion: HTMLElement | null;
  progressContainer: HTMLElement | null;
  progressBar: HTMLInputElement | null;
  currentTime: HTMLElement | null;
  totalTime: HTMLElement | null;
  playBtn: HTMLButtonElement | null;
  playIcon: HTMLElement | null;
  pauseIcon: HTMLElement | null;
  stopBtn: HTMLButtonElement | null;
  restartBtn: HTMLButtonElement | null;
  nextBtn: HTMLButtonElement | null;
  shuffleBtn: HTMLButtonElement | null;
  autoPlayBtn: HTMLButtonElement | null;
  loopABtn: HTMLButtonElement | null;
  loopBBtn: HTMLButtonElement | null;
  loopClearBtn: HTMLButtonElement | null;
  loopIndicator: HTMLElement | null;
  loopMarkerA: HTMLElement | null;
  loopMarkerB: HTMLElement | null;
  loopRegion: HTMLElement | null;
  speedSelect: HTMLSelectElement | null;
  sidebar: HTMLElement | null;
  sidebarToggle: HTMLButtonElement | null;
  sidebarBackdrop: HTMLElement | null;
  mobileMenuBtn: HTMLButtonElement | null;
  hideSidebarBtn: HTMLButtonElement | null;
  toast: HTMLElement | null;
  toastMessage: HTMLElement | null;
  shareBtn: HTMLButtonElement | null;
  volumeSlider: HTMLInputElement | null;
  exportBtn: HTMLButtonElement | null;
  downloadBtn: HTMLButtonElement | null;
  exportModal: HTMLElement | null;
  exportDuration: HTMLInputElement | null;
  exportMode: HTMLSelectElement | null;
  exportStemOptions: HTMLElement | null;
  exportChannelCheckboxes: HTMLElement | null;
  exportSampleRate: HTMLSelectElement | null;
  exportProgress: HTMLElement | null;
  exportProgressBar: HTMLElement | null;
  exportCancel: HTMLButtonElement | null;
  exportStart: HTMLButtonElement | null;
  dropZone: HTMLElement | null;
  dropOverlay: HTMLElement | null;
  helpBtn: HTMLButtonElement | null;
  helpModal: HTMLElement | null;
  helpCloseBtn: HTMLButtonElement | null;
  vizModeOsc: HTMLButtonElement | null;
  vizModeSpec: HTMLButtonElement | null;
  oscView: HTMLElement | null;
  specView: HTMLElement | null;
  oscChannels: HTMLElement | null;
  specChannels: HTMLElement | null;
  channelMutes: HTMLElement | null;
  toggleCollapseBtn: HTMLButtonElement | null;
}

export const elements: DOMElements = {
  searchInput: null,
  searchClear: null,
  searchCount: null,
  collectionTabs: null,
  totalTracks: null,
  filteredCount: null,
  trackList: null,
  trackListInner: null,
  stickyAuthorHeader: null,
  stickyAuthorCollection: null,
  stickyAuthorName: null,
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
  downloadBtn: null,
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
  dropOverlay: null,
  helpBtn: null,
  helpModal: null,
  helpCloseBtn: null,
  vizModeOsc: null,
  vizModeSpec: null,
  oscView: null,
  specView: null,
  oscChannels: null,
  specChannels: null,
  channelMutes: null,
  toggleCollapseBtn: null,
};

// ============================================================================
// Canvas references
// ============================================================================

export const canvases: CanvasElementMap = {
  oscMono: null,
  specCombined: null,
  waveformOverview: null,
};

export const contexts: CanvasContextMap = {
  oscMono: null,
  specCombined: null,
  waveformOverview: null,
};

// Dynamic channel UI references
export const channelCanvases: ChannelCanvases = { osc: [], spec: [] };
export const channelContexts: ChannelContexts = { osc: [], spec: [] };
export const channelNotes: (HTMLElement | null)[] = [];
export const channelMuteButtons: HTMLButtonElement[] = [];

// ============================================================================
// Initialize Elements
// ============================================================================

function getEl<T extends HTMLElement>(id: string): T | null {
  return document.getElementById(id) as T | null;
}

export function initElements(): void {
  elements.searchInput = getEl<HTMLInputElement>('searchInput');
  elements.searchClear = getEl<HTMLButtonElement>('searchClear');
  elements.searchCount = getEl('searchCount');
  elements.collectionTabs = getEl('collectionTabs');
  elements.totalTracks = getEl('totalTracks');
  elements.filteredCount = getEl('filteredCount');
  elements.trackList = getEl('trackList');
  elements.trackListInner = getEl('trackListInner');
  elements.stickyAuthorHeader = getEl('stickyAuthorHeader');
  elements.stickyAuthorCollection = getEl('stickyAuthorCollection');
  elements.stickyAuthorName = getEl('stickyAuthorName');
  elements.fileInput = getEl<HTMLInputElement>('fileInput');
  elements.songTitle = getEl('songTitle');
  elements.playerFavBtn = getEl<HTMLButtonElement>('playerFavBtn');
  elements.playerFavIcon = document.getElementById('playerFavIcon') as SVGElement | null;
  elements.songAuthor = getEl('songAuthor');
  elements.songFormat = getEl('songFormat');
  elements.songFrames = getEl('songFrames');
  elements.songChannels = getEl('songChannels');
  elements.subsongSelect = getEl<HTMLSelectElement>('subsongSelect');
  elements.envelopeShape = getEl('envelopeShape');
  elements.lmc1992Panel = getEl('lmc1992Panel');
  elements.lmcMasterVol = getEl('lmcMasterVol');
  elements.lmcLeftVol = getEl('lmcLeftVol');
  elements.lmcRightVol = getEl('lmcRightVol');
  elements.lmcBass = getEl('lmcBass');
  elements.lmcTreble = getEl('lmcTreble');
  elements.similarPanel = getEl('similarPanel');
  elements.similarAuthor = getEl('similarAuthor');
  elements.similarTracks = getEl('similarTracks');
  elements.waveformScrubber = getEl('waveformScrubber');
  elements.waveformOverview = getEl<HTMLCanvasElement>('waveformOverview');
  elements.waveformPlayhead = getEl('waveformPlayhead');
  elements.waveformCurrentTime = getEl('waveformCurrentTime');
  elements.waveformTotalTime = getEl('waveformTotalTime');
  elements.waveformLoopA = getEl('waveformLoopA');
  elements.waveformLoopB = getEl('waveformLoopB');
  elements.waveformLoopRegion = getEl('waveformLoopRegion');
  elements.progressContainer = getEl('progressContainer');
  elements.progressBar = getEl<HTMLInputElement>('progressBar');
  elements.currentTime = getEl('currentTime');
  elements.totalTime = getEl('totalTime');
  elements.playBtn = getEl<HTMLButtonElement>('playBtn');
  elements.playIcon = getEl('playIcon');
  elements.pauseIcon = getEl('pauseIcon');
  elements.stopBtn = getEl<HTMLButtonElement>('stopBtn');
  elements.restartBtn = getEl<HTMLButtonElement>('restartBtn');
  elements.nextBtn = getEl<HTMLButtonElement>('nextBtn');
  elements.shuffleBtn = getEl<HTMLButtonElement>('shuffleBtn');
  elements.autoPlayBtn = getEl<HTMLButtonElement>('autoPlayBtn');
  elements.loopABtn = getEl<HTMLButtonElement>('loopABtn');
  elements.loopBBtn = getEl<HTMLButtonElement>('loopBBtn');
  elements.loopClearBtn = getEl<HTMLButtonElement>('loopClearBtn');
  elements.loopIndicator = getEl('loopIndicator');
  elements.loopMarkerA = getEl('loopMarkerA');
  elements.loopMarkerB = getEl('loopMarkerB');
  elements.loopRegion = getEl('loopRegion');
  elements.speedSelect = getEl<HTMLSelectElement>('speedSelect');
  elements.sidebar = getEl('sidebar');
  elements.sidebarToggle = getEl<HTMLButtonElement>('sidebarToggle');
  elements.sidebarBackdrop = getEl('sidebarBackdrop');
  elements.mobileMenuBtn = getEl<HTMLButtonElement>('mobileMenuBtn');
  elements.hideSidebarBtn = getEl<HTMLButtonElement>('hideSidebarBtn');
  elements.toast = getEl('toast');
  elements.toastMessage = getEl('toastMessage');
  elements.shareBtn = getEl<HTMLButtonElement>('shareBtn');
  elements.volumeSlider = getEl<HTMLInputElement>('volumeSlider');
  elements.exportBtn = getEl<HTMLButtonElement>('exportBtn');
  elements.downloadBtn = getEl<HTMLButtonElement>('downloadBtn');
  elements.exportModal = getEl('exportModal');
  elements.exportDuration = getEl<HTMLInputElement>('exportDuration');
  elements.exportMode = getEl<HTMLSelectElement>('exportMode');
  elements.exportStemOptions = getEl('exportStemOptions');
  elements.exportChannelCheckboxes = getEl('exportChannelCheckboxes');
  elements.exportSampleRate = getEl<HTMLSelectElement>('exportSampleRate');
  elements.exportProgress = getEl('exportProgress');
  elements.exportProgressBar = getEl('exportProgressBar');
  elements.exportCancel = getEl<HTMLButtonElement>('exportCancel');
  elements.exportStart = getEl<HTMLButtonElement>('exportStart');
  elements.dropZone = getEl('dropZone');
  elements.dropOverlay = getEl('dropOverlay');
  elements.helpBtn = getEl<HTMLButtonElement>('helpBtn');
  elements.helpModal = getEl('helpModal');
  elements.helpCloseBtn = getEl<HTMLButtonElement>('helpCloseBtn');
  elements.vizModeOsc = getEl<HTMLButtonElement>('vizModeOsc');
  elements.vizModeSpec = getEl<HTMLButtonElement>('vizModeSpec');
  elements.oscView = getEl('oscView');
  elements.specView = getEl('specView');
  elements.oscChannels = getEl('oscChannels');
  elements.specChannels = getEl('specChannels');
  elements.channelMutes = getEl('channelMutes');
  elements.toggleCollapseBtn = getEl<HTMLButtonElement>('toggleCollapseBtn');

  // Static canvases
  canvases.oscMono = getEl<HTMLCanvasElement>('oscMono');
  canvases.specCombined = getEl<HTMLCanvasElement>('specCombined');
}
