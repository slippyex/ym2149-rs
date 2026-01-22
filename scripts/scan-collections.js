#!/usr/bin/env node
/**
 * Scan music collections and generate a JSON catalog for the web player
 * Includes pre-rendered waveform data and audio fingerprints
 *
 * Usage:
 *   node scripts/scan-collections.js [--no-waveforms]
 *
 * Output:
 *   website/demo/catalog.json
 */

const fs = require('fs');
const path = require('path');

const DEMO_DIR = path.join(__dirname, '..', 'website', 'demo');
const OUTPUT_FILE = path.join(DEMO_DIR, 'catalog.json');
const WASM_PATH = path.join(__dirname, '..', 'crates', 'ym2149-wasm', 'pkg');

// Waveform generation settings
const WAVEFORM_BARS = 150; // Number of peaks to store
const SCAN_SECONDS = 15;   // Scan first 15 seconds
const SAMPLE_RATE = 44100;

// Skip waveform generation with --no-waveforms flag
const GENERATE_WAVEFORMS = !process.argv.includes('--no-waveforms');

// Collection configurations
const COLLECTIONS = [
    {
        id: 'sndh',
        name: 'SNDH Collection',
        description: 'Atari ST/STE music from the SNDH archive',
        path: 'sndh_lf_2026',
        extensions: ['.sndh'],
        format: 'SNDH'
    },
    {
        id: 'ym',
        name: 'YM Collection',
        description: 'YM format chiptunes',
        path: 'ym',
        extensions: ['.ym'],
        format: 'YM'
    },
    {
        id: 'ay',
        name: 'Project AY',
        description: 'ZX Spectrum AY music',
        path: 'ProjectAY',
        extensions: ['.ay'],
        format: 'AY'
    },
    {
        id: 'arkos',
        name: 'Arkos Tracker',
        description: 'Arkos Tracker 2 songs',
        path: 'arkos',
        extensions: ['.aks'],
        format: 'AKS'
    }
];

/**
 * Read null-terminated string from buffer
 */
function readNullTerminatedString(buffer, offset) {
    let end = offset;
    while (end < buffer.length && buffer[end] !== 0) {
        end++;
    }
    return buffer.slice(offset, end).toString('latin1').trim();
}

/**
 * Parse SNDH header for metadata
 */
function parseSndhMetadata(buffer) {
    const meta = {
        title: null,
        author: null,
        year: null,
        subsongs: 1
    };

    // Check for SNDH magic
    if (buffer.length < 12) return meta;
    const magic = buffer.slice(0, 4).toString('ascii');
    if (magic !== 'SNDH') return meta;

    // Parse tags
    let pos = 4;
    const headerEnd = Math.min(buffer.length, 2048); // Limit header search

    while (pos < headerEnd - 4) {
        const tag = buffer.slice(pos, pos + 4).toString('ascii');

        if (tag === 'HDNS') break;

        if (tag === 'TITL') {
            pos += 4;
            meta.title = readNullTerminatedString(buffer, pos);
            pos += meta.title.length + 1;
            continue;
        }

        if (tag === 'COMM') {
            pos += 4;
            meta.author = readNullTerminatedString(buffer, pos);
            pos += meta.author.length + 1;
            continue;
        }

        if (tag === 'YEAR') {
            pos += 4;
            meta.year = readNullTerminatedString(buffer, pos);
            pos += (meta.year?.length || 0) + 1;
            continue;
        }

        if (tag === '##' + String.fromCharCode(buffer[pos + 2]) + String.fromCharCode(buffer[pos + 3])) {
            // Subsong count tag ##XX
            const countStr = buffer.slice(pos + 2, pos + 4).toString('ascii');
            const count = parseInt(countStr, 10);
            if (!isNaN(count)) meta.subsongs = count;
            pos += 4;
            continue;
        }

        // Skip unknown tags - move forward byte by byte
        pos++;
    }

    return meta;
}

/**
 * Parse YM file header for metadata
 * YM files can be LHA compressed
 */
function parseYmMetadata(buffer) {
    const meta = {
        title: null,
        author: null,
        year: null,
        subsongs: 1
    };

    // Check for LHA compression
    let data = buffer;
    if (buffer.length > 2) {
        const lhaId = buffer.slice(2, 5).toString('ascii');
        if (lhaId.startsWith('-lh')) {
            // LHA compressed - we can't easily decompress in Node without external libs
            // Just use filename for metadata
            return meta;
        }
    }

    // Check for YM magic
    const magic = data.slice(0, 4).toString('ascii');
    if (!magic.startsWith('YM')) return meta;

    // YM5/YM6 format has metadata after the header
    // This is simplified - full parsing would need more work
    if (magic === 'YM5!' || magic === 'YM6!') {
        // Skip to end of frame data (requires knowing frame count)
        // For now, just return basic info
        return meta;
    }

    return meta;
}

