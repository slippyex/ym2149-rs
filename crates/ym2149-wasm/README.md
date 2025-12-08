# ym2149-wasm

 WebAssembly bindings for the YM2149 PSG emulator - play YM chiptunes, Arkos Tracker projects, and Project AY rips directly in your browser!

## Features

- 🎵 Play YM2–YM6, Arkos Tracker `.aks`, and ZXAY/EMUL `.ay` files in the browser
- 🎮 Full playback control (play, pause, stop, seek)
- 🔊 Volume control and channel muting
- 📊 Real-time waveform data for visualizations
- 📝 Metadata extraction (title, author, comments)
- ⚡ High-performance cycle-accurate emulation
- 🎨 Web Audio API integration

## Installation

### Build from Source

```bash
# Install wasm-pack
curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh

# Build the WASM module
cd crates/ym2149-wasm
wasm-pack build --target web --out-dir pkg

# Or for bundler (webpack, rollup, etc.)
wasm-pack build --target bundler --out-dir pkg

# Shortcut: rebuild + copy into examples/pkg
./scripts/build-wasm-examples.sh --release
```

## Quick Start

### Basic Usage

```javascript
import init, { Ym2149Player } from './ym2149_wasm.js';

async function playYmFile(fileData) {
    // Initialize WASM module
    await init();

    // Create player from YM/AKS/AY file data
    const player = new Ym2149Player(fileData);

    // Get metadata
    const metadata = player.metadata;
    console.log(`Playing: ${metadata.title} by ${metadata.author}`);
    console.log(`Duration: ${metadata.duration_seconds}s`);

    // Start playback
    player.play();

    // Generate audio samples for Web Audio API
    const sampleRate = 44100;
    const samplesPerFrame = 882; // At 50Hz frame rate
    const samples = player.generateSamples(samplesPerFrame);

    // Use samples with Web Audio API (see examples below)
}
```

### Web Audio API Integration

```javascript
import init, { Ym2149Player } from './ym2149_wasm.js';

class YmWebPlayer {
    constructor() {
        this.audioContext = null;
        this.player = null;
        this.isPlaying = false;
    }

    async init() {
        await init();
        this.audioContext = new AudioContext({ sampleRate: 44100 });
    }

    async loadFile(fileData) {
        this.player = new Ym2149Player(fileData);
        console.log('Loaded:', this.player.metadata.title);
    }

    play() {
        if (!this.player || this.isPlaying) return;

        this.isPlaying = true;
        this.player.play();
        this.scheduleNextBuffer();
    }

    pause() {
        this.isPlaying = false;
        if (this.player) this.player.pause();
    }

    scheduleNextBuffer() {
        if (!this.isPlaying) return;

        const samplesPerFrame = 882; // 44.1kHz at 50Hz
        const samples = this.player.generateSamples(samplesPerFrame);

        // Create AudioBuffer
        const buffer = this.audioContext.createBuffer(
            1, // mono
            samples.length,
            this.audioContext.sampleRate
        );

        // Fill buffer
        buffer.getChannelData(0).set(samples);

        // Create and schedule source
        const source = this.audioContext.createBufferSource();
        source.buffer = buffer;
        source.connect(this.audioContext.destination);
        source.start();

        // Schedule next buffer
        setTimeout(() => this.scheduleNextBuffer(), 20); // 50Hz = 20ms
    }
}

// Usage
const player = new YmWebPlayer();
await player.init();

// Load file from user input
const input = document.getElementById('file-input');
input.addEventListener('change', async (e) => {
    const file = e.target.files[0];
    const arrayBuffer = await file.arrayBuffer();
    const uint8Array = new Uint8Array(arrayBuffer);
    await player.loadFile(uint8Array);
    player.play();
});
```

### Playback Control

```javascript
// Play/Pause
if (player.is_playing()) {
    player.pause();
} else {
    player.play();
}

// Volume control (0.0 to 1.0)
player.set_volume(0.5);

// Seek to position
player.seek_to_percentage(0.5); // Seek to 50%
player.seek_to_frame(1000);     // Seek to frame 1000

// Channel muting (for karaoke-style playback)
player.set_channel_mute(0, true);  // Mute channel A
player.set_channel_mute(1, false); // Unmute channel B
player.set_channel_mute(2, false); // Unmute channel C

// Get playback position
console.log(`Position: ${player.position_percentage() * 100}%`);
console.log(`Frame: ${player.frame_position()} / ${player.frame_count()}`);
```

### Metadata Access

```javascript
const metadata = player.metadata;

console.log(`Title: ${metadata.title}`);
console.log(`Author: ${metadata.author}`);
console.log(`Comments: ${metadata.comments}`);
console.log(`Format: ${metadata.format}`);
console.log(`Frames: ${metadata.frame_count}`);
console.log(`Frame Rate: ${metadata.frame_rate} Hz`);
console.log(`Duration: ${metadata.duration_seconds} seconds`);
```

