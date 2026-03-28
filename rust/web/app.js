const POLL_INTERVAL_MS = 2000;
const UPLOAD_FILE_MAX_BYTES = 512 * 1024 * 1024;
const UPLOAD_FILE_MAX_LABEL = "512MB";
const AUDIO_FILE_EXTENSIONS = [
  ".wav",
  ".mp3",
  ".flac",
  ".ogg",
  ".m4a",
  ".aac",
  ".wma",
  ".aiff",
  ".aif",
  ".opus",
  ".webm",
  ".mp4",
  ".m4b",
];

const state = {
  jobs: [],
  selectedJobId: null,
  selectedJob: null,
  jobStatus: null,
  jobFiles: [],
  sttStatus: null,
  sttProgress: null,
  transcripts: [],
  summaryStatus: null,
  summaryText: "",
  embeddingStatus: null,
  systemStatus: null,
  modelsStatus: null,
  searchResults: [],
  uploadQueue: [],
  pollers: new Map(),
  loading: {
    upload: false,
    batchProcess: false,
    stt: false,
    summary: false,
    embedding: false,
    search: false,
    prepareWhisper: false,
    prepareLlama: false,
  },
};

const elements = {};

document.addEventListener("DOMContentLoaded", () => {
  captureElements();
  bindEvents();
  renderAll();
  bootstrap();
});

function captureElements() {
  elements.globalCaption = document.getElementById("global-caption");
  elements.systemGrid = document.getElementById("system-grid");
  elements.modelGrid = document.getElementById("model-grid");
  elements.jobsList = document.getElementById("jobs-list");
  elements.selectedJobTitle = document.getElementById("selected-job-title");
  elements.jobOverview = document.getElementById("job-overview");
  elements.jobTasks = document.getElementById("job-tasks");
  elements.sttAudioOptions = document.getElementById("stt-audio-options");
  elements.sttStatus = document.getElementById("stt-status");
  elements.transcriptsView = document.getElementById("transcripts-view");
  elements.summaryStatus = document.getElementById("summary-status");
  elements.summaryTextView = document.getElementById("summary-text-view");
  elements.embeddingStatus = document.getElementById("embedding-status");
  elements.embeddingMetadata = document.getElementById("embedding-metadata");
  elements.filesView = document.getElementById("files-view");
  elements.searchResults = document.getElementById("search-results");

  elements.systemRefreshButton = document.getElementById("system-refresh-button");
  elements.jobsRefreshButton = document.getElementById("jobs-refresh-button");
  elements.selectedJobRefreshButton = document.getElementById("selected-job-refresh-button");
  elements.uploadForm = document.getElementById("upload-form");
  elements.uploadDropzone = document.getElementById("upload-dropzone");
  elements.uploadInput = document.getElementById("upload-input");
  elements.uploadSubmitButton = document.getElementById("upload-submit-button");
  elements.batchProcessButton = document.getElementById("batch-process-button");
  elements.uploadQueue = document.getElementById("upload-queue");
  elements.sttForm = document.getElementById("stt-form");
  elements.sttSubmitButton = document.getElementById("stt-submit-button");
  elements.summarySubmitButton = document.getElementById("summary-submit-button");
  elements.summaryForceCheckbox = document.getElementById("summary-force-checkbox");
  elements.embeddingSubmitButton = document.getElementById("embedding-submit-button");
  elements.searchForm = document.getElementById("search-form");
  elements.searchQueryInput = document.getElementById("search-query-input");
  elements.searchLimitInput = document.getElementById("search-limit-input");
  elements.searchMinScoreInput = document.getElementById("search-min-score-input");
  elements.searchSubmitButton = document.getElementById("search-submit-button");
}

function bindEvents() {
  elements.systemRefreshButton.addEventListener("click", () => {
    refreshSystemAndModels({ showMessage: true });
  });
  elements.jobsRefreshButton.addEventListener("click", () => {
    refreshJobs({ showMessage: true });
  });
  elements.selectedJobRefreshButton.addEventListener("click", () => {
    if (!state.selectedJobId) {
      setMessage("selected-job", "선택된 Job이 없습니다.", "info");
      return;
    }
    refreshSelectedJob(state.selectedJobId, { showMessage: true });
  });
  elements.uploadForm.addEventListener("submit", onUploadSubmit);
  elements.batchProcessButton.addEventListener("click", onBatchProcessSubmit);
  elements.uploadInput.addEventListener("change", onUploadInputChange);
  elements.uploadDropzone.addEventListener("dragenter", onUploadDragEnter);
  elements.uploadDropzone.addEventListener("dragover", onUploadDragOver);
  elements.uploadDropzone.addEventListener("dragleave", onUploadDragLeave);
  elements.uploadDropzone.addEventListener("drop", onUploadDrop);
  elements.sttSubmitButton.addEventListener("click", onSttSubmit);
  elements.summarySubmitButton.addEventListener("click", onSummarySubmit);
  elements.embeddingSubmitButton.addEventListener("click", onEmbeddingSubmit);
  elements.searchForm.addEventListener("submit", onSearchSubmit);
}

