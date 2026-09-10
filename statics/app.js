// 全局JavaScript函数

// Timeago 逻辑
function timeAgo(dateString) {
    // 兼容性处理：将 "2023-01-01 12:00:00" 转换为 "2023/01/01 12:00:00"
    // Safari 等浏览器不支持带横杠的日期解析
    let safeDateString = dateString.replace(/-/g, '/');
    
    // 如果没有时区信息，假设是 UTC 时间（因为后端返回的是 UTC 时间格式化后的字符串，但没有带 Z）
    // 或者如果后端返回的是本地时间但没带时区，这里需要根据实际情况调整
    // 观察到你的时间字符串是 "2025-12-18 17:45:42 +0800" 这种格式
    // Date.parse 能正确处理带时区的字符串，但需要确保格式标准
    
    // 尝试直接解析
    let date = new Date(dateString);
    
    // 如果直接解析失败（比如 Safari 不支持横杠），再尝试替换
    if (isNaN(date.getTime())) {
        date = new Date(safeDateString);
    }
    
    // 如果还是无效，且看起来像 "YYYY-MM-DD HH:mm:ss" 这种无时区格式
    // 且我们知道它是 UTC 时间，可以手动追加 "Z"
    // 但根据你的描述，它带了 "+0800"，所以应该能被正确解析为本地时间
    // 问题可能出在后端返回的时间字符串格式上，或者浏览器解析时的默认行为
    
    // 让我们用更稳健的方式：
    // 如果字符串包含 " +0800"，Date 对象会正确识别它。
    // 如果显示"多了8小时"，说明浏览器把它当成了 UTC 时间，然后又加了8小时显示为本地时间？
    // 或者它本身就是 UTC 时间，但被当成了本地时间？
    
    // 假设后端返回的是 "2025-12-18 17:45:42" (UTC)，而你想显示为 "x hours ago"
    // 此时 new Date("...") 会把它当做本地时间处理（即 UTC+8 的 17:45）
    // 实际 UTC 时间是 09:45。
    // 现在的 new Date() 是 UTC+8 的当前时间。
    // 两个一减，差值是对的。
    
    // 但如果后端返回的是 "2025-12-18 17:45:42 +0800"
    // new Date() 解析后，会得到一个绝对时间戳。
    // new Date() (当前时间) 也是一个绝对时间戳。
    // 两者相减，应该得到真实的秒数差。
    
    // 如果你觉得"多了8小时"，可能是因为后端返回的时间其实是 UTC 时间，但格式化成了 "YYYY-MM-DD HH:mm:ss" 且没带时区信息？
    // 这种情况下，浏览器会把它当成本地时间。
    // 比如 UTC 12:00，本地是 20:00。
    // 后端返回 "12:00"。浏览器认为是本地 12:00。
    // 实际当前时间是本地 20:00。
    // 算出来就是 "8 hours ago"。但实际上应该是 "Just now"。
    
    // 修复方案：如果后端给的是 UTC 时间但没带标记，我们需要把它当做 UTC 解析
    // 但如果后端给的是带 "+0800" 的，那解析应该是正确的。
    
    // 针对你的具体描述 "显示的都是多了8小时"，这通常意味着：
    // 真实时间是 "刚刚"，但显示 "8小时前"。
    // 这说明 dateString 被解析出的时间点，比当前时间早了8小时。
    // 比如现在是 18:00 (UTC+8)。
    // dateString 解析出来是 10:00 (UTC+8)。
    // 这意味着 dateString 内容是 "10:00"，且被当成了本地时间。
    // 但实际上那个事件发生在 18:00 (UTC+8)，也就是 10:00 (UTC)。
    // 所以后端给的字符串应该是 "10:00" (UTC时间)，但没带 "Z" 或 "+0000"。
    
    // 让我们尝试强制把输入当做 UTC 处理（如果它没有时区信息）
    if (!dateString.includes('+') && !dateString.includes('Z')) {
        // 假设是 UTC
        date = new Date(dateString + ' Z');
        // 如果加上 Z 后解析失败（比如 Safari），回退
        if (isNaN(date.getTime())) {
             date = new Date(safeDateString.replace(' ', 'T') + 'Z');
        }
    }
    
    // 如果还是无效，回退到原始解析
    if (isNaN(date.getTime())) {
        date = new Date(safeDateString);
    }

    const now = new Date();
    const seconds = Math.floor((now - date) / 1000);
    
    if (isNaN(seconds)) return dateString; // 如果解析失败，返回原字符串

    let interval = seconds / 31536000;
    if (interval > 1) return Math.floor(interval) + " years ago";
    
    interval = seconds / 2592000;
    if (interval > 1) return Math.floor(interval) + " months ago";
    
    interval = seconds / 86400;
    if (interval > 1) return Math.floor(interval) + " days ago";
    
    interval = seconds / 3600;
    if (interval > 1) return Math.floor(interval) + " hours ago";
    
    interval = seconds / 60;
    if (interval > 1) return Math.floor(interval) + " minutes ago";
    
    return Math.floor(seconds) + " seconds ago";
}