### Visualization

```javascript
// Get current register values for visualization
const registers = player.get_registers(); // Returns Uint8Array[16]

// Register layout:
// R0-R1:   Channel A period
// R2-R3:   Channel B period
// R4-R5:   Channel C period
// R6:      Noise period
// R7:      Mixer control
// R8-R10:  Channel volumes
// R11-R12: Envelope period
// R13:     Envelope shape
// R14-R15: I/O ports

// Calculate frequencies
const channelAPeriod = registers[0] | (registers[1] << 8);
const frequencyA = 2000000 / (16 * channelAPeriod); // Master clock / (16 * period)

// Draw waveform visualization
function drawWaveform(samples, canvas) {
    const ctx = canvas.getContext('2d');
    const width = canvas.width;
    const height = canvas.height;

    ctx.clearRect(0, 0, width, height);
    ctx.strokeStyle = '#00ff00';
    ctx.beginPath();

    for (let i = 0; i < samples.length; i++) {
        const x = (i / samples.length) * width;
        const y = ((samples[i] + 1) / 2) * height; // Normalize -1..1 to 0..height
        if (i === 0) ctx.moveTo(x, y);
        else ctx.lineTo(x, y);
    }

    ctx.stroke();
}
```

## API Reference

### `Ym2149Player`

#### Constructor

```typescript
constructor(data: Uint8Array): Ym2149Player
```

Creates a new player from YM file data.

#### Properties

- `metadata: YmMetadata` - Song metadata (read-only)

#### Methods

- `play(): void` - Start playback
- `pause(): void` - Pause playback
- `stop(): void` - Stop and reset to beginning
- `restart(): void` - Restart from beginning
- `is_playing(): boolean` - Check if currently playing
- `state(): string` - Get playback state as string

**Volume Control:**
- `set_volume(volume: number): void` - Set volume (0.0-1.0)
- `volume(): number` - Get current volume

**Seeking:**
- `seek_to_frame(frame: number): void` - Seek to specific frame
- `seek_to_percentage(percentage: number): void` - Seek to percentage (0.0-1.0)
- `frame_position(): number` - Get current frame
- `frame_count(): number` - Get total frames
- `position_percentage(): number` - Get position as percentage

**Channel Control:**
- `set_channel_mute(channel: number, mute: boolean): void` - Mute/unmute channel (0-2)
- `is_channel_muted(channel: number): boolean` - Check if channel is muted

**Audio Generation:**
- `generateSamples(count: number): Float32Array` - Generate audio samples
- `generateSamplesInto(buffer: Float32Array): void` - Generate into buffer (zero-alloc)

**Visualization:**
- `get_registers(): Uint8Array` - Get current PSG register values (16 bytes)

**Effects:**
- `set_color_filter(enabled: boolean): void` - Enable/disable ST color filter

### `YmMetadata`

```typescript
interface YmMetadata {
    title: string;          // Song title
    author: string;         // Composer/author
    comments: string;       // Song comments
    format: string;         // YM format version (e.g., "YM6")
    frame_count: number;    // Total frames
    frame_rate: number;     // Frame rate in Hz (typically 50)
    duration_seconds: number; // Duration in seconds
}
```

## Examples

See the `examples/` directory for complete working examples:

- `simple-player.html` - Minimal web player
- `advanced-player.html` - Full-featured player with UI
- `visualizer.html` - Player with oscilloscope and spectrum analyzer
- `bundler-example/` - Example using webpack/rollup

## Performance

The WASM module is highly optimized:

- ⚡ ~6ns per emulator clock cycle
- 🎵 Real-time generation of 44.1kHz audio
- 📦 Small bundle size (~100KB gzipped)
- 🔋 Minimal CPU usage (<1% on modern hardware)

## Browser Support

Works in all modern browsers that support:
- WebAssembly
- Web Audio API
- ES6 Modules (or use a bundler)

Tested on:
- ✅ Chrome/Edge 90+
- ✅ Firefox 88+
- ✅ Safari 15+
- ✅ Mobile browsers (iOS Safari, Chrome Mobile)

## Building

### Development Build

```bash
wasm-pack build --dev --target web
```

### Production Build

```bash
wasm-pack build --release --target web
```

### Build Options

```bash
# Target web (ES modules)
wasm-pack build --target web

# Target bundler (webpack, rollup, parcel)
wasm-pack build --target bundler

# Target Node.js
wasm-pack build --target nodejs

# With features
wasm-pack build --features effects,tracker,digidrums
```

## License

MIT - See main repository for details.

## Links

- [Main Repository](https://github.com/slippyex/ym2149-rs)
- [Documentation](https://docs.rs/ym2149)
- [NPM Package](https://www.npmjs.com/package/ym2149-wasm) (coming soon)
- [Examples](../../examples/web-player/)
