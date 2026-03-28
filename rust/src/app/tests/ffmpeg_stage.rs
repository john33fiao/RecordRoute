use super::super::*;
use super::support::*;
use crate::ffmpeg::Toolchain as FfmpegToolchain;
use crate::index::IndexStore;
use crate::test_support::test_job;
use std::fs;
#[test]
fn end_to_end_flow_uses_fake_toolchain() {
    let repo_root = temp_workspace();
    let scripts_dir = repo_root.join("scripts");
    let build_bin = repo_root
        .join(".build/ffmpeg")
        .join(crate::ffmpeg::target_dir_name())
        .join("install/bin");
    let input = repo_root.join("fixture.wav");
    let ffmpeg_log = repo_root.join("ffmpeg-args.log");
    fs::create_dir_all(&scripts_dir).expect("scripts dir");
    fs::create_dir_all(&build_bin).expect("toolchain dir");
    write_build_script(&build_script_path(&repo_root, "ffmpeg"));
    write_fake_ffprobe(&fake_command_path(&build_bin, "ffprobe"), 2, Some("stereo"));
    write_fake_ffmpeg(&fake_command_path(&build_bin, "ffmpeg"), &ffmpeg_log);
    write_test_wav(&input, 2);

    let summary = run_with_repo_root(&repo_root, &input).expect("run should succeed");

    assert!(summary.outputs.merged_mono_wav.exists());
    assert_eq!(summary.outputs.split_mono_wavs.len(), 2);
    assert!(summary.outputs.split_mono_wavs[0].path.exists());
    assert!(summary.outputs.split_mono_wavs[1].path.exists());

    let log = fs::read_to_string(ffmpeg_log).expect("ffmpeg log");
    assert!(log.contains("-filter_complex"));
    assert!(log.contains(&crate::ffmpeg::build_filter_complex(2)));

    let store = IndexStore::new(&repo_root);
    let index = store.read_index().expect("read index");
    assert_eq!(index.version, 4);
    assert_eq!(index.jobs.len(), 1);
    assert_eq!(index.jobs[0].status, crate::index::JobStatus::Completed);
    assert_eq!(index.jobs[0].probe.channels, Some(2));
    assert_eq!(index.jobs[0].outputs.split_mono_wavs.len(), 2);
}

#[test]
fn failure_marks_job_failed_and_cleans_partial_outputs() {
    let repo_root = temp_workspace();
    let scripts_dir = repo_root.join("scripts");
    let build_bin = repo_root
        .join(".build/ffmpeg")
        .join(crate::ffmpeg::target_dir_name())
        .join("install/bin");
    let input = repo_root.join("fixture.wav");
    fs::create_dir_all(&scripts_dir).expect("scripts dir");
    fs::create_dir_all(&build_bin).expect("toolchain dir");
    write_build_script(&build_script_path(&repo_root, "ffmpeg"));
    write_fake_ffprobe(&fake_command_path(&build_bin, "ffprobe"), 2, Some("stereo"));
    write_failing_ffmpeg(&fake_command_path(&build_bin, "ffmpeg"));
    write_test_wav(&input, 2);

    let error = run_with_repo_root(&repo_root, &input).expect_err("run should fail");

    assert!(error.contains("ffmpeg conversion failed"));

    let store = IndexStore::new(&repo_root);
    let job = store
        .list_jobs()
        .expect("list jobs")
        .into_iter()
        .next()
        .expect("job");
    assert_eq!(job.status, crate::index::JobStatus::Failed);
    assert!(
        job.error_message
            .as_deref()
            .expect("error")
            .contains("ffmpeg conversion failed")
    );

    let job_dir = store.job_dir(&job.job_id);
    assert!(job_dir.exists());
    assert!(!job_dir.join("channel_01.wav").exists());
    assert!(!job_dir.join("channel_02.wav").exists());
    assert!(!job_dir.join("mono_mix.wav").exists());
}

