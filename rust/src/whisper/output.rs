use std::fs;
use std::path::{Path, PathBuf};

const UTF8_BOM: &[u8] = &[0xEF, 0xBB, 0xBF];
const UTF16_LE_BOM: &[u8] = &[0xFF, 0xFE];
const UTF16_BE_BOM: &[u8] = &[0xFE, 0xFF];

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TranscriptText {
    pub(crate) text: String,
    pub(crate) repaired: bool,
}

pub(crate) fn read_transcript(output_text: &Path) -> Result<TranscriptText, String> {
    let bytes = fs::read(output_text).map_err(|error| {
        format!(
            "failed to read transcript {}: {error}",
            output_text.display()
        )
    })?;
    Ok(decode_transcript_bytes(&bytes))
}

pub(crate) fn postprocess_transcript(output_text: &Path) -> Result<(), String> {
    let TranscriptText { text, repaired } = read_transcript(output_text)?;
    let deduplicated = deduplicate_consecutive_lines(&text);
    if !repaired && deduplicated == text {
        return Ok(());
    }

    if repaired {
        eprintln!(
            "warning: repaired transcript encoding and normalized to UTF-8: {}",
            output_text.display()
        );
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

fn decode_transcript_bytes(bytes: &[u8]) -> TranscriptText {
    if let Some(stripped) = bytes.strip_prefix(UTF8_BOM) {
        return decode_utf8_bytes(stripped, true);
    }
    if let Some(stripped) = bytes.strip_prefix(UTF16_LE_BOM) {
        return TranscriptText {
            text: decode_utf16_bytes(stripped, Utf16Endian::Little),
            repaired: true,
        };
    }
    if let Some(stripped) = bytes.strip_prefix(UTF16_BE_BOM) {
        return TranscriptText {
            text: decode_utf16_bytes(stripped, Utf16Endian::Big),
            repaired: true,
        };
    }

    decode_utf8_bytes(bytes, false)
}

fn decode_utf8_bytes(bytes: &[u8], repaired: bool) -> TranscriptText {
    match String::from_utf8(bytes.to_vec()) {
        Ok(text) => TranscriptText { text, repaired },
        Err(_) => TranscriptText {
            text: String::from_utf8_lossy(bytes).into_owned(),
            repaired: true,
        },
    }
}

fn decode_utf16_bytes(bytes: &[u8], endian: Utf16Endian) -> String {
    let mut units = Vec::with_capacity(bytes.len() / 2);
    let mut pairs = bytes.chunks_exact(2);
    for pair in &mut pairs {
        let unit = match endian {
            Utf16Endian::Little => u16::from_le_bytes([pair[0], pair[1]]),
            Utf16Endian::Big => u16::from_be_bytes([pair[0], pair[1]]),
        };
        units.push(unit);
    }

    let mut decoded = String::from_utf16_lossy(&units);
    if !pairs.remainder().is_empty() {
        decoded.push(char::REPLACEMENT_CHARACTER);
    }
    decoded
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Utf16Endian {
    Little,
    Big,
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
