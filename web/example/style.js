/**
 * style.js - 自定义页面渲染示例
 * 
 * 这是一个示例文件，展示如何通过 catpaw.core.js 提供的机制
 * 来自定义 PoW 验证页面的渲染逻辑。
 * 
 * 注意：此文件不是必需的，没有它 core.js 也能正常完成 PoW 验证。
 * 
 * core.js 只提供核心数据：
 * - state: 当前状态 ('idle' | 'solving' | 'verifying' | 'success' | 'error')
 * - totalHashes: 已计算的哈希数量
 * - task: 解码后的任务数据对象
 * - redirect: 跳转 URL
 * - error: 错误信息
 * 
 * 其他计算（如进度百分比、算力、格式化）由用户自行实现。
 */

(function() {
    'use strict';

    // ===== 用户自定义计算示例 =====

    // 计算进度百分比（基于哈希数和难度）
    function computeProgress(totalHashes, bits) {
        if (!bits || bits <= 0) return 0;
        const reportAs = Math.max(1, bits / 4);
        const successProb = Math.pow(16, -reportAs);
        const missProb = Math.pow(1 - successProb, totalHashes);
        const progress = (1 - Math.pow(missProb, 2)) * 100;
        return Math.min(95, Math.max(0, progress));
    }

    // 格式化哈希数
    function formatHashes(total) {
        if (total >= 1000000000) return (total / 1000000000).toFixed(2) + ' B';
        if (total >= 1000000) return (total / 1000000).toFixed(2) + ' M';
        if (total >= 1000) return (total / 1000).toFixed(2) + ' K';
        return total.toString();
    }

    // ===== 监听 cowcat:state 事件 =====

    let lastUpdateTime = Date.now();
    let lastTotalHashes = 0;

    document.addEventListener('cowcat:state', function(e) {
        const data = e.detail;

        console.log('状态:', data.state, '| 哈希数:', data.totalHashes);

        // 计算算力（用户自行计算）
        const now = Date.now();
        const elapsed = (now - lastUpdateTime) / 1000;
        if (elapsed > 0 && data.totalHashes > lastTotalHashes) {
            const hashRate = Math.round((data.totalHashes - lastTotalHashes) / elapsed);
            console.log('算力:', hashRate, 'H/s');
        }
        lastUpdateTime = now;
        lastTotalHashes = data.totalHashes;

        // 计算进度（用户自行计算）
        if (data.task && data.task.bits) {
            const progress = computeProgress(data.totalHashes, data.task.bits);
            console.log('进度:', progress.toFixed(1) + '%');
        }

        // 处理不同状态
        switch (data.state) {
            case 'idle':
                console.log('任务数据:', data.task);
                console.log('跳转 URL:', data.redirect);
                break;
            case 'solving':
                console.log('正在计算...', formatHashes(data.totalHashes));
                break;
            case 'verifying':
                console.log('正在验证...');
                break;
            case 'success':
                console.log('验证成功！即将跳转到:', data.redirect);
                break;
            case 'error':
                console.error('错误:', data.error);
                break;
        }
    });

    // ===== 读取全局状态示例 =====
    
    // window.__cowcat__ 结构:
    // {
    //     state: 'idle' | 'solving' | 'verifying' | 'success' | 'error',
    //     totalHashes: number,
    //     task: object | null,    // 解码后的任务数据
    //     redirect: string | null, // 跳转 URL
    //     error: string | null
    // }

    // ===== 自定义 DOM 更新示例 =====

    // 用户需要自己更新 DOM，例如：
    // document.addEventListener('cowcat:state', function(e) {
    //     const progressBar = document.getElementById('my-progress-bar');
    //     if (progressBar && e.detail.task) {
    //         const progress = computeProgress(e.detail.totalHashes, e.detail.task.bits);
    //         progressBar.style.width = progress + '%';
    //     }
    //     
    //     const statusEl = document.getElementById('my-status');
    //     if (statusEl) {
    //         statusEl.textContent = e.detail.state;
    //     }
    // });

})();