async function bootstrap() {
  await Promise.all([refreshSystemAndModels(), refreshJobs()]);
}

async function onUploadSubmit(event) {
  event.preventDefault();
  const files = Array.from(elements.uploadInput.files || []);
  if (files.length === 0) {
    setMessage("upload", "업로드할 파일을 선택해 주세요.", "error");
    return;
  }

  setLoading("upload", true);
  try {
    let successCount = 0;
    let failedCount = 0;
    let selectedJobId = null;

    for (const file of files) {
      const formData = new FormData();
      formData.append("file", file);
      try {
        const { data } = await fetchJson("/jobs/upload", {
          method: "POST",
          body: formData,
        });
        successCount += 1;
        if (data?.job_id) {
          selectedJobId = data.job_id;
        }
      } catch (error) {
        failedCount += 1;
      }
    }

    const summaryParts = [];
    if (successCount > 0) {
      summaryParts.push(`${successCount}개 업로드 성공`);
    }
    if (failedCount > 0) {
      summaryParts.push(`${failedCount}개 업로드 실패`);
    }
    setMessage("upload", summaryParts.join(" / ") || "업로드 요청이 접수되었습니다.", failedCount > 0 ? "info" : "success");

    elements.uploadForm.reset();
    state.uploadQueue = [];
    renderUploadQueue();
    setUploadDropzoneDragState(false);
    await refreshJobs();
    if (selectedJobId) {
      await refreshSelectedJob(selectedJobId, { showMessage: true });
    }
  } catch (error) {
    setMessage("upload", error.message, "error");
  } finally {
    setLoading("upload", false);
  }
}

function onUploadInputChange(event) {
  applyUploadSelection(event.target.files, { source: "선택" });
}

function onUploadDragEnter(event) {
  event.preventDefault();
  setUploadDropzoneDragState(true);
}

function onUploadDragOver(event) {
  event.preventDefault();
  event.dataTransfer.dropEffect = "copy";
  setUploadDropzoneDragState(true);
}

function onUploadDragLeave(event) {
  event.preventDefault();
  if (event.currentTarget.contains(event.relatedTarget)) {
    return;
  }
  setUploadDropzoneDragState(false);
}

function onUploadDrop(event) {
  event.preventDefault();
  setUploadDropzoneDragState(false);

  applyUploadSelection(event.dataTransfer?.files, { source: "드래그 앤 드롭" });
}

function setUploadDropzoneDragState(isDragOver) {
  elements.uploadDropzone.classList.toggle("is-dragover", isDragOver);
}

function applyUploadSelection(fileList, { source }) {
  const files = Array.from(fileList || []);
  const accepted = [];
  const rejectedNonAudio = [];
  const rejectedOversized = [];

  for (const file of files) {
    if (!isAudioFile(file)) {
      rejectedNonAudio.push(file.name);
      continue;
    }
    if (file.size > UPLOAD_FILE_MAX_BYTES) {
      rejectedOversized.push(file.name);
      continue;
    }
    accepted.push(file);
  }

  const transfer = new DataTransfer();
  accepted.forEach((file) => transfer.items.add(file));
  elements.uploadInput.files = transfer.files;
  state.uploadQueue = accepted.map((file) => ({ name: file.name, size: file.size }));
  renderUploadQueue();

  if (accepted.length === 0) {
    const reasons = [];
    if (rejectedNonAudio.length > 0) {
      reasons.push("오디오 파일이 없습니다");
    }
    if (rejectedOversized.length > 0) {
      reasons.push(`${UPLOAD_FILE_MAX_LABEL} 초과 파일 제외`);
    }
    setMessage("upload", `${source}: 업로드 가능한 파일이 없습니다 (${reasons.join(", ")}).`, "error");
    return;
  }

  const messageParts = [`${source}: ${accepted.length}개 파일을 대기열에 추가했습니다.`];
  if (rejectedNonAudio.length > 0) {
    messageParts.push(`오디오 아님 ${rejectedNonAudio.length}개 제외`);
  }
  if (rejectedOversized.length > 0) {
    messageParts.push(`${UPLOAD_FILE_MAX_LABEL} 초과 ${rejectedOversized.length}개 제외`);
  }
  setMessage("upload", messageParts.join(" "), "info");
}

