let taskQueue = [];
let currentTask = null;
let taskIdCounter = 0;
let taskOrderCounter = 0;
const categoryOrder = ['stt', 'embedding', 'summary'];
let currentCategory = null;
let currentOverlayFile = null; // Track currently viewed file in overlay
let lastSimilarDocFilePath = null;
let lastSimilarDocUserFilename = null;

let overlayEditing = false;
let lastEditedRecordId = null;
let lastEditedFileIdentifier = null;

let progressSocket = null;

function initWebSocket() {
    progressSocket = new WebSocket('ws://localhost:8765');
    progressSocket.onmessage = (event) => {
        try {
            const data = JSON.parse(event.data);
            const tasks = [currentTask, ...taskQueue];
            const task = tasks.find(t => t && t.taskId === data.task_id);
            if (task) {
                task.progress = data.message;
                updateQueueDisplay();
            }
        } catch (e) {
            console.error('WebSocket message error:', e);
        }
    };
}

function isAudioFile(file) {
    if (!file) return false;
    if (file.type && file.type.startsWith('audio/')) {
        return true;
    }

    const audioExtensions = ['.flac', '.m4a', '.mp3', '.mp4', '.mpeg', '.mpga', '.oga', '.ogg', '.wav', '.webm'];
    const name = (file.name || '').toLowerCase();
    return audioExtensions.some(ext => name.endsWith(ext));
}

function initializeDropZone() {
    const dropZone = document.getElementById('dropZone');
    const fileInput = document.getElementById('fileInput');
    if (!dropZone || !fileInput) return;

    const preventDefaults = (event) => {
        event.preventDefault();
        event.stopPropagation();
    };

    ['dragenter', 'dragover', 'dragleave', 'drop'].forEach(eventName => {
        dropZone.addEventListener(eventName, preventDefaults, false);
    });

    const addHighlight = () => dropZone.classList.add('dragover');
    const removeHighlight = (event) => {
        const related = event?.relatedTarget;
        if (!related || !dropZone.contains(related)) {
            dropZone.classList.remove('dragover');
        }
    };

    dropZone.addEventListener('dragenter', addHighlight);
    dropZone.addEventListener('dragover', addHighlight);
    dropZone.addEventListener('dragleave', removeHighlight);
    dropZone.addEventListener('drop', (event) => {
        dropZone.classList.remove('dragover');

        const droppedFiles = Array.from(event.dataTransfer?.files || []);
        if (droppedFiles.length === 0) return;

        if (typeof DataTransfer === 'undefined') {
            showTemporaryStatus('이 브라우저에서는 드래그 앤 드롭 추가가 지원되지 않습니다. 파일 선택 버튼을 이용해주세요.', 'warning', 5000);
            return;
        }

        const dataTransfer = new DataTransfer();
        const existingFiles = Array.from(fileInput.files || []);
        const existingKeys = new Set(existingFiles.map(file => `${file.name}-${file.size}-${file.lastModified}`));
        existingFiles.forEach(file => dataTransfer.items.add(file));

        const acceptedFiles = [];
        const rejectedFiles = [];
        const duplicateFiles = [];

        droppedFiles.forEach(file => {
            if (!isAudioFile(file)) {
                rejectedFiles.push(file.name);
                return;
            }

            const key = `${file.name}-${file.size}-${file.lastModified}`;
            if (existingKeys.has(key)) {
                duplicateFiles.push(file.name);
                return;
            }

            dataTransfer.items.add(file);
            existingKeys.add(key);
            acceptedFiles.push(file.name);
        });

        if (acceptedFiles.length > 0) {
            fileInput.files = dataTransfer.files;
            showTemporaryStatus(`${acceptedFiles.length}개의 파일이 업로드 대기 목록에 추가되었습니다.`, 'success');
        }

        if (duplicateFiles.length > 0) {
            showTemporaryStatus(`이미 추가된 파일을 제외했습니다: ${duplicateFiles.join(', ')}`, 'info');
        }

        if (acceptedFiles.length === 0 && rejectedFiles.length === 0) {
            return;
        }

        if (rejectedFiles.length > 0) {
            showTemporaryStatus(`지원되지 않는 형식이 제외되었습니다: ${rejectedFiles.join(', ')}`, 'warning');
        }
    });

    ['dragenter', 'dragover', 'dragleave', 'drop'].forEach(eventName => {
        document.addEventListener(eventName, preventDefaults, false);
    });

    dropZone.addEventListener('click', () => fileInput.click());
    dropZone.addEventListener('keydown', (event) => {
        if (event.key === 'Enter' || event.key === ' ') {
            event.preventDefault();
            fileInput.click();
        }
    });

    fileInput.addEventListener('change', () => {
        const files = Array.from(fileInput.files || []);
        if (files.length > 0) {
            showTemporaryStatus(`${files.length}개의 파일이 업로드 대기 중입니다.`, 'info');
        }
    });
}

// Function to normalize Korean text to NFC form for proper display
function normalizeKorean(text) {
    if (typeof text !== 'string') return text;
    return text.normalize('NFC');
}

