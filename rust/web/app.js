const POLL_INTERVAL_MS = 2000;
const UPLOAD_FILE_MAX_BYTES = 512 * 1024 * 1024;
const UPLOAD_FILE_MAX_LABEL = "512MB";
const TRANSCRIPT_PREVIEW_LINES = 10;
const QUEUE_COLLAPSE_THRESHOLD = 10;
const JOBS_COLLAPSE_THRESHOLD = 10;
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
  ".qta",
];

const QUEUE_COLUMNS = [
  { key: "ffmpeg", label: "FFMPEG", description: "오디오 분리" },
  { key: "stt", label: "STT", description: "전사" },
  { key: "llm", label: "LLM", description: "요약" },
  { key: "embed", label: "EMBED", description: "임베딩" },
];

const BATCH_PROCESS_TARGETS = [
  { value: "all", label: "전체 작업" },
  { value: "ffmpeg", label: "오디오 분리" },
  { value: "stt", label: "전사" },
  { value: "summary", label: "요약" },
  { value: "embedding", label: "임베딩" },
];

const TAB_KEYS = ["upload", "jobs", "search", "queue", "dictionary"];

const state = {
  activeTab: "upload",
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
  summaryOneLine: "",
  embeddingStatus: null,
  systemStatus: null,
  modelsStatus: null,
  queueStatus: null,
  queueLoaded: false,
  queueExpanded: emptyQueueExpandedState(),
  jobsCollapsed: true,
  jobResetModal: {
    open: false,
    selection: emptyJobResetSelection(),
  },
  settingsModal: {
    open: false,
  },
  dictionaryKeywords: emptyDictionaryKeywords(),
  dictionaryLoaded: false,
  pendingDictionaryRefreshJobId: null,
  searchResults: [],
  uploadQueue: [],
  pollers: new Map(),
  loading: {
    upload: false,
    batchProcess: false,
    queuePause: false,
    queueCancel: false,
    jobReset: false,
    stt: false,
    summary: false,
    embedding: false,
    search: false,
    prepareWhisper: false,
    prepareLlama: false,
    dictionary: false,
  },
};

const elements = {};

function emptyQueueExpandedState() {
  return Object.fromEntries(QUEUE_COLUMNS.map((column) => [column.key, false]));
}

function emptyJobResetSelection() {
  return {
    all: false,
    ffmpeg: false,
    stt: false,
    summary: false,
    embedding: false,
  };
}

function normalizeJobResetSelection(selection) {
  const normalized = {
    ...emptyJobResetSelection(),
    ...(selection || {}),
  };
  normalized.ffmpeg = Boolean(normalized.ffmpeg);
  normalized.stt = Boolean(normalized.stt);
  normalized.summary = Boolean(normalized.summary);
  normalized.embedding = Boolean(normalized.embedding);
  normalized.all =
    Boolean(normalized.all) ||
    (normalized.ffmpeg && normalized.stt && normalized.summary && normalized.embedding);

  if (normalized.all) {
    return {
      all: true,
      ffmpeg: true,
      stt: true,
      summary: true,
      embedding: true,
    };
  }

  return normalized;
}

function jobResetSelectionHasAny(selection) {
  const normalized = normalizeJobResetSelection(selection);
  return normalized.ffmpeg || normalized.stt || normalized.summary || normalized.embedding;
}

function hasQueuedOrRunningTasks() {
  const jobs = [...state.jobs];
  if (
    state.selectedJob &&
    !jobs.some((job) => job.job_id === state.selectedJob.job_id)
  ) {
    jobs.push(state.selectedJob);
  }

  return jobs.some((job) =>
    Array.isArray(job?.tasks) &&
    job.tasks.some((task) => task.status === "queued" || task.status === "running")
  );
}

function queueIsIdleForReset() {
  if (!state.queueLoaded) {
    return false;
  }

  const hasActiveBatch = Boolean(state.queueStatus?.active_batch);
  const hasPendingBatch = Array.isArray(state.queueStatus?.pending_batches)
    ? state.queueStatus.pending_batches.length > 0
    : false;

  return !hasActiveBatch && !hasPendingBatch && !hasQueuedOrRunningTasks();
}

function canResetSelectedJob() {
  return Boolean(state.selectedJobId) && queueIsIdleForReset() && !state.loading.jobReset;
}

document.addEventListener("DOMContentLoaded", () => {
  captureElements();
  bindEvents();
  state.activeTab = resolveTabFromHash();
  if (!window.location.hash) {
    window.history.replaceState(null, "", `${window.location.pathname}${window.location.search}#upload`);
  }
  renderAll();
  bootstrap();
});

