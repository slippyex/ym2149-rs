/* tslint:disable */
/* eslint-disable */
/**
 * Set panic hook for better error messages in the browser console.
 */
export function init_panic_hook(): void;
/**
 * Main YM2149 player for WebAssembly.
 *
 * This player handles YM/AKS/AY file playback in the browser, generating audio samples
 * that can be fed into the Web Audio API.
 */
export class Ym2149Player {
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Create a new player from file data.
   *
   * Automatically detects the file format (YM, AKS, AY, or SNDH).
   *
   * # Arguments
   *
   * * `data` - File data as Uint8Array
   *
   * # Returns
   *
   * Result containing the player or an error message.
   */
  constructor(data: Uint8Array);
  /**
   * Start playback.
   */
  play(): void;
  /**
   * Pause playback.
   */
  pause(): void;
  /**
   * Stop playback and reset to beginning.
   */
  stop(): void;
  /**
   * Restart playback from the beginning.
   */
  restart(): void;
  /**
   * Get current playback state.
   */
  is_playing(): boolean;
  /**
   * Get current playback state as string.
   */
  state(): string;
  /**
   * Set volume (0.0 to 1.0). Applied to generated samples.
   */
  set_volume(volume: number): void;
  /**
   * Get current volume (0.0 to 1.0).
   */
  volume(): number;
  /**
   * Get current frame position.
   */
  frame_position(): number;
  /**
   * Get total frame count.
   */
  frame_count(): number;
  /**
   * Get playback position as percentage (0.0 to 1.0).
   */
  position_percentage(): number;
  /**
   * Seek to a specific frame (silently ignored for Arkos/AY backends).
   */
  seek_to_frame(frame: number): void;
  /**
   * Seek to a percentage of the song (0.0 to 1.0).
   *
   * Returns true if seek succeeded. Works for all SNDH files (uses fallback duration for older files).
   */
  seek_to_percentage(percentage: number): boolean;
  /**
   * Get duration in seconds.
   *
   * For SNDH < 2.2 without FRMS/TIME, returns 300 (5 minute fallback).
   */
  duration_seconds(): number;
  /**
   * Check if the duration is from actual metadata or estimated.
   *
   * Returns false for older SNDH files using the 5-minute fallback.
   */
  hasDurationInfo(): boolean;
  /**
   * Mute or unmute a channel (0-2).
   */
  set_channel_mute(channel: number, mute: boolean): void;
  /**
   * Check if a channel is muted.
   */
  is_channel_muted(channel: number): boolean;
  /**
   * Generate audio samples.
   *
   * Returns a Float32Array containing mono samples.
   * The number of samples generated depends on the sample rate and frame rate.
   *
   * For 44.1kHz at 50Hz frame rate: 882 samples per frame.
   */
  generateSamples(count: number): Float32Array;
  /**
   * Generate samples into a pre-allocated buffer (zero-allocation).
   *
   * This is more efficient than `generate_samples` as it reuses the same buffer.
   */
  generateSamplesInto(buffer: Float32Array): void;
  /**
   * Generate stereo audio samples (interleaved L/R).
   *
   * Returns frame_count * 2 samples. SNDH uses native stereo output,
   * other formats duplicate mono to stereo.
   */
  generateSamplesStereo(frame_count: number): Float32Array;
  /**
   * Generate stereo samples into a pre-allocated buffer (zero-allocation).
   *
   * Buffer length must be even (frame_count * 2). Interleaved L/R format.
   * SNDH uses native stereo output, other formats duplicate mono to stereo.
   */
  generateSamplesIntoStereo(buffer: Float32Array): void;
  /**
   * Get the current register values (for visualization).
   */
  get_registers(): Uint8Array;
  /**
   * Get channel states for visualization (frequency, amplitude, note, effects).
   *
   * Returns a JsValue containing an object with channel data:
   * ```json
   * {
   *   "channels": [
   *     { "frequency": 440.0, "note": "A4", "amplitude": 0.8, "toneEnabled": true, "noiseEnabled": false, "envelopeEnabled": false },
   *     ...
   *   ],
   *   "envelope": { "period": 256, "shape": 14, "shapeName": "/\\/\\" }
   * }
   * ```
   */
  getChannelStates(): any;
  /**
   * Enable or disable the ST color filter.
   */
  set_color_filter(enabled: boolean): void;
  /**
   * Get the number of subsongs (1 for most formats, >1 for multi-song SNDH files).
   */
  subsongCount(): number;
  /**
   * Get the current subsong index (1-based).
   */
  currentSubsong(): number;
  /**
   * Set the current subsong (1-based index). Returns true on success.
   */
  setSubsong(index: number): boolean;
  /**
   * Get metadata about the loaded file.
   */
  readonly metadata: YmMetadata;
}
/**
 * YM file metadata exposed to JavaScript.
 */
