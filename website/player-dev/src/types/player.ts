// ============================================================================
// Player Types - WASM Player and metadata interfaces
// ============================================================================

/** YM metadata from WASM player */
export interface YmMetadata {
  title: string;
  author: string;
  comment?: string;
  format: string;
  frame_count: number;
  duration_seconds: number;
  chip_frequency?: number;
  player_frequency?: number;
  loop_frame?: number;
  attributes?: number;
}

/** Individual channel state */
export interface ChannelState {
  /** Current note (e.g., "C-4", "---") */
  note: string;
  /** Amplitude level (0-15 or 0-1) */
  amplitude: number;
  /** Frequency in Hz */
  frequency: number;
  /** Tone generator enabled */
  toneEnabled: boolean;
  /** Noise generator enabled */
  noiseEnabled: boolean;
  /** Envelope enabled */
  envelopeEnabled: boolean;
  /** Is this a DAC channel (SNDH) */
  isDac?: boolean;
}

/** Envelope state */
export interface EnvelopeState {
  /** Shape number (0-15) */
  shape: number;
  /** Shape name (e.g., "\\", "/", "\\\\\\", etc.) */
  shapeName: string;
  /** Envelope period */
  period?: number;
}

/** LMC1992 mixer state (SNDH only) */
export interface Lmc1992State {
  masterVolume: number;
  leftVolume: number;
  rightVolume: number;
  bass: number;
  treble: number;
}

/** Combined channel states result from WASM */
export interface ChannelStatesResult {
  channels: ChannelState[];
  envelope?: EnvelopeState;
  lmc1992?: Lmc1992State;
}

/** Samples with individual channel data */
export interface SamplesWithChannels {
  mixed: Float32Array;
  channels: Float32Array[];
}

/** YM2149 Player class from WASM module */
export interface Ym2149Player {
  /** Start playback */
  play(): void;
  /** Pause playback */
  pause(): void;
  /** Stop playback and reset position */
  stop(): void;
  /** Restart from beginning */
  restart(): void;
  /** Check if currently playing */
  is_playing(): boolean;
  /** Get current position as percentage (0-1) */
  position_percentage(): number;
  /** Seek to position percentage (0-1), returns success */
  seek_to_percentage(pct: number): boolean;
  /** Get total duration in seconds */
  duration_seconds(): number;
  /** Get number of audio channels */
  channelCount(): number;
  /** Mute/unmute a channel */
  setChannelMute(channel: number, mute: boolean): void;
  /** Check if channel is muted */
  isChannelMuted(channel: number): boolean;
  /** Set master volume (0-1) */
  set_volume(volume: number): void;
  /** Generate audio samples into buffer */
  generateSamplesInto(buffer: Float32Array): void;
  /** Generate audio samples with per-channel data */
  generateSamplesWithChannels(count: number): SamplesWithChannels;
  /** Get current state of all channels */
  getChannelStates(): ChannelStatesResult;
  /** Get subsong count (SNDH only) */
  subsongCount?(): number;
  /** Get current subsong (SNDH only) */
  currentSubsong?(): number;
  /** Set current subsong (SNDH only) */
  setSubsong?(index: number): boolean;
  /** Player metadata */
  readonly metadata: YmMetadata;
}

/** YM2149 Player constructor from WASM */
export interface Ym2149PlayerConstructor {
  new (data: Uint8Array): Ym2149Player;
}

/** WASM module exports */
export interface WasmModule {
  /** Default export initializes WASM */
  default(): Promise<void>;
  /** Player constructor */
  Ym2149Player: Ym2149PlayerConstructor;
}
