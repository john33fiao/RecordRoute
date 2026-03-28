use crate::tool_runtime::command_output_details;
use std::process::Output;

#[derive(Debug, serde::Deserialize)]
struct EmbeddingJsonEnvelope {
    data: Vec<EmbeddingJsonItem>,
}

#[derive(Debug, serde::Deserialize)]
struct EmbeddingJsonItem {
    embedding: Vec<f32>,
}

pub(crate) fn parse_embedding_output(output: &Output) -> Result<Vec<f32>, String> {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let trimmed = stdout.trim();
    if trimmed.is_empty() {
        return Err(format!(
            "llama embedding completed without producing embedding output: {}",
            command_output_details(output)
        ));
    }

    if let Ok(vector) = serde_json::from_str::<Vec<f32>>(trimmed) {
        return Ok(vector);
    }

    if let Ok(vectors) = serde_json::from_str::<Vec<Vec<f32>>>(trimmed) {
        return vectors
            .into_iter()
            .next()
            .ok_or_else(|| "embedding output did not contain any vectors".to_string());
    }

    if let Ok(envelope) = serde_json::from_str::<EmbeddingJsonEnvelope>(trimmed) {
        return envelope
            .data
            .into_iter()
            .next()
            .map(|item| item.embedding)
            .ok_or_else(|| "embedding output did not contain any vectors".to_string());
    }

    Err("failed to parse embedding output: expected JSON embedding data".to_string())
}

pub(crate) fn extract_summary_text(stdout: &str, prompt: &str) -> String {
    let mut text = stdout;

    if !prompt.is_empty() {
        if let Some(index) = text.find(prompt) {
            text = &text[index + prompt.len()..];
        } else if let Some(index) = text.find("(truncated)") {
            text = &text[index + "(truncated)".len()..];
        }
    }

    if let Some(index) = find_summary_start(text) {
        text = &text[index..];
    }

    for marker in ["\n\n[ Prompt:", "\n[ Prompt:", "\nExiting..."] {
        if let Some(index) = text.find(marker) {
            text = &text[..index];
            break;
        }
    }

    normalize_summary_text(text)
}

fn find_summary_start(text: &str) -> Option<usize> {
    let mut earliest = None;

    for marker in [
        "## 회의록",
        "# 회의록",
        "회의록",
        "## 개요",
        "# 개요",
        "**개요**",
        "개요",
        "## 핵심 논의",
        "# 핵심 논의",
        "**핵심 논의**",
        "핵심 논의",
    ] {
        if text.starts_with(marker) {
            earliest = Some(earliest.unwrap_or(0).min(0));
        }

        let line_marker = format!("\n{marker}");
        if let Some(index) = text.find(&line_marker) {
            let candidate = index + 1;
            earliest = Some(earliest.map_or(candidate, |current| current.min(candidate)));
        }
    }

    earliest
}

fn normalize_summary_text(text: &str) -> String {
    let mut normalized_lines = Vec::new();

    for raw_line in text.trim().lines() {
        let trimmed = raw_line.trim();
        if is_summary_title(trimmed) {
            continue;
        }
        if let Some(heading) = canonical_summary_heading(trimmed) {
            normalized_lines.push(format!("## {heading}"));
            continue;
        }
        normalized_lines.push(raw_line.trim_end().to_string());
    }

    normalized_lines.join("\n").trim().to_string()
}

fn canonical_summary_heading(line: &str) -> Option<&'static str> {
    match normalized_heading_text(line) {
        "개요" => Some("개요"),
        "핵심 논의" => Some("핵심 논의"),
        "결정/합의" => Some("결정/합의"),
        "후속 조치" => Some("후속 조치"),
        _ => None,
    }
}

fn is_summary_title(line: &str) -> bool {
    normalized_heading_text(line) == "회의록"
}

fn normalized_heading_text(line: &str) -> &str {
    let trimmed = line.trim();
    let without_hashes = trimmed.trim_start_matches('#').trim();
    without_hashes
        .strip_prefix("**")
        .and_then(|inner| inner.strip_suffix("**"))
        .unwrap_or(without_hashes)
        .trim()
}