/**
 * Parse AY file header for metadata
 */
function parseAyMetadata(buffer) {
    const meta = {
        title: null,
        author: null,
        year: null,
        subsongs: 1
    };

    // Check for ZXAYEMUL magic
    if (buffer.length < 8) return meta;
    const magic = buffer.slice(0, 8).toString('ascii');
    if (magic !== 'ZXAYEMUL') return meta;

    // AY format structure:
    // 0-7: ZXAYEMUL
    // 8: File version
    // 9: Player version
    // 10-11: Pointer to author string (big-endian, relative to byte 12)
    // 12-13: Pointer to misc string (big-endian, relative to byte 12)
    // 14: Number of songs - 1
    // ...

    if (buffer.length < 15) return meta;

    meta.subsongs = buffer[14] + 1;

    // Read author pointer and string
    const authorOffset = (buffer[10] << 8) | buffer[11];
    if (authorOffset > 0 && 12 + authorOffset < buffer.length) {
        meta.author = readNullTerminatedString(buffer, 12 + authorOffset);
    }

    // Read misc pointer (often contains title info)
    const miscOffset = (buffer[12] << 8) | buffer[13];
    if (miscOffset > 0 && 12 + miscOffset < buffer.length) {
        meta.title = readNullTerminatedString(buffer, 12 + miscOffset);
    }

    return meta;
}

/**
 * Extract metadata from a music file
 */
function extractMetadata(filePath, ext) {
    try {
        const buffer = fs.readFileSync(filePath);

        switch (ext.toLowerCase()) {
            case '.sndh':
                return parseSndhMetadata(buffer);
            case '.ym':
                return parseYmMetadata(buffer);
            case '.ay':
                return parseAyMetadata(buffer);
            default:
                return { title: null, author: null, year: null, subsongs: 1 };
        }
    } catch (err) {
        console.error(`Error reading ${filePath}: ${err.message}`);
        return { title: null, author: null, year: null, subsongs: 1 };
    }
}

/**
 * Clean up filename to create display name
 */
function filenameToTitle(filename) {
    return filename
        .replace(/\.[^.]+$/, '')  // Remove extension
        .replace(/_/g, ' ')       // Replace underscores with spaces
        .replace(/-/g, ' - ')     // Add spaces around dashes
        .replace(/\s+/g, ' ')     // Collapse multiple spaces
        .trim();
}

/**
 * Scan a collection directory
 */
function scanCollection(collection) {
    const collectionPath = path.join(DEMO_DIR, collection.path);
    const tracks = [];

    if (!fs.existsSync(collectionPath)) {
        console.warn(`Collection path not found: ${collectionPath}`);
        return tracks;
    }

    function scanDir(dir, artistHint = null) {
        const entries = fs.readdirSync(dir, { withFileTypes: true });

        for (const entry of entries) {
            const fullPath = path.join(dir, entry.name);

            if (entry.isDirectory()) {
                // Use directory name as artist hint
                scanDir(fullPath, entry.name);
            } else if (entry.isFile()) {
                const ext = path.extname(entry.name).toLowerCase();
                if (collection.extensions.includes(ext)) {
                    // Get relative path from demo dir
                    const relativePath = path.relative(DEMO_DIR, fullPath);

                    // Extract metadata
                    const meta = extractMetadata(fullPath, ext);

                    // Build track entry
                    const track = {
                        path: relativePath,
                        title: meta.title || filenameToTitle(entry.name),
                        author: meta.author || artistHint || 'Unknown',
                        format: collection.format
                    };

                    if (meta.year) track.year = meta.year;
                    if (meta.subsongs > 1) track.subsongs = meta.subsongs;

                    tracks.push(track);
                }
            }
        }
    }

    console.log(`Scanning ${collection.name}...`);
    scanDir(collectionPath);
    console.log(`  Found ${tracks.length} tracks`);

    return tracks;
}

// ============================================================================
// WASM / Waveform Generation
// ============================================================================

let wasmModule = null;
let Ym2149Player = null;

/**
 * Initialize WASM module for waveform generation
 */
async function initWasm() {
    if (!GENERATE_WAVEFORMS) return false;

    try {
        // Dynamic import for ES module
        const wasmJs = await import(path.join(WASM_PATH, 'ym2149_wasm.js'));
        const wasmBuffer = fs.readFileSync(path.join(WASM_PATH, 'ym2149_wasm_bg.wasm'));

        // Initialize WASM synchronously with buffer
        wasmModule = wasmJs.initSync({ module: wasmBuffer });
        Ym2149Player = wasmJs.Ym2149Player;

        console.log('WASM module loaded for waveform generation\n');
        return true;
    } catch (err) {
        console.warn(`Could not load WASM module: ${err.message}`);
        console.warn('Continuing without waveform generation...\n');
        return false;
    }
}