function updateTimeAgo() {
    document.querySelectorAll('.timeago').forEach(el => {
        const timestamp = el.getAttribute('datetime');
        if (timestamp) {
            el.textContent = timeAgo(timestamp);
            el.title = timestamp; // 鼠标悬停显示完整时间
        }
    });
}

// 页面加载完成后执行
document.addEventListener('DOMContentLoaded', () => {
    updateTimeAgo();
    // 每分钟更新一次
    setInterval(updateTimeAgo, 60000);

    const flash = sessionStorage.getItem('gitx-status');
    if (flash) {
        sessionStorage.removeItem('gitx-status');
        try {
            const parsed = JSON.parse(flash);
            showMessage(parsed.text, parsed.type || 'success');
        } catch (e) {
            console.error('Failed to restore status message', e);
        }
    }
});

// 分支选择器：交换两个分支
function swapBranches() {
    const fromSelect = document.getElementById('from-branch');
    const toSelect = document.getElementById('to-branch');
    if (fromSelect && toSelect) {
        const temp = fromSelect.value;
        fromSelect.value = toSelect.value;
        toSelect.value = temp;
        fromSelect.form.submit();
    }
}

// 选择所有未禁用的checkbox
function toggleAll(checkbox) {
    const checkboxes = document.querySelectorAll('.commit-checkbox:not(:disabled)');
    checkboxes.forEach(cb => cb.checked = checkbox.checked);
}

// Cherry-pick选中的commits
function cherryPickSelected() {
    const checkboxes = document.querySelectorAll('.commit-checkbox:checked');
    // 获取所有选中的 commits，然后反转顺序（页面上是从新到旧，cherry-pick 需要从旧到新）
    const commits = Array.from(checkboxes).map(cb => cb.value).reverse();
    
    if (commits.length === 0) {
        showMessage('Please select at least one commit', 'warning');
        return;
    }
    
    const targetBranch = document.getElementById('to-branch').value;
    const repoName = document.body.dataset.repoName;
    const confirmMsg = `Cherry-pick ${commits.length} commit(s) to ${targetBranch}?\n\nThis will apply the changes locally. You'll need to push afterwards.`;
    
    if (!confirm(confirmMsg)) {
        return;
    }
    
    const btn = event.target;
    btn.disabled = true;
    btn.textContent = '⏳ Cherry-picking...';
    showMessage(`Cherry-picking ${commits.length} commits...`, 'info');
    
    fetch(`/${repoName}/api/cherry-pick`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
            commits: commits,
            target_branch: targetBranch
        })
    })
    .then(async res => {
        // 先检查 HTTP 状态
        if (!res.ok) {
            const text = await res.text();
            throw new Error(`HTTP ${res.status}: ${text}`);
        }
        // 尝试解析 JSON
        return res.json();
    })
    .then(data => {
        console.log('Cherry-pick response:', data);
        btn.disabled = false;
        btn.textContent = '🍒 Cherry-pick Selected';
        
        if (data.success) {
            let message = `✅ Successfully cherry-picked ${data.count} commits to ${targetBranch}!`;
            if (data.skipped > 0) {
                message += `\n(${data.skipped} empty commit(s) skipped)`;
            }
            message += `\nNext step: Click "Push to Remote" to sync with the server.`;
            console.log('Showing success message:', message);
            showMessage(message, 'success');
            
            const pushBtn = document.getElementById('push-btn');
            if (pushBtn) {
                pushBtn.style.display = 'block';
            }
            
            checkboxes.forEach(cb => {
                const row = cb.closest('tr');
                row.style.opacity = '0.5';
                row.style.background = '#f6f8fa';
                cb.disabled = true;
                cb.checked = false;
                
                const messageCell = row.cells[2];
                if (!messageCell.textContent.startsWith('✓ ')) {
                    const link = messageCell.querySelector('a');
                    if (link) {
                        link.textContent = '✓ ' + link.textContent;
                    }
                }
            });
            document.getElementById('select-all').checked = false;
            updateCherryPickedCount();
        } else {
            let errorMsg = `❌ Cherry-pick failed: ${data.error}\n\nPicked ${data.count} commits before failure.`;
            if (data.skipped > 0) {
                errorMsg += `\n(${data.skipped} empty commit(s) skipped)`;
            }
            showMessage(errorMsg, 'error');
        }
    })
    .catch(err => {
        btn.disabled = false;
        btn.textContent = '🍒 Cherry-pick Selected';
        showMessage(`❌ Error: ${err.message}`, 'error');
        console.error('Cherry-pick error:', err);
    });
}

