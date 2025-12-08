let wasm;

let cachedUint8ArrayMemory0 = null;

function getUint8ArrayMemory0() {
    if (cachedUint8ArrayMemory0 === null || cachedUint8ArrayMemory0.byteLength === 0) {
        cachedUint8ArrayMemory0 = new Uint8Array(wasm.memory.buffer);
    }
    return cachedUint8ArrayMemory0;
}

function getArrayU8FromWasm0(ptr, len) {
    ptr = ptr >>> 0;
    return getUint8ArrayMemory0().subarray(ptr / 1, ptr / 1 + len);
}

let cachedTextDecoder = new TextDecoder('utf-8', { ignoreBOM: true, fatal: true });

cachedTextDecoder.decode();

const MAX_SAFARI_DECODE_BYTES = 2146435072;
let numBytesDecoded = 0;
function decodeText(ptr, len) {
    numBytesDecoded += len;
    if (numBytesDecoded >= MAX_SAFARI_DECODE_BYTES) {
        cachedTextDecoder = new TextDecoder('utf-8', { ignoreBOM: true, fatal: true });
        cachedTextDecoder.decode();
        numBytesDecoded = len;
    }
    return cachedTextDecoder.decode(getUint8ArrayMemory0().subarray(ptr, ptr + len));
}

function getStringFromWasm0(ptr, len) {
    ptr = ptr >>> 0;
    return decodeText(ptr, len);
}

function addToExternrefTable0(obj) {
    const idx = wasm.__externref_table_alloc();
    wasm.__wbindgen_externrefs.set(idx, obj);
    return idx;
}

function handleError(f, args) {
    try {
        return f.apply(this, args);
    } catch (e) {
        const idx = addToExternrefTable0(e);
        wasm.__wbindgen_exn_store(idx);
    }
}

let WASM_VECTOR_LEN = 0;

const cachedTextEncoder = new TextEncoder();

if (!('encodeInto' in cachedTextEncoder)) {
    cachedTextEncoder.encodeInto = function (arg, view) {
        const buf = cachedTextEncoder.encode(arg);
        view.set(buf);
        return {
            read: arg.length,
            written: buf.length
        };
    }
}

function passStringToWasm0(arg, malloc, realloc) {

    if (realloc === undefined) {
        const buf = cachedTextEncoder.encode(arg);
        const ptr = malloc(buf.length, 1) >>> 0;
        getUint8ArrayMemory0().subarray(ptr, ptr + buf.length).set(buf);
        WASM_VECTOR_LEN = buf.length;
        return ptr;
    }

    let len = arg.length;
    let ptr = malloc(len, 1) >>> 0;

    const mem = getUint8ArrayMemory0();

    let offset = 0;

    for (; offset < len; offset++) {
        const code = arg.charCodeAt(offset);
        if (code > 0x7F) break;
        mem[ptr + offset] = code;
    }

    if (offset !== len) {
        if (offset !== 0) {
            arg = arg.slice(offset);
        }
        ptr = realloc(ptr, len, len = offset + arg.length * 3, 1) >>> 0;
        const view = getUint8ArrayMemory0().subarray(ptr + offset, ptr + len);
        const ret = cachedTextEncoder.encodeInto(arg, view);

        offset += ret.written;
        ptr = realloc(ptr, len, offset, 1) >>> 0;
    }

    WASM_VECTOR_LEN = offset;
    return ptr;
}

let cachedDataViewMemory0 = null;

function getDataViewMemory0() {
    if (cachedDataViewMemory0 === null || cachedDataViewMemory0.buffer.detached === true || (cachedDataViewMemory0.buffer.detached === undefined && cachedDataViewMemory0.buffer !== wasm.memory.buffer)) {
        cachedDataViewMemory0 = new DataView(wasm.memory.buffer);
    }
    return cachedDataViewMemory0;
}

let cachedFloat32ArrayMemory0 = null;

function getFloat32ArrayMemory0() {
    if (cachedFloat32ArrayMemory0 === null || cachedFloat32ArrayMemory0.byteLength === 0) {
        cachedFloat32ArrayMemory0 = new Float32Array(wasm.memory.buffer);
    }
    return cachedFloat32ArrayMemory0;
}

function getArrayF32FromWasm0(ptr, len) {
    ptr = ptr >>> 0;
    return getFloat32ArrayMemory0().subarray(ptr / 4, ptr / 4 + len);
}

function passArrayF32ToWasm0(arg, malloc) {
    const ptr = malloc(arg.length * 4, 4) >>> 0;
    getFloat32ArrayMemory0().set(arg, ptr / 4);
    WASM_VECTOR_LEN = arg.length;
    return ptr;
}