function escapeHtml(text) {
    if (typeof text !== 'string') return '';
    return text
        .replace(/&/g, '&amp;')
        .replace(/</g, '&lt;')
        .replace(/>/g, '&gt;')
        .replace(/"/g, '&quot;')
        .replace(/'/g, '&#39;');
}
function showTemporaryStatus(message, variant = 'success', duration = 3000) {
    const status = document.getElementById('status');
    if (!status) return;

    const variants = {
        success: { background: '#d4edda', border: '#c3e6cb', color: '#155724', icon: '✅' },
        error: { background: '#f8d7da', border: '#f5c6cb', color: '#721c24', icon: '⚠️' },
        warning: { background: '#fff3cd', border: '#ffeeba', color: '#856404', icon: '⚠️' },
        info: { background: '#d1ecf1', border: '#bee5eb', color: '#0c5460', icon: 'ℹ️' }
    };

    const style = variants[variant] || variants.info;
    const originalContent = status.innerHTML;
    const messageHtml = `<div style="background: ${style.background}; border: 1px solid ${style.border}; padding: 10px; border-radius: 5px; margin: 10px 0; color: ${style.color};">${style.icon} ${message}</div>`;

    status.innerHTML = messageHtml;

    if (duration > 0) {
        setTimeout(() => {
            if (status.innerHTML === messageHtml) {
                status.innerHTML = originalContent;
            }
        }, duration);
    }
}
const summaryPopup = document.getElementById('summaryPopup');
const summaryOnlyBtn = document.getElementById('summaryOnlyBtn');
const summaryCancelBtn = document.getElementById('summaryCancelBtn');
const sttConfirmPopup = document.getElementById('sttConfirmPopup');
const sttConfirmOkBtn = document.getElementById('sttConfirmOkBtn');
const sttConfirmCancelBtn = document.getElementById('sttConfirmCancelBtn');
const similarDocsPopup = document.getElementById('similarDocsPopup');
const similarDocsCloseBtn = document.getElementById('similarDocsCloseBtn');
const similarDocsRefreshBtn = document.getElementById('similarDocsRefreshBtn');
const similarDocsList = document.getElementById('similarDocsList');
const modelSettingsPopup = document.getElementById('modelSettingsPopup');
const modelSettingsCloseBtn = document.getElementById('modelSettingsCloseBtn');
const modelSettingsCancelBtn = document.getElementById('modelSettingsCancelBtn');
const modelSettingsConfirmBtn = document.getElementById('modelSettingsConfirmBtn');
const modelSettingsStopBtn = document.getElementById('modelSettingsStopBtn');
const overlayEdit = document.getElementById('overlayEdit');
const overlaySave = document.getElementById('overlaySave');
const overlayEditor = document.getElementById('overlayEditor');
const sttEditResetPopup = document.getElementById('sttEditResetPopup');
const sttEditResetConfirmBtn = document.getElementById('sttEditResetConfirmBtn');
const sttEditResetCloseBtn = document.getElementById('sttEditResetCloseBtn');
const resetAllBtn = document.getElementById('resetAllBtn');
const resetAllPopup = document.getElementById('resetAllPopup');
const resetAllMasterCheckbox = document.getElementById('resetAllMaster');
const resetAllOptionCheckboxes = Array.from(document.querySelectorAll('#resetAllPopup .reset-option'));
const resetAllConfirmBtn = document.getElementById('resetAllConfirmBtn');
const resetAllCancelBtn = document.getElementById('resetAllCancelBtn');
const themeToggle = document.getElementById('themeToggle');
const searchResultsContainer = document.getElementById('searchResults');
const keywordGroup = document.getElementById('keywordGroup');
const similarGroup = document.getElementById('similarGroup');
const keywordResultsList = document.getElementById('keywordResults');
const similarResultsList = document.getElementById('similarResults');
const searchMessage = document.getElementById('searchMessage');

function applyTheme(theme) {
    if (theme === 'dark') {
        document.body.classList.add('dark-mode');
        themeToggle.textContent = '☀️';
    } else {
        document.body.classList.remove('dark-mode');
        themeToggle.textContent = '🌙';
    }
    localStorage.setItem('theme', theme);
}

function initTheme() {
    const saved = localStorage.getItem('theme');
    if (saved) {
        applyTheme(saved);
    } else {
        const prefersDark = window.matchMedia && window.matchMedia('(prefers-color-scheme: dark)').matches;
        applyTheme(prefersDark ? 'dark' : 'light');
    }

    themeToggle.addEventListener('click', () => {
        const isDark = document.body.classList.contains('dark-mode');
        applyTheme(isDark ? 'light' : 'dark');
    });

    window.matchMedia('(prefers-color-scheme: dark)').addEventListener('change', e => {
        if (!localStorage.getItem('theme')) {
            applyTheme(e.matches ? 'dark' : 'light');
        }
    });
}

function showTextOverlay(url, fileType = null, displayName = null) {
    const overlay = document.getElementById('textOverlay');
    const content = document.getElementById('overlayContent');
    const download = document.getElementById('overlayDownload');

    let resolvedUrl = url;
    let identifier = null;

    if (typeof url === 'string') {
        try {
            const parsed = new URL(url, window.location.origin);
            resolvedUrl = parsed.pathname + parsed.search;
            if (parsed.pathname.startsWith('/download/')) {
                identifier = parsed.pathname.substring('/download/'.length);
            }
        } catch (err) {
            resolvedUrl = url;
        }

        if (!identifier && resolvedUrl.includes('/download/')) {
            identifier = resolvedUrl.split('/download/')[1];
        }
    }

    if (identifier) {
        identifier = identifier.split('?')[0];
    }

    const resolvedType = fileType || (resolvedUrl && resolvedUrl.includes('.summary.') ? 'summary' : 'stt');

    // Store current file info for deletion or editing
    currentOverlayFile = {
        url: url,
        type: resolvedType,
        identifier: identifier
    };

    lastEditedRecordId = null;
    lastEditedFileIdentifier = null;
    exitOverlayEditMode(false);
    hideSttEditResetPopup();

    if (overlayEdit) {
        if (resolvedType === 'stt') {
            overlayEdit.style.display = 'inline-block';
            overlayEdit.textContent = '수정';
        } else {
            overlayEdit.style.display = 'none';
        }
    }

    if (overlaySave) {
        overlaySave.style.display = 'none';
        overlaySave.disabled = false;
        overlaySave.textContent = '저장';
    }

    if (overlayEditor) {
        overlayEditor.value = '';
    }

    overlay.style.display = 'flex';
    content.textContent = '로딩중...';
    download.href = url;
    download.setAttribute('download', displayName ? normalizeKorean(displayName) : '');

    fetch(url)
        .then(resp => resp.text())
        .then(text => {
            content.textContent = text;
            if (overlayEditor) {
                overlayEditor.value = text;
            }
        })
        .catch(() => {
            const errorText = '파일을 불러오지 못했습니다.';
            content.textContent = errorText;
            if (overlayEditor) {
                overlayEditor.value = errorText;
            }
        });
}

function enterOverlayEditMode() {
    if (!overlayEditor) return;
    const content = document.getElementById('overlayContent');
    if (content) {
        overlayEditor.value = overlayEditor.value || content.textContent || '';
        content.style.display = 'none';
    }

    overlayEditor.style.display = 'block';
    overlayEditing = true;

    if (overlaySave) {
        overlaySave.style.display = 'inline-block';
        overlaySave.disabled = false;
        overlaySave.textContent = '저장';
    }

    if (overlayEdit) {
        overlayEdit.textContent = '취소';
    }

    setTimeout(() => {
        overlayEditor.focus();
        const length = overlayEditor.value.length;
        overlayEditor.setSelectionRange(length, length);
    }, 0);
}

function exitOverlayEditMode(resetEditorValue = true) {
    const content = document.getElementById('overlayContent');

    if (overlayEditor) {
        overlayEditor.style.display = 'none';
        if (resetEditorValue && content) {
            overlayEditor.value = content.textContent || '';
        }
    }

    if (content) {
        content.style.display = 'block';
    }

    overlayEditing = false;

    if (overlaySave) {
        overlaySave.style.display = 'none';
        overlaySave.disabled = false;
        overlaySave.textContent = '저장';
    }

    if (overlayEdit) {
        overlayEdit.textContent = '수정';
    }
}

function showSttEditResetPopup() {
    if (sttEditResetPopup) {
        sttEditResetPopup.style.display = 'flex';
    }
}

function hideSttEditResetPopup() {
    if (sttEditResetPopup) {
        sttEditResetPopup.style.display = 'none';
    }
}

function showResetAllPopup() {
    if (!resetAllPopup) return;

    resetAllPopup.style.display = 'flex';

    if (resetAllMasterCheckbox) {
        resetAllMasterCheckbox.checked = true;
    }

    if (resetAllOptionCheckboxes && resetAllOptionCheckboxes.length > 0) {
        resetAllOptionCheckboxes.forEach(checkbox => {
            checkbox.checked = true;
        });
    }

    updateResetAllConfirmState();
}

function hideResetAllPopup() {
    if (!resetAllPopup) return;
    resetAllPopup.style.display = 'none';

    if (resetAllConfirmBtn) {
        resetAllConfirmBtn.disabled = false;
        resetAllConfirmBtn.textContent = '확인';
    }
}

function updateResetAllConfirmState() {
    if (!resetAllConfirmBtn) return;

    const hasOptions = resetAllOptionCheckboxes && resetAllOptionCheckboxes.length > 0;
    const selectedCount = hasOptions ? resetAllOptionCheckboxes.filter(opt => opt.checked).length : 0;

    if (resetAllMasterCheckbox) {
        const allChecked = hasOptions && selectedCount === resetAllOptionCheckboxes.length;
        resetAllMasterCheckbox.checked = allChecked;
    }

    resetAllConfirmBtn.disabled = selectedCount === 0;
}

async function handleResetAllConfirm() {
    if (!resetAllOptionCheckboxes || resetAllOptionCheckboxes.length === 0) {
        showTemporaryStatus('초기화할 항목이 없습니다.', 'warning');
        return;
    }

    const selectedTasks = resetAllOptionCheckboxes
        .filter(opt => opt.checked)
        .map(opt => opt.dataset.task)
        .filter(Boolean);

    if (selectedTasks.length === 0) {
        showTemporaryStatus('초기화할 항목을 선택하세요.', 'warning');
        return;
    }

    if (!resetAllConfirmBtn) return;

    const originalText = resetAllConfirmBtn.textContent;
    resetAllConfirmBtn.disabled = true;
    resetAllConfirmBtn.textContent = '초기화 중...';

    try {
        const response = await fetch('/reset_all_tasks', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ tasks: selectedTasks })
        });

        const data = await response.json().catch(() => ({}));

        if (!response.ok || data.success === false) {
            const errorMessage = data.message || data.error || '초기화에 실패했습니다.';
            throw new Error(errorMessage);
        }

        const message = data.message || '선택한 항목이 초기화되었습니다.';
        showTemporaryStatus(message, 'success', 5000);

        clearQueuedTasksByCategory(new Set(selectedTasks));
        hideResetAllPopup();
        loadHistory();
    } catch (error) {
        console.error('Bulk reset error:', error);
        showTemporaryStatus(error.message || '초기화 중 오류가 발생했습니다.', 'error', 5000);
    } finally {
        if (resetAllConfirmBtn) {
            resetAllConfirmBtn.disabled = false;
            resetAllConfirmBtn.textContent = originalText;
        }
    }
}

// Progress updates are pushed via WebSocket; polling functions are no-ops
function startProgressPolling(task) {}
function stopProgressPolling() {}

document.getElementById('overlayClose').addEventListener('click', () => {
    const overlay = document.getElementById('textOverlay');
    overlay.style.display = 'none';
    exitOverlayEditMode();
    hideSttEditResetPopup();
    lastEditedRecordId = null;
    lastEditedFileIdentifier = null;
});

document.getElementById('overlayCopy').addEventListener('click', () => {
    const overlayContent = document.getElementById('overlayContent');
    if (!overlayContent) return;

    const text = normalizeKorean(overlayContent.textContent || '');
    if (!text.trim()) {
        alert('복사할 내용이 없습니다.');
        return;
    }

    const fallbackCopy = () => {
        const textarea = document.createElement('textarea');
        textarea.value = text;
        textarea.setAttribute('readonly', '');
        textarea.style.position = 'absolute';
        textarea.style.left = '-9999px';
        document.body.appendChild(textarea);
        textarea.select();
        try {
            document.execCommand('copy');
            alert('내용을 클립보드에 복사했습니다.');
        } catch (err) {
            alert('복사에 실패했습니다. 직접 복사해주세요.');
        }
        document.body.removeChild(textarea);
    };

    if (navigator.clipboard && navigator.clipboard.writeText) {
        navigator.clipboard.writeText(text)
            .then(() => {
                alert('내용을 클립보드에 복사했습니다.');
            })
            .catch(() => {
                fallbackCopy();
            });
    } else {
        fallbackCopy();
    }
});

if (overlayEdit) {
    overlayEdit.addEventListener('click', () => {
        if (!currentOverlayFile || currentOverlayFile.type !== 'stt') {
            return;
        }

        if (!overlayEditing) {
            enterOverlayEditMode();
        } else {
            exitOverlayEditMode();
        }
    });
}

if (overlaySave) {
    overlaySave.addEventListener('click', async () => {
        if (!overlayEditing || !overlayEditor) {
            return;
        }

        const identifier = currentOverlayFile && currentOverlayFile.identifier;
        if (!identifier) {
            showTemporaryStatus('편집할 파일 정보를 확인할 수 없습니다.', 'error', 4000);
            return;
        }

        const newText = overlayEditor.value;
        overlaySave.disabled = true;
        overlaySave.textContent = '저장중...';

        try {
            const response = await fetch('/update_stt_text', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({
                    file_identifier: identifier,
                    content: newText
                })
            });

            const data = await response.json().catch(() => ({}));

            if (!response.ok || data.success === false) {
                const errorMessage = data.error || '텍스트 저장에 실패했습니다.';
                throw new Error(errorMessage);
            }

            const overlayContent = document.getElementById('overlayContent');
            if (overlayContent) {
                overlayContent.textContent = newText;
            }

            overlayEditor.value = newText;
            exitOverlayEditMode(false);

            showTemporaryStatus('STT 텍스트가 저장되었습니다.', 'success');

            lastEditedRecordId = data.record_id || null;
            lastEditedFileIdentifier = identifier;

            if (lastEditedRecordId) {
                showSttEditResetPopup();
            }
        } catch (error) {
            console.error('STT update error:', error);
            showTemporaryStatus(error.message || '텍스트 저장 중 오류가 발생했습니다.', 'error', 5000);
        } finally {
            overlaySave.disabled = false;
            overlaySave.textContent = '저장';
        }
    });
}

document.getElementById('overlayDelete').addEventListener('click', () => {
    if (!currentOverlayFile) return;

    const fileTypeName = currentOverlayFile.type === 'summary' ? '요약을' : 'STT를';
    const confirmed = confirm(`현재 조회중인 ${fileTypeName} 삭제하시겠습니까?`);
    
    if (confirmed) {
        deleteCurrentFile();
    }
});