function isAudioFile(file) {
  if (!file) {
    return false;
  }
  if (typeof file.type === "string" && file.type.startsWith("audio/")) {
    return true;
  }
  const lowerName = (file.name || "").toLowerCase();
  return AUDIO_FILE_EXTENSIONS.some((extension) => lowerName.endsWith(extension));
}

function renderUploadQueue() {
  if (!elements.uploadQueue) {
    return;
  }
  if (state.uploadQueue.length === 0) {
    elements.uploadQueue.innerHTML = '<p class="muted">대기열이 비어 있습니다.</p>';
    return;
  }

  elements.uploadQueue.innerHTML = `
    <ul class="upload-queue-list">
      ${state.uploadQueue
        .map(
          (file) => `
            <li>
              <span>${escapeHtml(file.name)}</span>
              <code>${formatBytes(file.size)}</code>
            </li>
          `
        )
        .join("")}
    </ul>
  `;
}

async function onSttSubmit() {
  const jobId = requireSelectedJob();
  if (!jobId) {
    return;
  }
  const payload = buildSttPayload();
  if (!payload) {
    return;
  }

  setLoading("stt", true);
  try {
    const { status, data } = await fetchJson(`/jobs/${encodeURIComponent(jobId)}/stt`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify(payload),
    });
    setMessage(
      "selected-job",
      data.message || successMessage(status, "STT 요청이 접수되었습니다."),
      "info"
    );
    await refreshSelectedJob(jobId);
  } catch (error) {
    setMessage("selected-job", error.message, "error");
  } finally {
    setLoading("stt", false);
  }
}

async function onSummarySubmit() {
  const jobId = requireSelectedJob();
  if (!jobId) {
    return;
  }

  setLoading("summary", true);
  try {
    const { data } = await fetchJson(`/jobs/${encodeURIComponent(jobId)}/summary`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({
        force_regenerate: elements.summaryForceCheckbox.checked,
      }),
    });
    setMessage("selected-job", data.message || "요약 요청이 접수되었습니다.", "info");
    await refreshSelectedJob(jobId);
  } catch (error) {
    setMessage("selected-job", error.message, "error");
  } finally {
    setLoading("summary", false);
  }
}

async function onEmbeddingSubmit() {
  const jobId = requireSelectedJob();
  if (!jobId) {
    return;
  }

  setLoading("embedding", true);
  try {
    const { data } = await fetchJson(`/jobs/${encodeURIComponent(jobId)}/summary/embedding`, {
      method: "POST",
    });
    setMessage("selected-job", data.message || "Embedding 요청이 접수되었습니다.", "info");
    await refreshSelectedJob(jobId);
  } catch (error) {
    setMessage("selected-job", error.message, "error");
  } finally {
    setLoading("embedding", false);
  }
}

async function onSearchSubmit(event) {
  event.preventDefault();
  const query = elements.searchQueryInput.value.trim();
  if (!query) {
    setMessage("search", "검색어를 입력해 주세요.", "error");
    return;
  }

  const limit = Number(elements.searchLimitInput.value || 10);
  const minScoreRaw = elements.searchMinScoreInput.value.trim();
  const payload = {
    query,
    limit,
  };
  if (minScoreRaw) {
    payload.min_score = Number(minScoreRaw);
  }

  setLoading("search", true);
  try {
    const { data } = await fetchJson("/summary/search", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify(payload),
    });
    state.searchResults = Array.isArray(data?.results) ? data.results : [];
    renderSearchResults();
    const message =
      state.searchResults.length > 0
        ? `${state.searchResults.length}개의 결과를 찾았습니다. 검색 대상 Job에는 embedding이 미리 생성돼 있어야 합니다.`
        : "결과가 없습니다. 검색 대상 summary에 embedding이 생성돼 있는지 확인해 주세요.";
    setMessage("search", message, "info");
  } catch (error) {
    state.searchResults = [];
    renderSearchResults();
    setMessage("search", error.message, "error");
  } finally {
    setLoading("search", false);
  }
}

async function onBatchProcessSubmit() {
  setLoading("batchProcess", true);
  try {
    const { data } = await fetchJson("/jobs/batch-process", {
      method: "POST",
    });
    const message = [
      `일괄처리 큐 등록 완료`,
      `ffmpeg ${data?.ffmpeg_queued ?? 0}건`,
      `stt ${data?.stt_queued ?? 0}건`,
      `llm ${data?.summary_queued ?? 0}건`,
      `embed ${data?.embedding_queued ?? 0}건`,
    ].join(" · ");
    setMessage("upload", message, "success");
    await refreshJobs({ showMessage: true });
    if (state.selectedJobId) {
      await refreshSelectedJob(state.selectedJobId);
    }
  } catch (error) {
    setMessage("upload", error.message, "error");
  } finally {
    setLoading("batchProcess", false);
  }
}

