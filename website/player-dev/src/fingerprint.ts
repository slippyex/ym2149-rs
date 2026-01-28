// ============================================================================
// Fingerprint - Audio fingerprinting and similarity matching
// ============================================================================

import * as state from './state.ts';
import type { Fingerprint, Track } from './types/index.ts';

// ============================================================================
// Fingerprint Access
// ============================================================================

export function loadFingerprints(): void {
  // No longer needed - fingerprints are in catalog
}

export function saveFingerprints(): void {
  // No longer needed - fingerprints are in catalog
}

export function saveFingerprint(_path: string, _fingerprint: Fingerprint): void {
  // No longer needed - fingerprints are pre-computed in catalog
}

export function getFingerprint(trackOrPath: Track | string): Fingerprint | null {
  // Get fingerprint from catalog track object
  if (typeof trackOrPath === 'object' && trackOrPath.fp) {
    return trackOrPath.fp;
  }
  // Lookup by path
  if (state.catalog) {
    const track = state.catalog.tracks.find((t) => t.path === trackOrPath);
    return track?.fp ?? null;
  }
  return null;
}

// ============================================================================
// Fingerprint Distance Calculation
// ============================================================================

export function calculateFingerprintDistance(fp1: Fingerprint, fp2: Fingerprint): number {
  if (!fp1 || !fp2) return Infinity;

  // === Basic features ===
  const ampDiff =
    Math.abs(fp1.amp - fp2.amp) /
    Math.max(fp1.amp, fp2.amp, 0.001);
  const densityDiff =
    Math.abs(fp1.density - fp2.density) /
    Math.max(fp1.density, fp2.density, 1);
  const varianceDiff = Math.abs((fp1.variance ?? 0) - (fp2.variance ?? 0));
  const punchDiff =
    Math.abs((fp1.punch ?? 1) - (fp2.punch ?? 1)) / 10;
  const brightnessDiff = Math.abs(
    (fp1.brightness ?? 0) - (fp2.brightness ?? 0),
  );

  // histogram: energy distribution (8 bins)
  let histDiff = 0;
  if (fp1.hist && fp2.hist) {
    for (let i = 0; i < 8; i++) {
      histDiff += Math.abs((fp1.hist[i] ?? 0) - (fp2.hist[i] ?? 0));
    }
    histDiff /= 8 * 255;
  }

  // sections: song structure (4 quarters)
  let sectionsDiff = 0;
  if (fp1.sections && fp2.sections) {
    for (let i = 0; i < 4; i++) {
      sectionsDiff += Math.abs(
        (fp1.sections[i] ?? 0) - (fp2.sections[i] ?? 0),
      );
    }
    sectionsDiff /= 4 * 255;
  }

  // tempo: BPM-like value
  let tempoDiff = 0;
  if (fp1.tempo && fp2.tempo) {
    tempoDiff =
      Math.abs(fp1.tempo - fp2.tempo) /
      Math.max(fp1.tempo, fp2.tempo, 1);
  }

  // === Spectral features ===
  // Spectral centroid: center of mass of spectrum (0-1)
  let centroidDiff = 0;
  if (fp1.centroid !== undefined && fp2.centroid !== undefined) {
    centroidDiff = Math.abs(fp1.centroid - fp2.centroid);
  }

  // Spectral flatness: tonal vs noise (0-1)
  let flatnessDiff = 0;
  if (fp1.flatness !== undefined && fp2.flatness !== undefined) {
    flatnessDiff = Math.abs(fp1.flatness - fp2.flatness);
  }

  // Spectral bands: bass/low-mid/high-mid/treble energy (4 bins, 0-255)
  let bandsDiff = 0;
  if (fp1.bands && fp2.bands) {
    for (let i = 0; i < 4; i++) {
      bandsDiff += Math.abs((fp1.bands[i] ?? 0) - (fp2.bands[i] ?? 0));
    }
    bandsDiff /= 4 * 255;
  }

  // Chroma: pitch class histogram (12 bins, 0-255)
  let chromaDiff = 0;
  if (fp1.chroma && fp2.chroma) {
    for (let i = 0; i < 12; i++) {
      chromaDiff += Math.abs((fp1.chroma[i] ?? 0) - (fp2.chroma[i] ?? 0));
    }
    chromaDiff /= 12 * 255;
  }

  // === Rhythm features ===
  let rhythmRegDiff = 0;
  if (fp1.rhythm_reg !== undefined && fp2.rhythm_reg !== undefined) {
    rhythmRegDiff = Math.abs(fp1.rhythm_reg - fp2.rhythm_reg);
  }

  let rhythmStrDiff = 0;
  if (fp1.rhythm_str !== undefined && fp2.rhythm_str !== undefined) {
    rhythmStrDiff = Math.abs(fp1.rhythm_str - fp2.rhythm_str);
  }

  // === MFCCs - Industry standard for timbre similarity ===
  let mfccDiff = 0;
  if (fp1.mfcc && fp2.mfcc && fp1.mfcc.length === 13 && fp2.mfcc.length === 13) {
    for (let i = 0; i < 13; i++) {
      mfccDiff += Math.abs((fp1.mfcc[i] ?? 0) - (fp2.mfcc[i] ?? 0));
    }
    mfccDiff /= 13 * 255;
  }

  // MFCC Deltas
  let mfccDeltaDiff = 0;
  if (fp1.mfcc_d && fp2.mfcc_d && fp1.mfcc_d.length === 13 && fp2.mfcc_d.length === 13) {
    for (let i = 0; i < 13; i++) {
      mfccDeltaDiff += Math.abs((fp1.mfcc_d[i] ?? 0) - (fp2.mfcc_d[i] ?? 0));
    }
    mfccDeltaDiff /= 13 * 255;
  }

  // MFCC Delta-Deltas
  let mfccDeltaDeltaDiff = 0;
  if (fp1.mfcc_dd && fp2.mfcc_dd && fp1.mfcc_dd.length === 13 && fp2.mfcc_dd.length === 13) {
    for (let i = 0; i < 13; i++) {
      mfccDeltaDeltaDiff += Math.abs((fp1.mfcc_dd[i] ?? 0) - (fp2.mfcc_dd[i] ?? 0));
    }
    mfccDeltaDeltaDiff /= 13 * 255;
  }

  // Chromagram - Melodic/harmonic progression over 8 time segments
  let chromagramDiff = 0;
  if (fp1.chromagram && fp2.chromagram && fp1.chromagram.length === 96 && fp2.chromagram.length === 96) {
    for (let i = 0; i < 96; i++) {
      chromagramDiff += Math.abs((fp1.chromagram[i] ?? 0) - (fp2.chromagram[i] ?? 0));
    }
    chromagramDiff /= 96 * 255;
  }

  // Weighted sum - comprehensive audio similarity
  return (
    ampDiff * 0.01 +
    densityDiff * 0.01 +
    varianceDiff * 0.01 +
    punchDiff * 0.01 +
    brightnessDiff * 0.02 +
    histDiff * 0.02 +
    sectionsDiff * 0.03 +
    tempoDiff * 0.05 +
    centroidDiff * 0.02 +
    flatnessDiff * 0.01 +
    bandsDiff * 0.03 +
    chromaDiff * 0.08 +
    rhythmRegDiff * 0.03 +
    rhythmStrDiff * 0.05 +
    mfccDiff * 0.2 +
    mfccDeltaDiff * 0.1 +
    mfccDeltaDeltaDiff * 0.07 +
    chromagramDiff * 0.25
  );
}

// ============================================================================
// Find Similar Tracks
// ============================================================================

export function findSimilarByFingerprint(currentPath: string, maxResults = 10): Track[] {
  const currentTrack = state.catalog?.tracks.find((t) => t.path === currentPath);
  const currentFp = currentTrack?.fp;
  if (!currentFp || !state.catalog) return [];

  const currentFormat = currentTrack.format;
  const currentAuthor = currentTrack.author?.toLowerCase();

  const scored: Array<{ track: Track; distance: number }> = [];
  for (const track of state.catalog.tracks) {
    if (track.path === currentPath) continue;
    if (!track.fp) continue;
    // Only compare within same format
    if (track.format !== currentFormat) continue;

    let distance = calculateFingerprintDistance(currentFp, track.fp);

    // Boost same-author tracks (reduce distance by 30%)
    if (currentAuthor && track.author?.toLowerCase() === currentAuthor) {
      distance *= 0.7;
    }

    scored.push({ track, distance });
  }

  scored.sort((a, b) => a.distance - b.distance);
  return scored.slice(0, maxResults).map((s) => s.track);
}