export class YmMetadata {
  private constructor();
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Get the song title.
   */
  readonly title: string;
  /**
   * Get the song author.
   */
  readonly author: string;
  /**
   * Get the song comments.
   */
  readonly comments: string;
  /**
   * Get the YM format version.
   */
  readonly format: string;
  /**
   * Get frame count.
   */
  readonly frame_count: number;
  /**
   * Get frame rate in Hz.
   */
  readonly frame_rate: number;
  /**
   * Get duration in seconds.
   */
  readonly duration_seconds: number;
}

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
  readonly memory: WebAssembly.Memory;
  readonly __wbg_ym2149player_free: (a: number, b: number) => void;
  readonly ym2149player_new: (a: number, b: number) => [number, number, number];
  readonly ym2149player_metadata: (a: number) => number;
  readonly ym2149player_play: (a: number) => void;
  readonly ym2149player_pause: (a: number) => void;
  readonly ym2149player_stop: (a: number) => void;
  readonly ym2149player_restart: (a: number) => void;
  readonly ym2149player_is_playing: (a: number) => number;
  readonly ym2149player_state: (a: number) => [number, number];
  readonly ym2149player_set_volume: (a: number, b: number) => void;
  readonly ym2149player_volume: (a: number) => number;
  readonly ym2149player_frame_position: (a: number) => number;
  readonly ym2149player_frame_count: (a: number) => number;
  readonly ym2149player_position_percentage: (a: number) => number;
  readonly ym2149player_seek_to_frame: (a: number, b: number) => void;
  readonly ym2149player_seek_to_percentage: (a: number, b: number) => number;
  readonly ym2149player_duration_seconds: (a: number) => number;
  readonly ym2149player_hasDurationInfo: (a: number) => number;
  readonly ym2149player_set_channel_mute: (a: number, b: number, c: number) => void;
  readonly ym2149player_is_channel_muted: (a: number, b: number) => number;
  readonly ym2149player_generateSamples: (a: number, b: number) => [number, number];
  readonly ym2149player_generateSamplesInto: (a: number, b: number, c: number, d: any) => void;
  readonly ym2149player_generateSamplesStereo: (a: number, b: number) => [number, number];
  readonly ym2149player_generateSamplesIntoStereo: (a: number, b: number, c: number, d: any) => void;
  readonly ym2149player_get_registers: (a: number) => [number, number];
  readonly ym2149player_getChannelStates: (a: number) => any;
  readonly ym2149player_set_color_filter: (a: number, b: number) => void;
  readonly ym2149player_subsongCount: (a: number) => number;
  readonly ym2149player_currentSubsong: (a: number) => number;
  readonly ym2149player_setSubsong: (a: number, b: number) => number;
  readonly init_panic_hook: () => void;
  readonly __wbg_ymmetadata_free: (a: number, b: number) => void;
  readonly ymmetadata_title: (a: number) => [number, number];
  readonly ymmetadata_author: (a: number) => [number, number];
  readonly ymmetadata_comments: (a: number) => [number, number];
  readonly ymmetadata_format: (a: number) => [number, number];
  readonly ymmetadata_frame_count: (a: number) => number;
  readonly ymmetadata_frame_rate: (a: number) => number;
  readonly ymmetadata_duration_seconds: (a: number) => number;
  readonly __wbindgen_free: (a: number, b: number, c: number) => void;
  readonly __wbindgen_exn_store: (a: number) => void;
  readonly __externref_table_alloc: () => number;
  readonly __wbindgen_externrefs: WebAssembly.Table;
  readonly __wbindgen_malloc: (a: number, b: number) => number;
  readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
  readonly __externref_table_dealloc: (a: number) => void;
  readonly __wbindgen_start: () => void;
}

export type SyncInitInput = BufferSource | WebAssembly.Module;
/**
* Instantiates the given `module`, which can either be bytes or
* a precompiled `WebAssembly.Module`.
*
* @param {{ module: SyncInitInput }} module - Passing `SyncInitInput` directly is deprecated.
*
* @returns {InitOutput}
*/
export function initSync(module: { module: SyncInitInput } | SyncInitInput): InitOutput;

/**
* If `module_or_path` is {RequestInfo} or {URL}, makes a request and
* for everything else, calls `WebAssembly.instantiate` directly.
*
* @param {{ module_or_path: InitInput | Promise<InitInput> }} module_or_path - Passing `InitInput` directly is deprecated.
*
* @returns {Promise<InitOutput>}
*/
export default function __wbg_init (module_or_path?: { module_or_path: InitInput | Promise<InitInput> } | InitInput | Promise<InitInput>): Promise<InitOutput>;
