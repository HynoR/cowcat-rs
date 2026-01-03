'use strict';

let wasmInstancePromise;

async function loadWasm() {
    if (wasmInstancePromise) return wasmInstancePromise;

    wasmInstancePromise = (async () => {
        const url = '/__cowcatwaf/assets/catpaw.wasm';
        const imports = {};

        if (typeof WebAssembly.instantiateStreaming === 'function') {
            try {
                const { instance } = await WebAssembly.instantiateStreaming(fetch(url), imports);
                return instance;
            } catch (err) {
                const resp = await fetch(url);
                const bytes = await resp.arrayBuffer();
                const { instance } = await WebAssembly.instantiate(bytes, imports);
                return instance;
            }
        }

        const resp = await fetch(url);
        const bytes = await resp.arrayBuffer();
        const { instance } = await WebAssembly.instantiate(bytes, imports);
        return instance;
    })();

    return wasmInstancePromise;
}

function clampU32(n) {
    if (!Number.isFinite(n) || n < 0) return 0;
    return n >>> 0;
}

function hasLeadingZeroBits(hashBytes, bits) {
    if (bits === 0) return true;
    let remaining = bits;
    for (const b of hashBytes) {
        if (remaining <= 0) return true;
        const lz = Math.clz32(b) - 24;
        if (lz >= remaining) return true;
        if (lz !== 8) return false;
        remaining -= 8;
    }
    return remaining <= 0;
}

async function powSearchNative(prefix, bits, start, step, maxIters) {
    if (typeof crypto === 'undefined' || !crypto.subtle || typeof crypto.subtle.digest !== 'function') {
        throw new Error('WebCrypto unavailable');
    }

    const encoder = new TextEncoder();
    const batchSize = 1000;
    let totalHashes = 0;
    let currentNonce = start;
    const limit = maxIters === 0 ? Number.MAX_SAFE_INTEGER : maxIters;

    while (totalHashes < limit) {
        const remaining = limit - totalHashes;
        const currentBatch = Math.min(batchSize, remaining);

        for (let i = 0; i < currentBatch; i++) {
            const msgBytes = encoder.encode(prefix + String(currentNonce));
            const hashBuffer = await crypto.subtle.digest('SHA-256', msgBytes);
            if (hasLeadingZeroBits(new Uint8Array(hashBuffer), bits)) {
                return { found: true, nonce: currentNonce >>> 0 };
            }
            currentNonce = (currentNonce + step) >>> 0;
        }

        totalHashes += currentBatch;
        self.postMessage({
            type: 'progress',
            hashes: currentBatch
        });
    }

    return { found: false };
}

self.onmessage = async (event) => {
    try {
        const data = event.data || {};
        const prefix = data.prefix;
        const bits = clampU32(data.bits);
        const start = clampU32(data.start);
        const step = clampU32(data.step);
        const maxIters = clampU32(data.max_iters);
        const workerType = String(data.worker_type || '').trim().toLowerCase();

        if (typeof prefix !== 'string' || prefix.length === 0) {
            throw new Error('Invalid prefix');
        }
        if (step === 0) {
            throw new Error('Invalid step');
        }

        if (workerType === 'native') {
            const result = await powSearchNative(prefix, bits, start, step, maxIters);
            if (result.found) {
                self.postMessage({ nonce: String(result.nonce) });
                return;
            }
            throw new Error('Nonce not found');
        }

        const instance = await loadWasm();
        const exports = instance.exports;

        if (!exports || typeof exports.pow_search !== 'function' || typeof exports.alloc !== 'function' || typeof exports.dealloc !== 'function') {
            throw new Error('Invalid WASM exports');
        }

        const encoder = new TextEncoder();
        const prefixBytes = encoder.encode(prefix);

        const ptr = exports.alloc(prefixBytes.length);
        const mem = new Uint8Array(exports.memory.buffer, ptr, prefixBytes.length);
        mem.set(prefixBytes);

        // 分批计算，每批报告一次进度
        const batchSize = 100000; // 每批计算 10万次
        let totalHashes = 0;
        let currentNonce = start;
        let found = false;
        let foundNonce = 0;

        while (totalHashes < maxIters && !found) {
            const remaining = maxIters - totalHashes;
            const currentBatch = Math.min(batchSize, remaining);

            const nonce = exports.pow_search(ptr, prefixBytes.length, bits, currentNonce, step, currentBatch);
            totalHashes += currentBatch;

            // 报告进度
            self.postMessage({
                type: 'progress',
                hashes: currentBatch
            });

            if ((nonce >>> 0) !== 0xFFFFFFFF) {
                found = true;
                foundNonce = nonce;
                break;
            }

            currentNonce = (currentNonce + step * currentBatch) >>> 0;
        }

        exports.dealloc(ptr, prefixBytes.length);

        if (found) {
            self.postMessage({ nonce: String(foundNonce >>> 0) });
        } else {
            throw new Error('Nonce not found');
        }
    } catch (err) {
        // 捕获详细错误信息用于调试
        const errorDetails = {
            error: err && err.message ? err.message : String(err),
            errorType: err && err.name ? err.name : 'UnknownError',
            errorStack: err && err.stack ? err.stack : '',
            workerInfo: {
                hasTextEncoder: typeof TextEncoder !== 'undefined',
                hasWebAssembly: typeof WebAssembly !== 'undefined',
                hasWebAssemblyInstantiateStreaming: typeof WebAssembly !== 'undefined' && typeof WebAssembly.instantiateStreaming === 'function',
                userAgent: typeof navigator !== 'undefined' ? navigator.userAgent : 'unknown'
            }
        };
        self.postMessage(errorDetails);
    }
};