function captureElements() {
  elements.tabButtons = Array.from(document.querySelectorAll("[data-tab]"));
  elements.tabPanels = Array.from(document.querySelectorAll("[data-tab-panel]"));
  elements.settingsOpenButton = document.getElementById("settings-open-button");
  elements.settingsModal = document.getElementById("settings-modal");
  elements.settingsCloseButton = document.getElementById("settings-close-button");
  elements.globalCaption = document.getElementById("global-caption");
  elements.systemServerStatus = document.getElementById("system-server-status");
  elements.systemGrid = document.getElementById("system-grid");
  elements.modelGrid = document.getElementById("model-grid");
  elements.jobsList = document.getElementById("jobs-list");
  elements.jobsCollapseToggle = document.getElementById("jobs-collapse-toggle");
  elements.selectedJobTitle = document.getElementById("selected-job-title");
  elements.selectedJobMeta = document.getElementById("selected-job-meta");
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
  elements.queueCaption = document.getElementById("queue-caption");
  elements.queueBoard = document.getElementById("queue-board");
  elements.dictionaryForm = document.getElementById("dictionary-form");
  elements.dictionaryInput = document.getElementById("dictionary-input");
  elements.dictionaryList = document.getElementById("dictionary-list");
  elements.dictionaryRefreshButton = document.getElementById("dictionary-refresh-button");
  elements.searchResults = document.getElementById("search-results");

  elements.systemRefreshButton = document.getElementById("system-refresh-button");
  elements.jobsDeleteButton = document.getElementById("jobs-delete-button");
  elements.jobsRefreshButton = document.getElementById("jobs-refresh-button");
  elements.selectedJobRefreshButton = document.getElementById("selected-job-refresh-button");
  elements.uploadForm = document.getElementById("upload-form");
  elements.uploadDropzone = document.getElementById("upload-dropzone");
  elements.uploadInput = document.getElementById("upload-input");
  elements.uploadSubmitButton = document.getElementById("upload-submit-button");
  elements.batchProcessTargetSelect = document.getElementById("batch-process-target");
  elements.batchProcessButton = document.getElementById("batch-process-button");
  elements.queueCancelButton = document.getElementById("queue-cancel-button");
  elements.uploadQueue = document.getElementById("upload-queue");
  elements.sttForm = document.getElementById("stt-form");
  elements.sttSubmitButton = document.getElementById("stt-submit-button");
  elements.summarySubmitButton = document.getElementById("summary-submit-button");
  elements.summaryForceCheckbox = document.getElementById("summary-force-checkbox");
  elements.embeddingSubmitButton = document.getElementById("embedding-submit-button");
  elements.queuePauseButton = document.getElementById("queue-pause-button");
  elements.queueRefreshButton = document.getElementById("queue-refresh-button");
  elements.dictionarySubmitButton = document.getElementById("dictionary-submit-button");
  elements.searchForm = document.getElementById("search-form");
  elements.searchQueryInput = document.getElementById("search-query-input");
  elements.searchLimitInput = document.getElementById("search-limit-input");
  elements.searchMinScoreInput = document.getElementById("search-min-score-input");
  elements.searchSubmitButton = document.getElementById("search-submit-button");
  elements.jobResetModal = document.getElementById("job-reset-modal");
  elements.jobResetTitle = document.getElementById("job-reset-title");
  elements.jobResetCopy = document.getElementById("job-reset-copy");
  elements.jobResetHint = document.getElementById("job-reset-hint");
  elements.jobResetForm = document.getElementById("job-reset-form");
  elements.jobResetConfirmButton = document.getElementById("job-reset-confirm-button");
  elements.jobResetCloseButton = document.getElementById("job-reset-close-button");
  elements.jobResetCancelButton = document.getElementById("job-reset-cancel-button");
  elements.jobResetAllCheckbox = document.getElementById("job-reset-all-checkbox");
  elements.jobResetFfmpegCheckbox = document.getElementById("job-reset-ffmpeg-checkbox");
  elements.jobResetSttCheckbox = document.getElementById("job-reset-stt-checkbox");
  elements.jobResetSummaryCheckbox = document.getElementById("job-reset-summary-checkbox");
  elements.jobResetEmbeddingCheckbox = document.getElementById("job-reset-embedding-checkbox");
}