// Add Esc key listener for text overlay
document.addEventListener('keydown', (e) => {
    if (e.key === 'Escape') {
        const textOverlay = document.getElementById('textOverlay');
        if (textOverlay.style.display === 'flex') {
            textOverlay.style.display = 'none';
            exitOverlayEditMode();
            hideSttEditResetPopup();
            lastEditedRecordId = null;
            lastEditedFileIdentifier = null;
        }
        if (summaryPopup.style.display === 'flex') {
            hideSummaryPopup();
        }
        if (sttConfirmPopup.style.display === 'flex') {
            hideSttConfirmPopup();
        }
        if (similarDocsPopup.style.display === 'flex') {
            hideSimilarDocsPopup();
        }
        if (modelSettingsPopup.style.display === 'flex') {
            hideModelSettingsPopup();
        }
        if (sttEditResetPopup && sttEditResetPopup.style.display === 'flex') {
            hideSttEditResetPopup();
        }
    }
});

function hideSummaryPopup() {
    summaryPopup.style.display = 'none';
}

function showSttConfirmPopup() {
    sttConfirmPopup.style.display = 'flex';
}

function hideSttConfirmPopup() {
    sttConfirmPopup.style.display = 'none';
}

function showSimilarDocuments(filePath, userFilename = null, refresh = false) {
    if (!refresh) {
        lastSimilarDocFilePath = filePath;
        lastSimilarDocUserFilename = userFilename;
    }
    similarDocsPopup.style.display = 'flex';
    similarDocsList.innerHTML = '<p style="color: #6c757d; text-align: center;">로딩 중...</p>';

    // Extract the UUID or relative path from the download URL
    const fileIdentifier = filePath.replace('/download/', '');

    // Create request with optional user filename
    const requestData = { file_identifier: fileIdentifier };
    if (userFilename) {
        requestData.user_filename = userFilename;
    }
    if (refresh) {
        requestData.refresh = true;
    }

    fetch('/similar', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(requestData)
    })
        .then(response => response.json())
        .then(data => {
            if (data.error) {
                similarDocsList.innerHTML = `<p style="color: #dc3545; text-align: center;">${data.error}</p>`;
                return;
            }
            
            if (data.length === 0) {
                similarDocsList.innerHTML = '<p style="color: #6c757d; text-align: center;">유사한 문서가 없습니다.</p>';
                return;
            }
            
            similarDocsList.innerHTML = '';

            data.forEach(doc => {
                const rawFileName = doc.display_name || (doc.file ? doc.file.split('/').pop() : '');
                const displayName = rawFileName ? rawFileName.replace(/\.(md|txt)$/, '') : '';
                const similarityPercent = Math.round(doc.score * 100);
                const summaryRaw = typeof doc.title_summary === 'string' ? doc.title_summary : '';
                const summaryText = normalizeKorean(summaryRaw.trim());

                const item = document.createElement('div');
                item.className = 'similar-doc-item';
                item.tabIndex = 0;
                item.setAttribute('role', 'button');
                item.addEventListener('click', () => {
                    viewSimilarDocument(doc.link, rawFileName);
                });
                item.addEventListener('keydown', (event) => {
                    if (event.key === 'Enter' || event.key === ' ') {
                        event.preventDefault();
                        viewSimilarDocument(doc.link, rawFileName);
                    }
                });

                const nameDiv = document.createElement('div');
                nameDiv.className = 'similar-doc-name';
                nameDiv.textContent = normalizeKorean(displayName || '');
                item.appendChild(nameDiv);

                const scoreDiv = document.createElement('div');
                scoreDiv.className = 'similar-doc-score';
                scoreDiv.textContent = `유사도: ${similarityPercent}%`;
                item.appendChild(scoreDiv);

                if (summaryText) {
                    const summaryDiv = document.createElement('div');
                    summaryDiv.className = 'similar-doc-summary';
                    summaryDiv.textContent = summaryText;
                    item.appendChild(summaryDiv);
                }

                similarDocsList.appendChild(item);
            });
        })
        .catch(error => {
            console.error('유사 문서 검색 오류:', error);
            similarDocsList.innerHTML = '<p style="color: #dc3545; text-align: center;">검색 중 오류가 발생했습니다.</p>';
        });
}

function hideSimilarDocsPopup() {
    similarDocsPopup.style.display = 'none';
}

function showModelSettingsPopup() {
    modelSettingsPopup.style.display = 'flex';
    loadAvailableModels();
}

function hideModelSettingsPopup() {
    modelSettingsPopup.style.display = 'none';
}


async function deleteCurrentFile() {
    if (!currentOverlayFile) return;
    
    try {
        // Extract file identifier from URL
        const fileIdentifier = currentOverlayFile.url.replace('/download/', '');
        
        const response = await fetch('/delete', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ 
                file_identifier: fileIdentifier,
                file_type: currentOverlayFile.type
            })
        });
        
        if (response.ok) {
            // Close overlay
            document.getElementById('textOverlay').style.display = 'none';
            
            // Clear current overlay file
            currentOverlayFile = null;
            
            // Reload history to reflect changes
            loadHistory();
            
            // Show success message
            const status = document.getElementById('status');
            const originalContent = status.innerHTML;
            status.innerHTML = '<div style="background: #d4edda; border: 1px solid #c3e6cb; padding: 10px; border-radius: 5px; margin: 10px 0; color: #155724;">✅ 파일이 삭제되었습니다!</div>';
            
            // Clear message after 3 seconds
            setTimeout(() => {
                if (status.innerHTML.includes('파일이 삭제되었습니다')) {
                    status.innerHTML = originalContent;
                }
            }, 3000);
        } else {
            const errorData = await response.json();
            alert(`삭제 실패: ${errorData.error || '알 수 없는 오류'}`);
        }
    } catch (error) {
        console.error('Delete error:', error);
        alert('삭제 중 오류가 발생했습니다.');
    }
}

async function loadAvailableModels() {
    try {
        const response = await fetch('/models');
        if (response.ok) {
            const data = await response.json();
            
            // Update summarize model dropdown
            const summarizeSelect = document.getElementById('summarizeModel');
            summarizeSelect.innerHTML = '';
            
            if (data.models && data.models.length > 0) {
                data.models.forEach(model => {
                    const option = document.createElement('option');
                    option.value = model;
                    option.textContent = model;
                    if (model === data.default.summarize) {
                        option.textContent += ' (기본값)';
                        option.selected = true;
                    }
                    summarizeSelect.appendChild(option);
                });
            } else {
                const option = document.createElement('option');
                option.value = data.default.summarize;
                option.textContent = data.default.summarize + ' (기본값)';
                option.selected = true;
                summarizeSelect.appendChild(option);
            }
            
            // Update embedding model dropdown (usually only one model available)
            const embeddingSelect = document.getElementById('embeddingModel');
            embeddingSelect.innerHTML = '';
            
            const embeddingOption = document.createElement('option');
            embeddingOption.value = data.default.embedding;
            embeddingOption.textContent = data.default.embedding + ' (기본값)';
            embeddingOption.selected = true;
            embeddingSelect.appendChild(embeddingOption);
            
            // Set current values from localStorage if available
            const savedSettings = JSON.parse(localStorage.getItem('modelSettings') || '{}');
            if (savedSettings.whisper) {
                document.getElementById('whisperModel').value = savedSettings.whisper;
            }
            if (savedSettings.language) {
                document.getElementById('whisperLanguage').value = savedSettings.language;
            }
            if (savedSettings.summarize) {
                document.getElementById('summarizeModel').value = savedSettings.summarize;
            }
            if (savedSettings.embedding) {
                document.getElementById('embeddingModel').value = savedSettings.embedding;
            }
            
        } else {
            console.error('Failed to load models');
            const summarizeSelect = document.getElementById('summarizeModel');
            const embeddingSelect = document.getElementById('embeddingModel');
            summarizeSelect.innerHTML = '<option value="">모델 로딩 실패</option>';
            embeddingSelect.innerHTML = '<option value="">모델 로딩 실패</option>';
        }
    } catch (error) {
        console.error('Error loading models:', error);
        const summarizeSelect = document.getElementById('summarizeModel');
        const embeddingSelect = document.getElementById('embeddingModel');
        summarizeSelect.innerHTML = '<option value="">네트워크 오류</option>';
        embeddingSelect.innerHTML = '<option value="">네트워크 오류</option>';
    }
}

function saveModelSettings() {
    const settings = {
        whisper: document.getElementById('whisperModel').value,
        language: document.getElementById('whisperLanguage').value,
        summarize: document.getElementById('summarizeModel').value,
        embedding: document.getElementById('embeddingModel').value
    };

    localStorage.setItem('modelSettings', JSON.stringify(settings));
    hideModelSettingsPopup();

    // Show success message
    const status = document.getElementById('status');
    const originalContent = status.innerHTML;
    status.innerHTML = '<div style="background: #d4edda; border: 1px solid #c3e6cb; padding: 10px; border-radius: 5px; margin: 10px 0; color: #155724;">✅ 모델 설정이 저장되었습니다!</div>';

    // Clear message after 3 seconds
    setTimeout(() => {
        if (status.innerHTML.includes('모델 설정이 저장되었습니다')) {
            status.innerHTML = originalContent;
        }
    }, 3000);
}

async function requestServerShutdown() {
    const confirmShutdown = confirm('파이썬 서버를 종료하시겠습니까? 진행 중인 작업이 있다면 중단될 수 있습니다.');
    if (!confirmShutdown) {
        return;
    }

    const status = document.getElementById('status');
    const originalContent = status.innerHTML;

    try {
        const response = await fetch('/shutdown', { method: 'POST' });
        const data = await response.json().catch(() => ({}));

        if (response.ok && data.success !== false) {
            hideModelSettingsPopup();
            const message = data.message || '서버 종료 요청이 접수되었습니다. 잠시 후 서버가 종료됩니다.';
            status.innerHTML = '<div style="background: #fff3cd; border: 1px solid #ffeeba; padding: 10px; border-radius: 5px; margin: 10px 0; color: #856404;">🔌 ' + message + '</div>';
        } else {
            const errorMessage = data.error || '서버 종료 요청에 실패했습니다.';
            status.innerHTML = '<div style="background: #f8d7da; border: 1px solid #f5c6cb; padding: 10px; border-radius: 5px; margin: 10px 0; color: #721c24;">⚠️ ' + errorMessage + '</div>';
            setTimeout(() => {
                if (status.innerHTML.includes(errorMessage)) {
                    status.innerHTML = originalContent;
                }
            }, 5000);
        }
    } catch (error) {
        console.error('Server shutdown error:', error);
        const errorMessage = '서버 종료 요청 중 오류가 발생했습니다.';
        status.innerHTML = '<div style="background: #f8d7da; border: 1px solid #f5c6cb; padding: 10px; border-radius: 5px; margin: 10px 0; color: #721c24;">⚠️ ' + errorMessage + '</div>';
        setTimeout(() => {
            if (status.innerHTML.includes(errorMessage)) {
                status.innerHTML = originalContent;
            }
        }, 5000);
    }
}

