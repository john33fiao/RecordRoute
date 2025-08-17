let uploadedPath = null;
let fileType = null;
let recordId = null;

function updateWorkflowOptions(fileType) {
    const sttCheckbox = document.getElementById('stepStt');
    const sttLabel = sttCheckbox.parentElement;
    if (fileType === 'audio') {
        sttLabel.classList.remove('hidden', 'invisible');
        sttCheckbox.checked = false;
        sttCheckbox.disabled = false;
    } else {
        sttLabel.classList.add('hidden', 'invisible');
        sttCheckbox.checked = false;
        sttCheckbox.disabled = true;
    }

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
                document.getElementById('workflow').classList.remove('hidden');

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
                document.getElementById('workflow').classList.add('hidden');
            }

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
    downloads.innerHTML = '<p class="text-info">작업을 큐에 추가합니다...</p>';

    const history = await loadHistorySync();
    const currentRecord = history.find(record => record.id === recordId);
    const filename = currentRecord ? currentRecord.filename : 'Unknown File';

    steps.forEach(step => {
        const existingTask = taskQueue.find(t => t.recordId === recordId && t.task === step);
        if (!existingTask) {
            const tempElement = document.createElement('span');
            addTaskToQueue(recordId, uploadedPath, step, tempElement, filename);
        } else {
            console.log(`Task ${step} for record ${recordId} already in queue, skipping`);
        }
    });

    downloads.innerHTML = '<p class="text-success">선택한 작업들이 큐에 추가되었습니다.</p>';

    uploadedPath = null;
    fileType = null;
    recordId = null;
    document.getElementById('workflow').classList.add('hidden');
    document.getElementById('fileInput').value = '';
});

document.addEventListener('DOMContentLoaded', function() {
    loadHistory();
    checkRunningTasks();
    startTaskMonitoring();
});
