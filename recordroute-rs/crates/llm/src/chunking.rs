/// Text chunk
#[derive(Debug, Clone)]
pub struct TextChunk {
    /// Chunk text
    pub text: String,

    /// Start index in original text
    pub start: usize,

    /// End index in original text
    pub end: usize,
}

/// Split text into chunks by token count (approximate)
pub fn chunk_text(text: &str, max_tokens: usize, overlap: usize) -> Vec<TextChunk> {
    // Approximate: 1 token ≈ 4 characters (for Korean/English mix)
    let chars_per_token = 4;
    let max_chars = max_tokens * chars_per_token;
    let overlap_chars = overlap * chars_per_token;

    let mut chunks = Vec::new();
    let text_len = text.len();

    if text_len <= max_chars {
        // Text is short enough, return as single chunk
        return vec![TextChunk {
            text: text.to_string(),
            start: 0,
            end: text_len,
        }];
    }

    let mut start = 0;

    while start < text_len {
        let end = (start + max_chars).min(text_len);

        // Try to find a good breaking point (sentence boundary)
        let actual_end = if end < text_len {
            find_break_point(text, start, end)
        } else {
            end
        };

        let chunk_text = text[start..actual_end].to_string();
        chunks.push(TextChunk {
            text: chunk_text,
            start,
            end: actual_end,
        });

        // Move to next chunk with overlap
        start = if actual_end >= overlap_chars {
            actual_end - overlap_chars
        } else {
            actual_end
        };

        // Prevent infinite loop
        if start >= text_len {
            break;
        }
    }

    chunks
}

/// Find a good breaking point (sentence boundary)
fn find_break_point(text: &str, start: usize, ideal_end: usize) -> usize {
    // Look for sentence endings within the last 20% of the chunk
    let search_start = start + ((ideal_end - start) * 80 / 100);
    let search_text = &text[search_start..ideal_end];

    // Korean and English sentence endings
    let sentence_endings = [". ", ".\n", "! ", "!\n", "? ", "?\n", "。", "！", "？"];

    // Find the last sentence ending
    let mut best_pos = None;
    let mut best_idx = 0;

    for ending in &sentence_endings {
        if let Some(idx) = search_text.rfind(ending) {
            if idx > best_idx {
                best_idx = idx;
                best_pos = Some(search_start + idx + ending.len());
            }
        }
    }

    best_pos.unwrap_or(ideal_end)
}

/// Split text by paragraphs
pub fn split_paragraphs(text: &str) -> Vec<String> {
    text.split("\n\n")
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chunk_short_text() {
        let text = "This is a short text.";
        let chunks = chunk_text(text, 100, 10);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].text, text);
    }

    #[test]
    fn test_chunk_long_text() {
        let text = "First sentence. Second sentence. Third sentence. Fourth sentence. Fifth sentence.";
        let chunks = chunk_text(text, 10, 2); // Very small chunks for testing
        assert!(chunks.len() > 1);
    }

    #[test]
    fn test_split_paragraphs() {
        let text = "Paragraph 1.\n\nParagraph 2.\n\nParagraph 3.";
        let paras = split_paragraphs(text);
        assert_eq!(paras.len(), 3);
        assert_eq!(paras[0], "Paragraph 1.");
        assert_eq!(paras[1], "Paragraph 2.");
    }
}
