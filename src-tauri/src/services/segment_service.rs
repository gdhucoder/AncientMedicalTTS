use crate::error::{AppError, AppResult};
use unicode_segmentation::UnicodeSegmentation;

pub const TARGET_MIN_TOKENS: usize = 20;
pub const TARGET_MAX_TOKENS: usize = 80;
const FALLBACK_TARGET_TOKENS: usize = 70;

const PRIMARY_BOUNDARIES: &[&str] = &["。", "！", "？", "；", ".", "!", "?", ";", "\n"];
const SECONDARY_BOUNDARIES: &[&str] = &["，", "、", "：", ",", ":"];

#[derive(Debug, Clone, PartialEq, Eq)]
struct TokenRange {
    start: usize,
    end: usize,
}

pub fn segment_text(text: &str) -> AppResult<Vec<String>> {
    if text.trim().is_empty() {
        return Err(AppError::new("EMPTY_TEXT", "文本内容为空"));
    }

    let tokens: Vec<&str> = UnicodeSegmentation::graphemes(text, true).collect();
    let ranges = build_ranges(&tokens);
    let segments = ranges
        .into_iter()
        .map(|range| tokens[range.start..range.end].concat().trim().to_string())
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>();
    if segments.is_empty() {
        return Err(AppError::new("EMPTY_TEXT", "文本内容为空"));
    }
    Ok(segments)
}

fn build_ranges(tokens: &[&str]) -> Vec<TokenRange> {
    let mut ranges = Vec::new();
    let mut sentence_start = 0;
    for (index, token) in tokens.iter().enumerate() {
        if PRIMARY_BOUNDARIES.contains(token) {
            let end = index + 1;
            append_sentence_ranges(&mut ranges, tokens, sentence_start, end);
            sentence_start = end;
        }
    }
    if sentence_start < tokens.len() {
        append_sentence_ranges(&mut ranges, tokens, sentence_start, tokens.len());
    }
    merge_short_ranges(ranges, tokens)
}

fn append_sentence_ranges(ranges: &mut Vec<TokenRange>, tokens: &[&str], start: usize, end: usize) {
    if start >= end {
        return;
    }
    if end - start <= TARGET_MAX_TOKENS {
        ranges.push(TokenRange { start, end });
        return;
    }

    let mut cursor = start;
    while end - cursor > TARGET_MAX_TOKENS {
        let limit = cursor + TARGET_MAX_TOKENS;
        let split_at = (cursor + 1..=limit)
            .rev()
            .find(|index| SECONDARY_BOUNDARIES.contains(&tokens[*index - 1]))
            .unwrap_or_else(|| (cursor + FALLBACK_TARGET_TOKENS).min(limit));
        ranges.push(TokenRange {
            start: cursor,
            end: split_at,
        });
        cursor = split_at;
    }
    if cursor < end {
        ranges.push(TokenRange { start: cursor, end });
    }
}

fn merge_short_ranges(mut ranges: Vec<TokenRange>, tokens: &[&str]) -> Vec<TokenRange> {
    let mut index = 0;
    while index < ranges.len() {
        if ranges[index].end - ranges[index].start >= TARGET_MIN_TOKENS || ranges.len() == 1 {
            index += 1;
            continue;
        }

        if index > 0 {
            let previous = &ranges[index - 1];
            if previous.end - previous.start + ranges[index].end - ranges[index].start
                <= TARGET_MAX_TOKENS
            {
                ranges[index - 1].end = ranges[index].end;
                ranges.remove(index);
                continue;
            }
        }
        if index + 1 < ranges.len() {
            let current_length = ranges[index].end - ranges[index].start;
            let next_length = ranges[index + 1].end - ranges[index + 1].start;
            if current_length + next_length <= TARGET_MAX_TOKENS {
                ranges[index].end = ranges[index + 1].end;
                ranges.remove(index + 1);
                continue;
            }
        }
        index += 1;
    }

    // A range can only be empty if an upstream rule is changed. Keep this
    // defensive filter here so the persistence layer never receives one.
    ranges.retain(|range| range.start < range.end && range.end <= tokens.len());
    ranges
}