function bindEvents() {
  elements.settingsOpenButton.addEventListener("click", openSettingsModal);
  elements.systemRefreshButton.addEventListener("click", () => {
    refreshSystemAndModels({ showMessage: true });
  });
  elements.tabButtons.forEach((button) => {
    button.addEventListener("click", () => {
      setActiveTab(button.dataset.tab);
    });
  });
  elements.jobsDeleteButton.addEventListener("click", openJobResetModal);
  elements.jobsCollapseToggle.addEventListener("click", onJobsCollapseToggle);
  elements.jobsRefreshButton.addEventListener("click", () => {
    Promise.all([refreshJobs({ showMessage: true }), refreshQueue()]).catch((error) => {
      console.error(error);
    });
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
  elements.queueCancelButton.addEventListener("click", onQueueCancelPending);
  elements.uploadInput.addEventListener("change", onUploadInputChange);
  elements.uploadDropzone.addEventListener("dragenter", onUploadDragEnter);
  elements.uploadDropzone.addEventListener("dragover", onUploadDragOver);
  elements.uploadDropzone.addEventListener("dragleave", onUploadDragLeave);
  elements.uploadDropzone.addEventListener("drop", onUploadDrop);
  elements.sttSubmitButton.addEventListener("click", onSttSubmit);
  elements.summarySubmitButton.addEventListener("click", onSummarySubmit);
  elements.embeddingSubmitButton.addEventListener("click", onEmbeddingSubmit);
  elements.queuePauseButton.addEventListener("click", onQueuePauseToggle);
  elements.queueRefreshButton.addEventListener("click", () => {
    refreshQueue({ showMessage: true });
  });
  elements.queueBoard.addEventListener("click", onQueueBoardClick);
  elements.dictionaryRefreshButton.addEventListener("click", () => {
    refreshDictionary({ showMessage: true }).catch((error) => {
      console.error(error);
    });
  });
  elements.dictionaryForm.addEventListener("submit", onDictionarySubmit);
  elements.dictionaryList.addEventListener("click", onDictionaryListClick);
  elements.searchForm.addEventListener("submit", onSearchSubmit);
  elements.transcriptsView.addEventListener("click", onTranscriptToggleClick);
  elements.jobResetForm.addEventListener("submit", onJobResetSubmit);
  elements.jobResetModal.querySelectorAll("[data-job-reset-close]").forEach((button) => {
    button.addEventListener("click", closeJobResetModal);
  });
  elements.settingsModal.querySelectorAll("[data-settings-close]").forEach((button) => {
    button.addEventListener("click", closeSettingsModal);
  });
  [
    elements.jobResetAllCheckbox,
    elements.jobResetFfmpegCheckbox,
    elements.jobResetSttCheckbox,
    elements.jobResetSummaryCheckbox,
    elements.jobResetEmbeddingCheckbox,
  ].forEach((checkbox) => {
    checkbox.addEventListener("change", onJobResetCheckboxChange);
  });
  window.addEventListener("hashchange", onHashChange);
  document.addEventListener("keydown", onDocumentKeydown);
}

async function bootstrap() {
  await Promise.all([refreshSystemAndModels(), refreshJobs(), refreshQueue(), refreshDictionary()]);
}

function onJobsCollapseToggle() {
  if (state.jobs.length <= JOBS_COLLAPSE_THRESHOLD) {
    return;
  }

  state.jobsCollapsed = !state.jobsCollapsed;
  renderJobs();
}

function resolveTabFromHash(hash = window.location.hash) {
  const value = String(hash || "")
    .replace(/^#/, "")
    .trim()
    .toLowerCase();
  return TAB_KEYS.includes(value) ? value : "upload";
}

function onHashChange() {
  const nextTab = resolveTabFromHash();
  if (nextTab === state.activeTab) {
    return;
  }
  state.activeTab = nextTab;
  renderTabs();
  syncSelectedJobPoller();
}

function setActiveTab(tab) {
  if (!TAB_KEYS.includes(tab)) {
    return;
  }

  state.activeTab = tab;
  renderTabs();
  syncSelectedJobPoller();

  const nextHash = `#${tab}`;
  if (window.location.hash !== nextHash) {
    window.location.hash = tab;
  }

  if (tab === "jobs" && state.selectedJobId) {
    refreshSelectedJob(state.selectedJobId).catch((error) => {
      console.error(error);
    });
  }
}

function renderTabs() {
  elements.tabButtons.forEach((button) => {
    const selected = button.dataset.tab === state.activeTab;
    button.setAttribute("aria-selected", String(selected));
    button.tabIndex = selected ? 0 : -1;
  });

  elements.tabPanels.forEach((panel) => {
    panel.hidden = panel.dataset.tabPanel !== state.activeTab;
  });
}

function syncModalBodyState() {
  document.body.classList.toggle(
    "is-modal-open",
    Boolean(state.jobResetModal.open || state.settingsModal.open)
  );
}

function openSettingsModal() {
  state.settingsModal.open = true;
  renderSettingsModal();
  refreshSystemAndModels().catch((error) => {
    console.error(error);
  });
}

function closeSettingsModal() {
  state.settingsModal.open = false;
  renderSettingsModal();
}

function openJobResetModal() {
  if (!canResetSelectedJob()) {
    return;
  }
  closeSettingsModal();
  state.jobResetModal.open = true;
  state.jobResetModal.selection = emptyJobResetSelection();
  setMessage("job-reset", "", "info");
  renderJobResetModal();
}

function closeJobResetModal() {
  state.jobResetModal.open = false;
  state.jobResetModal.selection = emptyJobResetSelection();
  setMessage("job-reset", "", "info");
  renderJobResetModal();
}

function onJobResetCheckboxChange(event) {
  const checkbox = event.target;
  const key = checkbox?.dataset?.jobResetCheckbox;
  if (!key) {
    return;
  }

  if (key === "all") {
    const checked = Boolean(checkbox.checked);
    state.jobResetModal.selection = {
      all: checked,
      ffmpeg: checked,
      stt: checked,
      summary: checked,
      embedding: checked,
    };
    renderJobResetModal();
    return;
  }

  state.jobResetModal.selection = {
    ...state.jobResetModal.selection,
    [key]: Boolean(checkbox.checked),
  };
  const { ffmpeg, stt, summary, embedding } = state.jobResetModal.selection;
  state.jobResetModal.selection.all = ffmpeg && stt && summary && embedding;
  renderJobResetModal();
}

async function onJobResetSubmit(event) {
  event.preventDefault();
  if (!state.selectedJobId) {
    setMessage("job-reset", "선택된 Job이 없습니다.", "error");
    return;
  }

  const payload = normalizeJobResetSelection(state.jobResetModal.selection);
  if (!jobResetSelectionHasAny(payload)) {
    setMessage("job-reset", "삭제할 단계를 최소 1개 선택해 주세요.", "error");
    renderJobResetModal();
    return;
  }

  setLoading("jobReset", true);
  try {
    const { data } = await fetchJson(`/jobs/${encodeURIComponent(state.selectedJobId)}/reset`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify(payload),
    });
    closeJobResetModal();
    setMessage("jobs", data?.message || "선택한 작업 내역을 삭제했습니다.", "success");
    await Promise.all([refreshJobs(), refreshQueue()]);
    await refreshSelectedJob(state.selectedJobId, { showMessage: true });
  } catch (error) {
    setMessage("job-reset", error.message, "error");
    renderJobResetModal();
  } finally {
    setLoading("jobReset", false);
  }
}

function onDocumentKeydown(event) {
  if (event.key !== "Escape") {
    return;
  }

  if (state.jobResetModal.open) {
    closeJobResetModal();
    return;
  }

  if (state.settingsModal.open) {
    closeSettingsModal();
  }
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
    const queueMessage =
      successCount > 0
        ? `${summaryParts.join(" / ")} · 각 파일이 ffmpeg 큐에 등록되었습니다.`
        : "업로드 요청이 접수되었습니다.";
    setMessage("upload", queueMessage, failedCount > 0 ? "info" : "success");

    elements.uploadForm.reset();
    state.uploadQueue = [];
    renderUploadQueue();
    setUploadDropzoneDragState(false);
    await Promise.all([refreshJobs(), refreshQueue()]);
    if (selectedJobId) {
      await refreshSelectedJob(selectedJobId, { showMessage: true });
    }
  } catch (error) {
    setMessage("upload", error.message, "error");
  } finally {
    setLoading("upload", false);
  }
}

function onTranscriptToggleClick(event) {
  const toggle = event.target.closest("[data-transcript-toggle]");
  if (!toggle) {
    return;
  }

  const item = toggle.closest(".transcript-item");
  if (!item) {
    return;
  }

  const expanded = item.classList.toggle("is-expanded");
  toggle.setAttribute("aria-expanded", String(expanded));
  toggle.textContent = expanded ? "접기" : `펼치기 (${TRANSCRIPT_PREVIEW_LINES}줄)`;
}

function onQueueBoardClick(event) {
  const toggle = event.target.closest("[data-queue-toggle]");
  if (!toggle) {
    return;
  }

  const columnKey = toggle.dataset.queueToggle;
  if (!columnKey || !(columnKey in state.queueExpanded)) {
    return;
  }

  state.queueExpanded[columnKey] = !state.queueExpanded[columnKey];
  renderQueueBoard();
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
  state.uploadQueue = accepted.map((file) => ({
    name: file.name,
    size: file.size,
    status: "selected",
  }));
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
              <div>
                <span>${escapeHtml(file.name)}</span>
                <code>${formatBytes(file.size)}</code>
              </div>
              ${statusBadge("completed", file.status || "selected")}
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
    await Promise.all([refreshSelectedJob(jobId), refreshQueue()]);
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
    state.pendingDictionaryRefreshJobId = data?.reused ? null : jobId;
    setMessage("selected-job", data.message || "요약 요청이 접수되었습니다.", "info");
    await Promise.all([refreshSelectedJob(jobId), refreshQueue()]);
  } catch (error) {
    state.pendingDictionaryRefreshJobId = null;
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
    await Promise.all([refreshSelectedJob(jobId), refreshQueue()]);
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

function resolveBatchProcessTarget() {
  return elements.batchProcessTargetSelect?.value || "all";
}

function resolveBatchProcessQueuedCount(target, submission) {
  switch (target) {
    case "ffmpeg":
      return submission?.ffmpeg_queued ?? 0;
    case "stt":
      return submission?.stt_queued ?? 0;
    case "summary":
      return submission?.summary_queued ?? 0;
    case "embedding":
      return submission?.embedding_queued ?? 0;
    default:
      return (submission?.ffmpeg_queued ?? 0)
        + (submission?.stt_queued ?? 0)
        + (submission?.summary_queued ?? 0)
        + (submission?.embedding_queued ?? 0);
  }
}

function resolveBatchProcessTargetLabel(target) {
  return BATCH_PROCESS_TARGETS.find((option) => option.value === target)?.label || "선택한 카테고리";
}

function buildBatchProcessMessage(target, submission) {
  if (target === "all") {
    return {
      tone: "success",
      text: [
        `일괄처리 큐 등록 완료`,
        `ffmpeg ${submission?.ffmpeg_queued ?? 0}건`,
        `stt ${submission?.stt_queued ?? 0}건`,
        `llm ${submission?.summary_queued ?? 0}건`,
        `embed ${submission?.embedding_queued ?? 0}건`,
      ].join(" · "),
    };
  }

  const label = resolveBatchProcessTargetLabel(target);
  const queuedCount = resolveBatchProcessQueuedCount(target, submission);
  if (queuedCount > 0) {
    return {
      tone: "success",
      text: `${label} 일괄처리 큐 등록 완료 · ${queuedCount}건`,
    };
  }

  return {
    tone: "info",
    text:
      target === "ffmpeg"
        ? `${label} 대상으로 큐에 추가할 작업이 없습니다.`
        : `${label} 대상으로 큐에 추가할 작업이 없습니다. 선행 단계가 완료된 작업만 선택 처리됩니다.`,
  };
}

async function onBatchProcessSubmit() {
  const target = resolveBatchProcessTarget();
  setLoading("batchProcess", true);
  try {
    const { data } = await fetchJson("/jobs/batch-process", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ target }),
    });
    const message = buildBatchProcessMessage(target, data);
    setMessage("upload", message.text, message.tone);
    await Promise.all([refreshJobs({ showMessage: true }), refreshQueue()]);
    if (state.selectedJobId) {
      await refreshSelectedJob(state.selectedJobId);
    }
  } catch (error) {
    setMessage("upload", error.message, "error");
  } finally {
    setLoading("batchProcess", false);
  }
}

