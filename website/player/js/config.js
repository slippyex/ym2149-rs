// ============================================================================
// Configuration - Constants and colors for the YM2149 Web Player
// ============================================================================

export const SAMPLE_RATE = 44100;
export const BUFFER_SIZE = 4096;
export const WAVEFORM_SIZE = 256;
export const SPECTRUM_BINS = 64;
export const SPECTRUM_DECAY = 0.92;      // How fast bars fall (higher = slower fall)
export const SPECTRUM_ATTACK = 0.35;     // How fast bars rise (lower = more resistance)
export const SPECTRUM_BASE_FREQ = 32.703;
export const BINS_PER_OCTAVE = 8;
export const ROW_HEIGHT = 36;
export const BUFFER_ROWS = 10;

export const COLORS = {
    purple: "#8b5cf6",
    cyan: "#06b6d4",
    pink: "#ec4899",
    green: "#10b981",
    gridLight: "rgba(139, 92, 246, 0.05)",
};

// Channel colors (cycles through for multi-chip)
export const CHANNEL_COLORS = [
    "#8b5cf6", "#06b6d4", "#ec4899", // PSG 1: purple, cyan, pink
    "#10b981", "#f59e0b", "#ef4444", // PSG 2: green, amber, red
    "#6366f1", "#14b8a6", "#f43f5e", // PSG 3: indigo, teal, rose
    "#8b5cf6", "#06b6d4", "#ec4899", // PSG 4: repeat
];

export const CHANNEL_NAMES = ["A", "B", "C", "D", "E", "F", "G", "H", "I", "J", "K", "L"];

// SNDH: 3 YM channels + 2 DAC channels (Left/Right)
export const SNDH_CHANNEL_NAMES = ["A", "B", "C", "L", "R"];
export const SNDH_CHANNEL_COLORS = [
    "#8b5cf6", "#06b6d4", "#ec4899", // YM: purple, cyan, pink
    "#10b981", "#ef4444", // DAC: green (L), red (R)
];

// Maximum channels supported (4 PSG chips * 3 channels)
export const MAX_CHANNELS = 12;

// Visualization
export const AUDIO_VIS_BUFFER_SIZE = 2048;
export const AMPLITUDE_HISTORY_SIZE = 64;
export const NOTE_HISTORY_SIZE = 32;
export const NOTE_SCROLL_SPEED = 0.5;

// Recent tracks limit
export const RECENT_DISPLAY_LIMIT = 50;

// Storage keys
export const STORAGE_KEY_FAVORITES = "ym2149_favorites";
export const STORAGE_KEY_STATS = "ym2149_stats";
export const STORAGE_KEY_FINGERPRINTS = "ym2149_fingerprints";
export const STORAGE_KEY_PINNED = "ym2149_pinned_authors";
export const STORAGE_KEY_OWN_FILES = "ym2149_own_files";

// IndexedDB names
export const OWN_FILES_DB_NAME = "ym2149-own-files";
export const OWN_FILES_DB_VERSION = 1;
export const OWN_FILES_STORE = "files";
export const CATALOG_DB_NAME = "ym2149-catalog";
export const CATALOG_DB_VERSION = 1;
export const CATALOG_STORE = "catalog";
export const WAVEFORM_DB_NAME = "ym2149-waveforms";
export const WAVEFORM_STORE_NAME = "waveforms";
