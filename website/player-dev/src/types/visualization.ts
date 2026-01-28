// ============================================================================
// Visualization Types - Canvas and color types
// ============================================================================

/** RGB color object */
export interface RgbColor {
  r: number;
  g: number;
  b: number;
}

/** Canvas context map for main canvases */
export interface CanvasContextMap {
  oscMono: CanvasRenderingContext2D | null;
  specCombined: CanvasRenderingContext2D | null;
  waveformOverview: CanvasRenderingContext2D | null;
}

/** Canvas element map for main canvases */
export interface CanvasElementMap {
  oscMono: HTMLCanvasElement | null;
  specCombined: HTMLCanvasElement | null;
  waveformOverview: HTMLCanvasElement | null;
}

/** Channel canvases (oscilloscope and spectrum per channel) */
export interface ChannelCanvases {
  osc: HTMLCanvasElement[];
  spec: HTMLCanvasElement[];
}

/** Channel contexts (oscilloscope and spectrum per channel) */
export interface ChannelContexts {
  osc: (CanvasRenderingContext2D | null)[];
  spec: (CanvasRenderingContext2D | null)[];
}

/** Peak hold state for spectrum analyzer */
export interface PeakHoldState {
  peaks: Float32Array;
  holdTimers: Uint8Array;
}

/** Waveform rendering options */
export interface WaveformRenderOptions {
  color?: string;
  backgroundColor?: string;
  showGrid?: boolean;
  showCenterLine?: boolean;
}

/** Spectrum rendering options */
export interface SpectrumRenderOptions {
  color?: string;
  backgroundColor?: string;
  showGrid?: boolean;
  showPeaks?: boolean;
  barGap?: number;
}

/** Visualization mode */
export type VisualizationMode = 'oscilloscope' | 'spectrum';