async function onQueuePauseToggle() {
  const nextPaused = !state.queueStatus?.paused;
  setLoading("queuePause", true);
  renderQueueControls();
  try {
    const { data } = await fetchJson("/queue/pause", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ paused: nextPaused }),
    });
    state.queueStatus = data;
    state.queueLoaded = true;
    renderQueueBoard();
    syncQueuePoller();
    setMessage(
      "queue",
      nextPaused
        ? "큐를 일시정지했습니다. 현재 진행 중인 작업만 끝나고 다음 작업은 대기합니다."
        : "큐를 재개했습니다. 다음 작업부터 순서대로 이어집니다.",
      "info"
    );
  } catch (error) {
    setMessage("queue", error.message, "error");
  } finally {
    setLoading("queuePause", false);
    renderQueueControls();
  }
}

async function onQueueCancelPending() {
  const queuedCount = countQueuedEntries(state.queueStatus);
  if (queuedCount === 0) {
    setMessage("upload", "취소할 대기 작업이 없습니다.", "info");
    return;
  }
  if (!window.confirm(`현재 대기 중인 작업 ${queuedCount}건을 취소하시겠습니까?`)) {
    return;
  }

  setLoading("queueCancel", true);
  try {
    const { data } = await fetchJson("/queue/cancel-pending", {
      method: "POST",
    });
    const message = [
      `대기 작업 ${data?.total_cancelled ?? 0}건 취소`,
      `ffmpeg ${data?.ffmpeg_cancelled ?? 0}건`,
      `stt ${data?.stt_cancelled ?? 0}건`,
      `summary ${data?.summary_cancelled ?? 0}건`,
      `embedding ${data?.embedding_cancelled ?? 0}건`,
    ].join(" · ");
    setMessage("upload", message, "info");
    await Promise.all([
      refreshJobs(),
      refreshQueue(),
      state.selectedJobId ? refreshSelectedJob(state.selectedJobId) : Promise.resolve(),
    ]);
  } catch (error) {
    setMessage("upload", error.message, "error");
  } finally {
    setLoading("queueCancel", false);
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

async function onDictionarySubmit(event) {
  event.preventDefault();
  const keyword = elements.dictionaryInput.value.trim();
  if (!keyword) {
    setMessage("dictionary", "추가할 키워드를 입력해 주세요.", "error");
    return;
  }

  setLoading("dictionary", true);
  try {
    const { data } = await fetchJson("/dictionary/keywords", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ keyword }),
    });
    state.dictionaryKeywords = normalizeDictionaryKeywords(data);
    state.dictionaryLoaded = true;
    elements.dictionaryForm.reset();
    renderDictionary();
    setMessage("dictionary", "키워드를 추가했습니다.", "success");
  } catch (error) {
    setMessage("dictionary", error.message, "error");
  } finally {
    setLoading("dictionary", false);
    renderDictionary();
  }
}

