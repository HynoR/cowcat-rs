(function() {
    'use strict';

    // ===== State Management =====
    const state = {
        progress: 0,
        status: '准备验证挑战...',
        progressText: '初始化',
        error: false,
        errorMessage: '',
        errorDetails: null,
        showErrorDetails: false,
        hashRate: 0,
        totalHashes: 0,
        manualRedirect: false,  // 是否手动跳转模式
        pendingRedirectUrl: null  // 待跳转的URL
    };

    // ===== DOM Element References =====
    const elements = {};

    function initElements() {
        elements.statusContainer = document.getElementById('status-container');
        elements.errorContainer = document.getElementById('error-container');
        elements.errorMessage = document.getElementById('error-message');
        elements.errorDetails = document.getElementById('error-details');
        elements.errorDetailsToggle = document.getElementById('error-details-toggle');
        elements.errorDetailsContent = document.getElementById('error-details-content');
        elements.errorDetailsList = document.getElementById('error-details-list');
        elements.progressPercent = document.getElementById('progress-percent');
        elements.progressBar = document.getElementById('progress-bar');
        elements.progressText = document.getElementById('progress-text');
        elements.hashRateValue = document.getElementById('hash-rate-value');
        elements.totalHashesValue = document.getElementById('total-hashes-value');
        elements.hashStatsContainer = document.getElementById('hash-stats');
        elements.visualImage1 = document.getElementById('visual-image-1');
        elements.visualImage2 = document.getElementById('visual-image-2');
        elements.manualRedirectContainer = document.getElementById('manual-redirect-container');
        elements.manualRedirectBtn = document.getElementById('manual-redirect-btn');
    }

    // ===== Fine-grained DOM Update Functions =====

    function updateProgress(value, text) {
        const newProgress = Math.min(100, Math.max(0, value));
        if (state.progress !== newProgress) {
            state.progress = newProgress;
            if (elements.progressBar) {
                elements.progressBar.style.width = newProgress + '%';
            }
            if (elements.progressPercent) {
                elements.progressPercent.textContent = Math.round(newProgress) + '%';
            }
        }
        if (text !== undefined && state.progressText !== text) {
            state.progressText = text;
            if (elements.progressText) {
                elements.progressText.textContent = text;
            }
        }
    }

    function updateStatus(text) {
        if (state.status !== text) {
            state.status = text;
            if (elements.statusContainer) {
                elements.statusContainer.textContent = text;
            }
        }
    }

    function showError(message, details = null) {
        state.error = true;
        state.errorMessage = message;
        state.errorDetails = details;
        state.status = '';

        if (elements.statusContainer) {
            elements.statusContainer.style.display = 'none';
        }
        if (elements.errorContainer) {
            elements.errorContainer.style.display = 'block';
        }
        if (elements.errorMessage) {
            elements.errorMessage.textContent = message;
        }

        if (details && details.length > 0) {
            renderErrorDetails(details);
            if (elements.errorDetails) {
                elements.errorDetails.style.display = 'block';
            }
        }
    }

    function updateHashStats(rate, total) {
        state.hashRate = rate;
        if (elements.hashRateValue) {
            elements.hashRateValue.textContent = formatHashRate(rate);
        }

        if (elements.totalHashesValue) {
            elements.totalHashesValue.textContent = formatTotalHashes(total);
        }

        if (elements.hashStatsContainer) {
            elements.hashStatsContainer.style.display = rate > 0 ? 'grid' : 'none';
        }
    }

    // ===== Error Detail Rendering =====

    function escapeHtml(text) {
        const div = document.createElement('div');
        div.textContent = text;
        return div.innerHTML;
    }

    function renderErrorDetails(details) {
        if (!elements.errorDetailsList) return;

        const template = document.getElementById('error-detail-template');
        if (!template) {
            console.error('Error detail template not found');
            return;
        }

        elements.errorDetailsList.innerHTML = '';

        details.forEach((detail, index) => {
            const clone = template.content.cloneNode(true);
            const item = clone.querySelector('.error-detail-item');

            const label = item.querySelector('.error-detail-label');
            if (label) {
                let title = '错误 #' + (index + 1);
                if (detail.workerId !== undefined) {
                    title += ' (Worker ' + detail.workerId + ')';
                }
                label.textContent = title;
            }

            const setField = (selector, label, value) => {
                const el = item.querySelector(selector);
                if (el && value !== undefined && value !== '') {
                    el.innerHTML = '<strong>' + label + ':</strong> ' + escapeHtml(String(value));
                    el.style.display = 'block';
                } else if (el) {
                    el.style.display = 'none';
                }
            };

            setField('[data-field="phase"]', '阶段', detail.phase);
            setField('[data-field="error"]', '错误', detail.error);
            setField('[data-field="errorType"]', '类型', detail.errorType);
            setField('[data-field="filename"]', '文件',
                detail.filename ? detail.filename + ':' + detail.lineno + ':' + detail.colno : undefined
            );
            setField('[data-field="errorStack"]', '堆栈', detail.errorStack);

            if (detail.workerInfo) {
                const workerInfoEl = item.querySelector('[data-field="workerInfo"]');
                if (workerInfoEl) {
                    const info = detail.workerInfo;
                    workerInfoEl.innerHTML =
                        '<strong>Worker 环境:</strong><br>' +
                        '- TextEncoder: ' + (info.hasTextEncoder ? '✓' : '✗') + '<br>' +
                        '- WebAssembly: ' + (info.hasWebAssembly ? '✓' : '✗') + '<br>' +
                        '- instantiateStreaming: ' + (info.hasWebAssemblyInstantiateStreaming ? '✓' : '✗') + '<br>' +
                        '- UserAgent: ' + escapeHtml(info.userAgent);
                    workerInfoEl.style.display = 'block';
                }
            }

            if (detail.browserInfo) {
                const browserInfoEl = item.querySelector('[data-field="browserInfo"]');
                if (browserInfoEl) {
                    const info = detail.browserInfo;
                    browserInfoEl.innerHTML =
                        '<strong>浏览器环境:</strong><br>' +
                        '- Worker: ' + (info.hasWorker ? '✓' : '✗') + '<br>' +
                        '- WebAssembly: ' + (info.hasWebAssembly ? '✓' : '✗') + '<br>' +
                        '- TextEncoder: ' + (info.hasTextEncoder ? '✓' : '✗') + '<br>' +
                        '- UserAgent: ' + escapeHtml(info.userAgent);
                    browserInfoEl.style.display = 'block';
                }
            }

            elements.errorDetailsList.appendChild(clone);
        });
    }

    // ===== Event Listeners =====

    function initEventListeners() {
        if (elements.errorDetailsToggle) {
            elements.errorDetailsToggle.addEventListener('click', function() {
                state.showErrorDetails = !state.showErrorDetails;
                elements.errorDetailsToggle.textContent =
                    state.showErrorDetails ? '隐藏错误详情' : '显示错误详情';
                if (elements.errorDetailsContent) {
                    elements.errorDetailsContent.style.display =
                        state.showErrorDetails ? 'block' : 'none';
                }
            });
        }

        // 图片点击事件 - 切换为手动跳转模式（两个图片都要监听）
        if (elements.visualImage1) {
            elements.visualImage1.addEventListener('click', function() {
                state.manualRedirect = true;
                console.log('Manual redirect mode enabled');
            });
        }
        if (elements.visualImage2) {
            elements.visualImage2.addEventListener('click', function() {
                state.manualRedirect = true;
                console.log('Manual redirect mode enabled');
            });
        }

        // 手动跳转按钮点击事件
        if (elements.manualRedirectBtn) {
            elements.manualRedirectBtn.addEventListener('click', function() {
                if (state.pendingRedirectUrl) {
                    window.location.href = state.pendingRedirectUrl;
                }
            });
        }
    }

    // ===== Formatting Functions =====

    function formatHashRate(rate) {
        if (rate >= 1000000) {
            return (rate / 1000000).toFixed(2) + ' MH/s';
        } else if (rate >= 1000) {
            return (rate / 1000).toFixed(2) + ' KH/s';
        } else {
            return rate + ' H/s';
        }
    }

    function formatTotalHashes(total) {
        if (total >= 1000000000) {
            return (total / 1000000000).toFixed(2) + ' B';
        } else if (total >= 1000000) {
            return (total / 1000000).toFixed(2) + ' M';
        } else if (total >= 1000) {
            return (total / 1000).toFixed(2) + ' K';
        } else {
            return total.toString();
        }
    }

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

    async function encodeTaskRequest(redirect) {
        const wasm = await wasmCodecPromise;
        const { ptr, len } = encodeString(wasm, redirect);
        const outBytes = callCodec(wasm, wasm.encode_task_request, [ptr, len]);
        if (ptr && len > 0) {
            wasm.dealloc(ptr, len);
        }
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

    function computePowProgress(attempts, reportAs) {
        const attemptsCount = Math.max(0, Math.trunc(Number(attempts) || 0));
        const reportValue = Number(reportAs);
        if (!Number.isFinite(reportValue) || reportValue <= 0) return 0;
        const successProb = Math.pow(16, -reportValue);
        const missProb = Math.pow(1 - successProb, attemptsCount);
        const progress = (1 - Math.pow(missProb, 2)) * 100;
        if (!Number.isFinite(progress)) return 0;
        return Math.max(0, Math.min(100, progress));
    }

    function base64EncodeUtf8(value) {
        try {
            return btoa(unescape(encodeURIComponent(value)));
        } catch (err) {
            console.warn('Failed to base64 encode stats payload:', err);
            return '';
        }
    }

    function sendChallengeStats() {
        const endpoint = '/__cowcatwaf/challenge/fp';
        if (window.__cowcat_meta__ && typeof window.__cowcat_meta__.send === 'function') {
            return window.__cowcat_meta__.send(endpoint);
        }
        if (window.__challenge_fp__) {
            const payload = base64EncodeUtf8(JSON.stringify(window.__challenge_fp__));
            if (!payload) {
                return Promise.resolve();
            }
            return fetch(endpoint, { method: 'POST', body: payload, keepalive: true });
        }
        return Promise.resolve();
    }

    function sendChallengeStatsAllowFail(timeoutMs) {
        const statsPromise = Promise.resolve()
            .then(sendChallengeStats)
            .catch(function(err) {
                console.warn('Failed to send challenge stats:', err);
            });
        const timeoutPromise = new Promise(function(resolve) {
            setTimeout(resolve, timeoutMs);
        });
        return Promise.race([statsPromise, timeoutPromise]);
    }

    // ===== Progress Tracking =====

    let computeStartTime = null;
    let computeProgressInterval = null;
    let challengeStartTime = null;  // 记录挑战开始时间

    function startComputeProgress(reportAs) {
        computeStartTime = Date.now();
        updateProgress(10, '计算中...');

        computeProgressInterval = setInterval(function() {
            if (computeStartTime) {
                const elapsed = Date.now() - computeStartTime;
                const rawProgress = computePowProgress(state.totalHashes, reportAs);
                const newProgress = Math.min(95, Math.max(10, rawProgress));
                updateProgress(newProgress, 'Working... (elapsed ' + Math.round(elapsed / 1000) + 's)');
            }
        }, 100);
    }

    function stopComputeProgress() {
        if (computeProgressInterval) {
            clearInterval(computeProgressInterval);
            computeProgressInterval = null;
        }
        computeStartTime = null;
    }

    // ===== Challenge Solving =====

    async function solveChallenge(task) {
        const numWorkers = workerCount(3, 1);
        const prefix = makePrefix(task);
        const workerType = normalizeWorkerType(task.worker_type);
        const maxItersPerWorker = 50000000;

        const reportAs = Number.isFinite(Number(task.report_as))
            ? Number(task.report_as)
            : Math.max(1, task.bits / 4);
        startComputeProgress(reportAs);

        const workers = [];
        let settled = false;
        let errorCount = 0;
        const workerErrors = [];

        state.totalHashes = 0;
        state.hashRate = 0;
        updateHashStats(0, 0);

        let lastUpdateTime = Date.now();
        let lastTotalHashes = 0;

        const hashRateInterval = setInterval(function() {
            const now = Date.now();
            const elapsed = (now - lastUpdateTime) / 1000;
            const hashDiff = state.totalHashes - lastTotalHashes;

            if (elapsed > 0) {
                const newRate = Math.round(hashDiff / elapsed);
                updateHashStats(newRate, state.totalHashes);
                lastUpdateTime = now;
                lastTotalHashes = state.totalHashes;
            }
        }, 1000);

        const noncePromise = new Promise(function(resolve, reject) {
            const timeStampNow = Date.now();
            for (let i = 0; i < numWorkers; i++) {
                let w;
                try {
                    w = new Worker('/__cowcatwaf/assets/catpaw.worker.min.js?v=' + timeStampNow);
                } catch (err) {
                    const creationError = {
                        workerId: i,
                        phase: 'worker_creation',
                        error: err.message || String(err),
                        errorType: err.name || 'WorkerCreationError',
                        errorStack: err.stack || '',
                        browserInfo: {
                            hasWorker: typeof Worker !== 'undefined',
                            userAgent: navigator.userAgent
                        }
                    };
                    workerErrors.push(creationError);
                    errorCount++;
                    if (errorCount === numWorkers) {
                        settled = true;
                        stopComputeProgress();
                        clearInterval(hashRateInterval);
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
                        state.totalHashes += data.hashes;
                        return;
                    }

                    if (data && data.error) {
                        workerErrors.push({
                            workerId: i,
                            phase: 'worker_execution',
                            error: data.error,
                            errorType: data.errorType || 'WorkerError',
                            errorStack: data.errorStack || '',
                            workerInfo: data.workerInfo,
                            browserInfo: data.browserInfo
                        });
                        errorCount++;
                        if (errorCount === numWorkers) {
                            settled = true;
                            stopComputeProgress();
                            clearInterval(hashRateInterval);
                            const detailedError = new Error(data.error || '工作线程执行错误');
                            detailedError.details = workerErrors;
                            reject(detailedError);
                        }
                        return;
                    }

                    if (data && typeof data.nonce === 'string') {
                        settled = true;
                        stopComputeProgress();
                        clearInterval(hashRateInterval);
                        updateProgress(95, '计算完成');
                        resolve(data.nonce);
                    }
                };

                w.onerror = function(err) {
                    if (settled) return;
                    workerErrors.push({
                        workerId: i,
                        phase: 'worker_onerror',
                        error: err.message || '工作线程错误',
                        errorType: 'WorkerError',
                        filename: err.filename || '',
                        lineno: err.lineno || 0,
                        colno: err.colno || 0
                    });
                    errorCount++;
                    if (errorCount === numWorkers) {
                        settled = true;
                        stopComputeProgress();
                        clearInterval(hashRateInterval);
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
            clearInterval(hashRateInterval);
        }
    }

    // ===== Verification and Redirect =====

    async function verifyAndRedirect(taskId, nonce, redirect, task) {
        updateProgress(99, '正在验证解决方案...');
        updateStatus('正在验证解决方案...');

        try {
            if (typeof crypto !== 'undefined' && crypto.subtle && typeof crypto.subtle.digest === 'function') {
                try {
                    const msg = 'v1|' + task.seed + '|' + task.exp + '|' + task.bits + '|' + task.scope + '|' + task.ua_hash + '|' + nonce;
                    const msgBuffer = new TextEncoder().encode(msg);
                    const hashBuffer = await crypto.subtle.digest('SHA-256', msgBuffer);
                    const hashArray = Array.from(new Uint8Array(hashBuffer));
                    const hashHex = hashArray.map(function(b) { return b.toString(16).padStart(2, '0'); }).join('');
                    console.log('PoW OK:', hashHex);
                } catch (hashErr) {
                    console.log('PoW hash calculation skipped:', hashErr.message);
                }
            } else {
                console.log('PoW verification proceeding (crypto.subtle not available for hash logging)');
            }
            const payload = await encodeVerifyRequest(taskId, nonce, redirect);
            // 计算从开始计算到发送验证的时间（毫秒）
            let computeTimeParam = '';
            if (challengeStartTime) {
                const computeTimeMs = Date.now() - challengeStartTime;
                computeTimeParam = '?compute_time=' + encodeURIComponent(computeTimeMs);
            }
            const response = await fetch('/__cowcatwaf/verify' + computeTimeParam, {
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
            updateProgress(100, '验证成功！');

            // 切换图片：隐藏图片1，显示图片2
            if (elements.visualImage1) {
                elements.visualImage1.style.display = 'none';
            }
            if (elements.visualImage2) {
                elements.visualImage2.style.display = 'block';
            }

            await sendChallengeStatsAllowFail(800);

            // 检查是否是手动跳转模式
            if (state.manualRedirect) {
                // 手动跳转模式：显示按钮，不自动跳转
                updateStatus('验证成功！');
                state.pendingRedirectUrl = result.redirect;
                if (elements.manualRedirectContainer) {
                    elements.manualRedirectContainer.style.display = 'block';
                }
            } else {
                // 自动跳转模式
                updateStatus('验证成功！正在跳转...');
                setTimeout(function() {
                    window.location.href = result.redirect;
                }, 350);
            }
        } catch (err) {
            showError(err.message || '验证失败，请重试');

            let failProgress = 99;
            const failInterval = setInterval(function() {
                failProgress = Math.max(0, failProgress - 0.5);
                const progressMsg = failProgress > 0
                    ? '验证失败: ' + state.errorMessage + ' (' + Math.round(failProgress) + '%)'
                    : '验证失败';
                updateProgress(failProgress, progressMsg);

                if (failProgress <= 0) {
                    clearInterval(failInterval);
                    updateProgress(0, '验证失败');
                }
            }, 50);

            throw err;
        }
    }

    // ===== Main Function =====

    async function main() {
        try {
            const isSecureContext = window.location.protocol === 'https:' ||
                                  window.location.hostname === 'localhost' ||
                                  window.location.hostname === '127.0.0.1';

            if (!isSecureContext) {
                throw new Error('安全错误: 此页面必须在 HTTPS 环境下运行。当前协议: ' + window.location.protocol);
            }

            updateStatus('正在准备挑战任务...');
            updateProgress(0, '初始化');

            // 从 DOM 读取内嵌的任务数据
            const taskDataEl = document.getElementById('pow-task-data');
            if (!taskDataEl) {
                throw new Error('任务数据未找到');
            }

            let embeddedData;
            try {
                embeddedData = JSON.parse(taskDataEl.textContent);
            } catch (parseErr) {
                throw new Error('任务数据解析失败: ' + parseErr.message);
            }

            // 检查任务数据是否有效
            if (!embeddedData.task || embeddedData.task === '') {
                throw new Error('任务数据为空');
            }

            const redirect = embeddedData.redirect || '/';

            // 解码 Base64 任务数据
            let taskBytes;
            try {
                taskBytes = Uint8Array.from(atob(embeddedData.task), function(c) {
                    return c.charCodeAt(0);
                });
            } catch (decodeErr) {
                throw new Error('Base64 解码失败: ' + decodeErr.message);
            }

            // 使用现有 WASM 解码器解析任务
            const task = await decodeTaskResponse(taskBytes);
            const rawWorkerType = extractWorkerType(taskBytes);
            if (rawWorkerType) {
                task.worker_type = normalizeWorkerType(rawWorkerType);
            }

            if (task.error) {
                throw new Error(task.error || '获取挑战任务失败');
            }

            updateProgress(10, '任务获取成功');
            updateStatus('计算中...');

            // 重置挑战开始时间
            challengeStartTime = Date.now();
            const nonce = await solveChallenge(task);
            await verifyAndRedirect(task.task_id, nonce, redirect, task);

        } catch (err) {
            console.error('Error:', err);

            if (err.details) {
                showError(err.message || '发生未知错误', err.details);
                console.error('Detailed error info:', err.details);
            } else {
                showError(err.message || '发生未知错误', [{
                    error: err.message || String(err),
                    errorType: err.name || 'Error',
                    errorStack: err.stack || '',
                    browserInfo: {
                        userAgent: navigator.userAgent,
                        hasWorker: typeof Worker !== 'undefined',
                        hasWebAssembly: typeof WebAssembly !== 'undefined',
                        hasTextEncoder: typeof TextEncoder !== 'undefined'
                    }
                }]);
            }

            stopComputeProgress();

            let failProgress = state.progress;
            const failInterval = setInterval(function() {
                failProgress = Math.max(0, failProgress - 0.5);
                const progressMsg = failProgress > 0
                    ? '错误: ' + state.errorMessage + ' (' + Math.round(failProgress) + '%)'
                    : '验证失败';
                updateProgress(failProgress, progressMsg);

                if (failProgress <= 0) {
                    clearInterval(failInterval);
                    updateProgress(0, '验证失败');
                }
            }, 50);
        }
    }

    // ===== Initialization =====

    function init() {
        initElements();
        initEventListeners();
    }

    if (document.readyState === 'loading') {
        document.addEventListener('DOMContentLoaded', function() {
            init();
            main();
        });
    } else {
        init();
        main();
    }
})();
