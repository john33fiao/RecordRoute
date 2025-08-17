let uploadedPath = null;
let fileType = null;
let recordId = null;
let taskQueue = [];
let currentTask = null;
let taskIdCounter = 0;
const categoryOrder = ['stt', 'embedding', 'summary'];
let currentCategory = null;
const summaryPopup = document.getElementById('summaryPopup');
const summaryOnlyBtn = document.getElementById('summaryOnlyBtn');
const summaryCancelBtn = document.getElementById('summaryCancelBtn');
const sttConfirmPopup = document.getElementById('sttConfirmPopup');
const sttConfirmOkBtn = document.getElementById('sttConfirmOkBtn');
const sttConfirmCancelBtn = document.getElementById('sttConfirmCancelBtn');

function showTextOverlay(url) {
    const overlay = document.getElementById('textOverlay');
    const content = document.getElementById('overlayContent');
    const download = document.getElementById('overlayDownload');
    overlay.style.display = 'flex';
    content.textContent = '로딩중...';
    download.href = url;
    fetch(url)
        .then(resp => resp.text())
        .then(text => {
            content.textContent = text;
        })
        .catch(() => {
            content.textContent = '파일을 불러오지 못했습니다.';
        });
}

// Progress polling to update queue items
function startProgressPolling(task) {
    stopProgressPolling();
    window.progressPollingInterval = setInterval(() => {
        fetch(`/progress/${task.taskId}`)
            .then(response => response.json())
            .then(data => {
                if (data.message) {
                    task.progress = data.message;
                    updateQueueDisplay();
                }
            })
            .catch(error => {
                console.log('Progress polling error:', error);
            });
    }, 1000);
}

function stopProgressPolling() {
    if (window.progressPollingInterval) {
        clearInterval(window.progressPollingInterval);
        window.progressPollingInterval = null;
    }
}

document.getElementById('overlayClose').addEventListener('click', () => {
    document.getElementById('textOverlay').style.display = 'none';
});