async function onDictionaryListClick(event) {
  const button = event.target.closest("[data-dictionary-action]");
  if (!button || state.loading.dictionary) {
    return;
  }

  const keyword = button.dataset.dictionaryKeyword || "";
  const action = button.dataset.dictionaryAction || "";
  if (!keyword || !action) {
    return;
  }

  let requestPath = "";
  let requestOptions = {};
  let successMessage = "";

  switch (action) {
    case "delete-user":
      if (!window.confirm(`${keyword} 키워드를 삭제하시겠습니까?`)) {
        return;
      }
      requestPath = `/dictionary/keywords/${encodeURIComponent(keyword)}`;
      requestOptions = { method: "DELETE" };
      successMessage = "키워드를 삭제했습니다.";
      break;
    case "promote-auto":
      requestPath = `/dictionary/keywords/auto/${encodeURIComponent(keyword)}/promote`;
      requestOptions = { method: "POST" };
      successMessage = "자동 생성 키워드를 사용자 등록 키워드로 옮겼습니다.";
      break;
    case "delete-auto":
      requestPath = `/dictionary/keywords/auto/${encodeURIComponent(keyword)}`;
      requestOptions = { method: "DELETE" };
      successMessage = "자동 생성 키워드를 삭제했습니다.";
      break;
    default:
      return;
  }

  setLoading("dictionary", true);
  try {
    const { data } = await fetchJson(requestPath, requestOptions);
    state.dictionaryKeywords = normalizeDictionaryKeywords(data);
    state.dictionaryLoaded = true;
    renderDictionary();
    setMessage("dictionary", successMessage, "success");
  } catch (error) {
    setMessage("dictionary", error.message, "error");
  } finally {
    setLoading("dictionary", false);
    renderDictionary();
  }
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
  } finally {
    updateActionStates();
  }
}

async function refreshDictionary({ showMessage = false } = {}) {
  setLoading("dictionary", true);
  try {
    const { data } = await fetchJson("/dictionary/keywords");
    state.dictionaryKeywords = normalizeDictionaryKeywords(data);
    state.dictionaryLoaded = true;
    renderDictionary();
    if (showMessage) {
      setMessage("dictionary", "키워드 목록을 갱신했습니다.", "success");
    }
  } catch (error) {
    renderDictionary();
    setMessage("dictionary", error.message, "error");
  } finally {
    setLoading("dictionary", false);
    renderDictionary();
  }
}

async function refreshQueue({ showMessage = false } = {}) {
  try {
    const { data } = await fetchJson("/queue");
    state.queueStatus = data;
    state.queueLoaded = true;
    renderQueueBoard();
    syncQueuePoller();
    if (showMessage) {
      setMessage("queue", "전역 큐 상태를 갱신했습니다.", "success");
    }
  } catch (error) {
    renderQueueBoard();
    setMessage("queue", error.message, "error");
  } finally {
    updateActionStates();
  }
}

async function refreshSelectedJob(jobId, { showMessage = false } = {}) {
  state.selectedJobId = jobId;
  renderJobs();

  const encodedJobId = encodeURIComponent(jobId);
  const previousSummaryStatus = state.summaryStatus?.task?.status || null;
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
    const currentSummaryStatus = state.summaryStatus?.task?.status || null;
    state.transcripts = [];
    state.summaryText = "";
    state.summaryOneLine = "";

    await Promise.all([refreshTranscripts(jobId), refreshSummaryText(jobId)]);
    if (
      state.pendingDictionaryRefreshJobId === jobId &&
      currentSummaryStatus === "completed" &&
      previousSummaryStatus !== "completed"
    ) {
      await refreshDictionary();
      state.pendingDictionaryRefreshJobId = null;
    } else if (
      state.pendingDictionaryRefreshJobId === jobId &&
      currentSummaryStatus === "failed"
    ) {
      state.pendingDictionaryRefreshJobId = null;
    }
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
    state.summaryOneLine = data?.one_line_summary || "";
  } catch (_error) {
    state.summaryText = "";
    state.summaryOneLine = "";
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
  state.summaryOneLine = "";
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
  renderTabs();
  renderSettingsModal();
  renderUploadQueue();
  renderSystem();
  renderJobs();
  renderJobResetModal();
  renderSelectedJob();
  renderQueueBoard();
  renderDictionary();
  renderSearchResults();
  updateActionStates();
}