// Push到远程
function pushChanges() {
    const targetBranch = document.getElementById('to-branch').value;
    const repoName = document.body.dataset.repoName;
    
    if (!confirm(`Push local changes to origin/${targetBranch}?`)) {
        return;
    }
    
    const btn = document.getElementById('push-btn');
    btn.disabled = true;
    btn.textContent = '⏳ Pushing...';
    showMessage('Pushing to remote...', 'info');
    
    fetch(`/${repoName}/api/push`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ branch: targetBranch })
    })
    .then(res => res.json())
    .then(data => {
        if (data.success) {
            showMessage('✅ Successfully pushed to remote! Refreshing...', 'success');
            setTimeout(() => window.location.reload(), 1500);
        } else {
            btn.disabled = false;
            btn.textContent = '↑ Push to Remote';
            showMessage(`❌ Push failed: ${data.error}`, 'error');
        }
    })
    .catch(err => {
        btn.disabled = false;
        btn.textContent = '↑ Push to Remote';
        showMessage(`❌ Error: ${err.message}`, 'error');
    });
}

// 显示状态消息
function showMessage(text, type) {
    console.log('showMessage called:', { text, type });
    const msgDiv = document.getElementById('status-message');
    console.log('msgDiv element:', msgDiv);
    
    if (!msgDiv) {
        console.error('status-message element not found');
        alert(text); // 备用方案：使用 alert
        return;
    }
    
    // 移除 hidden 类并设置消息类型类
    msgDiv.className = `msg-${type}`;
    msgDiv.textContent = text;
    console.log('Message div updated:', { className: msgDiv.className, display: window.getComputedStyle(msgDiv).display });
    
    // 滚动到消息位置
    msgDiv.scrollIntoView({ behavior: 'smooth', block: 'nearest' });
    
    // 成功消息5秒后自动消失，其他消息保持显示
    if (type === 'success') {
        setTimeout(() => {
            msgDiv.className = 'hidden';
            console.log('Message hidden after timeout');
        }, 5000);
    }
}

// 更新已cherry-pick的数量
function updateCherryPickedCount() {
    const disabledCount = document.querySelectorAll('.commit-checkbox:disabled').length;
    const countSpan = document.getElementById('cherry-picked-count');
    if (countSpan) {
        countSpan.textContent = disabledCount > 0 ? `(✓ ${disabledCount} cherry-picked)` : '';
    }
}

// 手动同步仓库索引
function syncRepository() {
    const repoName = document.body.dataset.repoName;
    const btn = document.getElementById('sync-btn');
    
    btn.disabled = true;
    btn.textContent = '⏳ Syncing...';
    showMessage('Fetching from remote and re-indexing...', 'info');
    
    fetch(`/${repoName}/api/sync`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' }
    })
    .then(async res => {
        if (!res.ok) {
            const text = await res.text();
            throw new Error(`HTTP ${res.status}: ${text}`);
        }
        return res.json();
    })
    .then(data => {
        btn.disabled = false;
        btn.textContent = '🔄 Sync';
        
        if (data.success) {
            showMessage(`✅ ${data.message}. Refreshing page...`, 'success');
            setTimeout(() => window.location.reload(), 1500);
        } else {
            showMessage(`❌ Sync failed: ${data.message}`, 'error');
        }
    })
    .catch(err => {
        btn.disabled = false;
        btn.textContent = '🔄 Sync';
        showMessage(`❌ Error: ${err.message}`, 'error');
    });
}

// Merge branches
function mergeBranches() {
    const fromBranch = document.getElementById('from-branch').value;
    const toBranch = document.getElementById('to-branch').value;
    const repoName = document.body.dataset.repoName;
    
    if (fromBranch === toBranch) {
        showMessage('⚠️ Source and target branches are the same', 'warning');
        return;
    }
    
    const confirmMsg = `Merge "${fromBranch}" into "${toBranch}"?\n\nThis will perform a git merge locally. You'll need to push afterwards.`;
    
    if (!confirm(confirmMsg)) {
        return;
    }
    
    // Find the merge button
    const btns = document.querySelectorAll('.btn-merge');
    const btn = btns.length > 0 ? btns[0] : null;
    
    if (btn) {
        btn.disabled = true;
        btn.textContent = '⏳ Merging...';
    }
    showMessage(`Merging ${fromBranch} into ${toBranch}...`, 'info');
    
    fetch(`/${repoName}/api/merge`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
            source_branch: fromBranch,
            target_branch: toBranch
        })
    })
    .then(async res => {
        if (!res.ok) {
            const text = await res.text();
            throw new Error(`HTTP ${res.status}: ${text}`);
        }
        return res.json();
    })
    .then(data => {
        if (btn) {
            btn.disabled = false;
            btn.textContent = '🔀 Merge';
        }
        
        if (data.success) {
            let message = `✅ ${data.message}`;
            if (data.message && !data.message.includes('Already up to date')) {
                message += `\nNext step: Click "Push to Remote" to sync with the server.`;
            }
            sessionStorage.setItem('gitx-status', JSON.stringify({ text: message, type: 'success' }));
            window.location.reload();
        } else {
            showMessage(`❌ Merge failed: ${data.error}`, 'error');
        }
    })
    .catch(err => {
        if (btn) {
            btn.disabled = false;
            btn.textContent = '🔀 Merge';
        }
        showMessage(`❌ Error: ${err.message}`, 'error');
    });
}