function viewSimilarDocument(downloadLink, displayName = null) {
    if (!downloadLink) return;

    // Ensure the similar documents popup is hidden before showing the overlay
    hideSimilarDocsPopup();

    showTextOverlay(downloadLink, 'stt', displayName);
}

summaryCancelBtn.addEventListener('click', hideSummaryPopup);
sttConfirmCancelBtn.addEventListener('click', hideSttConfirmPopup);
similarDocsCloseBtn.addEventListener('click', hideSimilarDocsPopup);
similarDocsRefreshBtn.addEventListener('click', () => {
    if (lastSimilarDocFilePath) {
        showSimilarDocuments(lastSimilarDocFilePath, lastSimilarDocUserFilename, true);
    }
});
modelSettingsCloseBtn.addEventListener('click', hideModelSettingsPopup);
modelSettingsCancelBtn.addEventListener('click', hideModelSettingsPopup);
modelSettingsConfirmBtn.addEventListener('click', saveModelSettings);
modelSettingsStopBtn.addEventListener('click', requestServerShutdown);
if (resetAllBtn) {
    resetAllBtn.addEventListener('click', showResetAllPopup);
}
if (resetAllCancelBtn) {
    resetAllCancelBtn.addEventListener('click', hideResetAllPopup);
}
if (resetAllMasterCheckbox) {
    resetAllMasterCheckbox.addEventListener('change', () => {
        const checked = resetAllMasterCheckbox.checked;
        resetAllOptionCheckboxes.forEach(option => {
            option.checked = checked;
        });
        updateResetAllConfirmState();
    });
}
if (resetAllOptionCheckboxes && resetAllOptionCheckboxes.length > 0) {
    resetAllOptionCheckboxes.forEach(option => {
        option.addEventListener('change', updateResetAllConfirmState);
    });
}
if (resetAllConfirmBtn) {
    resetAllConfirmBtn.addEventListener('click', handleResetAllConfirm);
}
if (resetAllPopup) {
    resetAllPopup.addEventListener('click', (event) => {
        if (event.target === resetAllPopup) {
            hideResetAllPopup();
        }
    });
}
if (sttEditResetConfirmBtn) {
    sttEditResetConfirmBtn.addEventListener('click', async () => {
        if (!lastEditedRecordId) {
            hideSttEditResetPopup();
            return;
        }

        sttEditResetConfirmBtn.disabled = true;
        sttEditResetConfirmBtn.textContent = '초기화 중...';

        try {
            const response = await fetch('/reset_summary_embedding', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ record_id: lastEditedRecordId })
            });

            const data = await response.json().catch(() => ({}));

            if (!response.ok || data.success === false) {
                const errorMessage = data.error || '초기화에 실패했습니다.';
                throw new Error(errorMessage);
            }

            showTemporaryStatus('색인 및 요약이 초기화되었습니다.', 'success');
            hideSttEditResetPopup();
            lastEditedRecordId = null;
            lastEditedFileIdentifier = null;
            loadHistory();
        } catch (error) {
            console.error('Reset summary/embedding error:', error);
            showTemporaryStatus(error.message || '초기화 중 오류가 발생했습니다.', 'error', 5000);
        } finally {
            sttEditResetConfirmBtn.disabled = false;
            sttEditResetConfirmBtn.textContent = '초기화';
        }
    });
}

if (sttEditResetCloseBtn) {
    sttEditResetCloseBtn.addEventListener('click', () => {
        hideSttEditResetPopup();
        lastEditedRecordId = null;
        lastEditedFileIdentifier = null;
    });
}

function editFilename(recordId, currentFilename) {
    const filenameElement = document.getElementById(`filename-${recordId}`);
    const originalText = filenameElement.textContent;
    
    // Create input element
    const input = document.createElement('input');
    input.type = 'text';
    input.value = currentFilename;
    input.className = 'filename-input';
    
    // Replace filename display with input
    filenameElement.style.display = 'none';
    filenameElement.parentNode.insertBefore(input, filenameElement.nextSibling);
    
    input.focus();
    input.select();
    
    // Handle save on Enter or blur
    const saveEdit = async (newFilename) => {
        if (newFilename && newFilename.trim() !== '' && newFilename !== currentFilename) {
            try {
                const response = await fetch('/update_filename', {
                    method: 'POST',
                    headers: { 'Content-Type': 'application/json' },
                    body: JSON.stringify({ 
                        record_id: recordId, 
                        filename: newFilename.trim() 
                    })
                });
                
                if (response.ok) {
                    filenameElement.textContent = normalizeKorean(newFilename.trim());
                    loadHistory(); // Reload to reflect changes
                } else {
                    alert('파일명 수정에 실패했습니다.');
                    filenameElement.textContent = originalText;
                }
            } catch (error) {
                alert('파일명 수정 중 오류가 발생했습니다.');
                filenameElement.textContent = originalText;
            }
        } else {
            filenameElement.textContent = originalText;
        }
        
        // Remove input and show filename display
        input.remove();
        filenameElement.style.display = '';
    };
    
    // Cancel edit on Escape
    const cancelEdit = () => {
        input.remove();
        filenameElement.style.display = '';
    };
    
    input.addEventListener('keydown', (e) => {
        if (e.key === 'Enter') {
            e.preventDefault();
            saveEdit(input.value);
        } else if (e.key === 'Escape') {
            e.preventDefault();
            cancelEdit();
        }
    });
    
    input.addEventListener('blur', () => {
        saveEdit(input.value);
    });
}

function setQueuedState(span) {
    span.style.backgroundColor = '#17a2b8';
    span.style.color = 'white';
    span.title = '큐에 추가됨';
    span.onclick = null;
}

function showSummaryPopup(record, span) {
    summaryPopup.style.display = 'flex';
    summaryOnlyBtn.onclick = () => {
        addTaskToQueue(record.id, record.file_path, 'summary', span, record.filename);
        setQueuedState(span);
        hideSummaryPopup();
    };
}

function sortTaskQueue() {
    const sortOrder = document.getElementById('queueSortSelect').value;
    
    if (sortOrder === 'oldest') {
        // 추가순 (오래된 순): 추가 순서대로 정렬
        taskQueue.sort((a, b) => a.order - b.order);
    } else {
        // 기본값 (카테고리별): 현재 진행 중인 카테고리 우선 정렬
        let startIndex = 0;
        if (currentTask) {
            startIndex = categoryOrder.indexOf(currentTask.task);
        } else if (currentCategory) {
            startIndex = categoryOrder.indexOf(currentCategory);
        }
        const order = categoryOrder.slice(startIndex).concat(categoryOrder.slice(0, startIndex));
        taskQueue.sort((a, b) => {
            const diff = order.indexOf(a.task) - order.indexOf(b.task);
            return diff !== 0 ? diff : a.order - b.order;
        });
    }
}

function formatDateTime(isoString) {
    const date = new Date(isoString);
    return date.toLocaleString('ko-KR', {
        year: 'numeric',
        month: '2-digit',
        day: '2-digit',
        hour: '2-digit',
        minute: '2-digit',
        second: '2-digit'
    });
}

function resetSearchDisplay() {
    if (!searchResultsContainer) return;
    searchResultsContainer.classList.add('hidden');
    if (keywordGroup) keywordGroup.classList.add('hidden');
    if (similarGroup) similarGroup.classList.add('hidden');
    if (keywordResultsList) keywordResultsList.innerHTML = '';
    if (similarResultsList) similarResultsList.innerHTML = '';
    if (searchMessage) {
        searchMessage.textContent = '';
        searchMessage.classList.remove('error');
    }
}

function appendKeywordResult(item) {
    if (!keywordResultsList) return;
    const li = document.createElement('li');

    const link = document.createElement('a');
    link.href = encodeURI(item.link || `/download/${item.file}`);
    link.target = '_blank';
    link.rel = 'noopener noreferrer';
    link.textContent = normalizeKorean(item.display_name || item.file || '문서');
    li.appendChild(link);

    const meta = document.createElement('div');
    meta.className = 'search-meta';

    const countSpan = document.createElement('span');
    countSpan.textContent = `사용 횟수: ${item.count}`;
    meta.appendChild(countSpan);

    if (item.source_filename) {
        const originalSpan = document.createElement('span');
        originalSpan.textContent = `원본: ${normalizeKorean(item.source_filename)}`;
        meta.appendChild(originalSpan);
    }

    if (item.uploaded_at) {
        const uploadedSpan = document.createElement('span');
        uploadedSpan.textContent = `업로드: ${formatDateTime(item.uploaded_at)}`;
        meta.appendChild(uploadedSpan);
    }

    if (meta.childNodes.length > 0) {
        li.appendChild(meta);
    }

    keywordResultsList.appendChild(li);
}

