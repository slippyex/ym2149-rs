# ym2149-wasm

<p align="center">
  <strong>Hardware-accurate YM2149 PSG emulator in WebAssembly</strong><br>
  Play Atari ST, Amstrad CPC &amp; ZX Spectrum chiptunes directly in your browser
</p>

<p align="center">
  <a href="https://www.npmjs.com/package/ym2149-wasm"><img src="https://img.shields.io/npm/v/ym2149-wasm.svg" alt="npm version"></a>
  <a href="https://www.npmjs.com/package/ym2149-wasm"><img src="https://img.shields.io/npm/dm/ym2149-wasm.svg" alt="npm downloads"></a>
  <a href="https://github.com/slippyex/ym2149-rs/blob/main/LICENSE"><img src="https://img.shields.io/npm/l/ym2149-wasm.svg" alt="license"></a>
</p>

<p align="center">
  <a href="https://ym2149-rs.org">Website</a> &bull;
  <a href="https://ym2149-rs.org/demo/">Live Demo</a> &bull;
  <a href="https://github.com/slippyex/ym2149-rs">GitHub</a> &bull;
  <a href="https://ym2149-rs.org/tutorials.html">Tutorials</a>
</p>

---

## Features

- **Multi-Format Support** - SNDH, YM2-YM6, Arkos Tracker (`.aks`), and ZXAY/EMUL (`.ay`) files
- **Cycle-Accurate Emulation** - Based on Leonard/Oxygene's [AtariAudio](https://github.com/arnaud-carre/sndh-player/tree/main/AtariAudio)
- **Full SNDH Support** - 68000 CPU emulation with multi-subsong navigation
- **Full Playback Control** - Play, pause, stop, seek, volume, channel muting
- **Real-Time Visualization** - Waveform data, register access, and rich channel states (frequency, note names, envelope info)
- **TypeScript Support** - Full type definitions included
- **Tiny Bundle** - ~100KB gzipped
- **Zero Dependencies** - Pure WebAssembly, no runtime dependencies

## Installation

```bash
npm install ym2149-wasm
```

Or with yarn/pnpm:

```bash
yarn add ym2149-wasm
pnpm add ym2149-wasm
```

## Quick Start

```javascript
import init, { Ym2149Player } from 'ym2149-wasm';

async function play(fileData) {
  // Initialize the WASM module (only needed once)
  await init();

  // Create a player from file data (Uint8Array)
  const player = new Ym2149Player(fileData);

  // Access metadata
  console.log(`Now playing: ${player.metadata.title} by ${player.metadata.author}`);
  console.log(`Duration: ${player.metadata.duration_seconds}s`);

  // Start playback
  player.play();

  // Generate audio samples for Web Audio API
  const samples = player.generateSamples(4096);
}
```

## Web Audio API Integration

```javascript
import init, { Ym2149Player } from 'ym2149-wasm';

class ChiptunePlayer {
  constructor() {
    this.audioContext = null;
    this.player = null;
    this.scriptProcessor = null;
  }

  async init() {
    await init();
    this.audioContext = new AudioContext({ sampleRate: 44100 });
  }

  async load(fileData) {
    this.player = new Ym2149Player(new Uint8Array(fileData));
    return this.player.metadata;
  }

  play() {
    if (!this.player) return;
    this.player.play();

    // Create ScriptProcessor for real-time audio
    this.scriptProcessor = this.audioContext.createScriptProcessor(4096, 0, 1);
    this.scriptProcessor.onaudioprocess = (e) => {
      const output = e.outputBuffer.getChannelData(0);
      if (this.player.is_playing()) {
        const samples = this.player.generateSamples(output.length);
        output.set(samples);
      } else {
        output.fill(0);
      }
    };
    this.scriptProcessor.connect(this.audioContext.destination);
  }

  pause() { this.player?.pause(); }
  stop() {
    this.player?.stop();
    this.scriptProcessor?.disconnect();
  }
}

// Usage
const player = new ChiptunePlayer();
await player.init();

const response = await fetch('music.sndh');
const buffer = await response.arrayBuffer();
const meta = await player.load(buffer);
console.log(`Loaded: ${meta.title}`);
player.play();
```

## Multi-Subsong Support (SNDH)

Many SNDH files contain multiple songs. Use the subsong API to navigate them:

```javascript
const player = new Ym2149Player(sndhData);

// Check how many subsongs are available
const count = player.subsongCount();
console.log(`This file has ${count} subsong(s)`);

// Get current subsong (1-based index)
console.log(`Currently playing subsong ${player.currentSubsong()}`);

// Switch to a different subsong (1-based index)
player.setSubsong(2); // Play subsong 2
```

## Visualization