function requireSelectedJob() {
  if (!state.selectedJobId) {
    setMessage("selected-job", "먼저 Job을 선택해 주세요.", "error");
    return null;
  }
  return state.selectedJobId;
}

function buildSttPayload() {
  const mode = new FormData(elements.sttForm).get("stt-mode");
  if (mode === "mono") {
    return { mono_mix_only: true };
  }
  if (mode === "selected") {
    const selected = Array.from(
      elements.sttAudioOptions.querySelectorAll('input[type="checkbox"]:checked')
    ).map((input) => input.value);
    if (selected.length === 0) {
      setMessage("selected-job", "선택 audio 모드에서는 최소 1개 파일을 골라야 합니다.", "error");
      return null;
    }
    return { audio_files: selected };
  }
  return {};
}

async function refreshSystemAndModels({ showMessage = false } = {}) {
  try {
    const [systemResult, modelsResult] = await Promise.all([
      fetchJson("/system/status"),
      fetchJson("/models/status"),
    ]);
    state.systemStatus = systemResult.data;
    state.modelsStatus = modelsResult.data;
    renderSystem();
    syncModelPoller();
    if (showMessage) {
      setMessage("system", "시스템과 모델 상태를 갱신했습니다.", "success");
    }
  } catch (error) {
    setMessage("system", error.message, "error");
  }
}

async function refreshJobs({ showMessage = false } = {}) {
  try {
    const { data } = await fetchJson("/jobs");
    state.jobs = Array.isArray(data?.jobs) ? data.jobs : [];
    renderJobs();
    if (!state.selectedJobId && state.jobs[0]) {
      await refreshSelectedJob(state.jobs[0].job_id);
    } else if (state.selectedJobId && !state.jobs.find((job) => job.job_id === state.selectedJobId)) {
      clearSelectedJob();
    }
    if (showMessage) {
      setMessage("jobs", "작업 목록을 갱신했습니다.", "success");
    }
  } catch (error) {
    setMessage("jobs", error.message, "error");
  }
}

async function refreshSelectedJob(jobId, { showMessage = false } = {}) {
  state.selectedJobId = jobId;
  renderJobs();

  const encodedJobId = encodeURIComponent(jobId);
  try {
    const [jobResult, statusResult, filesResult, sttResult, progressResult, summaryResult, embeddingResult] =
      await Promise.all([
        fetchJson(`/jobs/${encodedJobId}`),
        fetchJson(`/jobs/${encodedJobId}/status`),
        fetchJson(`/jobs/${encodedJobId}/files`),
        fetchJson(`/jobs/${encodedJobId}/stt`),
        fetchJson(`/jobs/${encodedJobId}/stt/progress`),
        fetchJson(`/jobs/${encodedJobId}/summary`),
        fetchJson(`/jobs/${encodedJobId}/summary/embedding`),
      ]);

    state.selectedJob = jobResult.data;
    state.jobStatus = statusResult.data;
    state.jobFiles = Array.isArray(filesResult.data?.files) ? filesResult.data.files : [];
    state.sttStatus = sttResult.data;
    state.sttProgress = progressResult.data;
    state.summaryStatus = summaryResult.data;
    state.embeddingStatus = embeddingResult.data;
    state.transcripts = [];
    state.summaryText = "";

    await Promise.all([refreshTranscripts(jobId), refreshSummaryText(jobId)]);
    renderSelectedJob();
    syncSelectedJobPoller();
    if (showMessage) {
      setMessage("selected-job", `Job ${jobId} 상세를 갱신했습니다.`, "success");
    }
  } catch (error) {
    clearSelectedJob();
    setMessage("selected-job", error.message, "error");
  }
}

async function refreshTranscripts(jobId) {
  const sttHasOutput =
    state.sttProgress?.completed_files > 0 ||
    state.jobFiles.some((file) => file.startsWith("stt/"));
  if (!sttHasOutput) {
    return;
  }
  try {
    const { data } = await fetchJson(`/jobs/${encodeURIComponent(jobId)}/stt/texts`);
    state.transcripts = Array.isArray(data?.transcripts) ? data.transcripts : [];
  } catch (_error) {
    state.transcripts = [];
  }
}