function renderSystem() {
  const system = state.systemStatus;
  if (!system) {
    elements.systemServerStatus.textContent = "Checking";
    elements.systemServerStatus.dataset.status = "idle";
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
  const systemReady =
    errorCount === 0 &&
    tiles.every(([, ready]) => Boolean(ready));

  elements.systemServerStatus.textContent = systemReady ? "Ready" : "Needs setup";
  elements.systemServerStatus.dataset.status = systemReady ? "completed" : "failed";
  elements.globalCaption.textContent =
    errorCount > 0
      ? "환경 준비가 필요합니다. setup을 다시 실행하세요."
      : "모든 런타임과 모델이 준비되었습니다.";
}

function renderJobs() {
  if (!state.jobs.length) {
    elements.jobsList.innerHTML = '<li class="muted">아직 생성된 Job이 없습니다.</li>';
    elements.jobsCollapseToggle.hidden = true;
    elements.jobsCollapseToggle.setAttribute("aria-expanded", "false");
    return;
  }

  const isCollapsible = state.jobs.length > JOBS_COLLAPSE_THRESHOLD;
  const selectedIndex = state.jobs.findIndex((job) => job.job_id === state.selectedJobId);
  let visibleJobs =
    isCollapsible && state.jobsCollapsed ? state.jobs.slice(0, JOBS_COLLAPSE_THRESHOLD) : state.jobs;

  if (isCollapsible && state.jobsCollapsed && selectedIndex >= JOBS_COLLAPSE_THRESHOLD) {
    visibleJobs = [
      ...state.jobs.slice(0, JOBS_COLLAPSE_THRESHOLD - 1),
      state.jobs[selectedIndex],
    ];
  }

  const hiddenCount = Math.max(state.jobs.length - visibleJobs.length, 0);

  elements.jobsCollapseToggle.hidden = !isCollapsible;
  elements.jobsCollapseToggle.setAttribute("aria-expanded", String(!state.jobsCollapsed));
  elements.jobsCollapseToggle.textContent = state.jobsCollapsed ? `펼치기 (+${hiddenCount})` : "접기";

  elements.jobsList.innerHTML = visibleJobs
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

  if (state.jobsCollapsed && hiddenCount > 0) {
    elements.jobsList.insertAdjacentHTML(
      "beforeend",
      `<li class="jobs-list-summary muted">나머지 ${hiddenCount}개 Job은 펼치기 후 확인할 수 있습니다.</li>`
    );
  }

  elements.jobsList.querySelectorAll("[data-job-id]").forEach((button) => {
    button.addEventListener("click", () => {
      refreshSelectedJob(button.dataset.jobId, { showMessage: true });
    });
  });
}

function renderSettingsModal() {
  elements.settingsModal.hidden = !state.settingsModal.open;
  syncModelPoller();
  syncModalBodyState();
}

function renderJobResetModal() {
  const isOpen = state.jobResetModal.open;
  const selection = normalizeJobResetSelection(state.jobResetModal.selection);
  const selectedJob = state.jobs.find((job) => job.job_id === state.selectedJobId) || state.selectedJob;

  elements.jobResetModal.hidden = !isOpen;
  syncModalBodyState();

  if (selectedJob) {
    elements.jobResetTitle.textContent = `작업 삭제 · ${selectedJob.source_file_name || selectedJob.job_id}`;
    elements.jobResetCopy.textContent =
      "삭제 후에도 source와 job 자체는 유지됩니다. 체크한 단계만 지워 재작업 가능한 상태로 되돌립니다.";
  } else {
    elements.jobResetTitle.textContent = "작업 삭제";
    elements.jobResetCopy.textContent =
      "삭제 후에도 source와 job 자체는 유지됩니다. 체크한 단계만 지워 재작업 가능한 상태로 되돌립니다.";
  }

  elements.jobResetAllCheckbox.checked = selection.all;
  elements.jobResetFfmpegCheckbox.checked = selection.ffmpeg;
  elements.jobResetSttCheckbox.checked = selection.stt;
  elements.jobResetSummaryCheckbox.checked = selection.summary;
  elements.jobResetEmbeddingCheckbox.checked = selection.embedding;

  const resetAllowed = canResetSelectedJob();
  if (!state.queueLoaded) {
    elements.jobResetHint.textContent = "큐 상태를 불러온 뒤에 작업 삭제를 사용할 수 있습니다.";
  } else if (!queueIsIdleForReset()) {
    elements.jobResetHint.textContent =
      "진행 중이거나 대기 중인 작업이 있으면 작업 삭제를 사용할 수 없습니다.";
  } else {
    elements.jobResetHint.textContent =
      "삭제 가능 상태입니다. 하나 이상 체크하면 확인 버튼이 활성화됩니다.";
  }
  elements.jobResetHint.dataset.tone = resetAllowed ? "success" : "info";
  elements.jobResetConfirmButton.disabled =
    !resetAllowed || !jobResetSelectionHasAny(selection) || state.loading.jobReset;
}

function renderSelectedJob() {
  const job = state.selectedJob;
  updateActionStates();
  if (!job) {
    elements.selectedJobTitle.textContent = "선택된 Job 없음";
    elements.selectedJobMeta.textContent = "job id, timestamps, hash, task badges";
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

  elements.selectedJobTitle.textContent = `${job.source_file_name} · ${
    state.summaryOneLine || job.job_id
  }`;
  elements.selectedJobMeta.textContent = [
    job.job_id,
    formatDate(job.started_at),
    job.probe?.duration_seconds ? `${job.probe.duration_seconds}s` : null,
  ]
    .filter(Boolean)
    .join(" · ");
  elements.jobOverview.innerHTML = [
    ["job_id", job.job_id],
    ["status", job.status],
    ["started_at", formatDate(job.started_at)],
    ["finished_at", formatDate(job.finished_at)],
    ["source_ref", job.source_ref ?? "-"],
    ["source_kind", job.source_kind ?? "-"],
    ["source_content_sha256", job.source_content_sha256 ?? "-"],
    ["channels", job.probe?.channels ?? "-"],
    ["channel_layout", job.probe?.channel_layout ?? "-"],
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
          (item, index) => `
            <article class="transcript-item" style="--transcript-preview-lines: ${TRANSCRIPT_PREVIEW_LINES};">
              <div class="transcript-item-head">
                <h4>${escapeHtml(item.file_name)}</h4>
                <button
                  type="button"
                  class="ghost-button transcript-toggle"
                  data-transcript-toggle
                  aria-expanded="false"
                  aria-controls="transcript-body-${index}"
                >
                  펼치기 (${TRANSCRIPT_PREVIEW_LINES}줄)
                </button>
              </div>
              <pre class="text-view transcript-body" id="transcript-body-${index}">${escapeHtml(item.text || "")}</pre>
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
      '<p class="muted">검색 결과가 없으면 여기에 유사도 검색 결과가 표시됩니다.</p>';
    return;
  }

  elements.searchResults.innerHTML = `
    <div class="results-list">
      ${state.searchResults
        .map(
          (row) => `
            <article class="result-card">
              <div class="result-card-head">
                <div class="result-card-copy">
                  <p class="result-card-meta">${escapeHtml(row.job_id)}</p>
                  <p class="result-card-title">${escapeHtml(row.source_file_name)}</p>
                  <p>${escapeHtml(row.summary_file_name)}</p>
                </div>
                ${statusBadge("running", `score ${Number(row.score).toFixed(2)}`)}
              </div>
              <p class="result-card-meta">${escapeHtml(row.summary_excerpt)}</p>
              <div class="result-card-action">
                <button
                  class="ghost-button"
                  data-search-job-id="${escapeAttribute(row.job_id)}"
                  type="button"
                >
                  View
                </button>
              </div>
            </article>
          `
        )
        .join("")}
    </div>
  `;

  elements.searchResults.querySelectorAll("[data-search-job-id]").forEach((button) => {
    button.addEventListener("click", () => {
      setActiveTab("jobs");
      refreshSelectedJob(button.dataset.searchJobId, { showMessage: true });
    });
  });
}

function renderDictionary() {
  if (!elements.dictionaryList) {
    return;
  }

  if (!state.dictionaryLoaded) {
    elements.dictionaryList.innerHTML = '<p class="muted">키워드를 불러오는 중입니다.</p>';
    return;
  }

  elements.dictionaryList.innerHTML = `
    <div class="dictionary-groups">
      ${renderDictionaryGroup({
        title: "사용자 등록 키워드",
        description: "직접 추가한 키워드만 STT 기본 프롬프트에 주입됩니다.",
        emptyMessage: "사용자가 직접 등록한 키워드가 없습니다.",
        items: state.dictionaryKeywords.userKeywords,
        renderChip: renderUserDictionaryChip,
      })}
      ${renderDictionaryGroup({
        title: "자동 생성 키워드",
        description: "텍스트 요약 과정에서 발견한 후보 키워드입니다. + 버튼으로 사용자 키워드로 옮길 수 있습니다.",
        emptyMessage: "자동 생성된 키워드가 없습니다.",
        items: state.dictionaryKeywords.autoKeywords,
        renderChip: renderAutoDictionaryChip,
      })}
    </div>
  `;
}

function renderDictionaryGroup({ title, description, emptyMessage, items, renderChip }) {
  return `
    <section class="dictionary-group">
      <div class="dictionary-group-head">
        <h4>${escapeHtml(title)}</h4>
        <p>${escapeHtml(description)}</p>
      </div>
      ${
        items.length
          ? `<ul class="dictionary-chip-list">${items.map((keyword) => renderChip(keyword)).join("")}</ul>`
          : `<p class="muted">${escapeHtml(emptyMessage)}</p>`
      }
    </section>
  `;
}

function renderUserDictionaryChip(keyword) {
  return `
    <li class="dictionary-chip">
      <span>${escapeHtml(keyword)}</span>
      <div class="dictionary-chip-actions">
        <button
          type="button"
          class="dictionary-action-button dictionary-remove-button"
          data-dictionary-action="delete-user"
          data-dictionary-keyword="${escapeAttribute(keyword)}"
          ${state.loading.dictionary ? "disabled" : ""}
          aria-label="${escapeAttribute(`${keyword} 삭제`)}"
        >
          X
        </button>
      </div>
    </li>
  `;
}

function renderAutoDictionaryChip(keyword) {
  return `
    <li class="dictionary-chip">
      <span>${escapeHtml(keyword)}</span>
      <div class="dictionary-chip-actions">
        <button
          type="button"
          class="dictionary-action-button dictionary-promote-button"
          data-dictionary-action="promote-auto"
          data-dictionary-keyword="${escapeAttribute(keyword)}"
          ${state.loading.dictionary ? "disabled" : ""}
          aria-label="${escapeAttribute(`${keyword} 사용자 키워드로 이동`)}"
        >
          +
        </button>
        <button
          type="button"
          class="dictionary-action-button dictionary-remove-button"
          data-dictionary-action="delete-auto"
          data-dictionary-keyword="${escapeAttribute(keyword)}"
          ${state.loading.dictionary ? "disabled" : ""}
          aria-label="${escapeAttribute(`${keyword} 자동 키워드 삭제`)}"
        >
          X
        </button>
      </div>
    </li>
  `;
}

function renderQueueBoard() {
  if (!elements.queueBoard) {
    return;
  }

  renderQueueControls();
  const columns = buildQueueColumns();
  elements.queueBoard.innerHTML = columns.map(renderQueueColumn).join("");
}

function renderQueueControls() {
  const paused = Boolean(state.queueStatus?.paused);
  const hasRunningEntry = Boolean(state.queueStatus?.active_batch?.running);

  if (elements.queueCaption) {
    elements.queueCaption.textContent = paused
      ? hasRunningEntry
        ? "전역 큐가 일시정지되었습니다. 현재 진행 중인 작업만 끝나고 다음 작업은 대기합니다."
        : "전역 큐가 일시정지되었습니다. 재개 전까지 새 작업과 남은 작업은 모두 대기합니다."
      : "전역 큐의 남은 작업과 일괄처리로 미리 등록된 후속 단계를 칸반보드에서 확인합니다.";
  }

  if (elements.queuePauseButton) {
    elements.queuePauseButton.textContent = state.loading.queuePause
      ? paused
        ? "재개 중..."
        : "일시정지 중..."
      : paused
        ? "재개"
        : "일시정지";
    elements.queuePauseButton.dataset.active = paused ? "true" : "false";
  }
}

function buildQueueColumns() {
  const columns = new Map(
    QUEUE_COLUMNS.map((column) => [
      column.key,
      {
        ...column,
        active: false,
        running: null,
        entries: [],
      },
    ])
  );

  const activeBatch = state.queueStatus?.active_batch;
  if (activeBatch && columns.has(activeBatch.category)) {
    const column = columns.get(activeBatch.category);
    column.active = true;
    column.running = activeBatch.running || null;
    column.entries.push(...normalizeQueueEntries(activeBatch.entries));
  }

  const pendingBatches = Array.isArray(state.queueStatus?.pending_batches)
    ? state.queueStatus.pending_batches
    : [];
  for (const batch of pendingBatches) {
    if (!columns.has(batch.category)) {
      continue;
    }
    const column = columns.get(batch.category);
    if (!column.running && batch.running) {
      column.running = batch.running;
    }
    column.entries.push(...normalizeQueueEntries(batch.entries));
  }

  return QUEUE_COLUMNS.map((column) => {
    const item = columns.get(column.key);
    return {
      ...item,
      totalCount: item.entries.length + (item.running ? 1 : 0),
      emptyMessage: state.queueLoaded ? "남은 작업이 없습니다." : "큐 상태를 확인하는 중입니다.",
    };
  });
}

function normalizeQueueEntries(entries) {
  return Array.isArray(entries) ? entries.filter(Boolean) : [];
}

function renderQueueColumn(column) {
  const statusBadgeLabel = column.active ? "활성" : column.totalCount > 0 ? "대기" : "비어 있음";
  const statusTone = column.active ? "running" : "idle";
  const isExpanded = Boolean(state.queueExpanded[column.key]);
  const isCollapsible = column.entries.length > QUEUE_COLLAPSE_THRESHOLD;
  const visibleEntries = isExpanded ? column.entries : column.entries.slice(0, QUEUE_COLLAPSE_THRESHOLD);
  const hiddenCount = isCollapsible && !isExpanded ? column.entries.length - visibleEntries.length : 0;
  const cards = [];
  if (column.running) {
    cards.push(renderQueueCard(column.running, "running"));
  }
  for (const entry of visibleEntries) {
    cards.push(renderQueueCard(entry, "queued"));
  }
  const toggleButton = isCollapsible
    ? `
        <button
          class="ghost-button queue-column-toggle"
          data-queue-toggle="${escapeAttribute(column.key)}"
          aria-expanded="${isExpanded ? "true" : "false"}"
          type="button"
        >
          ${isExpanded ? "접기" : `더 보기 (${escapeHtml(String(hiddenCount))})`}
        </button>
      `
    : "";

  return `
    <section
      class="queue-column"
      data-active="${column.active ? "true" : "false"}"
      data-expanded="${isExpanded ? "true" : "false"}"
    >
      <div class="queue-column-head">
        <div class="queue-column-copy">
          <p class="queue-column-kicker">${escapeHtml(column.label)}</p>
          <h3>${escapeHtml(column.description)}</h3>
          <p>${column.active ? "현재 진행 중인 카테고리" : "남아 있는 작업 대기열"}</p>
        </div>
        <div class="queue-column-meta">
          ${statusBadge(statusTone, statusBadgeLabel)}
          <span class="queue-column-count">남은 ${escapeHtml(String(column.totalCount))}건</span>
        </div>
      </div>
      <div class="queue-column-body">
        ${cards.join("") || `<p class="queue-empty">${escapeHtml(column.emptyMessage)}</p>`}
        ${toggleButton}
      </div>
    </section>
  `;
}

function renderQueueCard(entry, stateLabel) {
  const isRunning = stateLabel === "running";
  return `
    <article class="queue-card ${isRunning ? "is-running" : ""}">
      <div class="queue-card-head">
        <p class="queue-card-title">${escapeHtml(resolveJobDisplayName(entry.job_id))}</p>
        <div class="badge-row">
          ${statusBadge(isRunning ? "running" : "idle", isRunning ? "진행 중" : "대기")}
          ${statusBadge("idle", formatTaskTypeLabel(entry.task_type))}
        </div>
      </div>
      <p class="queue-card-meta">${escapeHtml(entry.job_id)}</p>
      <p class="queue-card-meta">queued: ${escapeHtml(formatDate(entry.queued_at))}</p>
    </article>
  `;
}

function updateActionStates() {
  const hasJob = Boolean(state.selectedJobId);
  const runningTask = state.selectedJob?.tasks?.some((task) => task.status === "running");

  elements.settingsOpenButton.disabled = false;
  elements.uploadSubmitButton.disabled = state.loading.upload;
  elements.batchProcessTargetSelect.disabled = state.loading.batchProcess;
  elements.batchProcessButton.disabled = state.loading.batchProcess;
  elements.queueCancelButton.disabled = state.loading.queueCancel || !state.queueLoaded;
  elements.systemRefreshButton.disabled = false;
  elements.jobsRefreshButton.disabled = false;
  elements.jobsDeleteButton.disabled = !canResetSelectedJob();
  elements.selectedJobRefreshButton.disabled = !hasJob;
  elements.queuePauseButton.disabled = state.loading.queuePause || !state.queueLoaded;
  elements.queueRefreshButton.disabled = false;
  elements.dictionaryRefreshButton.disabled = state.loading.dictionary;
  elements.dictionarySubmitButton.disabled = state.loading.dictionary;
  elements.sttSubmitButton.disabled = !hasJob || state.loading.stt || runningTask;
  elements.summarySubmitButton.disabled = !hasJob || state.loading.summary || runningTask;
  elements.embeddingSubmitButton.disabled = !hasJob || state.loading.embedding || runningTask;
  elements.searchSubmitButton.disabled = state.loading.search;

  if (state.jobResetModal.open) {
    renderJobResetModal();
  }
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
  if (state.selectedJobId && runningTask && state.activeTab === "jobs") {
    startPoller("selected-job", () => refreshSelectedJob(state.selectedJobId));
  } else {
    stopPoller("selected-job");
  }
}

function syncQueuePoller() {
  const queuePaused = Boolean(state.queueStatus?.paused);
  const hasActiveBatch = Boolean(state.queueStatus?.active_batch);
  const hasPendingBatch = Array.isArray(state.queueStatus?.pending_batches)
    ? state.queueStatus.pending_batches.length > 0
    : false;
  if (queuePaused || hasActiveBatch || hasPendingBatch) {
    startPoller("queue", () => refreshQueue());
  } else {
    stopPoller("queue");
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

function normalizeDictionaryKeywords(data) {
  return {
    userKeywords: Array.isArray(data?.user_keywords) ? data.user_keywords.filter(Boolean) : [],
    autoKeywords: Array.isArray(data?.auto_keywords) ? data.auto_keywords.filter(Boolean) : [],
  };
}

function emptyDictionaryKeywords() {
  return {
    userKeywords: [],
    autoKeywords: [],
  };
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

function resolveJobDisplayName(jobId) {
  const selected = state.selectedJob?.job_id === jobId ? state.selectedJob : null;
  if (selected?.source_file_name) {
    return selected.source_file_name;
  }
  const job = state.jobs.find((item) => item.job_id === jobId);
  return job?.source_file_name || jobId;
}

function formatTaskTypeLabel(taskType) {
  switch (taskType) {
    case "ffmpeg":
      return "FFmpeg";
    case "stt":
      return "STT";
    case "summary":
      return "Summary";
    case "embedding":
      return "Embedding";
    default:
      return taskType || "task";
  }
}

function countQueuedEntries(queueStatus) {
  if (!queueStatus) {
    return 0;
  }

  const activeCount = Array.isArray(queueStatus.active_batch?.entries)
    ? queueStatus.active_batch.entries.length
    : 0;
  const pendingCount = Array.isArray(queueStatus.pending_batches)
    ? queueStatus.pending_batches.reduce((total, batch) => {
        return total + (Array.isArray(batch?.entries) ? batch.entries.length : 0);
      }, 0)
    : 0;
  return activeCount + pendingCount;
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
