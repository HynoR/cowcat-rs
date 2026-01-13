(function() {
    'use strict';

    // ===== WASM Codec =====

    const wasmCodecPromise = (async () => {
        const resp = await fetch('/__cowcatwaf/assets/catpaw.wasm');
        const bytes = await resp.arrayBuffer();
        const { instance } = await WebAssembly.instantiate(bytes, {});
        return instance.exports;
    })();

    function readU32LE(wasm, ptr) {
        return new DataView(wasm.memory.buffer).getUint32(ptr, true);
    }

    function copyBytes(wasm, ptr, len) {
        return new Uint8Array(wasm.memory.buffer, ptr, len).slice();
    }

    function encodeString(wasm, value) {
        const bytes = new TextEncoder().encode(value || '');
        if (bytes.length === 0) {
            return { ptr: 0, len: 0 };
        }
        const ptr = wasm.alloc(bytes.length);
        if (!ptr) {
            return { ptr: 0, len: 0 };
        }
        new Uint8Array(wasm.memory.buffer, ptr, bytes.length).set(bytes);
        return { ptr, len: bytes.length };
    }

    function callCodec(wasm, fn, args) {
        const outLenPtr = wasm.alloc(4);
        if (!outLenPtr) {
            throw new Error('Failed to allocate memory for output length');
        }
        new DataView(wasm.memory.buffer).setUint32(outLenPtr, 0, true);

        const outPtr = fn(...args, outLenPtr);
        const outLen = readU32LE(wasm, outLenPtr);

        let outBytes = new Uint8Array();
        if (outPtr && outLen > 0) {
            outBytes = copyBytes(wasm, outPtr, outLen);
            wasm.dealloc(outPtr, outLen);
        }
        wasm.dealloc(outLenPtr, 4);
        return outBytes;
    }

    async function encodeVerifyRequest(taskId, nonce, redirect) {
        const wasm = await wasmCodecPromise;
        const task = encodeString(wasm, taskId);
        const nonceBytes = encodeString(wasm, nonce);
        const redirectBytes = encodeString(wasm, redirect);
        const outBytes = callCodec(wasm, wasm.encode_verify_request, [
            task.ptr,
            task.len,
            nonceBytes.ptr,
            nonceBytes.len,
            redirectBytes.ptr,
            redirectBytes.len
        ]);
        if (task.ptr && task.len > 0) {
            wasm.dealloc(task.ptr, task.len);
        }
        if (nonceBytes.ptr && nonceBytes.len > 0) {
            wasm.dealloc(nonceBytes.ptr, nonceBytes.len);
        }
        if (redirectBytes.ptr && redirectBytes.len > 0) {
            wasm.dealloc(redirectBytes.ptr, redirectBytes.len);
        }
        return outBytes;
    }

    async function decodeTaskResponse(frameBytes) {
        const wasm = await wasmCodecPromise;
        if (!frameBytes || frameBytes.length === 0) {
            throw new Error('Empty frame bytes');
        }
        const ptr = wasm.alloc(frameBytes.length);
        if (!ptr) {
            throw new Error('Failed to allocate memory for frame');
        }
        new Uint8Array(wasm.memory.buffer, ptr, frameBytes.length).set(frameBytes);
        const outBytes = callCodec(wasm, wasm.decode_task_response, [ptr, frameBytes.length]);
        wasm.dealloc(ptr, frameBytes.length);
        return JSON.parse(new TextDecoder().decode(outBytes));
    }

    async function decodeVerifyResponse(frameBytes) {
        const wasm = await wasmCodecPromise;
        if (!frameBytes || frameBytes.length === 0) {
            throw new Error('Empty frame bytes');
        }
        const ptr = wasm.alloc(frameBytes.length);
        if (!ptr) {
            throw new Error('Failed to allocate memory for frame');
        }
        new Uint8Array(wasm.memory.buffer, ptr, frameBytes.length).set(frameBytes);
        const outBytes = callCodec(wasm, wasm.decode_verify_response, [ptr, frameBytes.length]);
        wasm.dealloc(ptr, frameBytes.length);
        return JSON.parse(new TextDecoder().decode(outBytes));
    }

    // ===== Helper Functions =====

    function workerCount(max = 3, reserve = 1) {
        const hw = Math.floor(navigator.hardwareConcurrency || 1);
        return Math.min(max, Math.max(1, hw - reserve));
    }

    function makePrefix(task) {
        return 'v1|' + task.seed + '|' + task.exp + '|' + task.bits + '|' + task.scope + '|' + task.ua_hash + '|';
    }

    function normalizeWorkerType(value) {
        const t = String(value || '').trim().toLowerCase();
        return t === 'native' ? 'native' : 'wasm';
    }

    function extractWorkerType(frameBytes) {
        if (!frameBytes || frameBytes.length < 8) return '';
        const keyBytes = new TextEncoder().encode('cowcatwaflibwafcatcow');
        const data = new Uint8Array(frameBytes);
        const deobfuscated = data.slice();
        for (let i = 0; i < deobfuscated.length; i++) {
            deobfuscated[i] ^= keyBytes[i % keyBytes.length];
        }

        if (deobfuscated[0] !== 0x43 || deobfuscated[1] !== 0x57 || deobfuscated[2] !== 0x01 || deobfuscated[3] !== 0x02) {
            return '';
        }

        const payloadLen = (
            (deobfuscated[4] << 24) |
            (deobfuscated[5] << 16) |
            (deobfuscated[6] << 8) |
            deobfuscated[7]
        ) >>> 0;
        if (payloadLen !== deobfuscated.length - 8) {
            return '';
        }

        const payload = deobfuscated.subarray(8);
        for (let i = 0; i + 3 <= payload.length;) {
            const t = payload[i];
            const len = (payload[i + 1] << 8) | payload[i + 2];
            i += 3;
            if (i + len > payload.length) return '';
            if (t === 0x0b) {
                return new TextDecoder().decode(payload.subarray(i, i + len));
            }
            i += len;
        }
        return '';
    }

    // ===== Global State (Minimal) =====

    /**
     * 全局状态对象 - 只提供核心数据，用户自行计算其他值
     * 
     * state: 'idle' | 'solving' | 'verifying' | 'success' | 'error'
     * totalHashes: 已计算的哈希数量
     * task: 解码后的任务数据对象
     * redirect: 跳转 URL
     * error: 错误信息
     */
    window.__cowcat__ = {
        state: 'idle',
        totalHashes: 0,
        hashRate: 0,
        task: null,
        redirect: null,
        error: null
    };

    const METRICS_DELAY_MS = 1000;
    const METRICS_INTERVAL_MS = 600;
    const METRICS_ALPHA = 0.3;
    const METRICS_DECAY = 0.85;

    let metricsDelayTimer = null;
    let metricsTimer = null;
    let metricsLastTotal = 0;
    let metricsLastTime = 0;
    let smoothedRate = 0;

    function emit(eventName, detail) {
        document.dispatchEvent(new CustomEvent('cowcat:' + eventName, { detail: detail }));
    }

    function snapshotState() {
        return {
            state: window.__cowcat__.state,
            totalHashes: window.__cowcat__.totalHashes,
            hashRate: window.__cowcat__.hashRate,
            task: window.__cowcat__.task,
            redirect: window.__cowcat__.redirect,
            error: window.__cowcat__.error
        };
    }

    function emitState() {
        emit('state', snapshotState());
    }

    function emitMetrics() {
        emit('metrics', snapshotState());
    }

    function setState(state, totalHashes, task, redirect, error) {
        if (state !== undefined) {
            window.__cowcat__.state = state;
            if (state !== 'solving') {
                window.__cowcat__.hashRate = 0;
            }
        }
        if (totalHashes !== undefined) window.__cowcat__.totalHashes = totalHashes;
        if (totalHashes !== undefined && totalHashes < 0) window.__cowcat__.totalHashes = 0;
        if (task !== undefined) window.__cowcat__.task = task;
        if (redirect !== undefined) window.__cowcat__.redirect = redirect;
        if (error !== undefined) window.__cowcat__.error = error;
        emitState();
    }

    function setTotalHashes(total) {
        window.__cowcat__.totalHashes = total < 0 ? 0 : total;
    }

    function resetMetricsState() {
        metricsLastTotal = window.__cowcat__.totalHashes || 0;
        metricsLastTime = Date.now();
        smoothedRate = 0;
        window.__cowcat__.hashRate = 0;
    }

    function reportMetrics() {
        const now = Date.now();
        const total = window.__cowcat__.totalHashes || 0;
        const delta = total - metricsLastTotal;
        const elapsed = (now - metricsLastTime) / 1000;
        let instantRate = 0;
        if (elapsed > 0 && delta > 0) {
            instantRate = delta / elapsed;
        }

        if (instantRate > 0) {
            smoothedRate = smoothedRate > 0
                ? (smoothedRate * (1 - METRICS_ALPHA) + instantRate * METRICS_ALPHA)
                : instantRate;
        } else if (smoothedRate > 0) {
            smoothedRate *= METRICS_DECAY;
            if (smoothedRate < 1) {
                smoothedRate = 0;
            }
        }

        metricsLastTotal = total;
        metricsLastTime = now;
        window.__cowcat__.hashRate = Math.round(smoothedRate);
        emitMetrics();
    }

    function startMetrics() {
        stopMetrics(false);
        metricsDelayTimer = setTimeout(function() {
            resetMetricsState();
            metricsTimer = setInterval(reportMetrics, METRICS_INTERVAL_MS);
        }, METRICS_DELAY_MS);
    }

    function stopMetrics(emitZero) {
        if (metricsDelayTimer) {
            clearTimeout(metricsDelayTimer);
            metricsDelayTimer = null;
        }
        if (metricsTimer) {
            clearInterval(metricsTimer);
            metricsTimer = null;
        }
        if (emitZero) {
            resetMetricsState();
            emitMetrics();
        }
    }

    // ===== Challenge Solving =====

    async function solveChallenge(task) {
        const numWorkers = workerCount(3, 1);
        const prefix = makePrefix(task);
        const workerType = normalizeWorkerType(task.worker_type);
        const maxItersPerWorker = 50000000;

        setState('solving', 0, undefined, undefined, null);
        startMetrics();

        const workers = [];
        let settled = false;
        let errorCount = 0;
        const workerErrors = [];
        let totalHashes = 0;

        const noncePromise = new Promise(function(resolve, reject) {
            const timeStampNow = Date.now();
            for (let i = 0; i < numWorkers; i++) {
                let w;
                try {
                    w = new Worker('/__cowcatwaf/assets/catpaw.worker.js?v=' + timeStampNow);
                } catch (err) {
                    workerErrors.push({
                        workerId: i,
                        phase: 'worker_creation',
                        error: err.message || String(err)
                    });
                    errorCount++;
                    if (errorCount === numWorkers) {
                        settled = true;
                        const detailedError = new Error('所有工作线程创建失败');
                        detailedError.details = workerErrors;
                        reject(detailedError);
                    }
                    continue;
                }

                workers.push(w);

                w.onmessage = function(event) {
                    if (settled) return;

                    const data = event.data || {};

                    if (data.type === 'progress' && typeof data.hashes === 'number') {
                        totalHashes += data.hashes;
                        setTotalHashes(totalHashes);
                        return;
                    }

                    if (data && data.error) {
                        workerErrors.push({
                            workerId: i,
                            phase: 'worker_execution',
                            error: data.error
                        });
                        errorCount++;
                        if (errorCount === numWorkers) {
                            settled = true;
                            const detailedError = new Error(data.error || '工作线程执行错误');
                            detailedError.details = workerErrors;
                            reject(detailedError);
                        }
                        return;
                    }

                    if (data && typeof data.nonce === 'string') {
                        settled = true;
                        resolve(data.nonce);
                    }
                };

                w.onerror = function(err) {
                    if (settled) return;
                    workerErrors.push({
                        workerId: i,
                        phase: 'worker_onerror',
                        error: err.message || '工作线程错误'
                    });
                    errorCount++;
                    if (errorCount === numWorkers) {
                        settled = true;
                        const detailedError = new Error('工作线程错误');
                        detailedError.details = workerErrors;
                        reject(detailedError);
                    }
                };

                w.postMessage({
                    prefix: prefix,
                    bits: task.bits,
                    start: i,
                    step: numWorkers,
                    max_iters: maxItersPerWorker,
                    worker_type: workerType,
                });
            }
        });

        try {
            return await noncePromise;
        } finally {
            for (const w of workers) w.terminate();
        }
    }

    // ===== Verification and Redirect =====

    async function verifyAndRedirect(taskId, nonce, redirect, task) {
        stopMetrics(true);
        setState('verifying', undefined, undefined, undefined, null);

        try {
            const payload = await encodeVerifyRequest(taskId, nonce, redirect);
            const response = await fetch('/__cowcatwaf/verify', {
                method: 'POST',
                headers: {
                    'Content-Type': 'application/octet-stream',
                },
                body: payload
            });

            const responseBytes = new Uint8Array(await response.arrayBuffer());
            const result = await decodeVerifyResponse(responseBytes);
            if (!response.ok || result.error) {
                throw new Error(result.error || '验证失败');
            }
            
            setState('success', undefined, undefined, result.redirect, null);

            // 发送统计信息
            if (window.__cowcat_meta__ && typeof window.__cowcat_meta__.send === 'function') {
                window.__cowcat_meta__.send('/__cowcatwaf/challenge/fp').catch(function() {});
            }

            // 自动跳转
            setTimeout(function() {
                window.location.href = result.redirect;
            }, 350);
        } catch (err) {
            setState('error', undefined, undefined, undefined, err.message || '验证失败');
            throw err;
        }
    }

    // ===== Auto Execution =====

    async function autoRun() {
        const taskDataEl = document.getElementById('pow-task-data');
        if (!taskDataEl) {
            return; // 没有任务数据，不执行
        }

        try {
            const isSecureContext = window.location.protocol === 'https:' ||
                                  window.location.hostname === 'localhost' ||
                                  window.location.hostname === '127.0.0.1';

            if (!isSecureContext) {
                throw new Error('安全错误: 此页面必须在 HTTPS 环境下运行');
            }

            let embeddedData;
            try {
                embeddedData = JSON.parse(taskDataEl.textContent);
            } catch (parseErr) {
                throw new Error('任务数据解析失败');
            }

            if (!embeddedData.task || embeddedData.task === '') {
                throw new Error('任务数据为空');
            }

            const redirect = embeddedData.redirect || '/';

            let taskBytes;
            try {
                taskBytes = Uint8Array.from(atob(embeddedData.task), function(c) {
                    return c.charCodeAt(0);
                });
            } catch (decodeErr) {
                throw new Error('Base64 解码失败');
            }

            const task = await decodeTaskResponse(taskBytes);
            const rawWorkerType = extractWorkerType(taskBytes);
            if (rawWorkerType) {
                task.worker_type = normalizeWorkerType(rawWorkerType);
            }

            if (task.error) {
                throw new Error(task.error || '获取挑战任务失败');
            }

            // 更新全局状态：任务数据和跳转 URL
            setState('idle', 0, task, redirect, null);

            const nonce = await solveChallenge(task);
            await verifyAndRedirect(task.task_id, nonce, redirect, task);

        } catch (err) {
            console.error('CowcatCore Error:', err);
            stopMetrics(true);
            setState('error', undefined, undefined, undefined, err.message || '发生未知错误');
        }
    }

    // ===== Public API =====

    window.CowcatCore = {
        // Codec functions
        decodeTaskResponse: decodeTaskResponse,
        decodeVerifyResponse: decodeVerifyResponse,

        // Main functions
        solveChallenge: solveChallenge,
        verifyAndRedirect: verifyAndRedirect,

        // Manual execution
        run: autoRun
    };

    // 自动执行
    if (document.readyState === 'loading') {
        document.addEventListener('DOMContentLoaded', autoRun);
    } else {
        autoRun();
    }
})();