async function refreshSummaryText(jobId) {
  const hasSummaryFile = state.jobFiles.some((file) => file === "summary/result.md");
  if (!hasSummaryFile) {
    return;
  }
  try {
    const { data } = await fetchJson(`/jobs/${encodeURIComponent(jobId)}/summary/text`);
    state.summaryText = data?.text || "";
  } catch (_error) {
    state.summaryText = "";
  }
}

function clearSelectedJob() {
  state.selectedJobId = null;
  state.selectedJob = null;
  state.jobStatus = null;
  state.jobFiles = [];
  state.sttStatus = null;
  state.sttProgress = null;
  state.transcripts = [];
  state.summaryStatus = null;
  state.summaryText = "";
  state.embeddingStatus = null;
  stopPoller("selected-job");
  renderSelectedJob();
  renderJobs();
}

async function prepareModel(kind) {
  const route = kind === "whisper" ? "/models/whisper/prepare" : "/models/llama/prepare";
  const loadingKey = kind === "whisper" ? "prepareWhisper" : "prepareLlama";
  setLoading(loadingKey, true);
  try {
    const { status, data } = await fetchJson(route, { method: "POST" });
    setMessage(
      "system",
      data.message || successMessage(status, "환경 준비 요청이 접수되었습니다."),
      status === 202 ? "info" : "success"
    );
    await refreshSystemAndModels();
  } catch (error) {
    setMessage("system", error.message, "error");
  } finally {
    setLoading(loadingKey, false);
  }
}

function renderAll() {
  renderUploadQueue();
  renderSystem();
  renderJobs();
  renderSelectedJob();
  renderSearchResults();
  updateActionStates();
}

function renderSystem() {
  const system = state.systemStatus;
  if (!system) {
    elements.systemGrid.innerHTML = '<p class="muted">시스템 상태를 불러오는 중입니다.</p>';
    elements.modelGrid.innerHTML = "";
    return;
  }

  const tiles = [
    ["FFmpeg", system.ffmpeg_available],
    ["Whisper", system.whisper_available],
    ["Llama", system.llama_available],
    ["Whisper Model", system.whisper_model_ready],
    ["Llama Model", system.llama_model_ready],
    ["Embedding Model", system.llama_embedding_model_ready],
  ];

  elements.systemGrid.innerHTML = tiles
    .map(
      ([label, ready]) => `
        <article class="system-tile">
          <strong>${label}</strong>
          <span class="badge" data-status="${ready ? "completed" : "failed"}">${ready ? "ready" : "missing"}</span>
        </article>
      `
    )
    .join("");

  const models = state.modelsStatus;
  if (!models) {
    elements.modelGrid.innerHTML = "";
    return;
  }

  const rows = [
    {
      key: "whisper",
      label: "Whisper",
      description: "STT 전사용 모델과 준비 상태",
      status: models.whisper,
      loading: state.loading.prepareWhisper,
    },
    {
      key: "llama",
      label: "Llama",
      description: "요약 및 embedding 준비 상태",
      status: models.llama,
      loading: state.loading.prepareLlama,
    },
  ];

  elements.modelGrid.innerHTML = rows
    .map((row) => {
      const isRunning = row.status?.preparation?.status === "running";
      const errors = collectModelErrors(row.status);
      return `
        <article class="model-row">
          <div>
            <h3>${row.label}</h3>
            <p>${row.description}</p>
            <div class="badge-row">
              ${statusBadge(row.status?.preparation?.status || "idle")}
              ${row.status?.ready ? statusBadge("completed", "model ready") : ""}
              ${row.status?.embedding_ready ? statusBadge("completed", "embedding ready") : ""}
            </div>
            ${errors.length > 0 ? `<p class="muted">${escapeHtml(errors.join(" / "))}</p>` : ""}
          </div>
          <div class="model-actions">
            <button class="primary-button" data-model-prepare="${row.key}" ${
              row.loading || isRunning ? "disabled" : ""
            }>
              ${row.loading ? "요청 중..." : isRunning ? "준비 중..." : `${row.label} 준비`}
            </button>
          </div>
        </article>
      `;
    })
    .join("");

  elements.modelGrid.querySelectorAll("[data-model-prepare]").forEach((button) => {
    button.addEventListener("click", () => prepareModel(button.dataset.modelPrepare));
  });

  const errorCount = Array.isArray(system.errors) ? system.errors.length : 0;
  elements.globalCaption.textContent =
    errorCount > 0
      ? "환경 준비가 필요합니다. setup을 다시 실행하세요."
      : "모든 런타임과 모델이 준비되었습니다.";
}

