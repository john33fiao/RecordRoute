use std::fs;
use std::path::{Path, PathBuf};

pub(crate) fn postprocess_transcript(output_text: &Path) -> Result<(), String> {
    let original = fs::read_to_string(output_text).map_err(|error| {
        format!(
            "failed to read transcript for post-processing {}: {error}",
            output_text.display()
        )
    })?;
    let deduplicated = deduplicate_consecutive_lines(&original);
    if deduplicated == original {
        return Ok(());
    }

    fs::write(output_text, deduplicated).map_err(|error| {
        format!(
            "failed to write post-processed transcript {}: {error}",
            output_text.display()
        )
    })
}

fn deduplicate_consecutive_lines(content: &str) -> String {
    let mut deduplicated = String::with_capacity(content.len());
    let mut previous_line: Option<String> = None;

    for segment in content.split_inclusive('\n') {
        let line = segment.strip_suffix('\n').unwrap_or(segment);
        let normalized = line.trim_end_matches('\r').trim();
        let is_duplicate = !normalized.is_empty() && previous_line.as_deref() == Some(normalized);
        if is_duplicate {
            continue;
        }

        deduplicated.push_str(segment);
        previous_line = if normalized.is_empty() {
            None
        } else {
            Some(normalized.to_string())
        };
    }

    deduplicated
}

pub(crate) fn transcript_output_prefix(output_text: &Path) -> Result<PathBuf, String> {
    let parent = output_text.parent().ok_or_else(|| {
        format!(
            "transcript output path has no parent directory: {}",
            output_text.display()
        )
    })?;
    let stem = output_text.file_stem().ok_or_else(|| {
        format!(
            "transcript output path has no file stem: {}",
            output_text.display()
        )
    })?;

    Ok(parent.join(stem))
}
