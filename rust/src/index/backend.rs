use super::types::{
    AudioArtifactRecord, IndexFile, SummaryEmbeddingVectorRecord, SummaryRecord, TranscriptRecord,
};
use serde::Serialize;
use serde::de::DeserializeOwned;

pub(crate) trait MetadataBackend {
    fn ensure_initialized(&self) -> Result<(), String>;
    fn read_index(&self) -> Result<IndexFile, String>;
    fn write_index(&self, index: &IndexFile) -> Result<(), String>;

    fn list_audio_artifacts(&self, job_id: &str) -> Result<Vec<AudioArtifactRecord>, String>;
    fn upsert_audio_artifact(&self, record: &AudioArtifactRecord) -> Result<(), String>;

    fn list_transcripts(&self, job_id: &str) -> Result<Vec<TranscriptRecord>, String>;
    fn get_transcript(
        &self,
        job_id: &str,
        transcript_id: &str,
    ) -> Result<Option<TranscriptRecord>, String>;
    fn upsert_transcript(&self, record: &TranscriptRecord) -> Result<(), String>;
    fn count_transcripts(&self, job_id: &str) -> Result<usize, String>;

    fn get_summary(&self, job_id: &str) -> Result<Option<SummaryRecord>, String>;
    fn upsert_summary(&self, record: &SummaryRecord) -> Result<(), String>;

    fn get_summary_embedding(
        &self,
        job_id: &str,
    ) -> Result<Option<SummaryEmbeddingVectorRecord>, String>;
    fn upsert_summary_embedding(
        &self,
        job_id: &str,
        record: &SummaryEmbeddingVectorRecord,
    ) -> Result<(), String>;
}

pub(crate) fn to_json<T: Serialize>(value: &T) -> Result<String, String> {
    serde_json::to_string(value).map_err(|error| format!("failed to serialize metadata: {error}"))
}

pub(crate) fn from_json<T: DeserializeOwned>(raw: &str) -> Result<T, String> {
    serde_json::from_str(raw).map_err(|error| format!("failed to parse metadata json: {error}"))
}
