use crate::error::{AppError, AppResult};
use unicode_segmentation::UnicodeSegmentation;

pub const MAX_TEXT_FILE_BYTES: u64 = 20 * 1024 * 1024;
pub const MAX_BOOK_HAN_CHARACTERS: usize = 5_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextChapter {
    pub title: String,
    pub text: String,
}

pub fn is_han_character(character: char) -> bool {
    let code_point = character as u32;
    matches!(
        code_point,
        0x3400..=0x4DBF
            | 0x4E00..=0x9FFF
            | 0xF900..=0xFAFF
            | 0x20000..=0x2A6DF
            | 0x2A700..=0x2B73F
            | 0x2B740..=0x2B81F
            | 0x2B820..=0x2CEAF
            | 0x2CEB0..=0x2EBEF
            | 0x2EBF0..=0x2EE5F
            | 0x2F800..=0x2FA1F
            | 0x30000..=0x3134F
            | 0x31350..=0x323AF
    )
}

pub fn count_han_characters(text: &str) -> usize {
    text.chars()
        .filter(|character| is_han_character(*character))
        .count()
}

pub fn split_into_chapters(text: &str) -> Vec<TextChapter> {
    let normalized = normalize_text(text);
    let lines = normalized.lines().collect::<Vec<_>>();
    let has_heading = lines.iter().any(|line| is_chapter_heading(line.trim()));
    if !has_heading {
        return vec![TextChapter {
            title: "正文".to_string(),
            text: normalized,
        }];
    }

    let mut chapters = Vec::new();
    let mut preamble = Vec::new();
    let mut current_title: Option<String> = None;
    let mut current_lines = Vec::new();

    for line in lines {
        let trimmed = line.trim();
        if is_chapter_heading(trimmed) {
            if let Some(title) = current_title.take() {
                chapters.push(TextChapter {
                    title,
                    text: join_non_empty_lines(&current_lines),
                });
                current_lines.clear();
            }
            current_title = Some(trimmed.to_string());
        } else if current_title.is_some() {
            current_lines.push(line);
        } else {
            preamble.push(line);
        }
    }

    if let Some(title) = current_title {
        chapters.push(TextChapter {
            title,
            text: join_non_empty_lines(&current_lines),
        });
    }

    let preamble_text = preamble
        .into_iter()
        .filter(|line| !is_document_title_line(line.trim()))
        .collect::<Vec<_>>();
    let preamble_text = join_non_empty_lines(&preamble_text);
    if !preamble_text.is_empty() {
        chapters.insert(
            0,
            TextChapter {
                title: "正文".to_string(),
                text: preamble_text,
            },
        );
    }
    chapters
}

fn join_non_empty_lines(lines: &[&str]) -> String {
    lines.join("\n").trim().to_string()
}

fn is_chapter_heading(line: &str) -> bool {
    if line.is_empty()
        || is_document_title_line(line)
        || UnicodeSegmentation::graphemes(line, true).count() > 80
        || line
            .chars()
            .any(|character| matches!(character, '。' | '！' | '？' | '；' | '.' | '!' | '?' | ';'))
    {
        return false;
    }

    let has_marker = line
        .chars()
        .any(|character| matches!(character, '篇' | '章' | '节' | '卷' | '回' | '部'));
    let has_ordinal = line.chars().any(is_ordinal_character);
    has_marker && has_ordinal && (line.contains('第') || has_adjacent_marker_and_ordinal(line))
}

fn has_adjacent_marker_and_ordinal(line: &str) -> bool {
    let characters = line.chars().collect::<Vec<_>>();
    characters.windows(2).any(|window| {
        (matches!(window[0], '篇' | '章' | '节' | '卷' | '回' | '部')
            && is_ordinal_character(window[1]))
            || (is_ordinal_character(window[0])
                && matches!(window[1], '篇' | '章' | '节' | '卷' | '回' | '部'))
    })
}

fn is_ordinal_character(character: char) -> bool {
    character.is_ascii_digit()
        || matches!(
            character,
            '〇' | '零'
                | '一'
                | '二'
                | '三'
                | '四'
                | '五'
                | '六'
                | '七'
                | '八'
                | '九'
                | '十'
                | '百'
                | '千'
                | '万'
                | '两'
        )
}

fn is_document_title_line(line: &str) -> bool {
    line.starts_with('《') && line.contains('》') && line.contains('卷')
}

pub fn read_utf8_txt(path: &str) -> AppResult<(String, String)> {
    let extension_is_txt = std::path::Path::new(path)
        .extension()
        .and_then(|value| value.to_str())
        .is_some_and(|value| value.eq_ignore_ascii_case("txt"));
    if !extension_is_txt {
        return Err(AppError::new(
            "UNSUPPORTED_FILE_FORMAT",
            "当前只支持 .txt 文件",
        ));
    }
    let metadata = std::fs::metadata(path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            AppError::new("FILE_NOT_FOUND", "TXT 文件不存在")
        } else {
            AppError::new("FILE_IO_ERROR", error.to_string())
        }
    })?;
    if metadata.len() > MAX_TEXT_FILE_BYTES {
        return Err(AppError::new(
            "TEXT_FILE_TOO_LARGE",
            "TXT 文件超过 20 MB 限制",
        ));
    }

    let bytes =
        std::fs::read(path).map_err(|error| AppError::new("FILE_IO_ERROR", error.to_string()))?;
    let text = String::from_utf8(bytes)
        .map_err(|_| AppError::new("INVALID_TEXT_ENCODING", "TXT 文件不是有效的 UTF-8 编码"))?;
    let file_name = std::path::Path::new(path)
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("未命名.txt")
        .to_string();
    Ok((normalize_text(&text), file_name))
}