/**
 * Generate waveform peaks and fingerprint for a track
 */
function generateWaveformData(filePath) {
    if (!Ym2149Player) return null;

    try {
        const fileData = fs.readFileSync(filePath);
        const player = new Ym2149Player(new Uint8Array(fileData));
        player.play();

        const duration = player.metadata?.duration_seconds || 180;
        const scanDuration = Math.min(SCAN_SECONDS, duration);
        const totalSamples = Math.floor(scanDuration * SAMPLE_RATE);
        const samplesPerBar = Math.floor(totalSamples / WAVEFORM_BARS);

        const peaks = [];
        let totalAmp = 0;
        let prevSample = 0;
        let zeroCrossings = 0;

        for (let bar = 0; bar < WAVEFORM_BARS; bar++) {
            const chunk = player.generateSamples(samplesPerBar);

            let maxPeak = 0;
            for (let i = 0; i < chunk.length; i += 4) {
                const sample = chunk[i];
                const abs = Math.abs(sample);
                if (abs > maxPeak) maxPeak = abs;
                totalAmp += abs;

                if ((prevSample < 0 && sample >= 0) || (prevSample >= 0 && sample < 0)) {
                    zeroCrossings++;
                }
                prevSample = sample;
            }
            // Normalize to 0-255 for compact storage
            peaks.push(Math.min(255, Math.round(maxPeak * 255)));
        }

        // Calculate fingerprint
        const avgAmp = totalAmp / (WAVEFORM_BARS * samplesPerBar / 4);
        const noteDensity = zeroCrossings / scanDuration;

        return {
            // Waveform as base64-encoded bytes (compact)
            waveform: Buffer.from(peaks).toString('base64'),
            // Simple fingerprint for similarity matching
            fp: {
                amp: Math.round(avgAmp * 1000) / 1000,
                density: Math.round(noteDensity)
            }
        };
    } catch (err) {
        // Silently skip tracks that fail to load
        return null;
    }
}

/**
 * Main function
 */
async function main() {
    console.log('YM2149-rs Collection Scanner\n');

    // Initialize WASM for waveform generation
    const wasmReady = await initWasm();

    const catalog = {
        version: '1.1', // Bumped version for waveform support
        generated: new Date().toISOString(),
        collections: [],
        tracks: []
    };

    for (const collection of COLLECTIONS) {
        const tracks = scanCollection(collection);

        catalog.collections.push({
            id: collection.id,
            name: collection.name,
            description: collection.description,
            format: collection.format,
            trackCount: tracks.length
        });

        // Add collection id to each track
        for (const track of tracks) {
            track.collection = collection.id;
            catalog.tracks.push(track);
        }
    }

    // Generate waveforms if WASM is available
    if (wasmReady) {
        console.log('\nGenerating waveforms and fingerprints...');
        let processed = 0;
        let failed = 0;

        for (const track of catalog.tracks) {
            const fullPath = path.join(DEMO_DIR, track.path);
            const waveData = generateWaveformData(fullPath);

            if (waveData) {
                track.w = waveData.waveform;  // Compact key for waveform
                track.fp = waveData.fp;       // Fingerprint
                processed++;
            } else {
                failed++;
            }

            // Progress indicator
            if ((processed + failed) % 100 === 0) {
                process.stdout.write(`\r  Processed ${processed + failed}/${catalog.tracks.length} tracks...`);
            }
        }
        console.log(`\r  Generated ${processed} waveforms (${failed} skipped)          `);
    }

    // Sort tracks by author, then title
    catalog.tracks.sort((a, b) => {
        const authorCmp = (a.author || '').localeCompare(b.author || '');
        if (authorCmp !== 0) return authorCmp;
        return (a.title || '').localeCompare(b.title || '');
    });

    // Write catalog
    const json = JSON.stringify(catalog, null, 2);
    fs.writeFileSync(OUTPUT_FILE, json);

    console.log(`\nCatalog written to ${OUTPUT_FILE}`);
    console.log(`Total: ${catalog.tracks.length} tracks in ${catalog.collections.length} collections`);

    // Also write a minified version for production
    const minifiedPath = OUTPUT_FILE.replace('.json', '.min.json');
    fs.writeFileSync(minifiedPath, JSON.stringify(catalog));
    console.log(`Minified: ${minifiedPath} (${(fs.statSync(minifiedPath).size / 1024).toFixed(1)} KB)`);
}

main().catch(err => {
    console.error('Fatal error:', err);
    process.exit(1);
});
