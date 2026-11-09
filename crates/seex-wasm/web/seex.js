/**
 * SeEx WebAssembly Engine - JavaScript Browser SDK
 * 
 * Provides client-side 3D STEP cleaning and EDA name restoration
 * running 100% in-browser via WebAssembly at near-native performance.
 */

let wasmInstance = null;
let memory = null;

/**
 * Outcome code mappings from Rust
 */
export const OUTCOME_NAMES = {
    0: 'Removed',
    1: 'NotFound',
    2: 'Kept',
    3: 'Unreadable'
};

/**
 * Initialize the SeEx WebAssembly module
 * @param {string|ArrayBuffer|Response} [wasmSource='seex_wasm.wasm'] - Path, URL, or buffer to the WASM file
 * @returns {Promise<WebAssembly.Instance>}
 */
export async function initSeex(wasmSource = 'seex_wasm.wasm') {
    if (wasmInstance) return wasmInstance;

    let wasmBytes;
    if (typeof wasmSource === 'string') {
        const resp = await fetch(wasmSource);
        if (!resp.ok) {
            throw new Error(`Failed to load WASM from "${wasmSource}": ${resp.status} ${resp.statusText}`);
        }
        wasmBytes = await resp.arrayBuffer();
    } else if (wasmSource instanceof Response) {
        wasmBytes = await wasmSource.arrayBuffer();
    } else {
        wasmBytes = wasmSource;
    }

    const { instance } = await WebAssembly.instantiate(wasmBytes, {});
    wasmInstance = instance;
    memory = instance.exports.memory;
    return wasmInstance;
}

/**
 * Get SeEx WASM Engine version
 * @returns {string}
 */
export function getVersion() {
    ensureInitialized();
    const { alloc, dealloc, get_version } = wasmInstance.exports;
    const lenPtr = alloc(4);
    try {
        const verPtr = get_version(lenPtr);
        const len = new Uint32Array(memory.buffer, lenPtr, 1)[0];
        const bytes = new Uint8Array(memory.buffer, verPtr, len);
        return new TextDecoder('utf-8').decode(bytes);
    } finally {
        dealloc(lenPtr, 4);
    }
}

/**
 * Clean 3D STEP Model in WebAssembly (100% client-side)
 * 
 * @param {Uint8Array|ArrayBuffer} stepData - Raw STEP file content
 * @param {Object} [options={}]
 * @param {boolean} [options.allMarkings=false] - If true, strips all markings & silkscreen with Pin 1 protection; if false, strips only EasyEDA/LCEDA logo
 * @returns {{
 *   cleanedBytes: Uint8Array,
 *   logosRemoved: number,
 *   piecesRemoved: number,
 *   facesRemoved: number,
 *   outcome: string,
 *   outcomeCode: number,
 *   durationMs: number
 * }}
 */
export function cleanStep(stepData, options = {}) {
    ensureInitialized();
    const { alloc, dealloc, clean_step } = wasmInstance.exports;

    const allMarkings = !!options.allMarkings;
    const mode = allMarkings ? 1 : 0;

    const inputBytes = stepData instanceof Uint8Array ? stepData : new Uint8Array(stepData);
    const inLen = inputBytes.length;
    const inPtr = alloc(inLen);

    new Uint8Array(memory.buffer, inPtr, inLen).set(inputBytes);

    const metaPtr = alloc(20); // 5 x u32
    const start = performance.now();

    try {
        const outPtr = clean_step(inPtr, inLen, mode, metaPtr);
        const durationMs = Math.round(performance.now() - start);

        if (!outPtr) {
            throw new Error('WebAssembly clean_step returned null pointer');
        }

        const meta = new Uint32Array(memory.buffer, metaPtr, 5);
        const outLen = meta[0];
        const logosRemoved = meta[1];
        const piecesRemoved = meta[2];
        const facesRemoved = meta[3];
        const outcomeCode = meta[4];

        // Copy cleaned bytes out of WASM memory
        const cleanedBytes = new Uint8Array(new Uint8Array(memory.buffer, outPtr, outLen));

        // Free result buffer in WASM
        dealloc(outPtr, outLen);

        return {
            cleanedBytes,
            logosRemoved,
            piecesRemoved,
            facesRemoved,
            outcome: OUTCOME_NAMES[outcomeCode] || 'Unknown',
            outcomeCode,
            durationMs
        };
    } finally {
        dealloc(inPtr, inLen);
        dealloc(metaPtr, 20);
    }
}

/**
 * Restore standard footprint name (strip _C<id> and restore decimals)
 * @param {string} title - Footprint title (e.g. "C0603_L1_6-W0_8_C999")
 * @param {boolean} [replaceDot=true] - Replace _ with . between numbers
 * @returns {string} Cleaned name (e.g. "C0603_L1.6-W0.8")
 */
export function restoreFootprintName(title, replaceDot = true) {
    ensureInitialized();
    const { alloc, dealloc, restore_footprint_name } = wasmInstance.exports;

    const encoder = new TextEncoder();
    const bytes = encoder.encode(title);
    const inPtr = alloc(bytes.length);
    new Uint8Array(memory.buffer, inPtr, bytes.length).set(bytes);

    const outLenPtr = alloc(4);
    try {
        const outPtr = restore_footprint_name(inPtr, bytes.length, replaceDot, outLenPtr);
        if (!outPtr) return title;
        const outLen = new Uint32Array(memory.buffer, outLenPtr, 1)[0];
        const resBytes = new Uint8Array(memory.buffer, outPtr, outLen);
        const result = new TextDecoder('utf-8').decode(resBytes);
        dealloc(outPtr, outLen);
        return result;
    } finally {
        dealloc(inPtr, bytes.length);
        dealloc(outLenPtr, 4);
    }
}

/**
 * Restore standard 3D STEP model file name
 * @param {string} title - Model title
 * @param {boolean} [replaceDot=true] - Replace _ with . between numbers
 * @returns {string} Cleaned filename (ending in .step)
 */
export function restoreStepModelName(title, replaceDot = true) {
    ensureInitialized();
    const { alloc, dealloc, restore_step_model_name } = wasmInstance.exports;

    const encoder = new TextEncoder();
    const bytes = encoder.encode(title);
    const inPtr = alloc(bytes.length);
    new Uint8Array(memory.buffer, inPtr, bytes.length).set(bytes);

    const outLenPtr = alloc(4);
    try {
        const outPtr = restore_step_model_name(inPtr, bytes.length, replaceDot, outLenPtr);
        if (!outPtr) return title;
        const outLen = new Uint32Array(memory.buffer, outLenPtr, 1)[0];
        const resBytes = new Uint8Array(memory.buffer, outPtr, outLen);
        const result = new TextDecoder('utf-8').decode(resBytes);
        dealloc(outPtr, outLen);
        return result;
    } finally {
        dealloc(inPtr, bytes.length);
        dealloc(outLenPtr, 4);
    }
}

function ensureInitialized() {
    if (!wasmInstance) {
        throw new Error('SeEx WASM module not initialized. Call "await initSeex()" first.');
    }
}

// Expose globally for browser non-module scripts
if (typeof window !== 'undefined') {
    window.SeexWasm = {
        initSeex,
        getVersion,
        cleanStep,
        restoreFootprintName,
        restoreStepModelName,
        OUTCOME_NAMES
    };
}

