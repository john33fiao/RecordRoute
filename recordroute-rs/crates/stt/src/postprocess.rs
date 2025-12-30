use crate::types::Segment;
use tracing::debug;

/// Phrases to discard (common Whisper hallucinations)
const DISCARD_PHRASES: &[&str] = &[
    "이 영상은 자막을 사용하였습니다.",
    "자막을 사용하였습니다.",
    "이 영상은 자막을 사용합니다.",
    "자막을 사용합니다.",
];

/// Common filler words in Korean
const FILLER_WORDS: &[&str] = &[
    "아", "으", "음", "어", "저", "그", "뭐", "얍", "흠", "네", "예",
];

/// Process segment text with filtering and normalization
///
/// # Arguments
/// * `text` - Raw segment text
/// * `filter_fillers` - Whether to filter out filler words
/// * `min_length` - Minimum text length to keep
/// * `normalize_punct` - Whether to normalize punctuation
pub fn process_segment_text(
    text: &str,
    filter_fillers: bool,
    min_length: usize,
    normalize_punct: bool,
) -> String {
    let text = text.trim();
    
    // Check if should keep this segment
    if !should_keep_segment(text, filter_fillers, min_length) {
        return String::new();
    }
    
    // Normalize text
    normalize_text(text, normalize_punct)
}

/// Determine if a segment should be kept
fn should_keep_segment(text: &str, enable_filter: bool, min_length: usize) -> bool {
    let text = text.trim();
    
    // Empty text
    if text.is_empty() {
        return false;
    }
    
    // Minimum length check
    if text.len() < min_length {
        return false;
    }
    
    // Check for discard phrases
    for phrase in DISCARD_PHRASES {
        if text == *phrase {
            debug!("Discarding phrase: {}", text);
            return false;
        }
    }
    
    // If filtering is disabled, keep it
    if !enable_filter {
        return true;
    }
    
    // Filter standalone filler words
    for filler in FILLER_WORDS {
        if text == *filler {
            debug!("Filtering filler word: {}", text);
            return false;
        }
    }
    
    // Check for repetitive patterns
    let words: Vec<&str> = text.split_whitespace().collect();
    
    // If 10+ words but only 1-2 unique, likely repetitive
    if words.len() >= 10 {
        let unique_words: std::collections::HashSet<_> = words.iter().collect();
        if unique_words.len() <= 2 {
            debug!("Discarding repetitive pattern: {} unique words in {} total", 
                   unique_words.len(), words.len());
            return false;
        }
    }
    
    // Check for number-only sequences (e.g., "1. 2. 3. 4...")
    if words.len() >= 10 {
        let is_number_only = words.iter().all(|w| {
            w.chars().all(|c| c.is_numeric() || c == '.' || c.is_whitespace())
        });
        if is_number_only {
            debug!("Discarding number-only sequence");
            return false;
        }
    }
    
    true
}

/// Normalize text by removing word repetitions and fixing punctuation
fn normalize_text(text: &str, normalize_punct: bool) -> String {
    let text = text.trim();
    
    // Remove word repetitions
    let text = remove_word_repetitions(text);
    
    if normalize_punct {
        // Normalize consecutive periods (4+ periods -> "...")
        let mut result = text.clone();
        while result.contains("....") {
            result = result.replace("....", "...");
        }
        
        // Normalize multiple spaces to single space
        let re = regex::Regex::new(r"\s+").unwrap();
        result = re.replace_all(&result, " ").to_string();
        
        result
    } else {
        text
    }
}