function appendSimilarResult(item) {
    if (!similarResultsList) return;
    const li = document.createElement('li');

    const link = document.createElement('a');
    link.href = encodeURI(item.link || `/download/${item.file}`);
    link.target = '_blank';
    link.rel = 'noopener noreferrer';
    link.textContent = normalizeKorean(item.display_name || item.file || '문서');
    li.appendChild(link);

    const meta = document.createElement('div');
    meta.className = 'search-meta';

    const scoreSpan = document.createElement('span');
    const scoreValue = typeof item.score === 'number' && !Number.isNaN(item.score)
        ? item.score.toFixed(3)
        : '정보 없음';
    scoreSpan.textContent = `유사도: ${scoreValue}`;
    meta.appendChild(scoreSpan);

    if (item.source_filename) {
        const originalSpan = document.createElement('span');
        originalSpan.textContent = `원본: ${normalizeKorean(item.source_filename)}`;
        meta.appendChild(originalSpan);
    }

    if (item.uploaded_at) {
        const uploadedSpan = document.createElement('span');
        uploadedSpan.textContent = `업로드: ${formatDateTime(item.uploaded_at)}`;
        meta.appendChild(uploadedSpan);
    }

    li.appendChild(meta);
    similarResultsList.appendChild(li);
}

function addTaskToQueue(recordId, filePath, task, taskElement, filename) {
    // Check for duplicate task before adding
    const existingTask = taskQueue.find(t => 
        t.recordId === recordId && 
        t.task === task
    );
    
    if (existingTask) {
        console.log(`Task ${task} for record ${recordId} already exists in queue`);
        return existingTask.id;
    }
    
    const taskId = 'task_' + Date.now() + '_' + Math.random().toString(36).substr(2, 9);  // Generate unique task ID
    const taskItem = {
        id: ++taskIdCounter,
        taskId: taskId,  // Server-side task ID for cancellation
        recordId: recordId,
        filePath: filePath,
        task: task,
        taskElement: taskElement,
        filename: filename,
        status: 'queued',
        abortController: null,
        order: ++taskOrderCounter
    };
    
    taskQueue.push(taskItem);
    sortTaskQueue();
    updateQueueDisplay();
    
    // Update history display to reflect queue state changes
    loadHistory();
    
    processNextTask();
    
    return taskItem.id;
}

function removeTaskFromQueue(taskId) {
    const taskIndex = taskQueue.findIndex(t => t.id === taskId);
    if (taskIndex !== -1) {
        const task = taskQueue[taskIndex];
        
        // If task is currently processing, send cancellation request to server
        if (task.status === 'processing' && task.taskId) {
            fetch('/cancel', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ task_id: task.taskId })
            }).then(response => response.json())
            .then(result => {
                console.log(`Task cancellation result for ${task.taskId}:`, result);
            })
            .catch(error => {
                console.error(`Error cancelling task ${task.taskId}:`, error);
            });
        }
        
        // Abort the request if it's in progress
        if (task.abortController) {
            task.abortController.abort();
        }
        
        taskQueue.splice(taskIndex, 1);
        updateQueueDisplay();
        currentCategory = taskQueue.length > 0 ? taskQueue[0].task : currentCategory;
        
        // Reload history to restore button states properly
        loadHistory();
    }
    
    // If this was the current task, process next
    if (currentTask && currentTask.id === taskId) {
        currentTask = null;
        currentCategory = taskQueue.length > 0 ? taskQueue[0].task : null;
        processNextTask();
    }
}

function moveTask(taskId, direction) {
    const index = taskQueue.findIndex(t => t.id === taskId);
    if (index === -1) return;
    const task = taskQueue[index];
    let swapIndex = index + direction;
    while (swapIndex >= 0 && swapIndex < taskQueue.length) {
        if (taskQueue[swapIndex].task === task.task) {
            const temp = task.order;
            task.order = taskQueue[swapIndex].order;
            taskQueue[swapIndex].order = temp;
            sortTaskQueue();
            updateQueueDisplay();
            break;
        }
        swapIndex += direction;
    }
}

function moveTaskUp(taskId) { moveTask(taskId, -1); }
function moveTaskDown(taskId) { moveTask(taskId, 1); }

function cancelAllTasks() {
    const tasks = [...taskQueue];
    tasks.forEach(t => removeTaskFromQueue(t.id));
}

document.getElementById('cancelAllBtn').addEventListener('click', cancelAllTasks);

function clearQueuedTasksByCategory(taskNames) {
    if (!taskNames || (taskNames instanceof Set && taskNames.size === 0)) {
        return;
    }

    const taskSet = taskNames instanceof Set ? taskNames : new Set(taskNames);

    const queuedMatches = taskQueue.filter(task => taskSet.has(task.task));
    queuedMatches.forEach(task => removeTaskFromQueue(task.id));

    if (currentTask && taskSet.has(currentTask.task)) {
        if (currentTask.taskId) {
            fetch('/cancel', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ task_id: currentTask.taskId })
            }).catch(error => {
                console.error(`Error cancelling running task ${currentTask.taskId}:`, error);
            });
        }

        if (currentTask.abortController) {
            currentTask.abortController.abort();
        }

        currentTask = null;
        currentCategory = taskQueue.length > 0 ? taskQueue[0].task : null;
        stopProgressPolling();
        updateQueueDisplay();
        processNextTask();
    }
}

function resetTaskElement(taskElement, task) {
    const taskNames = {
        'stt': 'STT',
        'embedding': '색인',
        'summary': '요약'
    };
    
    taskElement.textContent = taskNames[task] || task;
    taskElement.style.backgroundColor = '#6c757d';
    taskElement.style.color = 'white';
    taskElement.style.cursor = 'pointer';
    taskElement.title = '클릭하여 작업 시작';
}

function updateQueueDisplay() {
    const queueList = document.getElementById('queue-list');
    const cancelAllBtn = document.getElementById('cancelAllBtn');
    const sortOrder = document.getElementById('queueSortSelect').value;
    queueList.innerHTML = '';

    if (taskQueue.length === 0 && !currentTask) {
        queueList.innerHTML = '<p style="color: #6c757d; font-style: italic;">진행 중인 작업이 없습니다.</p>';
        cancelAllBtn.style.display = 'none';
        return;
    }

    cancelAllBtn.style.display = 'inline-block';

    const taskNames = {
        'stt': 'STT 변환',
        'summary': '요약'
    };

    const categoryNames = {
        'stt': 'STT 변환',
        'embedding': '색인 생성',
        'correct': '텍스트 교정', 
        'summary': '요약'
    };

    // Get saved model settings to display model info
    const savedSettings = JSON.parse(localStorage.getItem('modelSettings') || '{}');
    const getModelForTask = (task) => {
        if (task === 'stt') {
            return savedSettings.whisper || 'large-v3-turbo';
        } else if (task === 'summary') {
            return savedSettings.summarize || 'gpt-oss:20b';
        } else if (task === 'embedding') {
            return savedSettings.embedding || 'bge-m3:latest';
        }
        return '';
    };

    if (sortOrder === 'oldest') {
        // 추가순: 단순 리스트로 표시
        // Add currentTask first if it exists
        const allTasks = [];
        if (currentTask) {
            allTasks.push({...currentTask, status: 'processing'});
        }
        allTasks.push(...taskQueue);
        
        allTasks.forEach((task, index) => {
            const item = document.createElement('div');
            item.className = task.status === 'processing' ? 'queue-item queue-item-processing' : 'queue-item';
            
            const statusText = task.status === 'processing' ? '진행중' : `대기중 (${index + 1}번째)`;
            const taskName = taskNames[task.task] || task.task;
            const modelName = getModelForTask(task.task);
            
            const info = document.createElement('span');
            const statusClass = task.status === 'processing' ? 'status-processing' : 'status-waiting';
            info.innerHTML = `
                <strong>${normalizeKorean(task.filename)}</strong> - ${taskName}
                ${modelName ? `<span class="model-info">(${modelName})</span>` : ''}
                <span class="status-info ${statusClass}">[${statusText}]</span>
            `;

            const infoContainer = document.createElement('div');
            infoContainer.className = 'info-container';
            infoContainer.appendChild(info);
            
            if (task.status === 'processing' && task.progress) {
                const progressDiv = document.createElement('div');
                progressDiv.className = 'progress-info';
                
                // 진행률 퍼센트 추출
                const percentMatch = task.progress.match(/(\d+)%/);
                if (percentMatch) {
                    const percent = parseInt(percentMatch[1]);
                    
                    // 진행률 바 생성
                    const progressContainer = document.createElement('div');
                    progressContainer.className = 'progress-container';
                    
                    const progressBar = document.createElement('div');
                    progressBar.className = 'progress-bar';
                    progressBar.style.width = `${percent}%`;
                    
                    progressContainer.appendChild(progressBar);
                    progressDiv.appendChild(progressContainer);
                }
                
                const progressText = document.createElement('div');
                progressText.textContent = task.progress;
                progressDiv.appendChild(progressText);
                
                infoContainer.appendChild(progressDiv);
            }


            const cancelBtn = document.createElement('button');
            cancelBtn.textContent = '×';
            cancelBtn.className = 'cancel-btn';
            cancelBtn.title = '작업 취소';
            cancelBtn.onclick = () => removeTaskFromQueue(task.id);

            const btnContainer = document.createElement('div');
            btnContainer.className = 'queue-btn-group';
            if (task.status !== 'processing') {
                const upBtn = document.createElement('button');
                upBtn.textContent = '▲';
                upBtn.className = 'move-btn';
                upBtn.title = '위로 이동';
                upBtn.onclick = () => moveTaskUp(task.id);

                const downBtn = document.createElement('button');
                downBtn.textContent = '▼';
                downBtn.className = 'move-btn';
                downBtn.title = '아래로 이동';
                downBtn.onclick = () => moveTaskDown(task.id);

                btnContainer.appendChild(upBtn);
                btnContainer.appendChild(downBtn);
            }
            btnContainer.appendChild(cancelBtn);

            item.appendChild(infoContainer);
            item.appendChild(btnContainer);
            queueList.appendChild(item);
        });
    } else {
        // 기본값: 카테고리별 그룹화
        const tasksByCategory = {};
        categoryOrder.forEach(category => {
            tasksByCategory[category] = [];
        });

        // Add currentTask first if it exists
        if (currentTask) {
            if (tasksByCategory[currentTask.task]) {
                tasksByCategory[currentTask.task].push({...currentTask, status: 'processing'});
            }
        }
        
        taskQueue.forEach(task => {
            if (tasksByCategory[task.task]) {
                tasksByCategory[task.task].push(task);
            }
        });

        // Display tasks by category
        categoryOrder.forEach(category => {
            const categoryTasks = tasksByCategory[category];
            if (categoryTasks.length === 0) return;

            // Create category header
            const categoryHeader = document.createElement('div');
            categoryHeader.className = 'queue-category-header';
            categoryHeader.textContent = `${categoryNames[category]} (${categoryTasks.length}개)`;
            queueList.appendChild(categoryHeader);

            // Add tasks in this category
            categoryTasks.forEach((task, index) => {
                const item = document.createElement('div');
                let className = task.status === 'processing' ? 'queue-category-item queue-category-item-processing' : 'queue-category-item';
                if (index === categoryTasks.length - 1) {
                    className += ' last-item';
                }
                item.className = className;
                
                const globalIndex = taskQueue.findIndex(t => t.id === task.id);
                const statusText = task.status === 'processing' ? '진행중' : `대기중 (${globalIndex + 1}번째)`;
                const modelName = getModelForTask(task.task);
                const statusClass = task.status === 'processing' ? 'status-processing' : 'status-waiting';
                
                const info = document.createElement('span');
                info.innerHTML = `
                    <strong>${normalizeKorean(task.filename)}</strong>
                    ${modelName ? `<span class="model-info">(${modelName})</span>` : ''}
                    <span class="status-info ${statusClass}">[${statusText}]</span>
                `;

                const infoContainer = document.createElement('div');
                infoContainer.className = 'info-container';
                infoContainer.appendChild(info);

                if (task.status === 'processing' && task.progress) {
                    const progressDiv = document.createElement('div');
                    progressDiv.className = 'progress-info';
                    
                    // 진행률 퍼센트 추출
                    const percentMatch = task.progress.match(/(\d+)%/);
                    if (percentMatch) {
                        const percent = parseInt(percentMatch[1]);
                        
                        // 진행률 바 생성
                        const progressContainer = document.createElement('div');
                        progressContainer.className = 'progress-container';
                        
                        const progressBar = document.createElement('div');
                        progressBar.className = 'progress-bar';
                        progressBar.style.width = `${percent}%`;
                        
                        progressContainer.appendChild(progressBar);
                        progressDiv.appendChild(progressContainer);
                    }
                    
                    const progressText = document.createElement('div');
                    progressText.textContent = task.progress;
                    progressDiv.appendChild(progressText);
                    
                    infoContainer.appendChild(progressDiv);
                }

                const cancelBtn = document.createElement('button');
                cancelBtn.textContent = '×';
                cancelBtn.className = 'cancel-btn';
                cancelBtn.title = '작업 취소';
                cancelBtn.onclick = () => removeTaskFromQueue(task.id);

                const btnContainer = document.createElement('div');
                btnContainer.className = 'queue-btn-group';
                if (task.status !== 'processing') {
                    const upBtn = document.createElement('button');
                    upBtn.textContent = '▲';
                    upBtn.className = 'move-btn';
                    upBtn.title = '위로 이동';
                    upBtn.onclick = () => moveTaskUp(task.id);

                    const downBtn = document.createElement('button');
                    downBtn.textContent = '▼';
                    downBtn.className = 'move-btn';
                    downBtn.title = '아래로 이동';
                    downBtn.onclick = () => moveTaskDown(task.id);

                    btnContainer.appendChild(upBtn);
                    btnContainer.appendChild(downBtn);
                }
                btnContainer.appendChild(cancelBtn);

                item.appendChild(infoContainer);
                item.appendChild(btnContainer);
                queueList.appendChild(item);
            });
        });
    }
}