pub fn normalize_text(text: &str) -> String {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let text = text.replace("\r\n", "\n").replace('\r', "\n");
    let mut normalized = String::with_capacity(text.len());
    let mut consecutive_newlines = 0;
    for character in text.chars() {
        if character == '\n' {
            if consecutive_newlines < 2 {
                normalized.push(character);
            }
            consecutive_newlines += 1;
        } else {
            consecutive_newlines = 0;
            normalized.push(character);
        }
    }
    normalized.trim().to_string()
}

pub fn title_from_file_name(file_name: &str) -> String {
    let path = std::path::Path::new(file_name);
    path.file_stem()
        .and_then(|value| value.to_str())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("未命名古籍")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::{
        count_han_characters, is_chapter_heading, is_han_character, normalize_text, read_utf8_txt,
        split_into_chapters, title_from_file_name,
    };
    use uuid::Uuid;

    #[test]
    fn normalizes_bom_line_endings_and_extra_blank_lines() {
        assert_eq!(normalize_text("\u{feff} 甲\r\n\r\n\r\n乙 \r"), "甲\n\n乙");
        assert_eq!(normalize_text("  \r\n  "), "");
    }

    #[test]
    fn derives_title_from_file_name() {
        assert_eq!(title_from_file_name("黄帝内经.txt"), "黄帝内经");
        assert_eq!(title_from_file_name("/tmp/伤寒论"), "伤寒论");
    }

    #[test]
    fn rejects_invalid_utf8_without_guessing_encoding() {
        let path = std::env::temp_dir().join(format!("ancient-invalid-{}.txt", Uuid::now_v7()));
        std::fs::write(&path, [0xff, 0xfe, 0xfd]).expect("fixture should write");
        let error =
            read_utf8_txt(path.to_str().expect("temp path")).expect_err("invalid utf8 should fail");
        assert_eq!(error.code, "INVALID_TEXT_ENCODING");
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn reads_utf8_and_returns_source_file_name() {
        let path = std::env::temp_dir().join(format!("ancient-valid-{}.txt", Uuid::now_v7()));
        std::fs::write(&path, "\u{feff}黄帝内经𠮷𠀀").expect("fixture should write");
        let (text, file_name) =
            read_utf8_txt(path.to_str().expect("temp path")).expect("utf8 should read");
        assert_eq!(text, "黄帝内经𠮷𠀀");
        assert!(file_name.ends_with(".txt"));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn rejects_files_over_the_size_limit_before_reading() {
        let path = std::env::temp_dir().join(format!("ancient-large-{}.txt", Uuid::now_v7()));
        let file = std::fs::File::create(&path).expect("fixture should create");
        file.set_len(super::MAX_TEXT_FILE_BYTES + 1)
            .expect("fixture should grow");
        let error =
            read_utf8_txt(path.to_str().expect("temp path")).expect_err("large file should fail");
        assert_eq!(error.code, "TEXT_FILE_TOO_LARGE");
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn counts_bmp_and_supplementary_han_characters() {
        assert_eq!(count_han_characters("腧穴𠀀𠮷A"), 4);
        assert!(is_han_character('𠀀'));
        assert!(!is_han_character('A'));
    }

    #[test]
    fn splits_numbered_chapter_headings_and_omits_document_title() {
        let text = "《测试》第一卷\n\n第一篇 上古\n甲乙。\n\n第二篇 四时\n丙丁。";
        let chapters = split_into_chapters(text);
        assert_eq!(chapters.len(), 2);
        assert_eq!(chapters[0].title, "第一篇 上古");
        assert_eq!(chapters[0].text, "甲乙。");
        assert_eq!(chapters[1].title, "第二篇 四时");
        assert_eq!(chapters[1].text, "丙丁。");
    }

    #[test]
    fn keeps_unheaded_text_as_one_body_chapter() {
        let chapters = split_into_chapters("甲乙。\n丙丁。");
        assert_eq!(
            chapters,
            vec![super::TextChapter {
                title: "正文".to_string(),
                text: "甲乙。\n丙丁。".to_string(),
            }]
        );
    }

    #[test]
    fn chapter_heading_detection_is_conservative_for_long_sentences() {
        assert!(is_chapter_heading("第一篇 上古天真论"));
        assert!(is_chapter_heading("上古天真论篇第一"));
        assert!(!is_chapter_heading("第一篇，正文并未结束。"));
        assert!(!is_chapter_heading(
            &("第一篇 ".to_string() + &"甲".repeat(80))
        ));
    }
}