#[test]
fn reuses_completed_outputs_for_same_input() {
    let repo_root = temp_workspace();
    let scripts_dir = repo_root.join("scripts");
    let build_bin = repo_root
        .join(".build/ffmpeg")
        .join(crate::ffmpeg::target_dir_name())
        .join("install/bin");
    let input = repo_root.join("fixture.wav");
    let ffmpeg_log = repo_root.join("ffmpeg-args.log");
    let ffmpeg_count = repo_root.join("ffmpeg-count.txt");
    fs::create_dir_all(&scripts_dir).expect("scripts dir");
    fs::create_dir_all(&build_bin).expect("toolchain dir");
    write_build_script(&build_script_path(&repo_root, "ffmpeg"));
    write_fake_ffprobe(&fake_command_path(&build_bin, "ffprobe"), 2, Some("stereo"));
    write_counting_ffmpeg(
        &fake_command_path(&build_bin, "ffmpeg"),
        &ffmpeg_log,
        &ffmpeg_count,
    );
    write_test_wav(&input, 2);

    let first = run_with_repo_root(&repo_root, &input).expect("first run should succeed");

    fs::remove_file(fake_command_path(&build_bin, "ffmpeg")).expect("remove ffmpeg");
    fs::remove_file(fake_command_path(&build_bin, "ffprobe")).expect("remove ffprobe");

    let second = run_with_repo_root(&repo_root, &input).expect("second run should reuse");

    assert_eq!(first.job_id, second.job_id);
    assert_eq!(first.job_dir, second.job_dir);
    assert_eq!(
        first.outputs.merged_mono_wav,
        second.outputs.merged_mono_wav
    );
    assert_eq!(
        first.outputs.split_mono_wavs,
        second.outputs.split_mono_wavs
    );
    assert_eq!(read_run_count(&ffmpeg_count), 1);

    let store = IndexStore::new(&repo_root);
    assert_eq!(store.list_jobs().expect("list jobs").len(), 1);
}

#[test]
fn missing_reusable_output_triggers_new_conversion() {
    let repo_root = temp_workspace();
    let scripts_dir = repo_root.join("scripts");
    let build_bin = repo_root
        .join(".build/ffmpeg")
        .join(crate::ffmpeg::target_dir_name())
        .join("install/bin");
    let input = repo_root.join("fixture.wav");
    let ffmpeg_log = repo_root.join("ffmpeg-args.log");
    let ffmpeg_count = repo_root.join("ffmpeg-count.txt");
    fs::create_dir_all(&scripts_dir).expect("scripts dir");
    fs::create_dir_all(&build_bin).expect("toolchain dir");
    write_build_script(&build_script_path(&repo_root, "ffmpeg"));
    write_fake_ffprobe(&fake_command_path(&build_bin, "ffprobe"), 2, Some("stereo"));
    write_counting_ffmpeg(
        &fake_command_path(&build_bin, "ffmpeg"),
        &ffmpeg_log,
        &ffmpeg_count,
    );
    write_test_wav(&input, 2);

    let first = run_with_repo_root(&repo_root, &input).expect("first run should succeed");
    fs::remove_file(&first.outputs.split_mono_wavs[0].path).expect("remove split output");

    let second = run_with_repo_root(&repo_root, &input).expect("second run should create new job");

    assert_ne!(first.job_id, second.job_id);
    assert_eq!(read_run_count(&ffmpeg_count), 2);
    assert!(second.outputs.merged_mono_wav.exists());
    assert!(second.outputs.split_mono_wavs[0].path.exists());
    assert!(second.outputs.split_mono_wavs[1].path.exists());

    let store = IndexStore::new(&repo_root);
    assert_eq!(store.list_jobs().expect("list jobs").len(), 2);
}