// Add Esc key listener for text overlay
document.addEventListener('keydown', (e) => {
    if (e.key === 'Escape') {
        const textOverlay = document.getElementById('textOverlay');
        if (textOverlay.style.display === 'flex') {
            textOverlay.style.display = 'none';
        }
        if (summaryPopup.style.display === 'flex') {
            hideSummaryPopup();
        }
        if (sttConfirmPopup.style.display === 'flex') {
            hideSttConfirmPopup();
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

summaryCancelBtn.addEventListener('click', hideSummaryPopup);
sttConfirmCancelBtn.addEventListener('click', hideSttConfirmPopup);

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
                    filenameElement.textContent = newFilename.trim();
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
        taskQueue.sort((a, b) => a.id - b.id);
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
            return diff !== 0 ? diff : a.id - b.id;
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
        abortController: null
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

function cancelAllTasks() {
    const tasks = [...taskQueue];
    tasks.forEach(t => removeTaskFromQueue(t.id));
}

document.getElementById('cancelAllBtn').addEventListener('click', cancelAllTasks);

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
            item.style.cssText = `
                border: 1px solid #dee2e6;
                border-radius: 5px;
                padding: 8px 12px;
                margin-bottom: 8px;
                background-color: ${task.status === 'processing' ? '#fff3cd' : '#f8f9fa'};
                display: flex;
                justify-content: space-between;
                align-items: center;
            `;
            
            const statusText = task.status === 'processing' ? '진행중' : `대기중 (${index + 1}번째)`;
            const statusColor = task.status === 'processing' ? '#856404' : '#6c757d';
            const taskName = taskNames[task.task] || task.task;
            
            const info = document.createElement('span');
            info.innerHTML = `
                <strong>${task.filename}</strong> - ${taskName}
                <span style="color: ${statusColor}; font-size: 12px; margin-left: 10px;">[${statusText}]</span>
            `;

            const infoContainer = document.createElement('div');
            infoContainer.style.flex = '1';
            infoContainer.appendChild(info);
            
            if (task.status === 'processing' && task.progress) {
                const progressDiv = document.createElement('div');
                progressDiv.style.cssText = 'color: #856404; font-size: 12px; margin-top: 4px;';
                
                // 진행률 퍼센트 추출
                const percentMatch = task.progress.match(/(\d+)%/);
                if (percentMatch) {
                    const percent = parseInt(percentMatch[1]);
                    
                    // 진행률 바 생성
                    const progressContainer = document.createElement('div');
                    progressContainer.style.cssText = `
                        width: 100%;
                        height: 4px;
                        background-color: #e9ecef;
                        border-radius: 2px;
                        margin: 2px 0;
                        overflow: hidden;
                    `;
                    
                    const progressBar = document.createElement('div');
                    progressBar.style.cssText = `
                        width: ${percent}%;
                        height: 100%;
                        background-color: #007bff;
                        transition: width 0.3s ease;
                    `;
                    
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
            cancelBtn.style.cssText = `
                background: #dc3545;
                color: white;
                border: none;
                border-radius: 3px;
                width: 24px;
                height: 24px;
                cursor: pointer;
                font-size: 16px;
                display: flex;
                align-items: center;
                justify-content: center;
            `;
            cancelBtn.title = '작업 취소';
            cancelBtn.onclick = () => removeTaskFromQueue(task.id);
            
            item.appendChild(infoContainer);
            item.appendChild(cancelBtn);
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
            categoryHeader.style.cssText = `
                background: #007bff;
                color: white;
                padding: 6px 12px;
                margin: 10px 0 5px 0;
                border-radius: 5px 5px 0 0;
                font-weight: bold;
                font-size: 14px;
            `;
            categoryHeader.textContent = `${categoryNames[category]} (${categoryTasks.length}개)`;
            queueList.appendChild(categoryHeader);

            // Add tasks in this category
            categoryTasks.forEach((task, index) => {
                const item = document.createElement('div');
                item.style.cssText = `
                    border: 1px solid #dee2e6;
                    border-top: none;
                    padding: 8px 12px;
                    margin-bottom: ${index === categoryTasks.length - 1 ? '10px' : '0'};
                    background-color: ${task.status === 'processing' ? '#fff3cd' : '#f8f9fa'};
                    display: flex;
                    justify-content: space-between;
                    align-items: center;
                    ${index === categoryTasks.length - 1 ? 'border-radius: 0 0 5px 5px;' : ''}
                `;
                
                const globalIndex = taskQueue.findIndex(t => t.id === task.id);
                const statusText = task.status === 'processing' ? '진행중' : `대기중 (${globalIndex + 1}번째)`;
                const statusColor = task.status === 'processing' ? '#856404' : '#6c757d';
                
                const info = document.createElement('span');
                info.innerHTML = `
                    <strong>${task.filename}</strong>
                    <span style="color: ${statusColor}; font-size: 12px; margin-left: 10px;">[${statusText}]</span>
                `;

                const infoContainer = document.createElement('div');
                infoContainer.style.flex = '1';
                infoContainer.appendChild(info);

                if (task.status === 'processing' && task.progress) {
                    const progressDiv = document.createElement('div');
                    progressDiv.style.cssText = 'color: #856404; font-size: 12px; margin-top: 4px;';
                    
                    // 진행률 퍼센트 추출
                    const percentMatch = task.progress.match(/(\d+)%/);
                    if (percentMatch) {
                        const percent = parseInt(percentMatch[1]);
                        
                        // 진행률 바 생성
                        const progressContainer = document.createElement('div');
                        progressContainer.style.cssText = `
                            width: 100%;
                            height: 4px;
                            background-color: #e9ecef;
                            border-radius: 2px;
                            margin: 2px 0;
                            overflow: hidden;
                        `;
                        
                        const progressBar = document.createElement('div');
                        progressBar.style.cssText = `
                            width: ${percent}%;
                            height: 100%;
                            background-color: #007bff;
                            transition: width 0.3s ease;
                        `;
                        
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
                cancelBtn.style.cssText = `
                    background: #dc3545;
                    color: white;
                    border: none;
                    border-radius: 3px;
                    width: 24px;
                    height: 24px;
                    cursor: pointer;
                    font-size: 16px;
                    display: flex;
                    align-items: center;
                    justify-content: center;
                `;
                cancelBtn.title = '작업 취소';
                cancelBtn.onclick = () => removeTaskFromQueue(task.id);
                
                item.appendChild(infoContainer);
                item.appendChild(cancelBtn);
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
        span.title = '클릭하여 내용 보기';
        span.onclick = () => {
            showTextOverlay(downloadUrl);
        };
    } else if (record) {
        // Check if this task is already in queue
        const existingTask = taskQueue.find(t =>
            t.recordId === record.id &&
            t.task === task
        );
        
        if (existingTask) {
            // Task is already in queue - show queued state
            span.style.backgroundColor = '#17a2b8';
            span.style.color = 'white';
            span.style.cursor = 'default';
            span.title = '큐에 추가됨';
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
                
                // Double-check if this task is already in queue (in case of race condition)
                const existingTaskCheck = taskQueue.find(t =>
                    t.recordId === record.id &&
                    t.task === task
                );

                if (existingTaskCheck) {
                    // Task already exists, re-enable button and return
                    span.style.pointerEvents = 'auto';
                    span.style.opacity = '1';
                    console.log(`Task ${task} for record ${record.id} already in queue, skipping`);
                    return;
                }

                if (!existingTaskCheck) {
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
        
        // Add click event to filename for editing
        setTimeout(() => {
            const filenameElement = document.getElementById(`filename-${record.id}`);
            if (filenameElement) {
                filenameElement.onclick = () => editFilename(record.id, record.filename);
            }
        }, 0);

        const resetBtn = document.createElement('button');
        resetBtn.textContent = '초기화';
        resetBtn.style.cssText = `
            background: #dc3545;
            color: white;
            border: none;
            border-radius: 3px;
            padding: 2px 8px;
            cursor: pointer;
            font-size: 12px;
        `;

        const hasCompleted = Object.values(record.completed_tasks).some(v => v);
        const queued = taskQueue.some(t => t.recordId === record.id);

        if (hasCompleted || queued) {
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
            resetBtn.style.background = '#6c757d';
            resetBtn.style.cursor = 'not-allowed';
        }

        const batchBtn = document.createElement('button');
        batchBtn.textContent = '일괄 진행';
        batchBtn.style.cssText = `
            background: #007bff;
            color: white;
            border: none;
            border-radius: 3px;
            padding: 2px 8px;
            cursor: pointer;
            font-size: 12px;
            margin-right: 5px;
        `;

        const buttonContainer = document.createElement('div');
        buttonContainer.style.cssText = 'display:flex; align-items:center;';

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

        if (hasCompleted || queued) {
            batchBtn.disabled = true;
            batchBtn.style.background = '#6c757d';
            batchBtn.style.cursor = 'not-allowed';
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
                batchBtn.style.background = '#6c757d';
                batchBtn.style.cursor = 'not-allowed';
            };
        }

        item.appendChild(header);
        if (record.title_summary) {
            const summary = document.createElement('div');
            summary.style.cssText = 'margin:4px 0; color:#333; font-size:13px;';
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
        
        // Initialize progress message and start polling for updates
        currentTask.progress = '작업 준비 중...';
        updateQueueDisplay();
        startProgressPolling(currentTask);

        // Create AbortController for this task
        currentTask.abortController = new AbortController();

        const response = await fetch('/process', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ 
                file_path: currentTask.filePath, 
                steps: [currentTask.task],
                record_id: currentTask.recordId,
                task_id: currentTask.taskId  // Send task_id to server
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
                taskElement.title = '클릭하여 다운로드';
                taskElement.onclick = () => {
                    window.open(result[currentTask.task], '_blank');
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

function updateWorkflowOptions(fileType) {
    const sttCheckbox = document.getElementById('stepStt');
    const sttLabel = sttCheckbox.parentElement;
    if (fileType === 'audio') {
        // Show STT checkbox for audio files
        sttLabel.style.display = 'block';
        sttLabel.style.visibility = 'visible';
        sttCheckbox.checked = false;
        sttCheckbox.disabled = false;
    } else {
        // Hide STT checkbox for non-audio files
        sttLabel.style.display = 'none';
        sttLabel.style.visibility = 'hidden';
        sttCheckbox.checked = false;
        sttCheckbox.disabled = true;
    }

    // Reset embedding and summary checkboxes
    document.getElementById('stepEmbedding').checked = false;
    document.getElementById('stepSummary').checked = false;
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
                const fileData = data[0];
                uploadedPath = fileData.file_path;
                fileType = fileData.file_type;
                recordId = fileData.record_id;

                updateWorkflowOptions(fileType);
                document.getElementById('workflow').style.display = 'block';

                if (fileType === 'audio') {
                    status.textContent = 'Upload complete! Select workflow steps.';
                } else if (fileType === 'text') {
                    status.textContent = 'Upload complete! Select text processing steps.';
                } else if (fileType === 'pdf') {
                    status.textContent = 'Upload complete! Select summary step.';
                } else {
                    status.textContent = 'Upload complete! File type not fully supported, but you can try processing.';
                }
            } else {
                status.textContent = `${data.length}개의 파일이 업로드되었습니다. 히스토리에서 작업을 선택하세요.`;
                document.getElementById('workflow').style.display = 'none';
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

document.getElementById('processBtn').addEventListener('click', async () => {
    if (!uploadedPath) return;

    const steps = [];
    if (document.getElementById('stepStt').checked) steps.push('stt');
    if (document.getElementById('stepEmbedding').checked) steps.push('embedding');
    if (document.getElementById('stepSummary').checked) steps.push('summary');

    if (steps.length === 0) {
        alert('최소 하나의 작업을 선택해주세요.');
        return;
    }

    const downloads = document.getElementById('downloads');
    downloads.innerHTML = '<p style="color: blue; font-weight: bold;">작업을 큐에 추가합니다...</p>';

    // Find the current file's name for queue display
    const history = await loadHistorySync();
    const currentRecord = history.find(record => record.id === recordId);
    const filename = currentRecord ? currentRecord.filename : 'Unknown File';

    // Add each step to the queue individually
    steps.forEach(step => {
        // Check for existing task before adding
        const existingTask = taskQueue.find(t => t.recordId === recordId && t.task === step);
        if (!existingTask) {
            // Create a temporary task element for queue tracking
            const tempElement = document.createElement('span');
            addTaskToQueue(recordId, uploadedPath, step, tempElement, filename);
        } else {
            console.log(`Task ${step} for record ${recordId} already in queue, skipping`);
        }
    });

    downloads.innerHTML = `<p style="color: green;">선택한 작업들이 큐에 추가되었습니다.</p>`;
    
    // Clear the current upload context
    uploadedPath = null;
    fileType = null;
    recordId = null;
    document.getElementById('workflow').style.display = 'none';
    document.getElementById('fileInput').value = '';
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
    if (!q) return;
    
    const searchBtn = document.getElementById('searchBtn');
    const originalText = searchBtn.textContent;
    
    try {
        // 검색 중 표시
        searchBtn.textContent = '검색 중...';
        searchBtn.disabled = true;
        
        const resp = await fetch(`/search?q=${encodeURIComponent(q)}`);
        const data = await resp.json();
        
        const list = document.getElementById('searchResults');
        list.innerHTML = '';
        
        if (!resp.ok) {
            // 서버 오류 처리
            const errorMsg = data.error || '검색 중 오류가 발생했습니다.';
            list.innerHTML = `<li style="color: #dc3545; font-weight: bold;">${errorMsg}</li>`;
            if (data.details) {
                console.error('검색 오류 상세:', data.details);
            }
        } else if (Array.isArray(data) && data.length === 0) {
            list.innerHTML = '<li style="color: #6c757d; font-style: italic;">검색 결과가 없습니다.</li>';
        } else if (Array.isArray(data)) {
            data.forEach(item => {
                const li = document.createElement('li');
                const a = document.createElement('a');
                a.href = item.link;
                a.textContent = `${item.file} (유사도: ${item.score.toFixed(3)})`;
                a.target = '_blank';
                a.style.textDecoration = 'none';
                a.style.color = '#007bff';
                a.onmouseover = () => a.style.textDecoration = 'underline';
                a.onmouseout = () => a.style.textDecoration = 'none';
                li.appendChild(a);
                list.appendChild(li);
            });
        } else {
            list.innerHTML = '<li style="color: #dc3545;">예상치 못한 응답 형식입니다.</li>';
        }
    } catch (err) {
        const list = document.getElementById('searchResults');
        list.innerHTML = `<li style="color: #dc3545; font-weight: bold;">네트워크 오류: ${err.message}</li>`;
        console.error('검색 네트워크 오류:', err);
    } finally {
        // 검색 버튼 복원
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
            
            // Check which steps are incomplete
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

document.addEventListener('DOMContentLoaded', function() {
    loadHistory();
    checkRunningTasks();
    startTaskMonitoring();
});