#[cfg(test)]
mod tests {
    use super::{segment_text, TARGET_MAX_TOKENS};
    use unicode_segmentation::UnicodeSegmentation;

    fn joined_without_boundary_whitespace(segments: &[String]) -> String {
        segments
            .join("")
            .chars()
            .filter(|character| !character.is_whitespace())
            .collect()
    }

    fn source_without_boundary_whitespace(text: &str) -> String {
        text.chars()
            .filter(|character| !character.is_whitespace())
            .collect()
    }

    #[test]
    fn preserves_unicode_graphemes_and_order() {
        let source = "黄帝内经𠮷𠀀，天地玄黄。人法地，地法天。";
        let segments = segment_text(source).expect("segmenting succeeds");
        assert_eq!(
            joined_without_boundary_whitespace(&segments),
            source_without_boundary_whitespace(source)
        );
        assert!(segments.join("").contains("𠮷𠀀"));
    }

    #[test]
    fn prefers_sentence_boundaries_and_merges_short_sentences() {
        let source = "甲乙丙丁。戊己庚辛。天地玄黄宇宙洪荒日月盈昃辰宿列张。";
        let segments = segment_text(source).expect("segmenting succeeds");
        assert!(segments.len() <= 2);
        assert_eq!(joined_without_boundary_whitespace(&segments), source);
    }

    #[test]
    fn splits_long_unpunctuated_text_without_exceeding_target() {
        let source = "天地玄黄宇宙洪荒".repeat(20);
        let segments = segment_text(&source).expect("segmenting succeeds");
        assert!(segments.len() > 1);
        assert!(segments
            .iter()
            .all(|segment| segment.chars().count() <= TARGET_MAX_TOKENS));
        assert_eq!(joined_without_boundary_whitespace(&segments), source);
    }

    #[test]
    fn supports_english_and_chinese_punctuation() {
        let source = "甲乙丙丁; 戊己庚辛, 壬癸。子丑寅卯! 辰巳午未?";
        let segments = segment_text(source).expect("segmenting succeeds");
        assert_eq!(
            joined_without_boundary_whitespace(&segments),
            source_without_boundary_whitespace(source)
        );
    }

    #[test]
    fn reports_sample_fixture_metrics() {
        let source = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../tests/fixtures/sample_medical_classic.txt"
        ));
        let segments = segment_text(source).expect("sample should segment");
        let token_count = UnicodeSegmentation::graphemes(source.trim(), true).count();
        let shortest = segments
            .iter()
            .map(|segment| UnicodeSegmentation::graphemes(segment.as_str(), true).count())
            .min()
            .unwrap_or(0);
        let longest = segments
            .iter()
            .map(|segment| UnicodeSegmentation::graphemes(segment.as_str(), true).count())
            .max()
            .unwrap_or(0);
        println!("sample_medical_classic.txt: grapheme_tokens={token_count}, segments={}, shortest={shortest}, longest={longest}", segments.len());
        assert!(!segments.is_empty());
    }

    #[test]
    fn reports_pronunciation_fixture_metrics() {
        let source = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../tests/fixtures/pronunciation_classic.txt"
        ));
        let segments = segment_text(source).expect("pronunciation fixture should segment");
        let shortest = segments
            .iter()
            .map(|segment| UnicodeSegmentation::graphemes(segment.as_str(), true).count())
            .min()
            .unwrap_or(0);
        let longest = segments
            .iter()
            .map(|segment| UnicodeSegmentation::graphemes(segment.as_str(), true).count())
            .max()
            .unwrap_or(0);
        println!(
            "pronunciation_classic.txt: segments={}, shortest={shortest}, longest={longest}",
            segments.len()
        );
        assert_eq!(segments.len(), 22);
        assert!(segments.iter().all(|segment| (20..=80)
            .contains(&UnicodeSegmentation::graphemes(segment.as_str(), true).count())));
    }
}
