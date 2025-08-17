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

    const taskId = 'task_' + Date.now() + '_' + Math.random().toString(36).substr(2, 9);
    const taskItem = {
        id: ++taskIdCounter,
        taskId: taskId,
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
        span.style.backgroundColor = '#28a745';
        span.style.color = 'white';
        span.style.cursor = 'pointer';
        span.style.textDecoration = 'underline';
        if (task === 'embedding') {
            span.title = '클릭하여 내용 및 유사 문서 보기';
            span.onclick = () => {
                showEmbeddingOverlay(downloadUrl);
            };
        } else {
            span.title = '클릭하여 내용 보기';
            span.onclick = () => {
                showTextOverlay(downloadUrl);
            };
        }
    } else if (record) {
        const existingTask = taskQueue.find(t =>
            t.recordId === record.id &&
            t.task === task
        );

        if (existingTask) {
            span.style.backgroundColor = '#17a2b8';
            span.style.color = 'white';
            span.style.cursor = 'default';
            span.title = '큐에 추가됨';
            span.onclick = null;
        } else {
            span.style.backgroundColor = '#6c757d';
            span.style.color = 'white';
            span.style.cursor = 'pointer';
            span.title = '클릭하여 작업 시작';
            span.onclick = () => {
                span.style.pointerEvents = 'none';
                span.style.opacity = '0.7';

                const existingTaskCheck = taskQueue.find(t =>
                    t.recordId === record.id &&
                    t.task === task
                );

                if (existingTaskCheck) {
                    span.style.pointerEvents = 'auto';
                    span.style.opacity = '1';
                    console.log(`Task ${task} for record ${record.id} already in queue, skipping`);
                    return;
                }

                if (!existingTaskCheck) {
                    if (task === 'summary' && record.file_type === 'audio' && !record.completed_tasks.stt) {
                        span.style.pointerEvents = 'auto';
                        span.style.opacity = '1';
                        showSttConfirmPopup();
                        const handleConfirm = () => {
                            hideSttConfirmPopup();
                            const steps = ['stt', 'summary'];
                            steps.forEach(step => {
                                const existingTask = taskQueue.find(t => t.recordId === record.id && t.task === step);
                                if (!existingTask) {
                                    const stepSpan = document.getElementById(`task-${record.id}-${step}`);
                                    if (stepSpan) {
                                        const addedId = addTaskToQueue(record.id, record.file_path, step, stepSpan, record.filename);
                                        if (addedId) setQueuedState(stepSpan);
                                    } else {
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

                    if (task === 'embedding' && record.file_type === 'audio' && !record.completed_tasks.stt) {
                        span.style.pointerEvents = 'auto';
                        span.style.opacity = '1';
                        fetch('/check_existing_stt', {
                            method: 'POST',
                            headers: { 'Content-Type': 'application/json' },
                            body: JSON.stringify({ file_path: record.file_path })
                        })
                        .then(response => response.json())
                        .then(data => {
                            if (data.has_stt) {
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

                    addTaskToQueue(record.id, record.file_path, task, span, record.filename);
                    setQueuedState(span);
                } else {
                    span.style.pointerEvents = 'auto';
                    span.style.opacity = '1';
                }
            };
        }
    } else {
        span.style.backgroundColor = '#e9ecef';
        span.style.color = '#6c757d';
    }

    return span;
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
                task_id: currentTask.taskId
            }),
            signal: currentTask.abortController.signal
        });

        if (response.ok) {
            const result = await response.json();

            if (result.error) {
                if (result.error === 'STT_DEPENDENCY_NOT_MET') {
                    currentTask.status = 'queued';
                    currentTask.retryCount = (currentTask.retryCount || 0) + 1;
                    currentTask.lastRetryTime = Date.now();

                    if (currentTask.retryCount < 20) {
                        taskQueue.push(currentTask);
                        taskElement.textContent = 'STT 대기';
                        taskElement.style.backgroundColor = '#ffc107';
                        taskElement.style.color = 'black';
                        taskElement.title = result.message || 'STT 작업 완료 대기 중';
                        currentTask = null;
                        currentCategory = taskQueue.length > 0 ? taskQueue[0].task : null;
                        updateQueueDisplay();
                        stopProgressPolling();
                        setTimeout(() => processNextTask(), 5000);
                        return;
                    } else {
                        taskElement.textContent = '오류';
                        taskElement.style.backgroundColor = '#dc3545';
                        taskElement.style.color = 'white';
                        taskElement.title = 'STT 작업을 기다리는 중 시간 초과';
                    }
                } else {
                    taskElement.textContent = '오류';
                    taskElement.style.backgroundColor = '#dc3545';
                    taskElement.style.color = 'white';
                    taskElement.title = `오류: ${result.error}`;
                }
            } else if (result[currentTask.task]) {
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
            taskElement.textContent = '오류';
            taskElement.style.backgroundColor = '#dc3545';
            taskElement.style.color = 'white';
            taskElement.title = '처리 중 오류가 발생했습니다';
        }
    } catch (error) {
        if (error.name === 'AbortError') {
            console.log('Task was cancelled');
        } else {
            const taskElement = currentTask.taskElement;
            taskElement.textContent = '오류';
            taskElement.style.backgroundColor = '#dc3545';
            taskElement.style.color = 'white';
            taskElement.title = `오류: ${error.message}`;
            console.error('Error processing task:', error);
        }
        stopProgressPolling();
    } finally {
        if (currentTask) {
            const isRetrying = taskQueue.some(t => t.id === currentTask.id);
            if (!isRetrying) {
                const taskIndex = taskQueue.findIndex(t => t.id === currentTask.id);
                if (taskIndex !== -1) {
                    taskQueue.splice(taskIndex, 1);
                }
            }
        }

        currentTask = null;
        currentCategory = taskQueue.length > 0 ? taskQueue[0].task : null;
        updateQueueDisplay();
        loadHistory();
        stopProgressPolling();
        setTimeout(() => processNextTask(), 100);
    }
}

async function checkRunningTasks() {
    try {
        const response = await fetch('/tasks');
        if (response.ok) {
            const runningTasks = await response.json();
            console.log('Running tasks found:', runningTasks);

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

let previousTaskCount = 0;
function startTaskMonitoring() {
    setInterval(async () => {
        try {
            const response = await fetch('/tasks');
            if (response.ok) {
                const runningTasks = await response.json();
                const taskCount = Object.keys(runningTasks).length;

                if (taskCount > 0) {
                    document.title = `(${taskCount}) RecordRoute File Upload`;
                } else {
                    document.title = 'RecordRoute File Upload';
                }

                if (previousTaskCount > 0 && taskCount < previousTaskCount) {
                    console.log('Tasks completed, reloading history...');
                    loadHistory();

                    if (taskCount === 0) {
                        const status = document.getElementById('status');
                        if (status.innerHTML.includes('백그라운드에서 실행 중인')) {
                            status.innerHTML = `
                                <div style="background: #d4edda; border: 1px solid #c3e6cb; padding: 10px; border-radius: 5px; margin: 10px 0; color: #155724;">
                                    ✅ 모든 백그라운드 작업이 완료되었습니다!
                                </div>
                            `;
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
    }, 2000);
}

document.getElementById('queueSortSelect').addEventListener('change', function() {
    sortTaskQueue();
    updateQueueDisplay();
});
