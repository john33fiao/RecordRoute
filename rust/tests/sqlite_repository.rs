use anyhow::Result;
use record_route_api::config::AppConfig;
use record_route_api::models::{NewRecording, ProcessingStatus, ProcessingStep, StructuredSummary};
use record_route_api::storage::{LocalFileStore, RecordingRepository, SqliteRepository};
use tempfile::tempdir;
use uuid::Uuid;

#[tokio::test]
async fn config_defaults_db_path_to_storage_root() {
    let storage = tempdir().unwrap();
    std::env::set_var("APP_STORAGE_ROOT", storage.path());
    std::env::remove_var("APP_DB_PATH");
    std::env::set_var("WHISPER_BASE_URL", "http://127.0.0.1:8080");
    std::env::set_var("LLAMA_SUMMARY_BASE_URL", "http://127.0.0.1:8081");
    std::env::set_var("LLAMA_EMBED_BASE_URL", "http://127.0.0.1:8082");
    std::env::set_var("WHISPER_MODEL", "unused");
    std::env::set_var("SUMMARY_MODEL", "summary-model");
    std::env::set_var("EMBED_MODEL", "embed-model");

    let config = AppConfig::from_env().unwrap();
    assert_eq!(config.app_db_path, storage.path().join("record-route.db"));
}

#[tokio::test]
async fn sqlite_repository_persists_recordings_and_searches_via_artifacts() -> Result<()> {
    let temp = tempdir().unwrap();
    let storage_root = temp.path().join("storage");
    let db_path = storage_root.join("record-route.db");
    let file_store = LocalFileStore::new(storage_root.clone());
    file_store.ensure_root().await?;
    let repo = SqliteRepository::new(db_path, storage_root.clone()).await?;

    let recording_id = Uuid::new_v4();
    let (recording, job) = repo
        .insert_recording_with_job(NewRecording {
            id: recording_id,
            original_filename: "weekly-sync.wav".to_string(),
            original_content_type: Some("audio/wav".to_string()),
            file_size_bytes: 128,
            original_rel_path: format!("{recording_id}/original/weekly-sync.wav"),
        })
        .await?;

    assert_eq!(recording.status, ProcessingStatus::Queued);
    assert_eq!(job.step, ProcessingStep::UploadSaved);

    repo.save_transcription(recording_id, job.id, Some("ko"), "meeting transcript about launch plans")
        .await?;
    let summary = StructuredSummary {
        title: "Weekly sync".to_string(),
        abstract_text: "launch plan overview".to_string(),
        bullet_points: vec!["confirm scope".to_string(), "assign owner".to_string(), "review risk".to_string()],
    };
    repo.save_summary(recording_id, job.id, &summary, &summary.canonical_text())
        .await?;

    file_store
        .write_json_artifact(
            recording_id,
            "embedding.json",
            &serde_json::json!({
                "dimensions": 2,
                "embedding": [0.9, 0.1]
            }),
        )
        .await?;
    repo.save_embedding(recording_id, &[0.9, 0.1]).await?;
    repo.complete_job(recording_id, job.id).await?;

    let bundle = repo.get_recording_bundle(recording_id).await?.unwrap();
    assert_eq!(bundle.recording.current_step, ProcessingStep::Embedded);
    assert_eq!(bundle.job.unwrap().status, ProcessingStatus::Completed);

    let items = repo.search_recordings("launch", Some(&[0.9, 0.1]), 10).await?;
    assert_eq!(items.len(), 1);
    assert!(items[0].keyword_hit);
    assert!(items[0].similarity_score.is_some());

    Ok(())
}

#[tokio::test]
async fn sqlite_search_skips_broken_embedding_artifacts() -> Result<()> {
    let temp = tempdir().unwrap();
    let storage_root = temp.path().join("storage");
    let db_path = storage_root.join("record-route.db");
    let file_store = LocalFileStore::new(storage_root.clone());
    file_store.ensure_root().await?;
    let repo = SqliteRepository::new(db_path, storage_root.clone()).await?;

    let keyword_id = Uuid::new_v4();
    let (_, keyword_job) = repo
        .insert_recording_with_job(NewRecording {
            id: keyword_id,
            original_filename: "keyword-hit.wav".to_string(),
            original_content_type: Some("audio/wav".to_string()),
            file_size_bytes: 32,
            original_rel_path: format!("{keyword_id}/original/keyword-hit.wav"),
        })
        .await?;
    repo.save_transcription(keyword_id, keyword_job.id, Some("ko"), "launch transcript")
        .await?;

    let vector_id = Uuid::new_v4();
    let (_, vector_job) = repo
        .insert_recording_with_job(NewRecording {
            id: vector_id,
            original_filename: "vector-only.wav".to_string(),
            original_content_type: Some("audio/wav".to_string()),
            file_size_bytes: 32,
            original_rel_path: format!("{vector_id}/original/vector-only.wav"),
        })
        .await?;
    repo.save_summary(
        vector_id,
        vector_job.id,
        &StructuredSummary {
            title: "Vector only".to_string(),
            abstract_text: "no keyword hit".to_string(),
            bullet_points: vec!["a".to_string(), "b".to_string(), "c".to_string()],
        },
        "vector only content",
    )
    .await?;
    file_store
        .write_json_artifact(vector_id, "embedding.json", &serde_json::json!({ "dimensions": 3, "embedding": [1.0] }))
        .await?;
    repo.save_embedding(vector_id, &[1.0, 0.0, 0.0]).await?;

    let items = repo.search_recordings("launch", Some(&[0.9, 0.1]), 10).await?;
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].id, keyword_id);
    assert!(items[0].keyword_hit);
    assert!(items[0].similarity_score.is_none());

    Ok(())
}