```javascript
// Get raw PSG register values (16 bytes)
const registers = player.get_registers();

// Get rich channel state data for visualization
const states = player.getChannelStates();

// Each channel has detailed info:
states.channels.forEach((ch, i) => {
  console.log(`Channel ${i}:`);
  console.log(`  Frequency: ${ch.frequency} Hz`);
  console.log(`  Note: ${ch.note}`);           // e.g., "A4", "C#5"
  console.log(`  Amplitude: ${ch.amplitude}`);  // 0.0 - 1.0
  console.log(`  Tone: ${ch.toneEnabled}`);
  console.log(`  Noise: ${ch.noiseEnabled}`);
  console.log(`  Envelope: ${ch.envelopeEnabled}`);
});

// Envelope info
console.log(`Envelope period: ${states.envelope.period}`);
console.log(`Envelope shape: ${states.envelope.shape}`);
console.log(`Envelope name: ${states.envelope.shapeName}`); // e.g., "/\\/\\"
```

## API Reference

### `init(): Promise<void>`

Initialize the WASM module. Must be called once before creating players.

### `Ym2149Player`

```typescript
class Ym2149Player {
  constructor(data: Uint8Array);

  // Metadata
  readonly metadata: YmMetadata;

  // Playback Control
  play(): void;
  pause(): void;
  stop(): void;
  restart(): void;
  is_playing(): boolean;
  state(): string;  // "Playing", "Paused", "Stopped"

  // Volume (0.0 - 1.0)
  set_volume(volume: number): void;
  volume(): number;

  // Seeking
  seek_to_frame(frame: number): void;
  seek_to_percentage(pct: number): void;  // 0.0 - 1.0
  frame_position(): number;
  frame_count(): number;
  position_percentage(): number;

  // Channel Muting (0 = A, 1 = B, 2 = C)
  set_channel_mute(channel: number, mute: boolean): void;
  is_channel_muted(channel: number): boolean;

  // Audio Generation
  generateSamples(count: number): Float32Array;
  generateSamplesInto(buffer: Float32Array): void;  // Zero-allocation

  // Visualization
  get_registers(): Uint8Array;        // 16 bytes of PSG registers
  getChannelStates(): ChannelStates;  // Rich channel data

  // Multi-Subsong (SNDH)
  subsongCount(): number;             // Number of subsongs (1 for most formats)
  currentSubsong(): number;           // Current subsong (1-based)
  setSubsong(index: number): boolean; // Switch subsong (1-based)

  // Effects
  set_color_filter(enabled: boolean): void;  // ST color filter emulation
}
```

### `YmMetadata`

```typescript
interface YmMetadata {
  title: string;
  author: string;
  comments: string;
  format: string;           // "YM5", "YM6", "AKS", "SNDH", "AY"
  frame_count: number;
  frame_rate: number;       // Usually 50 (PAL) or 60 (NTSC)
  duration_seconds: number;
}
```

### `ChannelStates`

```typescript
interface ChannelStates {
  channels: Array<{
    frequency: number;      // Frequency in Hz
    note: string;           // Note name (e.g., "A4", "C#5", "--")
    amplitude: number;      // Normalized amplitude (0.0 - 1.0)
    toneEnabled: boolean;
    noiseEnabled: boolean;
    envelopeEnabled: boolean;
  }>;
  envelope: {
    period: number;
    shape: number;
    shapeName: string;      // Visual representation (e.g., "/\\/\\")
  };
}
```

## Supported Formats

| Format | Extension | Description |
|--------|-----------|-------------|
| SNDH | `.sndh` | Atari ST native format (68000 CPU emulation, multi-subsong) |
| YM | `.ym` | ST-Sound format (YM2-YM6) |
| AKS | `.aks` | Arkos Tracker 2 |
| AY | `.ay` | ZXAY/EMUL (Z80 CPU emulation) |

## Browser Support

- Chrome/Edge 90+
- Firefox 88+
- Safari 15+
- Mobile browsers (iOS Safari, Chrome Mobile)

## Bundle Size

| File | Size | Gzipped |
|------|------|---------|
| `ym2149_wasm.js` | ~15 KB | ~5 KB |
| `ym2149_wasm_bg.wasm` | ~280 KB | ~95 KB |

## Try It Live

Check out the **[Live Demo](https://ym2149-rs.org/demo/)** to hear it in action!

## Related

This is the WebAssembly build of the [ym2149-rs](https://github.com/slippyex/ym2149-rs) Rust ecosystem:

- [`ym2149`](https://crates.io/crates/ym2149) - Core YM2149 emulation (Rust)
- [`bevy_ym2149`](https://crates.io/crates/bevy_ym2149) - Bevy game engine integration

## Links

- **Website**: [https://ym2149-rs.org](https://ym2149-rs.org)
- **Live Demo**: [https://ym2149-rs.org/demo/](https://ym2149-rs.org/demo/)
- **GitHub**: [https://github.com/slippyex/ym2149-rs](https://github.com/slippyex/ym2149-rs)
- **Tutorials**: [https://ym2149-rs.org/tutorials.html](https://ym2149-rs.org/tutorials.html)
- **Downloads**: [https://ym2149-rs.org/downloads.html](https://ym2149-rs.org/downloads.html)

## Credits

- **Leonard/Oxygene (Arnaud Carré)** - [AtariAudio](https://github.com/arnaud-carre/sndh-player/tree/main/AtariAudio) reference implementation
- **Atari ST demoscene community** - Original music and SNDH archive

## License

MIT - See [LICENSE](https://github.com/slippyex/ym2149-rs/blob/main/LICENSE) for details.
