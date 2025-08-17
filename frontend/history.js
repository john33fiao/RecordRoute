function editFilename(recordId, currentFilename) {
    const filenameElement = document.getElementById(`filename-${recordId}`);
    const originalText = filenameElement.textContent;

    const input = document.createElement('input');
    input.type = 'text';
    input.value = currentFilename;
    input.className = 'filename-input';

    filenameElement.style.display = 'none';
    filenameElement.parentNode.insertBefore(input, filenameElement.nextSibling);

    input.focus();
    input.select();

    const saveEdit = async (newFilename) => {
        if (newFilename && newFilename.trim() !== '' && newFilename !== currentFilename) {
            try {
                const response = await fetch('/update_filename', {
                    method: 'POST',
                    headers: { 'Content-Type': 'application/json' },
                    body: JSON.stringify({
                        record_id: recordId,
                        filename: newFilename.trim(),
                    }),
                });

                if (response.ok) {
                    filenameElement.textContent = newFilename.trim();
                    loadHistory();
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

        input.remove();
        filenameElement.style.display = '';
    };

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

function formatDateTime(isoString) {
    const date = new Date(isoString);
    return date.toLocaleString('ko-KR', {
        year: 'numeric',
        month: '2-digit',
        day: '2-digit',
        hour: '2-digit',
        minute: '2-digit',
        second: '2-digit',
    });
}

function displayHistory(history) {
    const historyList = document.getElementById('history-list');
    historyList.innerHTML = '';

    if (history.length === 0) {
        historyList.innerHTML = '<p style="color: #6c757d; font-style: italic;">업로드 기록이 없습니다.</p>';
        return;
    }

    history.forEach((record) => {
        const item = document.createElement('div');
        item.style.cssText = `
            border: 1px solid #dee2e6;
            border-radius: 5px;
            padding: 10px;
            margin-bottom: 10px;
            background-color: #f8f9fa;
        `;

        const typeLabel = record.file_type === 'audio' ? '오디오' : record.file_type === 'pdf' ? 'PDF' : '텍스트';
        const dateTime = formatDateTime(record.timestamp);
        const duration = record.duration ? ` ${record.duration}` : '';

        const header = document.createElement('div');
        header.style.cssText = 'display:flex; justify-content:space-between; align-items:center; margin-bottom:5px;';

        const info = document.createElement('span');
        info.innerHTML = `
            <strong>[${typeLabel}]</strong>
            ${dateTime}
            <strong id="filename-${record.id}" class="filename-display" title="클릭하여 파일명 수정">${record.filename}</strong><span class="duration">${duration}</span>
        `;

        setTimeout(() => {
            const filenameElement = document.getElementById(`filename-${record.id}`);
            if (filenameElement) {
                filenameElement.onclick = () => editFilename(record.id, record.filename);
            }
        }, 0);

        header.appendChild(info);

        const actions = document.createElement('div');
        actions.style.cssText = 'display:flex; gap:10px;';

        const tasks = document.createElement('div');
        tasks.style.cssText = 'margin-top:8px;';

        const createTaskSpan = (taskName, isCompleted, downloadUrl) => {
            const span = createTaskElement(taskName, isCompleted, downloadUrl, record);
            span.id = `task-${record.id}-${taskName}`;
            return span;
        };

        tasks.appendChild(createTaskSpan('stt', record.completed_tasks.stt, record.download_links?.stt));
        tasks.appendChild(createTaskSpan('embedding', record.completed_tasks.embedding, record.download_links?.embedding));
        tasks.appendChild(createTaskSpan('summary', record.completed_tasks.summary, record.download_links?.summary));

        item.appendChild(header);
        item.appendChild(actions);
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

async function processAllIncomplete() {
    try {
        const history = await loadHistorySync();
        let tasksAdded = 0;

        history.forEach((record) => {
            const steps = [];

            if (record.file_type === 'audio') {
                if (!record.completed_tasks.stt) steps.push('stt');
                if (!record.completed_tasks.embedding) steps.push('embedding');
                if (!record.completed_tasks.summary) steps.push('summary');
            } else {
                if (!record.completed_tasks.embedding) steps.push('embedding');
                if (!record.completed_tasks.summary) steps.push('summary');
            }

            steps.forEach((step) => {
                const existingTask = taskQueue.find(
                    (t) => t.recordId === record.id && t.task === step
                );
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

document.getElementById('processAllBtn').addEventListener('click', processAllIncomplete);
