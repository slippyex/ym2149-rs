/* tslint:disable */
/* eslint-disable */
/**
 * Set panic hook for better error messages in the browser console
 */
export function init_panic_hook(): void;
/**
 * Main YM2149 player for WebAssembly
 *
 * This player handles YM file playback in the browser, generating audio samples
 * that can be fed into the Web Audio API.
 */
export class Ym2149Player {
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Get current playback state
   */
  is_playing(): boolean;
  /**
   * Set volume (0.0 to 1.0)
   * Note: Volume control is done in JavaScript via Web Audio API gain node
   */
  set_volume(_volume: number): void;
  /**
   * Get total frame count
   */
  frame_count(): number;
  /**
   * Get the current register values (for visualization)
   */
  get_registers(): Uint8Array;
  /**
   * Seek to a specific frame
   * Note: Seeking is implemented by stopping and restarting playback
   */
  seek_to_frame(_frame: number): void;
  /**
   * Get current frame position
   */
  frame_position(): number;
  /**
   * Generate audio samples
   *
   * Returns a Float32Array containing interleaved stereo samples.
   * The number of samples generated depends on the sample rate and frame rate.
   *
   * For 44.1kHz at 50Hz frame rate: 882 samples per frame
   */
  generateSamples(count: number): Float32Array;
  /**
   * Check if a channel is muted
   */
  is_channel_muted(channel: number): boolean;
  /**
   * Mute or unmute a channel (0-2)
   */
  set_channel_mute(channel: number, mute: boolean): void;
  /**
   * Enable or disable the ST color filter
   */
  set_color_filter(enabled: boolean): void;
  /**
   * Seek to a percentage of the song (0.0 to 1.0)
   * Note: Seeking is implemented by stopping and restarting playback
   */
  seek_to_percentage(_percentage: number): void;
  /**
   * Get playback position as percentage (0.0 to 1.0)
   */
  position_percentage(): number;
  /**
   * Generate samples into a pre-allocated buffer (zero-allocation)
   *
   * This is more efficient than `generate_samples` as it reuses the same buffer.
   */
  generateSamplesInto(buffer: Float32Array): void;
  /**
   * Create a new player from YM file data
   *
   * # Arguments
   *
   * * `data` - YM file data as Uint8Array
   *
   * # Returns
   *
   * Result containing the player or an error message
   */
  constructor(data: Uint8Array);
  /**
   * Start playback
   */
  play(): void;
  /**
   * Stop playback and reset to beginning
   */
  stop(): void;
  /**
   * Pause playback
   */
  pause(): void;
  /**
   * Get current playback state as string
   */
  state(): string;
  /**
   * Get current volume
   * Note: Always returns 1.0 as volume is handled in JavaScript
   */
  volume(): number;
  /**
   * Restart playback from the beginning
   */
  restart(): void;
  /**
   * Get metadata about the loaded YM file
   */
  readonly metadata: YmMetadata;
}
/**
 * YM file metadata exposed to JavaScript
 */
export class YmMetadata {
  private constructor();
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Get frame rate in Hz
   */
  readonly frame_rate: number;
  /**
   * Get frame count
   */
  readonly frame_count: number;
  /**
   * Get duration in seconds
   */
  readonly duration_seconds: number;
  /**
   * Get the song title
   */
  readonly title: string;
  /**
   * Get the song author
   */
  readonly author: string;
  /**
   * Get the YM format version
   */
  readonly format: string;
  /**
   * Get the song comments
   */
  readonly comments: string;
}

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
  readonly memory: WebAssembly.Memory;
  readonly __wbg_ym2149player_free: (a: number, b: number) => void;
  readonly __wbg_ymmetadata_free: (a: number, b: number) => void;
  readonly ym2149player_frame_count: (a: number) => number;
  readonly ym2149player_frame_position: (a: number) => number;
  readonly ym2149player_generateSamples: (a: number, b: number) => [number, number];
  readonly ym2149player_generateSamplesInto: (a: number, b: number, c: number, d: any) => void;
  readonly ym2149player_get_registers: (a: number) => [number, number];
  readonly ym2149player_is_channel_muted: (a: number, b: number) => number;
  readonly ym2149player_is_playing: (a: number) => number;
  readonly ym2149player_metadata: (a: number) => number;
  readonly ym2149player_new: (a: number, b: number) => [number, number, number];
  readonly ym2149player_pause: (a: number) => void;
  readonly ym2149player_play: (a: number) => [number, number];
  readonly ym2149player_position_percentage: (a: number) => number;
  readonly ym2149player_restart: (a: number) => [number, number];
  readonly ym2149player_seek_to_frame: (a: number, b: number) => void;
  readonly ym2149player_seek_to_percentage: (a: number, b: number) => void;
  readonly ym2149player_set_channel_mute: (a: number, b: number, c: number) => void;
  readonly ym2149player_set_color_filter: (a: number, b: number) => void;
  readonly ym2149player_state: (a: number) => [number, number];
  readonly ym2149player_stop: (a: number) => [number, number];
  readonly ym2149player_volume: (a: number) => number;
  readonly ymmetadata_author: (a: number) => [number, number];
  readonly ymmetadata_comments: (a: number) => [number, number];
  readonly ymmetadata_duration_seconds: (a: number) => number;
  readonly ymmetadata_format: (a: number) => [number, number];
  readonly ymmetadata_frame_count: (a: number) => number;
  readonly ymmetadata_frame_rate: (a: number) => number;
  readonly ymmetadata_title: (a: number) => [number, number];
  readonly init_panic_hook: () => void;
  readonly ym2149player_set_volume: (a: number, b: number) => void;
  readonly __wbindgen_free: (a: number, b: number, c: number) => void;
  readonly __wbindgen_malloc: (a: number, b: number) => number;
  readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
  readonly __wbindgen_externrefs: WebAssembly.Table;
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