#[test]
fn reuses_previous_completed_job_when_latest_job_failed() {
    let repo_root = temp_workspace();
    let scripts_dir = repo_root.join("scripts");
    let build_bin = repo_root
        .join(".build/ffmpeg")
        .join(crate::ffmpeg::target_dir_name())
        .join("install/bin");
    let input = repo_root.join("fixture.wav");
    let ffmpeg_log = repo_root.join("ffmpeg-args.log");
    let ffmpeg_count = repo_root.join("ffmpeg-count.txt");
    fs::create_dir_all(&scripts_dir).expect("scripts dir");
    fs::create_dir_all(&build_bin).expect("toolchain dir");
    write_build_script(&build_script_path(&repo_root, "ffmpeg"));
    write_fake_ffprobe(&fake_command_path(&build_bin, "ffprobe"), 2, Some("stereo"));
    write_counting_ffmpeg(
        &fake_command_path(&build_bin, "ffmpeg"),
        &ffmpeg_log,
        &ffmpeg_count,
    );
    write_test_wav(&input, 2);

    let first = run_with_repo_root(&repo_root, &input).expect("first run should succeed");

    let store = IndexStore::new(&repo_root);
    let first_job = store
        .find_job(&first.job_id)
        .expect("lookup first job")
        .expect("first job");
    let mut failed_job = test_job(
        "job-failed",
        "2026-01-01T00:00:02Z",
        &first_job.source_ref,
        &first_job.source_content_sha256,
        &first_job.source_file_name,
    );
    failed_job
        .mark_failed(
            "2026-01-01T00:00:03Z".to_string(),
            "synthetic failure".to_string(),
        )
        .expect("mark failed job");
    store.insert_job(failed_job).expect("insert failed job");

    fs::remove_file(fake_command_path(&build_bin, "ffmpeg")).expect("remove ffmpeg");
    fs::remove_file(fake_command_path(&build_bin, "ffprobe")).expect("remove ffprobe");

    let second = run_with_repo_root(&repo_root, &input).expect("run should reuse old job");

    assert_eq!(second.job_id, first.job_id);
    assert_eq!(second.job_dir, first.job_dir);
    assert_eq!(read_run_count(&ffmpeg_count), 1);

    let jobs = store.list_jobs().expect("list jobs");
    assert_eq!(jobs.len(), 2);
    assert!(jobs.iter().any(|job| {
        job.job_id == "job-failed" && job.status == crate::index::JobStatus::Failed
    }));
}

#[test]
fn missing_toolchain_reports_bootstrap_path() {
    let repo_root = temp_workspace();
    let input = repo_root.join("fixture.wav");
    fs::create_dir_all(repo_root.join("scripts")).expect("scripts dir");
    write_build_script(&build_script_path(&repo_root, "ffmpeg"));
    write_test_wav(&input, 1);

    let error = run_with_repo_root(&repo_root, &input).expect_err("toolchain should be required");

    assert!(
        error.contains(
            build_script_path(&repo_root, "ffmpeg")
                .to_string_lossy()
                .as_ref()
        )
    );
}

#[test]
fn toolchain_discovers_test_target() {
    let repo_root = temp_workspace();
    let scripts_dir = repo_root.join("scripts");
    let build_bin = repo_root
        .join(".build/ffmpeg")
        .join(crate::ffmpeg::target_dir_name())
        .join("install/bin");
    fs::create_dir_all(&scripts_dir).expect("scripts dir");
    fs::create_dir_all(&build_bin).expect("toolchain dir");
    write_build_script(&build_script_path(&repo_root, "ffmpeg"));
    write_fake_ffprobe(&fake_command_path(&build_bin, "ffprobe"), 1, Some("mono"));
    write_fake_ffmpeg(
        &fake_command_path(&build_bin, "ffmpeg"),
        &repo_root.join("ffmpeg.log"),
    );

    let toolchain = FfmpegToolchain::discover(&repo_root).expect("toolchain");

    assert_eq!(
        toolchain.ffmpeg_path,
        fake_command_path(&build_bin, "ffmpeg")
    );
    assert_eq!(
        toolchain.ffprobe_path,
        fake_command_path(&build_bin, "ffprobe")
    );
}