function renderJobs() {
  if (!state.jobs.length) {
    elements.jobsList.innerHTML = '<li class="muted">아직 생성된 Job이 없습니다.</li>';
    return;
  }

  elements.jobsList.innerHTML = state.jobs
    .map((job) => {
      const selectedClass = job.job_id === state.selectedJobId ? "is-selected" : "";
      const taskBadges = Array.isArray(job.tasks)
        ? job.tasks
            .map((task) => statusBadge(task.status, `${task.task_type}:${task.status}`))
            .join("")
        : "";
      return `
        <li>
          <button class="job-item ${selectedClass}" data-job-id="${job.job_id}" type="button">
            <div class="job-item-head">
              <p class="job-item-title">${escapeHtml(job.source_file_name || job.job_id)}</p>
              ${statusBadge(job.status)}
            </div>
            <p class="job-item-meta">${escapeHtml(job.job_id)} · ${escapeHtml(formatDate(job.started_at))}</p>
            <div class="badge-row">${taskBadges}</div>
          </button>
        </li>
      `;
    })
    .join("");

  elements.jobsList.querySelectorAll("[data-job-id]").forEach((button) => {
    button.addEventListener("click", () => {
      refreshSelectedJob(button.dataset.jobId, { showMessage: true });
    });
  });
}

function renderSelectedJob() {
  const job = state.selectedJob;
  updateActionStates();
  if (!job) {
    elements.selectedJobTitle.textContent = "선택된 Job 없음";
    elements.jobOverview.innerHTML = '<dt>상태</dt><dd class="muted">왼쪽 목록에서 Job을 선택하세요.</dd>';
    elements.jobTasks.innerHTML = "";
    elements.sttAudioOptions.innerHTML = "";
    elements.sttStatus.textContent = "";
    elements.summaryStatus.textContent = "";
    elements.embeddingStatus.textContent = "";
    elements.transcriptsView.innerHTML = '<p class="muted">전사 결과가 여기에 표시됩니다.</p>';
    elements.summaryTextView.textContent = "";
    elements.embeddingMetadata.innerHTML = '<dt>상태</dt><dd class="muted">embedding metadata 없음</dd>';
    elements.filesView.innerHTML = '<p class="muted">파일 목록이 여기에 표시됩니다.</p>';
    return;
  }

  elements.selectedJobTitle.textContent = `${job.source_file_name} · ${job.job_id}`;
  elements.jobOverview.innerHTML = [
    ["job_id", job.job_id],
    ["status", job.status],
    ["started_at", formatDate(job.started_at)],
    ["finished_at", formatDate(job.finished_at)],
    ["channels", job.probe?.channels ?? "-"],
    ["channel_layout", job.probe?.channel_layout ?? "-"],
    ["job_dir", job.job_dir],
    ["error", job.error_message ?? "-"],
  ]
    .map(([key, value]) => `<dt>${escapeHtml(String(key))}</dt><dd>${escapeHtml(String(value))}</dd>`)
    .join("");

  const tasks = Array.isArray(state.jobStatus?.tasks) ? state.jobStatus.tasks : job.tasks || [];
  elements.jobTasks.innerHTML = tasks.length
    ? tasks
        .map((task) => {
          const suffix = task.status === "running" ? '<span class="running-dot" aria-hidden="true"></span>' : "";
          return `
            <article class="task-card">
              <div class="badge-row">
                ${statusBadge(task.status, task.task_type)}
                ${suffix}
              </div>
              <p>started: ${escapeHtml(formatDate(task.started_at))}</p>
              <p>finished: ${escapeHtml(formatDate(task.finished_at))}</p>
              <p>retry: ${escapeHtml(String(task.retry_count ?? 0))}</p>
              <p>error: ${escapeHtml(task.last_error || "-")}</p>
            </article>
          `;
        })
        .join("")
    : '<p class="muted">task 정보가 없습니다.</p>';

  renderSttArea();
  renderSummaryArea();
  renderEmbeddingArea();
  renderFiles();
}

