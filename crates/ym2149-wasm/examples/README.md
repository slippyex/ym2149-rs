# YM2149 WASM Web Player - Testing Guide

## Quick Start

### Option 1: Using the provided script (Recommended)

```bash
cd crates/ym2149-wasm/examples
./start-server.sh
```

Then open your browser to: **http://localhost:8000/simple-player.html**

### Option 2: Manual server start

```bash
cd crates/ym2149-wasm/examples
python3 -m http.server 8000
```

Then open: **http://localhost:8000/simple-player.html**

### Option 3: Using npx (if you have Node.js)

```bash
cd crates/ym2149-wasm/examples
npx serve -p 8000
```

Then open: **http://localhost:8000/simple-player.html**

## What to Test

1. **Load a YM/AKS file:**
   - Click "📁 Load YM / AKS File"
   - Select any `.ym` or `.aks` file from the examples directory
   - Test files included:
     - `Ashtray.ym`
     - `Credits.ym`
     - `ND-Toxygene.ym`
     - `Prelude.ym`
     - `Scout.ym`
     - `Steps.ym`
     - `Doclands-TinyThings.aks`
     - `Digitest.aks`

2. **Playback Controls:**
   - ▶️ Play - Start playback
   - ⏸️ Pause - Pause playback
   - ⏹️ Stop - Stop and reset
   - 🔄 Restart - Restart from beginning

3. **Metadata Display:**
   - Song title
   - Author
   - Format (YM2, YM3, YM5, YM6)
   - Duration
   - Frame count

4. **Volume Control:**
   - Drag the volume slider (0-100%)

5. **Channel Muting:**
   - Click "Mute" on Channel A, B, or C
   - Click again to unmute

6. **Progress Bar:**
   - Click anywhere on the progress bar to seek (Note: seeking not yet implemented, but the bar shows position)

7. **Console Log:**
   - Watch the log at the bottom for debug information

## Troubleshooting

### CORS Errors

If you see CORS errors in the browser console, make sure you're:
1. Using a local web server (not opening the HTML file directly with `file://`)
2. All files (HTML, JS, WASM) are in the same directory

### WASM Module Not Loading

Check the browser console for errors. Common issues:
- Missing `.wasm` file in the examples directory
- Wrong path in import statement
- Browser doesn't support WASM (unlikely on modern browsers)

### No Sound

Make sure:
- Your browser allows audio playback (some browsers require user interaction first)
- Volume is not at 0
- Browser tab is not muted
- System volume is up

## Browser Compatibility

Tested and working on:
- ✅ Chrome/Edge 90+
- ✅ Firefox 88+
- ✅ Safari 15+
- ✅ Mobile browsers (iOS Safari, Chrome Mobile)

## Development

### Rebuild WASM module

```bash
cd crates/ym2149-wasm
wasm-pack build --target web
```

### Copy updated files to examples

```bash
cp pkg/ym2149_wasm* examples/
```

> If the browser still shows the old UI (e.g., no AKS entries), ensure
> the freshly built files from `pkg/` replaced the ones under
> `examples/pkg/` before refreshing the page.

#### Shortcut

From the repo root you can run:

```bash
scripts/build-wasm-examples.sh --release
```

This script runs `wasm-pack`, copies the `pkg/` folder into
`crates/ym2149-wasm/examples/pkg/`, and prints a reminder to reload the
page.

### Re-test

Refresh your browser (Ctrl+F5 or Cmd+Shift+R for hard reload)

## Technical Details

- **WASM Size:** ~124 KB (uncompressed)
- **Sample Rate:** 44.1 kHz
- **Frame Rate:** 50 Hz (VBL sync)
- **Samples per Frame:** 882
- **Audio Buffer:** ~20ms chunks

## Performance

On modern hardware:
- CPU Usage: <1%
- Memory Usage: ~2-5 MB
- Latency: ~20-40ms

## Known Limitations

1. **Seeking:** Not yet implemented (progress bar shows position but clicking doesn't seek)
2. **Loop Control:** Loops automatically, no UI toggle yet
3. **Visualization:** Waveform/spectrum not yet implemented (register access is available)
4. **Export:** Cannot export to WAV/MP3 from browser yet

## Next Steps

Want to improve the player? Check out:
- `/Users/markusvelten/workspaces/private/ym2149-rs/crates/ym2149-wasm/README.md` - Full API documentation
- `/Users/markusvelten/workspaces/private/ym2149-rs/crates/ym2149-wasm/src/lib.rs` - WASM source code
- `/Users/markusvelten/workspaces/private/ym2149-rs/crates/ym2149-wasm/examples/simple-player.html` - Web player source

## Reporting Issues

If you find bugs, please include:
- Browser and version
- YM file that caused the issue
- Console error messages
- Steps to reproduce
