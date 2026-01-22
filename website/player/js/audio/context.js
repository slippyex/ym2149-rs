// ============================================================================
// Audio Context - AudioContext and ScriptProcessor management
// ============================================================================

import { SAMPLE_RATE, BUFFER_SIZE, WAVEFORM_SIZE, SPECTRUM_BINS, AUDIO_VIS_BUFFER_SIZE } from '../config.js';
import * as state from '../state.js';

// ============================================================================
// AudioContext Management
// ============================================================================

export async function ensureAudioContext() {
    if (!state.audioContext) {
        state.setAudioContext(new (window.AudioContext || window.webkitAudioContext)({ sampleRate: SAMPLE_RATE }));
    }
    if (state.audioContext.state === "suspended") {
        await state.audioContext.resume();
    }
    // iOS Safari needs a silent buffer played to fully unlock audio
    if (!state.audioUnlocked && state.audioContext.state === "running") {
        const silentBuffer = state.audioContext.createBuffer(1, 1, SAMPLE_RATE);
        const source = state.audioContext.createBufferSource();
        source.buffer = silentBuffer;
        source.connect(state.audioContext.destination);
        source.start(0);
        state.setAudioUnlocked(true);
    }
}

// ============================================================================
// Channel Arrays Initialization
// ============================================================================

export function initChannelArrays(count) {
    const channelWaveforms = [];
    const channelPhases = [];
    const channelSpectrums = [];
    const amplitudeHistory = [];
    const sidModeDetected = [];
    const noteHistories = [];

    for (let i = 0; i < count; i++) {
        channelWaveforms.push(new Float32Array(WAVEFORM_SIZE));
        channelPhases.push(0);
        channelSpectrums.push(new Float32Array(SPECTRUM_BINS));
        amplitudeHistory.push(new Float32Array(state.AMPLITUDE_HISTORY_SIZE || 64));
        sidModeDetected.push(false);
        noteHistories.push([]);
    }

    state.setChannelWaveforms(channelWaveforms);
    state.setChannelPhases(channelPhases);
    state.setChannelSpectrums(channelSpectrums);
    state.setAmplitudeHistory(amplitudeHistory);
    state.setSidModeDetected(sidModeDetected);
    state.setNoteHistories(noteHistories);
    state.setMonoWaveform(new Float32Array(WAVEFORM_SIZE));
    state.setCombinedSpectrum(new Float32Array(SPECTRUM_BINS));
    state.setNoteScrollOffset(0);

    // Initialize per-channel sample buffers
    initChannelSampleBuffers(count);
}

export function initChannelSampleBuffers(count) {
    const channelSampleBuffers = [];
    for (let i = 0; i < count; i++) {
        channelSampleBuffers.push(new Float32Array(AUDIO_VIS_BUFFER_SIZE));
    }
    state.setChannelSampleBuffers(channelSampleBuffers);
    state.setChannelSampleWritePos(0);
    state.setCurrentChannelCount(count);
}

// ============================================================================
// Audio Processing
// ============================================================================

export function startAudioProcessing() {
    if (state.scriptProcessor) return;

    const processor = state.audioContext.createScriptProcessor(BUFFER_SIZE, 1, 1);
    processor.onaudioprocess = (e) => {
        const output = e.outputBuffer.getChannelData(0);
        if (!state.isPlaying || !state.wasmPlayer) {
            output.fill(0);
            return;
        }

        // For playback speed, generate more/fewer samples and resample
        const neededSamples = Math.ceil(output.length * state.playbackSpeed);

        // Ensure we have enough samples in the buffer
        let speedResampleBuffer = state.speedResampleBuffer;
        let speedResamplePos = state.speedResamplePos;

        while (speedResampleBuffer.length - speedResamplePos < neededSamples) {
            // Use generateSamplesWithChannels to get both mono and per-channel outputs
            const result = state.wasmPlayer.generateSamplesWithChannels(BUFFER_SIZE);
            const newSamples = result.mono;
            const channelData = result.channels;
            const chCount = result.channelCount;

            // Initialize per-channel buffers if needed
            if (state.channelSampleBuffers.length !== chCount) {
                initChannelSampleBuffers(chCount);
            }

            // Copy all samples to visualization ring buffers
            let audioSampleWritePos = state.audioSampleWritePos;
            let channelSampleWritePos = state.channelSampleWritePos;

            for (let i = 0; i < newSamples.length; i++) {
                // Mono buffer
                state.audioSampleBuffer[audioSampleWritePos] = newSamples[i];

                // Per-channel buffers
                for (let ch = 0; ch < chCount; ch++) {
                    state.channelSampleBuffers[ch][channelSampleWritePos] = channelData[i * chCount + ch];
                }

                audioSampleWritePos = (audioSampleWritePos + 1) % AUDIO_VIS_BUFFER_SIZE;
                channelSampleWritePos = (channelSampleWritePos + 1) % AUDIO_VIS_BUFFER_SIZE;
            }

            state.setAudioSampleWritePos(audioSampleWritePos);
            state.setChannelSampleWritePos(channelSampleWritePos);

            const newBuffer = new Float32Array(
                speedResampleBuffer.length - speedResamplePos + newSamples.length,
            );
            newBuffer.set(speedResampleBuffer.subarray(speedResamplePos));
            newBuffer.set(newSamples, speedResampleBuffer.length - speedResamplePos);
            speedResampleBuffer = newBuffer;
            speedResamplePos = 0;
        }

        // Resample to output
        for (let i = 0; i < output.length; i++) {
            const srcPos = speedResamplePos + i * state.playbackSpeed;
            const srcIdx = Math.floor(srcPos);
            const frac = srcPos - srcIdx;
            if (srcIdx + 1 < speedResampleBuffer.length) {
                // Linear interpolation
                output[i] = speedResampleBuffer[srcIdx] * (1 - frac) + speedResampleBuffer[srcIdx + 1] * frac;
            } else {
                output[i] = speedResampleBuffer[srcIdx] || 0;
            }
        }
        speedResamplePos += Math.floor(output.length * state.playbackSpeed);

        // Trim buffer if it gets too large
        if (speedResamplePos > BUFFER_SIZE * 4) {
            speedResampleBuffer = speedResampleBuffer.subarray(speedResamplePos);
            speedResamplePos = 0;
        }

        state.setSpeedResampleBuffer(speedResampleBuffer);
        state.setSpeedResamplePos(speedResamplePos);
    };

    // MediaStream approach for iOS/mobile
    try {
        const mediaStreamDest = state.audioContext.createMediaStreamDestination();
        processor.connect(mediaStreamDest);
        const audioElement = new Audio();
        audioElement.srcObject = mediaStreamDest.stream;
        audioElement.play().catch(() => {
            // Fallback to direct connection
            processor.disconnect();
            processor.connect(state.audioContext.destination);
        });
        state.setMediaStreamDest(mediaStreamDest);
        state.setAudioElement(audioElement);
    } catch (e) {
        // Fallback for browsers without MediaStream support
        processor.connect(state.audioContext.destination);
    }

    state.setScriptProcessor(processor);
}

export function stopAudioProcessing() {
    // Reset speed resample buffer
    state.setSpeedResampleBuffer(new Float32Array(0));
    state.setSpeedResamplePos(0);

    if (state.scriptProcessor) {
        state.scriptProcessor.disconnect();
        state.setScriptProcessor(null);
    }
    if (state.audioElement) {
        state.audioElement.pause();
        state.audioElement.srcObject = null;
        state.setAudioElement(null);
    }
    state.setMediaStreamDest(null);
}