function renderSttArea() {
  const progress = state.sttProgress;
  const task = state.sttStatus?.task;
  const statusParts = [];
  if (task?.status) {
    statusParts.push(`task: ${task.status}`);
  }
  if (progress?.phase) {
    statusParts.push(`phase: ${progress.phase}`);
  }
  if (typeof progress?.completed_files === "number" && typeof progress?.total_files === "number") {
    statusParts.push(`files: ${progress.completed_files}/${progress.total_files}`);
  }
  if (typeof progress?.progress_percent === "number") {
    statusParts.push(`progress: ${progress.progress_percent}%`);
  }
  setInlineStatus("stt-status", statusParts.join(" · ") || "STT 미실행", toneForStatus(task?.status));

  const audioFiles = state.jobFiles.filter((file) => isSelectableAudioFile(file));
  elements.sttAudioOptions.innerHTML = audioFiles.length
    ? audioFiles
        .map(
          (file) => `
            <label>
              <input type="checkbox" value="${escapeAttribute(file)}" />
              <span>${escapeHtml(file)}</span>
            </label>
          `
        )
        .join("")
    : '<p class="muted">선택 가능한 audio 파일이 없습니다.</p>';

  elements.transcriptsView.innerHTML = state.transcripts.length
    ? state.transcripts
        .map(
          (item) => `
            <article class="transcript-item">
              <h4>${escapeHtml(item.file_name)}</h4>
              <pre class="text-view">${escapeHtml(item.text || "")}</pre>
            </article>
          `
        )
        .join("")
    : '<p class="muted">전사 결과가 없으면 이 영역이 비어 있습니다.</p>';
}

function renderSummaryArea() {
  const task = state.summaryStatus?.task;
  const parts = [task?.status ? `task: ${task.status}` : "task: idle"];
  if (task?.last_error) {
    parts.push(`error: ${task.last_error}`);
  }
  setInlineStatus("summary-status", parts.join(" · "), toneForStatus(task?.status));
  elements.summaryTextView.textContent =
    state.summaryText || "요약 결과가 생성되면 여기 표시됩니다.";
}

function renderEmbeddingArea() {
  const task = state.embeddingStatus?.task;
  const metadata = state.embeddingStatus?.metadata;
  const parts = [task?.status ? `task: ${task.status}` : metadata ? "task: completed" : "task: idle"];
  if (metadata?.dimension) {
    parts.push(`dimension: ${metadata.dimension}`);
  }
  setInlineStatus(
    "embedding-status",
    parts.join(" · "),
    toneForStatus(task?.status || (metadata ? "completed" : "idle"))
  );

  elements.embeddingMetadata.innerHTML = metadata
    ? [
        ["model_id", metadata.model_id],
        ["dimension", metadata.dimension],
        ["normalized", metadata.normalized],
        ["created_at", formatDate(metadata.created_at)],
        ["file_path", metadata.file_path],
      ]
        .map(([key, value]) => `<dt>${escapeHtml(String(key))}</dt><dd>${escapeHtml(String(value))}</dd>`)
        .join("")
    : '<dt>상태</dt><dd class="muted">생성된 embedding metadata가 없습니다.</dd>';
}

function renderFiles() {
  if (!state.jobFiles.length || !state.selectedJobId) {
    elements.filesView.innerHTML = '<p class="muted">다운로드 가능한 파일이 없습니다.</p>';
    return;
  }

  elements.filesView.innerHTML = state.jobFiles
    .map((file) => {
      const href = `/jobs/${encodeURIComponent(state.selectedJobId)}/files/${encodePath(file)}`;
      return `<a href="${href}" target="_blank" rel="noreferrer">${escapeHtml(file)}</a>`;
    })
    .join("");
}

function renderSearchResults() {
  if (!state.searchResults.length) {
    elements.searchResults.innerHTML =
      '<p class="muted">검색 결과가 없으면 여기에 similarity search 결과가 표시됩니다.</p>';
    return;
  }

  elements.searchResults.innerHTML = `
    <table>
      <thead>
        <tr>
          <th>Score</th>
          <th>Source</th>
          <th>Summary</th>
          <th>Excerpt</th>
        </tr>
      </thead>
      <tbody>
        ${state.searchResults
          .map(
            (row) => `
              <tr data-search-job-id="${row.job_id}">
                <td>${escapeHtml(Number(row.score).toFixed(4))}</td>
                <td>${escapeHtml(row.source_file_name)}</td>
                <td>${escapeHtml(row.summary_file_name)}</td>
                <td>${escapeHtml(row.summary_excerpt)}</td>
              </tr>
            `
          )
          .join("")}
      </tbody>
    </table>
  `;

  elements.searchResults.querySelectorAll("[data-search-job-id]").forEach((row) => {
    row.addEventListener("click", () => {
      refreshSelectedJob(row.dataset.searchJobId, { showMessage: true });
    });
  });
}

function updateActionStates() {
  const hasJob = Boolean(state.selectedJobId);
  const runningTask = state.selectedJob?.tasks?.some((task) => task.status === "running");

  elements.uploadSubmitButton.disabled = state.loading.upload;
  elements.batchProcessButton.disabled = state.loading.batchProcess;
  elements.systemRefreshButton.disabled = false;
  elements.jobsRefreshButton.disabled = false;
  elements.selectedJobRefreshButton.disabled = !hasJob;
  elements.sttSubmitButton.disabled = !hasJob || state.loading.stt || runningTask;
  elements.summarySubmitButton.disabled = !hasJob || state.loading.summary || runningTask;
  elements.embeddingSubmitButton.disabled = !hasJob || state.loading.embedding || runningTask;
  elements.searchSubmitButton.disabled = state.loading.search;
}

