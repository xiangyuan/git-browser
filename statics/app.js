// 全局JavaScript函数

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
    const commits = Array.from(checkboxes).map(cb => cb.value);
    
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
    .then(res => res.json())
    .then(data => {
        btn.disabled = false;
        btn.textContent = '🍒 Cherry-pick Selected';
        
        if (data.success) {
            showMessage(
                `✅ Successfully cherry-picked ${data.count} commits to ${targetBranch}!\n` +
                `Next step: Click "Push to Remote" to sync with the server.`,
                'success'
            );
            document.getElementById('push-btn').style.display = 'block';
            
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
            showMessage(`❌ Cherry-pick failed: ${data.error}\n\nPicked ${data.count} commits before failure.`, 'error');
        }
    })
    .catch(err => {
        btn.disabled = false;
        btn.textContent = '🍒 Cherry-pick Selected';
        showMessage(`❌ Error: ${err.message}`, 'error');
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
    const msgDiv = document.getElementById('status-message');
    if (!msgDiv) return;
    
    msgDiv.style.display = 'block';
    msgDiv.textContent = text;
    msgDiv.className = `msg-${type}`;
}

// 更新已cherry-pick的数量
function updateCherryPickedCount() {
    const disabledCount = document.querySelectorAll('.commit-checkbox:disabled').length;
    const countSpan = document.getElementById('cherry-picked-count');
    if (countSpan) {
        countSpan.textContent = disabledCount > 0 ? `(✓ ${disabledCount} cherry-picked)` : '';
    }
}