function createTaskElement(task, isCompleted, downloadUrl, record = null) {
    const taskNames = {
        'stt': 'STT',
        'embedding': '색인',
        'summary': '요약'
    };
    
    const span = document.createElement('span');
    span.textContent = taskNames[task] || task;
    span.style.margin = '0 5px';
    span.style.padding = '2px 6px';
    span.style.borderRadius = '3px';
    span.style.fontSize = '12px';
    span.dataset.task = task;
    
    if (isCompleted && downloadUrl) {
        // Completed task - green with download link
        span.style.backgroundColor = '#28a745';
        span.style.color = 'white';
        span.style.cursor = 'pointer';
        span.style.textDecoration = 'underline';
        
        if (task === 'embedding') {
            span.title = '클릭하여 유사 문서 보기';
            span.onclick = () => {
                showSimilarDocuments(downloadUrl, record ? record.filename : null);
            };
        } else {
            span.title = '클릭하여 내용 보기';
            span.onclick = () => {
                showTextOverlay(downloadUrl, task);
            };
        }
    } else if (record) {
        // Check if this task is already in queue or currently processing
        const existingTask = taskQueue.find(t =>
            t.recordId === record.id &&
            t.task === task
        );
        
        const isCurrentlyProcessing = currentTask && 
            currentTask.recordId === record.id && 
            currentTask.task === task;
        
        if (existingTask || isCurrentlyProcessing) {
            // Task is already in queue or processing - show queued/processing state
            span.style.backgroundColor = isCurrentlyProcessing ? '#ffc107' : '#17a2b8';
            span.style.color = isCurrentlyProcessing ? 'black' : 'white';
            span.style.cursor = 'default';
            span.title = isCurrentlyProcessing ? '처리 중' : '큐에 추가됨';
            span.onclick = null;
        } else {
            // Incomplete task - clickable to add to queue or show popup
            span.style.backgroundColor = '#6c757d';
            span.style.color = 'white';
            span.style.cursor = 'pointer';
            span.title = '클릭하여 작업 시작';
            span.onclick = () => {
                // Disable the button immediately to prevent multiple clicks
                span.style.pointerEvents = 'none';
                span.style.opacity = '0.7';
                
                // Double-check if this task is already in queue or processing (in case of race condition)
                const existingTaskCheck = taskQueue.find(t =>
                    t.recordId === record.id &&
                    t.task === task
                );
                
                const isCurrentlyProcessingCheck = currentTask && 
                    currentTask.recordId === record.id && 
                    currentTask.task === task;

                if (existingTaskCheck || isCurrentlyProcessingCheck) {
                    // Task already exists, re-enable button and return
                    span.style.pointerEvents = 'auto';
                    span.style.opacity = '1';
                    console.log(`Task ${task} for record ${record.id} already in queue or processing, skipping`);
                    return;
                }

                if (!existingTaskCheck && !isCurrentlyProcessingCheck) {
                    // Check if this is a summary task for audio file without STT completion
                    if (task === 'summary' && record.file_type === 'audio' && !record.completed_tasks.stt) {
                        // Re-enable the button for popup handling
                        span.style.pointerEvents = 'auto';
                        span.style.opacity = '1';
                        
                        // Show STT confirmation popup
                        showSttConfirmPopup();
                        
                        // Set up one-time event listener for confirm button
                        const handleConfirm = () => {
                            hideSttConfirmPopup();
                            // Add both STT and summary tasks (like batch process)
                            const steps = ['stt', 'summary'];
                            steps.forEach(step => {
                                // Double check for existing task before adding
                                const existingTask = taskQueue.find(t => t.recordId === record.id && t.task === step);
                                if (!existingTask) {
                                    // Find the task element by ID
                                    const stepSpan = document.getElementById(`task-${record.id}-${step}`);
                                    if (stepSpan) {
                                        const addedId = addTaskToQueue(record.id, record.file_path, step, stepSpan, record.filename);
                                        if (addedId) setQueuedState(stepSpan);
                                    } else {
                                        // Fallback: create a dummy span
                                        const dummySpan = document.createElement('span');
                                        addTaskToQueue(record.id, record.file_path, step, dummySpan, record.filename);
                                    }
                                } else {
                                    console.log(`Task ${step} for record ${record.id} already exists, skipping`);
                                }
                            });
                            sttConfirmOkBtn.removeEventListener('click', handleConfirm);
                        };
                        
                        sttConfirmOkBtn.addEventListener('click', handleConfirm);
                        return;
                    }
                    
                    // Check if this is an embedding task for audio file without STT completion
                    if (task === 'embedding' && record.file_type === 'audio' && !record.completed_tasks.stt) {
                        // Check if existing STT result exists, if so proceed with incremental embedding
                        // Otherwise show alert
                        span.style.pointerEvents = 'auto';
                        span.style.opacity = '1';
                        
                        // Try to find existing STT result by attempting embedding with existing file check
                        // If no existing STT found, show alert
                        fetch('/check_existing_stt', {
                            method: 'POST',
                            headers: { 'Content-Type': 'application/json' },
                            body: JSON.stringify({ file_path: record.file_path })
                        })
                        .then(response => response.json())
                        .then(data => {
                            if (data.has_stt) {
                                // Proceed with embedding using existing STT
                                console.log(`Adding embedding task for record ${record.id} to queue`);
                                const taskId = addTaskToQueue(record.id, record.file_path, task, span, record.filename);
                                console.log(`Task added with ID: ${taskId}`);
                                setQueuedState(span);
                            } else {
                                alert('STT 작업이 완료되지 않았습니다. STT를 먼저 실행해주세요.');
                            }
                        })
                        .catch(() => {
                            alert('STT 완료 여부를 확인할 수 없습니다. STT를 먼저 실행해주세요.');
                        });
                        return;
                    }
                    
                    // Normal case - just add the single task
                    addTaskToQueue(record.id, record.file_path, task, span, record.filename);
                    setQueuedState(span);
                } else {
                    // Re-enable the button if task already exists
                    span.style.pointerEvents = 'auto';
                    span.style.opacity = '1';
                }
            };
        }
    } else {
        // Default state - not clickable
        span.style.backgroundColor = '#e9ecef';
        span.style.color = '#6c757d';
    }
    
    return span;
}

