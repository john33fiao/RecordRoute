use std::fs;
use std::path::{Path, PathBuf};

const UTF8_BOM: &[u8; 3] = b"\xEF\xBB\xBF";
const UTF16_LE_BOM: &[u8; 2] = b"\xFF\xFE";
const UTF16_BE_BOM: &[u8; 2] = b"\xFE\xFF";

#[derive(Debug, Clone, PartialEq, Eq)]
struct DecodedTranscript {
    text: String,
    repaired: bool,
}

pub(crate) fn postprocess_transcript(output_text: &Path) -> Result<(), String> {
    let original = read_transcript_text_details(output_text).map_err(|error| {
        format!(
            "failed to read transcript for post-processing {}: {error}",
            output_text.display()
        )
    })?;
    let deduplicated = deduplicate_consecutive_lines(&original.text);
    let needs_write = original.repaired || deduplicated != original.text;
    if !needs_write {
        return Ok(());
    }
    if original.repaired {
        eprintln!(
            "normalized transcript {} after repairing non-UTF-8 content",
            output_text.display()
        );
    }

    let normalized = if deduplicated == original.text {
        original.text
    } else {
        deduplicated
    };
    fs::write(output_text, normalized).map_err(|error| {
        format!(
            "failed to write post-processed transcript {}: {error}",
            output_text.display()
        )
    })
}

pub(crate) fn read_transcript_text(path: &Path) -> Result<String, String> {
    Ok(read_transcript_text_details(path)?.text)
}

fn read_transcript_text_details(path: &Path) -> Result<DecodedTranscript, String> {
    let bytes = fs::read(path).map_err(|error| error.to_string())?;
    decode_transcript_bytes(&bytes)
}

fn decode_transcript_bytes(bytes: &[u8]) -> Result<DecodedTranscript, String> {
    if let Some(stripped) = bytes.strip_prefix(UTF8_BOM) {
        return Ok(DecodedTranscript {
            text: decode_utf8_bytes(stripped),
            repaired: true,
        });
    }
    if let Some(stripped) = bytes.strip_prefix(UTF16_LE_BOM) {
        return decode_utf16_bytes(stripped, true);
    }
    if let Some(stripped) = bytes.strip_prefix(UTF16_BE_BOM) {
        return decode_utf16_bytes(stripped, false);
    }

    match String::from_utf8(bytes.to_vec()) {
        Ok(text) => Ok(DecodedTranscript {
            text,
            repaired: false,
        }),
        Err(_) => Ok(DecodedTranscript {
            text: String::from_utf8_lossy(bytes).into_owned(),
            repaired: true,
        }),
    }
}

fn decode_utf8_bytes(bytes: &[u8]) -> String {
    match String::from_utf8(bytes.to_vec()) {
        Ok(text) => text,
        Err(_) => String::from_utf8_lossy(bytes).into_owned(),
    }
}

fn decode_utf16_bytes(bytes: &[u8], little_endian: bool) -> Result<DecodedTranscript, String> {
    if bytes.len() % 2 != 0 {
        return Err(format!(
            "utf-16 transcript had an odd number of bytes after BOM: {}",
            bytes.len()
        ));
    }

    let code_units = bytes
        .chunks_exact(2)
        .map(|chunk| {
            if little_endian {
                u16::from_le_bytes([chunk[0], chunk[1]])
            } else {
                u16::from_be_bytes([chunk[0], chunk[1]])
            }
        })
        .collect::<Vec<_>>();
    Ok(DecodedTranscript {
        text: String::from_utf16_lossy(&code_units),
        repaired: true,
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn read_transcript_text_reads_valid_utf8_without_repair() {
        let path = temp_path("utf8");
        fs::write(&path, "안녕하세요").expect("write transcript");

        let decoded = read_transcript_text_details(&path).expect("decode transcript");

        assert_eq!(
            decoded,
            DecodedTranscript {
                text: "안녕하세요".to_string(),
                repaired: false,
            }
        );
    }

    #[test]
    fn read_transcript_text_strips_utf8_bom() {
        let path = temp_path("utf8-bom");
        fs::write(&path, [UTF8_BOM.as_slice(), "hello".as_bytes()].concat())
            .expect("write transcript");

        let decoded = read_transcript_text_details(&path).expect("decode transcript");

        assert_eq!(
            decoded,
            DecodedTranscript {
                text: "hello".to_string(),
                repaired: true,
            }
        );
    }

    #[test]
    fn read_transcript_text_decodes_utf16_with_bom() {
        let le_path = temp_path("utf16-le");
        let be_path = temp_path("utf16-be");
        fs::write(
            &le_path,
            [UTF16_LE_BOM.as_slice(), &[0x41, 0x00, 0x42, 0x00]].concat(),
        )
        .expect("write le transcript");
        fs::write(
            &be_path,
            [UTF16_BE_BOM.as_slice(), &[0x00, 0x43, 0x00, 0x44]].concat(),
        )
        .expect("write be transcript");

        let decoded_le = read_transcript_text_details(&le_path).expect("decode le transcript");
        let decoded_be = read_transcript_text_details(&be_path).expect("decode be transcript");

        assert_eq!(decoded_le.text, "AB");
        assert!(decoded_le.repaired);
        assert_eq!(decoded_be.text, "CD");
        assert!(decoded_be.repaired);
    }

    #[test]
    fn read_transcript_text_lossily_repairs_invalid_utf8() {
        let path = temp_path("lossy");
        fs::write(&path, b"hello \xFF world").expect("write transcript");

        let decoded = read_transcript_text_details(&path).expect("decode transcript");

        assert_eq!(decoded.text, "hello \u{FFFD} world");
        assert!(decoded.repaired);
    }

    #[test]
    fn read_transcript_text_rejects_odd_utf16_payload() {
        let path = temp_path("odd-utf16");
        fs::write(&path, [UTF16_LE_BOM.as_slice(), &[0x41]].concat()).expect("write transcript");

        let error = read_transcript_text_details(&path).expect_err("odd utf16 should fail");

        assert!(error.contains("odd number of bytes"));
    }

    #[test]
    fn postprocess_transcript_rewrites_repaired_file_as_utf8() {
        let path = temp_path("postprocess");
        fs::write(&path, [UTF8_BOM.as_slice(), b"repeat\nrepeat\n"].concat())
            .expect("write transcript");

        postprocess_transcript(&path).expect("postprocess transcript");

        let bytes = fs::read(&path).expect("read normalized transcript");
        assert!(!bytes.starts_with(UTF8_BOM));
        assert_eq!(
            String::from_utf8(bytes).expect("utf8 transcript"),
            "repeat\n"
        );
    }

    fn temp_path(label: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("recordroute-whisper-output-{label}-{unique}"));
        fs::create_dir_all(&dir).expect("temp dir");
        dir.join("sample.txt")
    }
}
