use super::super::embedding_stage::execute_summary_embedding_job;
use super::support::*;
use crate::index::{IndexStore, TaskStatus, TaskType};
use crate::test_support::{env_lock, mark_job_completed_with_audio, seed_summary, test_job};
use std::fs;

#[test]
fn execute_summary_embedding_job_commits_job_task_and_vector_record() {
    let _guard = env_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    unsafe {
        std::env::remove_var("RECORDROUTE_LLAMA_MODEL");
        std::env::remove_var("RECORDROUTE_LLAMA_EMBEDDING_MODEL");
    }

    let repo_root = temp_workspace();
    let store = IndexStore::new(&repo_root);
    let llama_bin = repo_root
        .join(".build/llama")
        .join(crate::ffmpeg::target_dir_name())
        .join("bin");
    let embedding_log = repo_root.join("llama-embedding.log");
    fs::create_dir_all(repo_root.join("scripts")).expect("scripts dir");
    fs::create_dir_all(&llama_bin).expect("llama bin");

    write_build_script(&build_script_path(&repo_root, "llama"));
    write_fake_llama_cli(
        &fake_command_path(&llama_bin, "llama-cli"),
        &repo_root.join("llama.log"),
        &repo_root.join("unused-prompt.txt"),
        false,
    );
    write_fake_llama_embedding(
        &fake_command_path(&llama_bin, "llama-embedding"),
        &embedding_log,
    );

    let mut job = test_job(
        "job-1",
        "2026-01-01T00:00:00Z",
        "sources/job-1/source.wav",
        "hash-job-1",
        "input.wav",
    );
    mark_job_completed_with_audio(&store, &mut job, "2026-01-01T00:00:01Z", &["mono_mix.wav"])
        .expect("mark completed");
    job.enqueue_task(TaskType::Embedding, "2026-01-01T00:00:02Z".to_string());
    store.insert_job(job).expect("insert job");
    seed_summary(&store, "job-1", "회의 요약 본문").expect("seed summary");

    let updated = execute_summary_embedding_job(&repo_root, "job-1").expect("execute embedding");

    let task = updated.task(TaskType::Embedding).expect("embedding task");
    assert_eq!(task.status, TaskStatus::Completed);
    assert!(updated.summary_embedding.is_some());

    let persisted = store
        .get_summary_embedding("job-1")
        .expect("read embedding")
        .expect("embedding row");
    assert_eq!(persisted.metadata, updated.summary_embedding.unwrap());
    assert_eq!(persisted.vector, vec![0.25, 0.75]);

    let embedding_log = fs::read_to_string(embedding_log).expect("embedding log");
    assert!(embedding_log.contains("-hf"));
    assert!(embedding_log.contains("Qwen/Qwen3-Embedding-4B-GGUF"));
}