function displayHistory(history) {
    const historyList = document.getElementById('history-list');
    historyList.innerHTML = '';
    
    if (history.length === 0) {
        historyList.innerHTML = '<p style="color: #6c757d; font-style: italic;">업로드 기록이 없습니다.</p>';
        return;
    }
    
    history.forEach(record => {
        const item = document.createElement('div');
        item.className = 'history-item';

        const typeLabel = record.file_type === 'audio' ? '오디오' : record.file_type === 'pdf' ? 'PDF' : '텍스트';
        const dateTime = formatDateTime(record.timestamp);
        const duration = record.duration ? ` ${record.duration}` : '';

        const header = document.createElement('div');
        header.className = 'history-header';

        const info = document.createElement('span');
        info.innerHTML = `
            <strong>[${typeLabel}]</strong>
            ${dateTime}
            <strong id="filename-${record.id}" class="filename-display" title="클릭하여 파일명 수정">${normalizeKorean(record.filename)}</strong><span class="duration">${duration}</span>
        `;
        
        // Add click event to filename for editing
        setTimeout(() => {
            const filenameElement = document.getElementById(`filename-${record.id}`);
            if (filenameElement) {
                filenameElement.onclick = () => editFilename(record.id, record.filename);
            }
        }, 0);

        const resetBtn = document.createElement('button');
        resetBtn.textContent = '초기화';
        resetBtn.className = 'reset-btn';

        const hasCompleted = Object.values(record.completed_tasks).some(v => v);
        const queued = taskQueue.some(t => t.recordId === record.id);
        const isProcessing = currentTask && currentTask.recordId === record.id;

        if (hasCompleted || queued || isProcessing) {
            resetBtn.onclick = async () => {
                if (!confirm('기존 작업내역을 초기화 하시겠습니까?')) return;

                // Remove related tasks from queue
                const relatedTasks = taskQueue.filter(t => t.recordId === record.id);
                relatedTasks.forEach(t => removeTaskFromQueue(t.id));

                const resp = await fetch('/reset', {
                    method: 'POST',
                    headers: { 'Content-Type': 'application/json' },
                    body: JSON.stringify({ record_id: record.id })
                });

                if (resp.ok) {
                    loadHistory();
                } else {
                    alert('초기화에 실패했습니다.');
                }
            };
        } else {
            resetBtn.disabled = true;
        }

        const batchBtn = document.createElement('button');
        batchBtn.textContent = '일괄 진행';
        batchBtn.className = 'batch-btn';

        const buttonContainer = document.createElement('div');
        buttonContainer.className = 'button-container';

        buttonContainer.appendChild(batchBtn);
        buttonContainer.appendChild(resetBtn);

        header.appendChild(info);
        header.appendChild(buttonContainer);

        const tasks = document.createElement('div');

        const taskElements = {};

        // Only show STT button for audio files
        if (record.file_type === 'audio') {
            taskElements.stt = createTaskElement('stt', record.completed_tasks.stt, record.download_links.stt, record);
            taskElements.stt.id = `task-${record.id}-stt`;
            tasks.appendChild(taskElements.stt);
        }

        taskElements.embedding = createTaskElement('embedding', record.completed_tasks.embedding, record.download_links.embedding, record);
        taskElements.embedding.id = `task-${record.id}-embedding`;
        tasks.appendChild(taskElements.embedding);

        taskElements.summary = createTaskElement('summary', record.completed_tasks.summary, record.download_links.summary, record);
        taskElements.summary.id = `task-${record.id}-summary`;
        tasks.appendChild(taskElements.summary);

        if (hasCompleted || queued || isProcessing) {
            batchBtn.disabled = true;
        } else {
            batchBtn.onclick = () => {
                const steps = [];
                if (record.file_type === 'audio') {
                    if (!record.completed_tasks.stt) steps.push('stt');
                    if (!record.completed_tasks.embedding) steps.push('embedding');
                    if (!record.completed_tasks.summary) steps.push('summary');
                } else {
                    if (!record.completed_tasks.embedding) steps.push('embedding');
                    if (!record.completed_tasks.summary) steps.push('summary');
                }

                // 모든 작업을 일단 큐에 추가하고, 작업 실행 시 파일 존재 여부를 서버에서 확인
                steps.forEach(step => {
                    const alreadyCompleted = record.completed_tasks[step];
                    const existingTask = taskQueue.find(t => t.recordId === record.id && t.task === step);
                    if (!alreadyCompleted && !existingTask) {
                        const span = taskElements[step] || document.createElement('span');
                        const addedId = addTaskToQueue(record.id, record.file_path, step, span, record.filename);
                        if (addedId && taskElements[step]) {
                            setQueuedState(taskElements[step]);
                        }
                    } else if (existingTask) {
                        console.log(`Task ${step} for record ${record.id} already in queue, skipping`);
                    }
                });

                batchBtn.disabled = true;
            };
        }

        item.appendChild(header);
        if (record.title_summary) {
            const summary = document.createElement('div');
            summary.className = 'task-summary';
            summary.textContent = record.title_summary;
            item.appendChild(summary);
        }
        item.appendChild(tasks);
        historyList.appendChild(item);
    });
}

async function loadHistory() {
    try {
        const response = await fetch('/history');
        if (response.ok) {
            const history = await response.json();
            displayHistory(history);
        } else {
            console.error('Failed to load history');
        }
    } catch (error) {
        console.error('Error loading history:', error);
    }
}

async function loadHistorySync() {
    try {
        const response = await fetch('/history');
        if (response.ok) {
            return await response.json();
        }
        return [];
    } catch (error) {
        console.error('Error loading history:', error);
        return [];
    }
}

async function processNextTask() {
    if (currentTask || taskQueue.length === 0) {
        return;
    }
    
    // Find the next task that's ready to process
    let nextTaskIndex = -1;
    const now = Date.now();
    
    for (let i = 0; i < taskQueue.length; i++) {
        const task = taskQueue[i];
        
        // Skip tasks that were recently retried (within 3 seconds)
        if (task.lastRetryTime && (now - task.lastRetryTime) < 3000) {
            continue;
        }
        
        // Skip tasks that have too many retries for STT dependency
        if (task.retryCount && task.retryCount >= 20) {
            continue;
        }
        
        nextTaskIndex = i;
        break;
    }
    
    // If no task is ready, wait and try again
    if (nextTaskIndex === -1) {
        setTimeout(() => processNextTask(), 2000);
        return;
    }
    
    // Move the selected task to the front and process it
    currentTask = taskQueue.splice(nextTaskIndex, 1)[0];
    currentTask.status = 'processing';
    currentCategory = currentTask.task;
    updateQueueDisplay();
    
    try {
        // Show loading state on the task element
        const taskElement = currentTask.taskElement;
        const originalText = taskElement.textContent;
        taskElement.textContent = '처리중...';
        taskElement.style.backgroundColor = '#ffc107';
        taskElement.style.color = 'black';
        taskElement.style.cursor = 'default';
        taskElement.onclick = null;
        
        // Initialize progress message; updates will come via WebSocket
        currentTask.progress = '작업 준비 중...';
        updateQueueDisplay();

        // Create AbortController for this task
        currentTask.abortController = new AbortController();

        // Get saved model settings
        const savedSettings = JSON.parse(localStorage.getItem('modelSettings') || '{}');
        
        const response = await fetch('/process', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ 
                file_path: currentTask.filePath, 
                steps: [currentTask.task],
                record_id: currentTask.recordId,
                task_id: currentTask.taskId,  // Send task_id to server
                model_settings: savedSettings  // Send model settings to server
            }),
            signal: currentTask.abortController.signal
        });

        if (response.ok) {
            const result = await response.json();
            
            if (result.error) {
                // Check if this is a dependency error (STT not completed for embedding)
                if (result.error === 'STT_DEPENDENCY_NOT_MET') {
                    // Put the task back to the queue and retry later
                    currentTask.status = 'queued';
                    currentTask.retryCount = (currentTask.retryCount || 0) + 1;
                    currentTask.lastRetryTime = Date.now();
                    
                    if (currentTask.retryCount < 20) { // Max 20 retries
                        // Add task back to the END of queue (not front) to avoid immediate retry
                        taskQueue.push(currentTask);
                        
                        // Show waiting state
                        taskElement.textContent = 'STT 대기';
                        taskElement.style.backgroundColor = '#ffc107';
                        taskElement.style.color = 'black';
                        taskElement.title = result.message || 'STT 작업 완료 대기 중';
                        
                        // Don't remove task from queue, let it retry
                        currentTask = null;
                        currentCategory = taskQueue.length > 0 ? taskQueue[0].task : null;
                        updateQueueDisplay();
                        
                        // Stop progress polling and retry after 5 seconds
                        stopProgressPolling();
                        setTimeout(() => processNextTask(), 5000);
                        return;
                    } else {
                        // Too many retries, treat as error
                        taskElement.textContent = '오류';
                        taskElement.style.backgroundColor = '#dc3545';
                        taskElement.style.color = 'white';
                        taskElement.title = 'STT 작업을 기다리는 중 시간 초과';
                    }
                } else if (result.error === 'FILE_NOT_FOUND' || result.error === 'NO_TARGET_FILE') {
                    // 작업 대상 파일이 없는 경우 - 큐에서 자동삭제
                    console.log(`Task ${currentTask.task} for record ${currentTask.recordId} has no target file, removing from queue`);
                    
                    // Show notification and remove task element
                    taskElement.textContent = '파일 없음';
                    taskElement.style.backgroundColor = '#ffc107';
                    taskElement.style.color = 'black';
                    taskElement.title = '작업 대상 파일이 없어 큐에서 제거됨';
                    
                    // The task will be automatically removed in the finally block
                    // No need to re-add to queue
                } else {
                    // Show error state
                    taskElement.textContent = '오류';
                    taskElement.style.backgroundColor = '#dc3545';
                    taskElement.style.color = 'white';
                    taskElement.title = `오류: ${result.error}`;
                }
            } else if (result[currentTask.task]) {
                // Show success state with download link
                taskElement.textContent = originalText;
                taskElement.style.backgroundColor = '#28a745';
                taskElement.style.color = 'white';
                taskElement.style.cursor = 'pointer';
                taskElement.style.textDecoration = 'underline';
                taskElement.title = '클릭하여 내용 보기';
                taskElement.onclick = () => {
                    showTextOverlay(result[currentTask.task], currentTask.task);
                };
            }
        } else {
            // Show error state
            taskElement.textContent = '오류';
            taskElement.style.backgroundColor = '#dc3545';
            taskElement.style.color = 'white';
            taskElement.title = '처리 중 오류가 발생했습니다';
        }
    } catch (error) {
        if (error.name === 'AbortError') {
            console.log('Task was cancelled');
        } else {
            // Show error state
            const taskElement = currentTask.taskElement;
            taskElement.textContent = '오류';
            taskElement.style.backgroundColor = '#dc3545';
            taskElement.style.color = 'white';
            taskElement.title = `오류: ${error.message}`;
            console.error('Error processing task:', error);
        }
        
        // Stop progress polling on error
        stopProgressPolling();
    } finally {
        // Only remove task from queue if it's not being retried (not already re-added to queue)
        if (currentTask) {
            const isRetrying = taskQueue.some(t => t.id === currentTask.id);
            if (!isRetrying) {
                // Task completed or failed permanently, remove it
                const taskIndex = taskQueue.findIndex(t => t.id === currentTask.id);
                if (taskIndex !== -1) {
                    taskQueue.splice(taskIndex, 1);
                }
            }
        }

        currentTask = null;
        currentCategory = taskQueue.length > 0 ? taskQueue[0].task : null;
        updateQueueDisplay();
        
        // Reload history to show updated completion status
        loadHistory();
        
        // Stop progress polling when task completes
        stopProgressPolling();
        
        // Process next task in queue
        setTimeout(() => processNextTask(), 100);
    }
}

