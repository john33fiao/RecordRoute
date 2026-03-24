#[tokio::test]
#[ignore = "requires manually running whisper-server and llama-server sidecars"]
async fn real_sidecar_smoke_test_placeholder() {
    if std::env::var("RUN_REAL_SIDECAR_SMOKE").is_err() {
        return;
    }

    assert!(
        std::env::var("WHISPER_BASE_URL").is_ok()
            && std::env::var("LLAMA_SUMMARY_BASE_URL").is_ok()
            && std::env::var("LLAMA_EMBED_BASE_URL").is_ok(),
        "set sidecar base URL env vars before enabling the real smoke test"
    );
}