function syncModelPoller() {
  const whisperRunning = state.modelsStatus?.whisper?.preparation?.status === "running";
  const llamaRunning = state.modelsStatus?.llama?.preparation?.status === "running";
  if (whisperRunning || llamaRunning) {
    startPoller("models", () => refreshSystemAndModels());
  } else {
    stopPoller("models");
  }
}

function syncSelectedJobPoller() {
  const runningTask = state.selectedJob?.tasks?.some((task) => task.status === "running");
  if (state.selectedJobId && runningTask) {
    startPoller("selected-job", () => refreshSelectedJob(state.selectedJobId));
  } else {
    stopPoller("selected-job");
  }
}

function startPoller(key, callback) {
  if (state.pollers.has(key)) {
    return;
  }
  const handle = window.setInterval(() => {
    callback().catch((error) => {
      console.error(error);
    });
  }, POLL_INTERVAL_MS);
  state.pollers.set(key, handle);
}

function stopPoller(key) {
  const handle = state.pollers.get(key);
  if (handle) {
    window.clearInterval(handle);
    state.pollers.delete(key);
  }
}

function setMessage(section, message, tone = "info") {
  const target = document.getElementById(`${section}-message`);
  if (!target) {
    return;
  }
  target.textContent = message || "";
  if (message) {
    target.dataset.tone = tone;
  } else {
    delete target.dataset.tone;
  }
}

function setInlineStatus(id, message, tone = "info") {
  const target = document.getElementById(id);
  if (!target) {
    return;
  }
  target.textContent = message;
  target.dataset.tone = tone;
}

function setLoading(key, value) {
  state.loading[key] = value;
  updateActionStates();
}

function formatBytes(bytes) {
  const amount = Number(bytes) || 0;
  if (amount < 1024) {
    return `${amount} B`;
  }
  if (amount < 1024 * 1024) {
    return `${(amount / 1024).toFixed(1)} KB`;
  }
  if (amount < 1024 * 1024 * 1024) {
    return `${(amount / (1024 * 1024)).toFixed(1)} MB`;
  }
  return `${(amount / (1024 * 1024 * 1024)).toFixed(2)} GB`;
}

async function fetchJson(path, options = {}) {
  const response = await fetch(path, options);
  const contentType = response.headers.get("content-type") || "";
  let payload;
  if (contentType.includes("application/json")) {
    payload = await response.json();
  } else {
    payload = await response.text();
  }

  if (!response.ok) {
    const message =
      typeof payload === "object" && payload && "message" in payload
        ? payload.message
        : `Request failed with status ${response.status}`;
    throw new Error(message);
  }

  return { status: response.status, data: payload };
}

function collectModelErrors(modelStatus) {
  const messages = [];
  const seen = new Set();
  if (modelStatus?.error) {
    seen.add(modelStatus.error);
  }
  if (modelStatus?.embedding_error) {
    seen.add(modelStatus.embedding_error);
  }
  if (modelStatus?.preparation?.last_error) {
    seen.add(modelStatus.preparation.last_error);
  }
  for (const message of seen) {
    if (message) {
      messages.push(message);
    }
  }
  return messages;
}

function statusBadge(status, label = status) {
  const normalized = status || "idle";
  return `<span class="badge" data-status="${escapeAttribute(normalized)}">${escapeHtml(label)}</span>`;
}

function successMessage(status, fallback) {
  if (status === 202) {
    return "요청이 접수되었고 백그라운드 처리가 시작되었습니다.";
  }
  return fallback;
}

function toneForStatus(status) {
  if (status === "completed") {
    return "success";
  }
  if (status === "failed") {
    return "error";
  }
  if (status === "running") {
    return "info";
  }
  return "info";
}

function formatDate(value) {
  if (!value) {
    return "-";
  }
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) {
    return value;
  }
  return date.toLocaleString();
}

function encodePath(path) {
  return path
    .split("/")
    .map((segment) => encodeURIComponent(segment))
    .join("/");
}

function isSelectableAudioFile(file) {
  return file === "mono_mix.wav" || /^channel_\d+\.wav$/i.test(file);
}

function escapeHtml(value) {
  return String(value)
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;")
    .replaceAll("'", "&#39;");
}

function escapeAttribute(value) {
  return escapeHtml(value);
}