/// Remove word repetitions from text
///
/// Removes both consecutive and non-consecutive duplicate words
fn remove_word_repetitions(text: &str) -> String {
    let words: Vec<&str> = text.split_whitespace().collect();
    
    if words.len() <= 1 {
        return text.to_string();
    }
    
    // First pass: remove consecutive duplicates
    let mut cleaned_words = Vec::new();
    let mut prev_word: Option<&str> = None;
    let mut consecutive_count = 0;
    
    for word in &words {
        if Some(*word) == prev_word {
            consecutive_count += 1;
            // Skip if same word appears 3+ times consecutively
            if consecutive_count >= 3 {
                continue;
            }
        } else {
            consecutive_count = 1;
        }
        
        cleaned_words.push(*word);
        prev_word = Some(*word);
    }
    
    // Second pass: limit total word frequency
    if cleaned_words.len() > 1 {
        let mut word_positions: std::collections::HashMap<&str, Vec<usize>> = 
            std::collections::HashMap::new();
        let mut final_words = Vec::new();
        
        for (i, word) in cleaned_words.iter().enumerate() {
            let positions = word_positions.entry(*word).or_insert_with(Vec::new);
            
            // If this word already appeared 5+ times, skip it
            if positions.len() >= 5 {
                continue;
            }
            
            // Short words (2 chars or less) limited to 3 appearances
            if word.len() <= 2 && positions.len() >= 3 {
                continue;
            }
            
            positions.push(i);
            final_words.push(*word);
        }
        
        final_words.join(" ")
    } else {
        cleaned_words.join(" ")
    }
}

/// Merge consecutive segments with same text and close timestamps
///
/// # Arguments
/// * `segments` - Input segments
/// * `max_gap` - Maximum time gap (in seconds) to merge
pub fn merge_segments(segments: Vec<Segment>, max_gap: f32) -> Vec<Segment> {
    if segments.is_empty() {
        return Vec::new();
    }
    
    let mut merged = Vec::new();
    merged.push(segments[0].clone());
    
    for segment in segments.iter().skip(1) {
        let current = merged.last_mut().unwrap();
        
        let same_text = segment.text.trim() == current.text.trim();
        let time_continuous = segment.start <= current.end + max_gap;
        
        if same_text && time_continuous {
            // Merge: extend end time
            current.end = current.end.max(segment.end);
        } else {
            merged.push(segment.clone());
        }
    }
    
    debug!("Merged {} segments into {} segments", segments.len(), merged.len());
    
    merged
}

/// Write data to file atomically (temp file + rename)
pub fn write_atomic(path: &std::path::Path, data: &str) -> std::io::Result<()> {
    use std::fs;
    use std::io::Write;
    
    let tmp_path = path.with_extension("tmp");
    
    // Write to temp file
    let mut file = fs::File::create(&tmp_path)?;
    file.write_all(data.as_bytes())?;
    file.sync_all()?;
    drop(file);
    
    // Atomic rename
    fs::rename(&tmp_path, path)?;
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_should_keep_segment() {
        assert!(!should_keep_segment("", false, 2));
        assert!(!should_keep_segment("a", false, 2));
        assert!(should_keep_segment("hello world", false, 2));
        assert!(!should_keep_segment("아", true, 2));
        assert!(!should_keep_segment("이 영상은 자막을 사용하였습니다.", false, 2));
    }
    
    #[test]
    fn test_remove_word_repetitions() {
        let input = "hello hello hello world";
        let output = remove_word_repetitions(input);
        // Should keep at most 2 consecutive
        assert!(!output.contains("hello hello hello"));
        
        let input = "a a a a a a a a a a";
        let output = remove_word_repetitions(input);
        // Should limit total frequency
        assert!(output.split_whitespace().count() < 5);
    }
    
    #[test]
    fn test_merge_segments() {
        let segments = vec![
            Segment::new(0.0, 2.0, "hello".to_string()),
            Segment::new(2.0, 4.0, "hello".to_string()),
            Segment::new(4.5, 6.0, "world".to_string()),
        ];
        
        let merged = merge_segments(segments, 0.3);
        
        // First two should merge (same text, consecutive)
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0].text, "hello");
        assert_eq!(merged[0].end, 4.0);
    }
    
    #[test]
    fn test_normalize_text() {
        let input = "hello....world";
        let output = normalize_text(input, true);
        assert_eq!(output, "hello...world");
        
        let input = "hello    world";
        let output = normalize_text(input, true);
        assert_eq!(output, "hello world");
    }
}