document.getElementById('uploadBtn').addEventListener('click', async () => {
    const input = document.getElementById('fileInput');
    const status = document.getElementById('status');
    const files = Array.from(input.files);
    if (files.length === 0) {
        status.textContent = 'Please select a file first.';
        return;
    }

    const formData = new FormData();
    files.forEach(f => formData.append('files', f));

    try {
        const resp = await fetch('/upload', {
            method: 'POST',
            body: formData
        });
        if (resp.ok) {
            const data = await resp.json();
            if (data.length === 1) {
                status.textContent = 'Upload complete! 히스토리에서 작업을 선택하거나 "전체 진행" 버튼을 사용하세요.';
            } else {
                status.textContent = `${data.length}개의 파일이 업로드되었습니다. 히스토리에서 작업을 선택하거나 "전체 진행" 버튼을 사용하세요.`;
            }

            // Reload history to show the new upload(s)
            loadHistory();
            input.value = '';
        } else {
            status.textContent = 'Upload failed.';
        }
    } catch (err) {
        status.textContent = 'Error: ' + err.message;
    }
});

// Check for running tasks on page load
async function checkRunningTasks() {
    try {
        const response = await fetch('/tasks');
        if (response.ok) {
            const runningTasks = await response.json();
            console.log('Running tasks found:', runningTasks);
            
            // If there are running tasks, show a warning
            if (Object.keys(runningTasks).length > 0) {
                const status = document.getElementById('status');
                const taskCount = Object.keys(runningTasks).length;
                
                let taskDetails = '';
                for (const [taskId, info] of Object.entries(runningTasks)) {
                    const duration = Math.round(info.duration || 0);
                    taskDetails += `<li>작업 ID: ${taskId} (실행시간: ${duration}초)</li>`;
                }
                
                status.innerHTML = `
                    <div class="warning-box">
                        <strong>⚠️ 백그라운드에서 실행 중인 작업이 있습니다!</strong><br>
                        페이지를 새로고침했지만 서버에서 ${taskCount}개의 작업이 계속 실행 중입니다.<br>
                        <ul style="margin: 5px 0;">${taskDetails}</ul>
                        작업 완료 후 히스토리를 자동으로 업데이트됩니다.
                    </div>
                `;
            }
        }
    } catch (error) {
        console.error('Error checking running tasks:', error);
    }
}

// Periodically check for running tasks
let previousTaskCount = 0;
function startTaskMonitoring() {
    setInterval(async () => {
        try {
            const response = await fetch('/tasks');
            if (response.ok) {
                const runningTasks = await response.json();
                const taskCount = Object.keys(runningTasks).length;
                
                // Update page title to show running tasks
                if (taskCount > 0) {
                    document.title = `(${taskCount}) RecordRoute File Upload`;
                } else {
                    document.title = 'RecordRoute File Upload';
                }
                
                // If task count decreased, some tasks completed - reload history
                if (previousTaskCount > 0 && taskCount < previousTaskCount) {
                    console.log('Tasks completed, reloading history...');
                    loadHistory();
                    
                    // Clear the warning message if no tasks are running
                    if (taskCount === 0) {
                        const status = document.getElementById('status');
                        if (status.innerHTML.includes('백그라운드에서 실행 중인')) {
                            status.innerHTML = `
                                <div style="background: #d4edda; border: 1px solid #c3e6cb; padding: 10px; border-radius: 5px; margin: 10px 0; color: #155724;">
                                    ✅ 모든 백그라운드 작업이 완료되었습니다!
                                </div>
                            `;
                            // Clear this message after 3 seconds
                            setTimeout(() => {
                                if (status.innerHTML.includes('모든 백그라운드 작업이 완료')) {
                                    status.innerHTML = '';
                                }
                            }, 3000);
                        }
                    }
                }
                
                previousTaskCount = taskCount;
            }
        } catch (error) {
            console.error('Error monitoring tasks:', error);
        }
    }, 2000); // Check every 2 seconds
}

// Load history on page load
document.getElementById('searchBtn').addEventListener('click', async () => {
    const q = document.getElementById('searchInput').value.trim();

    if (!q) {
        resetSearchDisplay();
        if (searchMessage) {
            searchMessage.textContent = '검색어를 입력하세요.';
        }
        return;
    }

    if (currentTask || taskQueue.length > 0) {
        alert('현재 다른 작업이 진행 중입니다. 작업이 모두 완료된 후 검색을 이용해 주세요.');
        return;
    }

    const searchBtn = document.getElementById('searchBtn');
    const originalText = searchBtn.textContent;

    resetSearchDisplay();

    try {
        searchBtn.textContent = '검색 중...';
        searchBtn.disabled = true;

        const resp = await fetch(`/search?q=${encodeURIComponent(q)}`);
        const data = await resp.json();

        if (!resp.ok) {
            const errorMsg = data.error || '검색 중 오류가 발생했습니다.';
            if (searchMessage) {
                searchMessage.textContent = errorMsg;
                searchMessage.classList.add('error');
            }
            if (data.details) {
                console.error('검색 오류 상세:', data.details);
            }
            return;
        }

        if (Array.isArray(data)) {
            if (data.length === 0) {
                if (searchMessage) {
                    searchMessage.textContent = '검색 결과가 없습니다.';
                }
                return;
            }

            if (searchResultsContainer) {
                searchResultsContainer.classList.remove('hidden');
            }
            if (similarGroup) {
                similarGroup.classList.remove('hidden');
            }

            data.forEach(item => {
                appendSimilarResult({
                    display_name: item.display_name || item.file,
                    file: item.file,
                    link: item.link || `/download/${item.file}`,
                    score: item.score,
                    uploaded_at: item.uploaded_at,
                    source_filename: item.source_filename
                });
            });
            return;
        }

        const keywordMatches = Array.isArray(data.keywordMatches) ? data.keywordMatches : [];
        const similarDocuments = Array.isArray(data.similarDocuments) ? data.similarDocuments : [];

        if (keywordMatches.length === 0 && similarDocuments.length === 0) {
            if (searchMessage) {
                searchMessage.textContent = '검색 결과가 없습니다.';
            }
            return;
        }

        if (searchResultsContainer) {
            searchResultsContainer.classList.remove('hidden');
        }

        if (keywordMatches.length > 0 && keywordGroup) {
            keywordGroup.classList.remove('hidden');
            keywordMatches.forEach(item => appendKeywordResult(item));
        } else if (keywordGroup) {
            keywordGroup.classList.add('hidden');
        }

        if (similarDocuments.length > 0 && similarGroup) {
            similarGroup.classList.remove('hidden');
            similarDocuments.forEach(item => appendSimilarResult(item));
        } else if (similarGroup) {
            similarGroup.classList.add('hidden');
        }

    } catch (err) {
        if (searchMessage) {
            searchMessage.textContent = `네트워크 오류: ${err.message}`;
            searchMessage.classList.add('error');
        }
        console.error('검색 네트워크 오류:', err);
    } finally {
        searchBtn.textContent = originalText;
        searchBtn.disabled = false;
    }
});

// Process all incomplete tasks
async function processAllIncomplete() {
    try {
        const history = await loadHistorySync();
        let tasksAdded = 0;
        
        history.forEach(record => {
            const steps = [];
            
            // Check which steps are incomplete - 모든 작업을 큐에 추가
            if (record.file_type === 'audio') {
                if (!record.completed_tasks.stt) steps.push('stt');
                if (!record.completed_tasks.embedding) steps.push('embedding');
                if (!record.completed_tasks.summary) steps.push('summary');
            } else {
                if (!record.completed_tasks.embedding) steps.push('embedding');
                if (!record.completed_tasks.summary) steps.push('summary');
            }
            
            // Add incomplete steps to queue if they're not already queued
            steps.forEach(step => {
                const existingTask = taskQueue.find(t => t.recordId === record.id && t.task === step);
                if (!existingTask) {
                    const span = document.createElement('span');
                    const addedId = addTaskToQueue(record.id, record.file_path, step, span, record.filename);
                    if (addedId) {
                        tasksAdded++;
                    }
                } else {
                    console.log(`Task ${step} for record ${record.id} already in queue, skipping`);
                }
            });
        });
        
        if (tasksAdded > 0) {
            alert(`${tasksAdded}개의 작업이 큐에 추가되었습니다.`);
        } else {
            alert('진행할 미완료 작업이 없습니다.');
        }
    } catch (error) {
        console.error('Error processing all incomplete tasks:', error);
        alert('전체 진행 중 오류가 발생했습니다.');
    }
}

// Add event listener for queue sort dropdown
document.getElementById('queueSortSelect').addEventListener('change', function() {
    sortTaskQueue();
    updateQueueDisplay();
});

// Add event listener for process all button
document.getElementById('processAllBtn').addEventListener('click', processAllIncomplete);

// Add event listener for settings button
document.getElementById('settingsBtn').addEventListener('click', showModelSettingsPopup);

document.addEventListener('DOMContentLoaded', function() {
    initTheme();
    loadHistory();
    checkRunningTasks();
    startTaskMonitoring();
    initWebSocket();
    initializeDropZone();
});
