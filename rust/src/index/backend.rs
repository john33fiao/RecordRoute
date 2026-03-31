use super::types::{
    AudioArtifactRecord, DictionaryKeywordSource, DictionaryKeywords, IndexFile, JobRecord,
    JobResetSelection, SummaryEmbeddingRecord, SummaryEmbeddingVectorRecord, SummaryRecord,
    TranscriptRecord,
};
use serde::Serialize;
use serde::de::DeserializeOwned;

pub(crate) trait MetadataBackend {
    fn ensure_initialized(&self) -> Result<(), String>;
    fn read_index(&self) -> Result<IndexFile, String>;
    fn write_index(&self, index: &IndexFile) -> Result<(), String>;

    fn list_stt_dictionary_keywords(&self) -> Result<DictionaryKeywords, String>;
    fn upsert_stt_dictionary_keyword(
        &self,
        keyword: &str,
        source: DictionaryKeywordSource,
    ) -> Result<(), String>;
    fn delete_stt_dictionary_keyword(
        &self,
        keyword: &str,
        source: DictionaryKeywordSource,
    ) -> Result<bool, String>;
    fn delete_stt_dictionary_keywords_by_source(
        &self,
        source: DictionaryKeywordSource,
    ) -> Result<(), String>;
    fn promote_stt_dictionary_keyword(&self, keyword: &str) -> Result<bool, String>;

    fn list_audio_artifacts(&self, job_id: &str) -> Result<Vec<AudioArtifactRecord>, String>;
    fn upsert_audio_artifact(&self, record: &AudioArtifactRecord) -> Result<(), String>;

    fn list_transcripts(&self, job_id: &str) -> Result<Vec<TranscriptRecord>, String>;
    fn get_transcript(
        &self,
        job_id: &str,
        transcript_id: &str,
    ) -> Result<Option<TranscriptRecord>, String>;
    fn upsert_transcript(&self, record: &TranscriptRecord) -> Result<(), String>;
    fn delete_transcript(&self, job_id: &str, transcript_id: &str) -> Result<bool, String>;

    fn get_summary(&self, job_id: &str) -> Result<Option<SummaryRecord>, String>;
    #[cfg(test)]
    fn upsert_summary(&self, record: &SummaryRecord) -> Result<(), String>;
    fn write_index_with_summary_and_keywords(
        &self,
        index: &IndexFile,
        record: &SummaryRecord,
        auto_keywords: &[String],
    ) -> Result<(), String>;

    fn get_summary_embedding(
        &self,
        job_id: &str,
    ) -> Result<Option<SummaryEmbeddingVectorRecord>, String>;
    fn write_index_with_summary_embedding(
        &self,
        index: &IndexFile,
        job_id: &str,
        record: &SummaryEmbeddingVectorRecord,
    ) -> Result<(), String>;
    fn write_index_with_job_reset(
        &self,
        index: &IndexFile,
        job_id: &str,
        selection: JobResetSelection,
    ) -> Result<(), String>;
}

pub(crate) struct SerializedJobRecord {
    pub job_id: String,
    pub status_json: String,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub source_ref: String,
    pub source_kind_json: String,
    pub source_content_sha256: String,
    pub source_file_name: String,
    pub probe_json: String,
    pub split_strategy_json: String,
    pub outputs_json: String,
    pub error_message: Option<String>,
    pub summary_embedding_json: Option<String>,
}

pub(crate) const LEGACY_DICTIONARY_AUTO_DEMO_CLEANUP_FLAG: &str =
    "legacy_dictionary_auto_demo_cleanup_v1";
pub(crate) const LEGACY_DICTIONARY_AUTO_DEMO_KEYWORDS: [&str; 3] = ["회의록", "배포", "액션아이템"];

pub(crate) fn to_json<T: Serialize>(value: &T) -> Result<String, String> {
    serde_json::to_string(value).map_err(|error| format!("failed to serialize metadata: {error}"))
}

pub(crate) fn from_json<T: DeserializeOwned>(raw: &str) -> Result<T, String> {
    serde_json::from_str(raw).map_err(|error| format!("failed to parse metadata json: {error}"))
}

pub(crate) fn serialize_job_record(job: &JobRecord) -> Result<SerializedJobRecord, String> {
    Ok(SerializedJobRecord {
        job_id: job.job_id.clone(),
        status_json: to_json(&job.status)?,
        started_at: job.started_at.clone(),
        finished_at: job.finished_at.clone(),
        source_ref: job.source_ref.clone(),
        source_kind_json: to_json(&job.source_kind)?,
        source_content_sha256: job.source_content_sha256.clone(),
        source_file_name: job.source_file_name.clone(),
        probe_json: to_json(&job.probe)?,
        split_strategy_json: to_json(&job.split_strategy)?,
        outputs_json: to_json(&job.outputs)?,
        error_message: job.error_message.clone(),
        summary_embedding_json: job.summary_embedding.as_ref().map(to_json).transpose()?,
    })
}

pub(crate) fn deserialize_job_record(raw: SerializedJobRecord) -> Result<JobRecord, String> {
    Ok(JobRecord {
        job_id: raw.job_id,
        status: from_json(&raw.status_json)?,
        started_at: raw.started_at,
        finished_at: raw.finished_at,
        source_ref: raw.source_ref,
        source_kind: from_json(&raw.source_kind_json)?,
        source_content_sha256: raw.source_content_sha256,
        source_file_name: raw.source_file_name,
        probe: from_json(&raw.probe_json)?,
        split_strategy: from_json(&raw.split_strategy_json)?,
        outputs: from_json(&raw.outputs_json)?,
        error_message: raw.error_message,
        summary_embedding: raw
            .summary_embedding_json
            .map(|json| from_json::<SummaryEmbeddingRecord>(&json))
            .transpose()?,
        tasks: Vec::new(),
    })
}
