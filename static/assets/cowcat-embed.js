(function() {
    'use strict';

    function normalizeBaseUrl(baseUrl) {
        if (typeof baseUrl === 'string' && baseUrl.trim() !== '') {
            return baseUrl.replace(/\/$/, '');
        }
        var script = document.currentScript;
        if (script && script.src) {
            try {
                return new URL(script.src, window.location.href).origin;
            } catch (e) {
                return '';
            }
        }
        var scripts = document.getElementsByTagName('script');
        for (var i = scripts.length - 1; i >= 0; i--) {
            if (scripts[i].src && scripts[i].src.indexOf('cowcat-embed.js') !== -1) {
                try {
                    return new URL(scripts[i].src, window.location.href).origin;
                } catch (e) {
                    return '';
                }
            }
        }
        return '';
    }

    function postJSON(url, data) {
        return fetch(url, {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json'
            },
            body: JSON.stringify(data)
        }).then(function(resp) {
            return resp.text().then(function(text) {
                var payload;
                try {
                    payload = text ? JSON.parse(text) : null;
                } catch (e) {
                    payload = null;
                }
                if (!resp.ok) {
                    var message = payload && payload.error ? payload.error : 'request failed';
                    throw new Error(message);
                }
                return payload;
            });
        });
    }

    function buildPrefix(task) {
        return 'v1|' + task.seed + '|' + task.exp + '|' + task.bits + '|' + task.scope + '|' + task.ua_hash + '|';
    }

    function resolveWorkerCount(task) {
        var maxWorkers = Number(task.workers) || 1;
        var cores = navigator.hardwareConcurrency || 1;
        var suggested = Math.max(1, cores - 1);
        return Math.max(1, Math.min(maxWorkers, suggested));
    }

    function solveTask(task, options) {
        var prefix = buildPrefix(task);
        var bits = Number(task.bits) || 0;
        var workerType = String(task.worker_type || '').trim().toLowerCase() || 'wasm';
        var workerCount = resolveWorkerCount(task);
        var baseUrl = options.baseUrl;
        var workerUrl = baseUrl + '/__cowcatwaf/assets/catpaw.worker.min.js';
        var maxIters = 50000000;
        var startedAt = Date.now();

        return new Promise(function(resolve, reject) {
            var workers = [];
            var finished = false;
            var failures = 0;
            var totalHashes = 0;

            function cleanup() {
                for (var i = 0; i < workers.length; i++) {
                    workers[i].terminate();
                }
            }

            function onFailure(err) {
                if (finished) {
                    return;
                }
                failures += 1;
                if (failures >= workerCount) {
                    finished = true;
                    cleanup();
                    reject(err);
                }
            }

            for (var i = 0; i < workerCount; i++) {
                var worker;
                try {
                    worker = new Worker(workerUrl + '?v=' + startedAt);
                } catch (err) {
                    cleanup();
                    reject(err);
                    return;
                }

                (function(workerIndex) {
                    worker.onmessage = function(ev) {
                        if (finished) {
                            return;
                        }
                        var data = ev.data || {};
                        if (data.type === 'progress' && typeof data.hashes === 'number') {
                            totalHashes += data.hashes;
                            if (typeof options.onProgress === 'function') {
                                options.onProgress({
                                    hashes: data.hashes,
                                    totalHashes: totalHashes,
                                    workerCount: workerCount
                                });
                            }
                            return;
                        }
                        if (data && data.error) {
                            onFailure(new Error(data.error));
                            return;
                        }
                        if (typeof data.nonce === 'string') {
                            finished = true;
                            cleanup();
                            resolve(data.nonce);
                        }
                    };

                    worker.onerror = function(err) {
                        onFailure(err);
                    };

                    worker.postMessage({
                        prefix: prefix,
                        bits: bits,
                        start: workerIndex,
                        step: workerCount,
                        max_iters: maxIters,
                        worker_type: workerType
                    });
                })(i);

                workers.push(worker);
            }
        });
    }

    function requestToken(options) {
        var opts = options || {};
        var baseUrl = normalizeBaseUrl(opts.baseUrl);
        var clientId = String(opts.clientId || '').trim();
        var action = String(opts.action || '').trim();
        var publicKey = String(opts.publicKey || '').trim();

        if (!clientId || !action || !publicKey) {
            return Promise.reject(new Error('clientId, action, publicKey are required'));
        }

        var taskUrl = baseUrl + '/__cowcatwaf/task';
        var verifyUrl = baseUrl + '/__cowcatwaf/verify';

        return postJSON(taskUrl, {
            client_id: clientId,
            action: action,
            public_key: publicKey
        }).then(function(task) {
            if (!task || !task.task_id) {
                throw new Error('invalid task response');
            }
            return solveTask(task, {
                baseUrl: baseUrl,
                onProgress: opts.onProgress
            }).then(function(nonce) {
                return postJSON(verifyUrl, {
                    client_id: clientId,
                    action: action,
                    public_key: publicKey,
                    task_id: task.task_id,
                    nonce: nonce
                });
            });
        }).then(function(resp) {
            if (!resp || !resp.token) {
                throw new Error('token missing');
            }
            if (typeof opts.onToken === 'function') {
                opts.onToken(resp.token, resp);
            }
            return resp.token;
        }).catch(function(err) {
            if (typeof opts.onError === 'function') {
                opts.onError(err);
            }
            throw err;
        });
    }

    function bind(target, options) {
        var element = target;
        if (typeof target === 'string') {
            element = document.querySelector(target);
        }
        if (!element) {
            throw new Error('target not found');
        }

        var running = false;
        element.addEventListener('click', function() {
            if (running) {
                return;
            }
            running = true;
            element.setAttribute('data-cowcat-busy', 'true');
            requestToken(options).finally(function() {
                running = false;
                element.removeAttribute('data-cowcat-busy');
            });
        });
    }

    window.Cowcat = {
        requestToken: requestToken,
        bind: bind
    };
})();