function passArray8ToWasm0(arg, malloc) {
    const ptr = malloc(arg.length * 1, 1) >>> 0;
    getUint8ArrayMemory0().set(arg, ptr / 1);
    WASM_VECTOR_LEN = arg.length;
    return ptr;
}

function takeFromExternrefTable0(idx) {
    const value = wasm.__wbindgen_externrefs.get(idx);
    wasm.__externref_table_dealloc(idx);
    return value;
}
/**
 * Set panic hook for better error messages in the browser console.
 */
export function init_panic_hook() {
    wasm.init_panic_hook();
}

const Ym2149PlayerFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_ym2149player_free(ptr >>> 0, 1));
/**
 * Main YM2149 player for WebAssembly.
 *
 * This player handles YM/AKS/AY file playback in the browser, generating audio samples
 * that can be fed into the Web Audio API.
 */
export class Ym2149Player {

    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        Ym2149PlayerFinalization.unregister(this);
        return ptr;
    }

    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_ym2149player_free(ptr, 0);
    }
    /**
     * Get current playback state.
     * @returns {boolean}
     */
    is_playing() {
        const ret = wasm.ym2149player_is_playing(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * Set volume (0.0 to 1.0). Applied to generated samples.
     * @param {number} volume
     */
    set_volume(volume) {
        wasm.ym2149player_set_volume(this.__wbg_ptr, volume);
    }
    /**
     * Get total frame count.
     * @returns {number}
     */
    frame_count() {
        const ret = wasm.ym2149player_frame_count(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Set the current subsong (1-based index). Returns true on success.
     * @param {number} index
     * @returns {boolean}
     */
    setSubsong(index) {
        const ret = wasm.ym2149player_setSubsong(this.__wbg_ptr, index);
        return ret !== 0;
    }
    /**
     * Get the current register values (for visualization).
     * @returns {Uint8Array}
     */
    get_registers() {
        const ret = wasm.ym2149player_get_registers(this.__wbg_ptr);
        var v1 = getArrayU8FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 1, 1);
        return v1;
    }
    /**
     * Seek to a specific frame (silently ignored for Arkos/AY backends).
     * @param {number} frame
     */
    seek_to_frame(frame) {
        wasm.ym2149player_seek_to_frame(this.__wbg_ptr, frame);
    }
    /**
     * Get the number of subsongs (1 for most formats, >1 for multi-song SNDH files).
     * @returns {number}
     */
    subsongCount() {
        const ret = wasm.ym2149player_subsongCount(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Get current frame position.
     * @returns {number}
     */
    frame_position() {
        const ret = wasm.ym2149player_frame_position(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Get the current subsong index (1-based).
     * @returns {number}
     */
    currentSubsong() {
        const ret = wasm.ym2149player_currentSubsong(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Generate audio samples.
     *
     * Returns a Float32Array containing mono samples.
     * The number of samples generated depends on the sample rate and frame rate.
     *
     * For 44.1kHz at 50Hz frame rate: 882 samples per frame.
     * @param {number} count
     * @returns {Float32Array}
     */
    generateSamples(count) {
        const ret = wasm.ym2149player_generateSamples(this.__wbg_ptr, count);
        var v1 = getArrayF32FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 4, 4);
        return v1;
    }
    /**
     * Check if a channel is muted.
     * @param {number} channel
     * @returns {boolean}
     */
    is_channel_muted(channel) {
        const ret = wasm.ym2149player_is_channel_muted(this.__wbg_ptr, channel);
        return ret !== 0;
    }
    /**
     * Mute or unmute a channel (0-2).
     * @param {number} channel
     * @param {boolean} mute
     */
    set_channel_mute(channel, mute) {
        wasm.ym2149player_set_channel_mute(this.__wbg_ptr, channel, mute);
    }
    /**
     * Enable or disable the ST color filter.
     * @param {boolean} enabled
     */
    set_color_filter(enabled) {
        wasm.ym2149player_set_color_filter(this.__wbg_ptr, enabled);
    }
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
     * @returns {any}
     */
    getChannelStates() {
        const ret = wasm.ym2149player_getChannelStates(this.__wbg_ptr);
        return ret;
    }
    /**
     * Seek to a percentage of the song (0.0 to 1.0, silently ignored for Arkos/AY backends).
     * @param {number} percentage
     */
    seek_to_percentage(percentage) {
        wasm.ym2149player_seek_to_percentage(this.__wbg_ptr, percentage);
    }
    /**
     * Get playback position as percentage (0.0 to 1.0).
     * @returns {number}
     */
    position_percentage() {
        const ret = wasm.ym2149player_position_percentage(this.__wbg_ptr);
        return ret;
    }
    /**
     * Generate samples into a pre-allocated buffer (zero-allocation).
     *
     * This is more efficient than `generate_samples` as it reuses the same buffer.
     * @param {Float32Array} buffer
     */
    generateSamplesInto(buffer) {
        var ptr0 = passArrayF32ToWasm0(buffer, wasm.__wbindgen_malloc);
        var len0 = WASM_VECTOR_LEN;
        wasm.ym2149player_generateSamplesInto(this.__wbg_ptr, ptr0, len0, buffer);
    }
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
     * @param {Uint8Array} data
     */
    constructor(data) {
        const ptr0 = passArray8ToWasm0(data, wasm.__wbindgen_malloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.ym2149player_new(ptr0, len0);
        if (ret[2]) {
            throw takeFromExternrefTable0(ret[1]);
        }
        this.__wbg_ptr = ret[0] >>> 0;
        Ym2149PlayerFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
    /**
     * Start playback.
     */
    play() {
        wasm.ym2149player_play(this.__wbg_ptr);
    }
    /**
     * Stop playback and reset to beginning.
     */
    stop() {
        wasm.ym2149player_stop(this.__wbg_ptr);
    }
    /**
     * Pause playback.
     */
    pause() {
        wasm.ym2149player_pause(this.__wbg_ptr);
    }
    /**
     * Get current playback state as string.
     * @returns {string}
     */
    state() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.ym2149player_state(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * Get current volume (0.0 to 1.0).
     * @returns {number}
     */
    volume() {
        const ret = wasm.ym2149player_volume(this.__wbg_ptr);
        return ret;
    }
    /**
     * Restart playback from the beginning.
     */
    restart() {
        wasm.ym2149player_restart(this.__wbg_ptr);
    }
    /**
     * Get metadata about the loaded file.
     * @returns {YmMetadata}
     */
    get metadata() {
        const ret = wasm.ym2149player_metadata(this.__wbg_ptr);
        return YmMetadata.__wrap(ret);
    }
}
if (Symbol.dispose) Ym2149Player.prototype[Symbol.dispose] = Ym2149Player.prototype.free;

const YmMetadataFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_ymmetadata_free(ptr >>> 0, 1));
/**
 * YM file metadata exposed to JavaScript.
 */
export class YmMetadata {

    static __wrap(ptr) {
        ptr = ptr >>> 0;
        const obj = Object.create(YmMetadata.prototype);
        obj.__wbg_ptr = ptr;
        YmMetadataFinalization.register(obj, obj.__wbg_ptr, obj);
        return obj;
    }

    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        YmMetadataFinalization.unregister(this);
        return ptr;
    }

    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_ymmetadata_free(ptr, 0);
    }
    /**
     * Get frame rate in Hz.
     * @returns {number}
     */
    get frame_rate() {
        const ret = wasm.ymmetadata_frame_rate(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Get frame count.
     * @returns {number}
     */
    get frame_count() {
        const ret = wasm.ymmetadata_frame_count(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Get duration in seconds.
     * @returns {number}
     */
    get duration_seconds() {
        const ret = wasm.ymmetadata_duration_seconds(this.__wbg_ptr);
        return ret;
    }
    /**
     * Get the song title.
     * @returns {string}
     */
    get title() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.ymmetadata_title(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * Get the song author.
     * @returns {string}
     */
    get author() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.ymmetadata_author(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * Get the YM format version.
     * @returns {string}
     */
    get format() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.ymmetadata_format(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * Get the song comments.
     * @returns {string}
     */
    get comments() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.ymmetadata_comments(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
}
if (Symbol.dispose) YmMetadata.prototype[Symbol.dispose] = YmMetadata.prototype.free;

const EXPECTED_RESPONSE_TYPES = new Set(['basic', 'cors', 'default']);

async function __wbg_load(module, imports) {
    if (typeof Response === 'function' && module instanceof Response) {
        if (typeof WebAssembly.instantiateStreaming === 'function') {
            try {
                return await WebAssembly.instantiateStreaming(module, imports);

            } catch (e) {
                const validResponse = module.ok && EXPECTED_RESPONSE_TYPES.has(module.type);

                if (validResponse && module.headers.get('Content-Type') !== 'application/wasm') {
                    console.warn("`WebAssembly.instantiateStreaming` failed because your server does not serve Wasm with `application/wasm` MIME type. Falling back to `WebAssembly.instantiate` which is slower. Original error:\n", e);

                } else {
                    throw e;
                }
            }
        }

        const bytes = await module.arrayBuffer();
        return await WebAssembly.instantiate(bytes, imports);

    } else {
        const instance = await WebAssembly.instantiate(module, imports);

        if (instance instanceof WebAssembly.Instance) {
            return { instance, module };

        } else {
            return instance;
        }
    }
}

function __wbg_get_imports() {
    const imports = {};
    imports.wbg = {};
    imports.wbg.__wbg___wbindgen_copy_to_typed_array_33fbd71146904370 = function(arg0, arg1, arg2) {
        new Uint8Array(arg2.buffer, arg2.byteOffset, arg2.byteLength).set(getArrayU8FromWasm0(arg0, arg1));
    };
    imports.wbg.__wbg___wbindgen_throw_b855445ff6a94295 = function(arg0, arg1) {
        throw new Error(getStringFromWasm0(arg0, arg1));
    };
    imports.wbg.__wbg_error_7534b8e9a36f1ab4 = function(arg0, arg1) {
        let deferred0_0;
        let deferred0_1;
        try {
            deferred0_0 = arg0;
            deferred0_1 = arg1;
            console.error(getStringFromWasm0(arg0, arg1));
        } finally {
            wasm.__wbindgen_free(deferred0_0, deferred0_1, 1);
        }
    };
    imports.wbg.__wbg_log_8cec76766b8c0e33 = function(arg0) {
        console.log(arg0);
    };
    imports.wbg.__wbg_new_1acc0b6eea89d040 = function() {
        const ret = new Object();
        return ret;
    };
    imports.wbg.__wbg_new_8a6f238a6ece86ea = function() {
        const ret = new Error();
        return ret;
    };
    imports.wbg.__wbg_new_e17d9f43105b08be = function() {
        const ret = new Array();
        return ret;
    };
    imports.wbg.__wbg_push_df81a39d04db858c = function(arg0, arg1) {
        const ret = arg0.push(arg1);
        return ret;
    };
    imports.wbg.__wbg_set_c2abbebe8b9ebee1 = function() { return handleError(function (arg0, arg1, arg2) {
        const ret = Reflect.set(arg0, arg1, arg2);
        return ret;
    }, arguments) };
    imports.wbg.__wbg_stack_0ed75d68575b0f3c = function(arg0, arg1) {
        const ret = arg1.stack;
        const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
        getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
    };
    imports.wbg.__wbindgen_cast_2241b6af4c4b2941 = function(arg0, arg1) {
        // Cast intrinsic for `Ref(String) -> Externref`.
        const ret = getStringFromWasm0(arg0, arg1);
        return ret;
    };
    imports.wbg.__wbindgen_cast_d6cd19b81560fd6e = function(arg0) {
        // Cast intrinsic for `F64 -> Externref`.
        const ret = arg0;
        return ret;
    };
    imports.wbg.__wbindgen_init_externref_table = function() {
        const table = wasm.__wbindgen_externrefs;
        const offset = table.grow(4);
        table.set(0, undefined);
        table.set(offset + 0, undefined);
        table.set(offset + 1, null);
        table.set(offset + 2, true);
        table.set(offset + 3, false);
        ;
    };

    return imports;
}

function __wbg_finalize_init(instance, module) {
    wasm = instance.exports;
    __wbg_init.__wbindgen_wasm_module = module;
    cachedDataViewMemory0 = null;
    cachedFloat32ArrayMemory0 = null;
    cachedUint8ArrayMemory0 = null;


    wasm.__wbindgen_start();
    return wasm;
}

function initSync(module) {
    if (wasm !== undefined) return wasm;


    if (typeof module !== 'undefined') {
        if (Object.getPrototypeOf(module) === Object.prototype) {
            ({module} = module)
        } else {
            console.warn('using deprecated parameters for `initSync()`; pass a single object instead')
        }
    }

    const imports = __wbg_get_imports();

    if (!(module instanceof WebAssembly.Module)) {
        module = new WebAssembly.Module(module);
    }

    const instance = new WebAssembly.Instance(module, imports);

    return __wbg_finalize_init(instance, module);
}

async function __wbg_init(module_or_path) {
    if (wasm !== undefined) return wasm;


    if (typeof module_or_path !== 'undefined') {
        if (Object.getPrototypeOf(module_or_path) === Object.prototype) {
            ({module_or_path} = module_or_path)
        } else {
            console.warn('using deprecated parameters for the initialization function; pass a single object instead')
        }
    }

    if (typeof module_or_path === 'undefined') {
        module_or_path = new URL('ym2149_wasm_bg.wasm', import.meta.url);
    }
    const imports = __wbg_get_imports();

    if (typeof module_or_path === 'string' || (typeof Request === 'function' && module_or_path instanceof Request) || (typeof URL === 'function' && module_or_path instanceof URL)) {
        module_or_path = fetch(module_or_path);
    }

    const { instance, module } = await __wbg_load(await module_or_path, imports);

    return __wbg_finalize_init(instance, module);
}

export { initSync };
export default __wbg_init;
