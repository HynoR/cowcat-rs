/**
 * catpaw.style.js - 完整的页面渲染逻辑
 * 
 * 基于 catpaw.core.js 提供的核心数据，实现与 catpaw.html 一致的界面效果。
 * 需要配合 catpaw.css 使用。
 */

(function() {
    'use strict';

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

    // ===== State =====
    let manualRedirect = false;
    let pendingRedirectUrl = null;
    let showErrorDetails = false;
    let startTime = null;

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

    function computeProgress(totalHashes, bits) {
        if (!bits || bits <= 0) return 0;
        const reportAs = Math.max(1, bits / 4);
        const successProb = Math.pow(16, -reportAs);
        const missProb = Math.pow(1 - successProb, totalHashes);
        const progress = (1 - Math.pow(missProb, 2)) * 100;
        if (!Number.isFinite(progress)) return 0;
        return Math.min(95, Math.max(0, progress));
    }

    function resetHashStats() {
        updateHashStats(0, 0);
    }

    // ===== DOM Update Functions =====

    function updateProgress(value, text) {
        const progress = Math.min(100, Math.max(0, value));
        if (elements.progressBar) {
            elements.progressBar.style.width = progress + '%';
        }
        if (elements.progressPercent) {
            elements.progressPercent.textContent = Math.round(progress) + '%';
        }
        if (elements.progressText && text) {
            elements.progressText.textContent = text;
        }
    }

    function updateStatus(text) {
        if (elements.statusContainer) {
            elements.statusContainer.textContent = text;
        }
    }

    function updateHashStats(rate, total) {
        if (elements.hashRateValue) {
            elements.hashRateValue.textContent = formatHashRate(rate);
        }
        if (elements.totalHashesValue) {
            elements.totalHashesValue.textContent = formatTotalHashes(total);
        }
        if (elements.hashStatsContainer) {
            elements.hashStatsContainer.style.display = (rate > 0 || total > 0) ? 'grid' : 'none';
        }
    }

    function showError(message) {
        if (elements.statusContainer) {
            elements.statusContainer.style.display = 'none';
        }
        if (elements.errorContainer) {
            elements.errorContainer.style.display = 'block';
        }
        if (elements.errorMessage) {
            elements.errorMessage.textContent = message;
        }
    }

    function escapeHtml(text) {
        const div = document.createElement('div');
        div.textContent = text;
        return div.innerHTML;
    }

    // ===== Event Listeners =====

    function initEventListeners() {
        if (elements.errorDetailsToggle) {
            elements.errorDetailsToggle.addEventListener('click', function() {
                showErrorDetails = !showErrorDetails;
                elements.errorDetailsToggle.textContent =
                    showErrorDetails ? '隐藏错误详情' : '显示错误详情';
                if (elements.errorDetailsContent) {
                    elements.errorDetailsContent.style.display =
                        showErrorDetails ? 'block' : 'none';
                }
            });
        }

        // 图片点击事件 - 切换为手动跳转模式
        if (elements.visualImage1) {
            elements.visualImage1.addEventListener('click', function() {
                manualRedirect = true;
                console.log('Manual redirect mode enabled');
            });
        }
        if (elements.visualImage2) {
            elements.visualImage2.addEventListener('click', function() {
                manualRedirect = true;
                console.log('Manual redirect mode enabled');
            });
        }

        // 手动跳转按钮点击事件
        if (elements.manualRedirectBtn) {
            elements.manualRedirectBtn.addEventListener('click', function() {
                if (pendingRedirectUrl) {
                    window.location.href = pendingRedirectUrl;
                }
            });
        }
    }

    // ===== State Change Handler =====

    function handleStateChange(data) {
        const { state, redirect, error } = data;

        // 根据状态更新 UI
        switch (state) {
            case 'idle':
                startTime = Date.now();
                resetHashStats();
                updateStatus('正在准备挑战任务...');
                updateProgress(0, '初始化');
                break;

            case 'solving':
                if (!startTime) startTime = Date.now();
                updateStatus('计算中...');
                updateProgress(10, 'Working...');
                break;

            case 'verifying':
                updateStatus('正在验证解决方案...');
                updateProgress(99, '正在验证解决方案...');
                break;

            case 'success':
                updateProgress(100, '验证成功！');

                // 切换图片
                if (elements.visualImage1) {
                    elements.visualImage1.style.display = 'none';
                }
                if (elements.visualImage2) {
                    elements.visualImage2.style.display = 'block';
                }

                // 检查是否是手动跳转模式
                if (manualRedirect) {
                    updateStatus('验证成功！');
                    pendingRedirectUrl = redirect;
                    if (elements.manualRedirectContainer) {
                        elements.manualRedirectContainer.style.display = 'block';
                    }
                } else {
                    updateStatus('验证成功！正在跳转...');
                }
                break;

            case 'error':
                showError(error || '发生未知错误');
                resetHashStats();
                
                // 进度条回退动画
                let failProgress = 99;
                const failInterval = setInterval(function() {
                    failProgress = Math.max(0, failProgress - 0.5);
                    const progressMsg = failProgress > 0
                        ? '错误: ' + error + ' (' + Math.round(failProgress) + '%)'
                        : '验证失败';
                    updateProgress(failProgress, progressMsg);

                    if (failProgress <= 0) {
                        clearInterval(failInterval);
                        updateProgress(0, '验证失败');
                    }
                }, 50);
                break;
        }
    }

    function handleMetricsChange(data) {
        const { state, totalHashes, hashRate, task } = data;
        if (state !== 'solving') {
            if (!totalHashes && !hashRate) {
                updateHashStats(0, 0);
            }
            return;
        }

        if (!startTime) startTime = Date.now();
        let progress = 0;
        if (task && task.bits && totalHashes > 0) {
            progress = computeProgress(totalHashes, task.bits);
        }

        const elapsedSecs = Math.round((Date.now() - startTime) / 1000);
        const progressText = 'Working... (elapsed ' + elapsedSecs + 's)';
        updateProgress(Math.max(10, progress), progressText);
        updateHashStats(hashRate || 0, totalHashes || 0);
    }

    // ===== Initialization =====

    function init() {
        initElements();
        initEventListeners();

        // 初始化状态显示
        updateStatus('正在准备挑战任务...');
        updateProgress(0, '初始化');

        // 监听 cowcat:state 事件
        document.addEventListener('cowcat:state', function(e) {
            handleStateChange(e.detail);
        });
        document.addEventListener('cowcat:metrics', function(e) {
            handleMetricsChange(e.detail);
        });
    }

    if (document.readyState === 'loading') {
        document.addEventListener('DOMContentLoaded', init);
    } else {
        init();
    }
})();
