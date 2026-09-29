//! Importer for the platform-neutral Ancient Annotated Book Format.
//!
//! The parser produces a normalized in-memory document before it touches
//! SQLite. Legacy datasets are adapted here and never leak a legacy branch
//! into pronunciation, TTS, or publication code.

use crate::{
    db::Database,
    error::{AppError, AppResult},
    models::{
        AnnotatedImportIssue, AnnotatedImportPreflight, Annotation, Chapter, ImportResult,
        ImportedPronunciationMissing, ImportedTtsPreflight,
    },
    services::{book_service, pronunciation_service, text_service},
};
use serde::Deserialize;
use std::{
    collections::{HashMap, HashSet},
    fs::{self, File},
    io::Read,
    path::{Component, Path},
};
use time::{format_description::well_known::Rfc3339, OffsetDateTime};
use unicode_segmentation::UnicodeSegmentation;
use uuid::Uuid;
use zip::ZipArchive;

const FORMAT: &str = "ancient-annotated-book";
const FORMAT_VERSION: &str = "1.0";
const READER_SELECTED_FORMAT: &str = "ancient-medical-reader-selected-content";
const MAX_ZIP_FILES: usize = 512;
const MAX_ZIP_COMPRESSED_BYTES: u64 = 100 * 1024 * 1024;
const MAX_ZIP_EXPANDED_BYTES: u64 = 200 * 1024 * 1024;

#[derive(Debug, Deserialize, Default, Clone)]
struct RawManifest {
    format: Option<String>,
    format_version: Option<String>,
    dataset: Option<String>,
    version: Option<String>,
    id: Option<String>,
    title: Option<String>,
    book: Option<RawBook>,
    pronunciation: Option<RawPronunciation>,
    #[serde(default)]
    chapters: Vec<RawChapterRef>,
    #[serde(default)]
    lessons: Vec<RawLessonRef>,
    sources: Option<RawSources>,
}

#[derive(Debug, Deserialize, Default, Clone)]
struct RawBook {
    id: Option<String>,
    title: Option<String>,
    author: Option<String>,
    dynasty: Option<String>,
    edition: Option<String>,
}

#[derive(Debug, Deserialize, Default, Clone)]
struct RawPronunciation {
    mode: Option<String>,
}

#[derive(Debug, Deserialize, Default, Clone)]
struct RawSources {}

#[derive(Debug, Deserialize, Default, Clone)]
struct RawChapterRef {
    id: Option<String>,
    order: Option<i64>,
    collection: Option<String>,
    title: Option<String>,
    subtitle: Option<String>,
    #[serde(alias = "reader_subtitle")]
    reader_subtitle: Option<String>,
    file: Option<String>,
    segments: Option<Vec<RawSegment>>,
}

#[derive(Debug, Deserialize, Default, Clone)]
struct RawLessonRef {
    id: Option<String>,
    day: Option<i64>,
    source_book: Option<String>,
    source_chapter: Option<String>,
    display_title: Option<String>,
    file: Option<String>,
}

#[derive(Debug, Deserialize, Default, Clone)]
struct RawLesson {
    source_book: Option<String>,
    source_chapter: Option<String>,
    display_title: Option<String>,
    segments: Option<Vec<RawSegment>>,
}

#[derive(Debug, Deserialize, Default, Clone)]
struct RawSegment {
    id: Option<String>,
    order: Option<i64>,
    text: String,
    pinyin_numeric: Option<String>,
    pinyin_tone_marks: Option<String>,
    tokens: Option<Vec<RawToken>>,
    translation: Option<String>,
}

#[derive(Debug, Deserialize, Default, Clone)]
struct RawToken {
    text: String,
    #[serde(alias = "pinyin")]
    pinyin_numeric: Option<String>,
    pinyin_tone_marks: Option<String>,
}

#[derive(Debug, Clone)]
struct NormalizedDocument {
    dataset_id: Option<String>,
    title: String,
    author: Option<String>,
    dynasty: Option<String>,
    edition: Option<String>,
    pronunciation_mode: String,
    chapters: Vec<NormalizedChapter>,
    warnings: Vec<AnnotatedImportIssue>,
}

#[derive(Debug, Clone)]
struct NormalizedChapter {
    order: i64,
    collection: Option<String>,
    title: String,
    subtitle: Option<String>,
    segments: Vec<NormalizedSegment>,
}

#[derive(Debug, Clone)]
struct NormalizedSegment {
    order: i64,
    text: String,
    translation: Option<String>,
    tokens: Vec<NormalizedToken>,
}

#[derive(Debug, Clone)]
struct NormalizedToken {
    text: String,
    pinyin: Option<String>,
}

#[derive(Debug)]
struct SourceFiles {
    files: HashMap<String, Vec<u8>>,
}

pub async fn get_import_preflight(
    path: &str,
    trust_imported_pronunciation: bool,
) -> AppResult<AnnotatedImportPreflight> {
    match parse_source(path) {
        Ok(document) => Ok(preflight_from_document(
            &document,
            trust_imported_pronunciation,
        )),
        Err(error) => Ok(AnnotatedImportPreflight {
            can_import: false,
            format: FORMAT.to_string(),
            format_version: FORMAT_VERSION.to_string(),
            title: None,
            chapter_count: 0,
            segment_count: 0,
            han_character_count: 0,
            pinyin_covered_han_count: 0,
            pinyin_coverage_percent: 0.0,
            translation_segment_count: 0,
            pronunciation_mode: "unknown".to_string(),
            effective_pronunciation_mode: "unknown".to_string(),
            warnings: Vec::new(),
            errors: vec![issue(&error.code, &error.message, None, None)],
        }),
    }
}

pub async fn import_book(
    database: &Database,
    path: &str,
    trust_imported_pronunciation: bool,
) -> AppResult<ImportResult> {
    let document = parse_source(path)?;
    let preflight = preflight_from_document(&document, trust_imported_pronunciation);
    if !preflight.can_import {
        return Err(AppError::new(
            "ANNOTATED_IMPORT_INVALID",
            preflight
                .errors
                .first()
                .map(|item| item.message.clone())
                .unwrap_or_else(|| "已注音古籍预检未通过".to_string()),
        ));
    }
    let effective_mode =
        effective_mode(&document.pronunciation_mode, trust_imported_pronunciation)?;
    let now = now_rfc3339()?;
    let book_id = Uuid::now_v7().to_string();
    let source_file = Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .map(str::to_string);
    let mut transaction = database.pool().begin().await.map_err(transaction_error)?;

    sqlx::query(
        "INSERT INTO books (id, title, author, dynasty, edition, source_file, import_format, import_format_version, import_pronunciation_mode, import_dataset_id, imported_at, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&book_id)
    .bind(&document.title)
    .bind(&document.author)
    .bind(&document.dynasty)
    .bind(&document.edition)
    .bind(source_file)
    .bind(FORMAT)
    .bind(FORMAT_VERSION)
    .bind(&effective_mode)
    .bind(&document.dataset_id)
    .bind(&now)
    .bind(&now)
    .bind(&now)
    .execute(&mut *transaction)
    .await
    .map_err(transaction_error)?;

    let mut chapter_models = Vec::with_capacity(document.chapters.len());
    let mut segment_count = 0_i64;
    for chapter in &document.chapters {
        let chapter_id = Uuid::now_v7().to_string();
        sqlx::query(
            "INSERT INTO chapters (id, book_id, title, collection, subtitle, order_index, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&chapter_id)
        .bind(&book_id)
        .bind(&chapter.title)
        .bind(&chapter.collection)
        .bind(&chapter.subtitle)
        .bind(chapter.order)
        .bind(&now)
        .bind(&now)
        .execute(&mut *transaction)
        .await
        .map_err(transaction_error)?;

        chapter_models.push(Chapter {
            id: chapter_id.clone(),
            book_id: book_id.clone(),
            title: Some(chapter.title.clone()),
            collection: chapter.collection.clone(),
            subtitle: chapter.subtitle.clone(),
            order_index: chapter.order,
            created_at: now.clone(),
            updated_at: now.clone(),
        });

        for segment in &chapter.segments {
            let segment_id = Uuid::now_v7().to_string();
            let has_any_pinyin = segment.tokens.iter().any(|token| token.pinyin.is_some());
            let status = if effective_mode == "reference" && has_any_pinyin {
                "needs_review"
            } else {
                "analyzed"
            };
            sqlx::query(
                "INSERT INTO segments (id, chapter_id, order_index, original_text, translation, status, current_audio_id, created_at, updated_at)
                 VALUES (?, ?, ?, ?, ?, ?, NULL, ?, ?)",
            )
            .bind(&segment_id)
            .bind(&chapter_id)
            .bind(segment.order)
            .bind(&segment.text)
            .bind(&segment.translation)
            .bind(status)
            .bind(&now)
            .bind(&now)
            .execute(&mut *transaction)
            .await
            .map_err(transaction_error)?;

            for (token_index, token) in segment.tokens.iter().enumerate() {
                let Some(pinyin) = token.pinyin.as_deref() else {
                    continue;
                };
                let is_authoritative = effective_mode == "authoritative";
                let annotation = Annotation {
                    id: Uuid::now_v7().to_string(),
                    segment_id: segment_id.clone(),
                    start_token: token_index,
                    end_token: token_index + 1,
                    surface_text: token.text.clone(),
                    default_pinyin: Some(pinyin.to_string()),
                    target_pinyin: is_authoritative.then(|| pinyin.to_string()),
                    candidate_pinyin: vec![pinyin.to_string()],
                    risk_type: if is_authoritative {
                        "imported_authoritative".to_string()
                    } else {
                        "imported_reference".to_string()
                    },
                    reason: Some(if is_authoritative {
                        "来自已注音古籍导入，已作为最终发音".to_string()
                    } else {
                        "来自已注音古籍导入，仅作为参考注音".to_string()
                    }),
                    review_status: if is_authoritative {
                        "confirmed".to_string()
                    } else {
                        "needs_review".to_string()
                    },
                    analyzer_version: Some("ancient-annotated-book-v1".to_string()),
                    source: Some(if is_authoritative {
                        "imported_authoritative".to_string()
                    } else {
                        "imported_reference".to_string()
                    }),
                    rule_type: Some("imported".to_string()),
                    confidence: Some(if is_authoritative {
                        "verified".to_string()
                    } else {
                        "medium".to_string()
                    }),
                    source_rule_id: None,
                    created_at: now.clone(),
                    updated_at: now.clone(),
                };
                pronunciation_service::insert_annotation(&mut transaction, &annotation, &now)
                    .await?;
            }
            segment_count += 1;
        }
    }
    transaction.commit().await.map_err(transaction_error)?;

    let book = book_service::get_book(database, &book_id).await?;
    Ok(ImportResult {
        book,
        chapter: chapter_models
            .first()
            .cloned()
            .ok_or_else(|| AppError::new("ANNOTATED_IMPORT_INVALID", "导入文件没有章节"))?,
        chapters: chapter_models,
        segment_count,
    })
}

pub async fn get_tts_preflight(
    database: &Database,
    book_id: &str,
) -> AppResult<ImportedTtsPreflight> {
    let chapters = book_service::list_chapters(database, book_id).await?;
    let mut total = 0_i64;
    let mut covered = 0_i64;
    let mut missing_segments = Vec::new();
    for chapter_summary in chapters {
        let chapter = chapter_summary.chapter;
        let first_page = book_service::list_segments(database, &chapter.id, 0, 100).await?;
        let total_segments = first_page.total;
        let mut pages = vec![first_page.items];
        if total_segments > 100 {
            let mut offset = 100_i64;
            while offset < total_segments {
                pages.push(
                    book_service::list_segments(database, &chapter.id, offset, 100)
                        .await?
                        .items,
                );
                offset += 100;
            }
        }
        for segment in pages.into_iter().flatten() {
            if !segment.speak_enabled {
                continue;
            }
            let tokens = pronunciation_service::grapheme_tokens(segment.effective_text());
            let han_indexes = tokens
                .iter()
                .enumerate()
                .filter(|(_, token)| pronunciation_service::is_han_token(&token.text))
                .map(|(index, _)| index)
                .collect::<Vec<_>>();
            total += han_indexes.len() as i64;
            let mut forced = vec![false; tokens.len()];
            for range in
                pronunciation_service::build_effective_forced_pronunciations(database, &segment.id)
                    .await?
            {
                for index in range.start_token..range.end_token.min(forced.len()) {
                    if pronunciation_service::is_han_token(&tokens[index].text) {
                        forced[index] = true;
                    }
                }
            }
            let segment_covered = han_indexes.iter().filter(|index| forced[**index]).count();
            covered += segment_covered as i64;
            if segment_covered < han_indexes.len() {
                let segment_id = segment.id.clone();
                let preview = segment.effective_text().chars().take(48).collect();
                missing_segments.push(ImportedPronunciationMissing {
                    segment_id,
                    chapter_title: chapter.title.clone(),
                    segment_order: segment.order_index,
                    preview,
                });
            }
        }
    }
    Ok(ImportedTtsPreflight {
        can_generate_strict: total == covered,
        total_han_count: total,
        covered_han_count: covered,
        missing_han_count: total - covered,
        missing_segments,
    })
}

fn parse_source(path: &str) -> AppResult<NormalizedDocument> {
    let source = read_source_files(path)?;
    let manifest_bytes = source
        .files
        .get("manifest.json")
        .ok_or_else(|| AppError::new("ANNOTATED_MANIFEST_MISSING", "缺少 manifest.json"))?;
    let manifest: RawManifest = serde_json::from_slice(manifest_bytes).map_err(|error| {
        AppError::new(
            "ANNOTATED_INVALID_JSON",
            format!("manifest.json 无效：{error}"),
        )
    })?;
    if manifest.format.as_deref() == Some(READER_SELECTED_FORMAT) {
        return parse_reader_selected_source(&source, &manifest);
    }
    let is_legacy = manifest.format.is_none();
    if !is_legacy && manifest.format.as_deref() != Some(FORMAT) {
        return Err(AppError::new(
            "ANNOTATED_UNSUPPORTED_FORMAT",
            "不是 ancient-annotated-book 格式",
        ));
    }
    let version = manifest
        .format_version
        .clone()
        .or_else(|| manifest.version.clone())
        .unwrap_or_else(|| FORMAT_VERSION.to_string());
    if !is_legacy && version != FORMAT_VERSION {
        return Err(AppError::new(
            "ANNOTATED_UNSUPPORTED_VERSION",
            format!("不支持的已注音古籍版本：{version}"),
        ));
    }
    let raw_book = manifest.book.clone().unwrap_or_default();
    let title = raw_book
        .title
        .clone()
        .or_else(|| manifest.dataset.clone())
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| AppError::new("ANNOTATED_BOOK_TITLE_MISSING", "已注音古籍缺少书名"))?;
    let pronunciation_mode = manifest
        .pronunciation
        .as_ref()
        .and_then(|pronunciation| pronunciation.mode.clone())
        .unwrap_or_else(|| "reference".to_string());
    if !matches!(pronunciation_mode.as_str(), "reference" | "authoritative") {
        return Err(AppError::new(
            "ANNOTATED_PRONUNCIATION_MODE_INVALID",
            "pronunciation.mode 只能是 reference 或 authoritative",
        ));
    }

    let mut chapters = Vec::with_capacity(manifest.chapters.len());
    let mut chapter_ids = HashSet::new();
    let mut chapter_orders = HashSet::new();
    let mut all_segment_ids = HashSet::new();
    for (chapter_index, chapter_ref) in manifest.chapters.iter().enumerate() {
        let chapter_id = chapter_ref
            .id
            .clone()
            .unwrap_or_else(|| format!("chapter-{:03}", chapter_index + 1));
        let order = chapter_ref.order.unwrap_or(chapter_index as i64 + 1);
        if !chapter_ids.insert(chapter_id.clone()) {
            return Err(AppError::new(
                "ANNOTATED_DUPLICATE_CHAPTER_ID",
                "章节 ID 重复",
            ));
        }
        if !chapter_orders.insert(order) {
            return Err(AppError::new(
                "ANNOTATED_DUPLICATE_CHAPTER_ORDER",
                "章节顺序重复",
            ));
        }
        let raw_chapter = if let Some(segments) = &chapter_ref.segments {
            RawChapterRef {
                segments: Some(segments.clone()),
                ..chapter_ref.clone()
            }
        } else {
            let file = chapter_ref.file.as_deref().ok_or_else(|| {
                AppError::new(
                    "ANNOTATED_CHAPTER_FILE_MISSING",
                    "章节缺少 file 或 segments",
                )
            })?;
            let file = safe_relative_path(file)?;
            let bytes = source.files.get(&file).ok_or_else(|| {
                AppError::new(
                    "ANNOTATED_CHAPTER_FILE_MISSING",
                    format!("找不到章节文件：{file}"),
                )
            })?;
            serde_json::from_slice::<RawChapterRef>(bytes).map_err(|error| {
                AppError::new(
                    "ANNOTATED_INVALID_CHAPTER_JSON",
                    format!("章节 {file} 无效：{error}"),
                )
            })?
        };
        let raw_segments = raw_chapter
            .segments
            .ok_or_else(|| AppError::new("ANNOTATED_SEGMENTS_MISSING", "章节缺少 segments"))?;
        let mut segments = Vec::with_capacity(raw_segments.len());
        let mut segment_orders = HashSet::new();
        for (segment_index, raw_segment) in raw_segments.iter().enumerate() {
            let segment_id = raw_segment
                .id
                .clone()
                .unwrap_or_else(|| format!("{chapter_id}-seg-{:03}", segment_index + 1));
            let segment_order = raw_segment.order.unwrap_or(segment_index as i64 + 1);
            if !all_segment_ids.insert(segment_id.clone()) {
                return Err(AppError::new(
                    "ANNOTATED_DUPLICATE_SEGMENT_ID",
                    "Segment ID 重复",
                ));
            }
            if !segment_orders.insert(segment_order) {
                return Err(AppError::new(
                    "ANNOTATED_DUPLICATE_SEGMENT_ORDER",
                    "Segment 顺序重复",
                ));
            }
            if raw_segment.text.trim().is_empty() {
                return Err(AppError::new(
                    "ANNOTATED_TEXT_EMPTY",
                    "Segment text 不能为空",
                ));
            }
            let tokens = normalize_segment_tokens(raw_segment)?;
            segments.push(NormalizedSegment {
                order: segment_order,
                text: raw_segment.text.clone(),
                translation: raw_segment.translation.clone(),
                tokens,
            });
        }
        chapters.push(NormalizedChapter {
            order,
            collection: chapter_ref
                .collection
                .clone()
                .or(raw_chapter.collection.clone()),
            title: chapter_ref
                .title
                .clone()
                .or(raw_chapter.title.clone())
                .unwrap_or_else(|| format!("第{}章", order)),
            subtitle: chapter_ref
                .subtitle
                .clone()
                .or(chapter_ref.reader_subtitle.clone())
                .or(raw_chapter.subtitle.clone())
                .or(raw_chapter.reader_subtitle.clone()),
            segments,
        });
    }
    if chapters.is_empty() {
        return Err(AppError::new(
            "ANNOTATED_CHAPTER_MISSING",
            "已注音古籍没有章节",
        ));
    }
    chapters.sort_by_key(|chapter| chapter.order);
    Ok(NormalizedDocument {
        dataset_id: raw_book.id.or(manifest.dataset.clone()),
        title,
        author: raw_book.author,
        dynasty: raw_book.dynasty,
        edition: raw_book.edition,
        pronunciation_mode,
        chapters,
        warnings: build_warnings(&manifest, is_legacy),
    })
}

fn parse_reader_selected_source(
    source: &SourceFiles,
    manifest: &RawManifest,
) -> AppResult<NormalizedDocument> {
    let version = manifest.format_version.as_deref().unwrap_or(FORMAT_VERSION);
    if version != FORMAT_VERSION {
        return Err(AppError::new(
            "ANNOTATED_UNSUPPORTED_VERSION",
            format!("不支持的精选内容包版本：{version}"),
        ));
    }
    let title = manifest
        .title
        .clone()
        .or_else(|| manifest.id.clone())
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| AppError::new("ANNOTATED_BOOK_TITLE_MISSING", "已注音古籍缺少书名"))?;
    if manifest.lessons.is_empty() {
        return Err(AppError::new(
            "ANNOTATED_CHAPTER_MISSING",
            "精选内容包没有 lessons",
        ));
    }

    let mut chapters = Vec::with_capacity(manifest.lessons.len());
    let mut chapter_ids = HashSet::new();
    let mut chapter_orders = HashSet::new();
    let mut all_segment_ids = HashSet::new();
    for (lesson_index, lesson_ref) in manifest.lessons.iter().enumerate() {
        let chapter_id = lesson_ref
            .id
            .clone()
            .unwrap_or_else(|| format!("chapter-{:03}", lesson_index + 1));
        let order = lesson_ref.day.unwrap_or(lesson_index as i64 + 1);
        if !chapter_ids.insert(chapter_id.clone()) {
            return Err(AppError::new(
                "ANNOTATED_DUPLICATE_CHAPTER_ID",
                "精选内容包 lesson ID 重复",
            ));
        }
        if !chapter_orders.insert(order) {
            return Err(AppError::new(
                "ANNOTATED_DUPLICATE_CHAPTER_ORDER",
                "精选内容包 lesson 顺序重复",
            ));
        }
        let file = lesson_ref.file.as_deref().ok_or_else(|| {
            AppError::new(
                "ANNOTATED_CHAPTER_FILE_MISSING",
                "精选内容包 lesson 缺少 file",
            )
        })?;
        let file = safe_relative_path(file)?;
        let bytes = source.files.get(&file).ok_or_else(|| {
            AppError::new(
                "ANNOTATED_CHAPTER_FILE_MISSING",
                format!("找不到 lesson 文件：{file}"),
            )
        })?;
        let lesson: RawLesson = serde_json::from_slice(bytes).map_err(|error| {
            AppError::new(
                "ANNOTATED_INVALID_CHAPTER_JSON",
                format!("lesson {file} 无效：{error}"),
            )
        })?;
        let raw_segments = lesson.segments.ok_or_else(|| {
            AppError::new(
                "ANNOTATED_SEGMENTS_MISSING",
                "精选内容包 lesson 缺少 segments",
            )
        })?;
        let mut segments = Vec::with_capacity(raw_segments.len());
        let mut segment_orders = HashSet::new();
        for (segment_index, raw_segment) in raw_segments.iter().enumerate() {
            let segment_id = raw_segment
                .id
                .clone()
                .unwrap_or_else(|| format!("{chapter_id}-seg-{:03}", segment_index + 1));
            let segment_order = raw_segment.order.unwrap_or(segment_index as i64 + 1);
            if !all_segment_ids.insert(segment_id) {
                return Err(AppError::new(
                    "ANNOTATED_DUPLICATE_SEGMENT_ID",
                    "精选内容包 Segment ID 重复",
                ));
            }
            if !segment_orders.insert(segment_order) {
                return Err(AppError::new(
                    "ANNOTATED_DUPLICATE_SEGMENT_ORDER",
                    "精选内容包 Segment 顺序重复",
                ));
            }
            if raw_segment.text.trim().is_empty() {
                return Err(AppError::new(
                    "ANNOTATED_TEXT_EMPTY",
                    "Segment text 不能为空",
                ));
            }
            segments.push(NormalizedSegment {
                order: segment_order,
                text: raw_segment.text.clone(),
                translation: raw_segment.translation.clone(),
                tokens: normalize_segment_tokens_with_legacy_neutral_tone(raw_segment)?,
            });
        }
        chapters.push(NormalizedChapter {
            order,
            collection: lesson_ref
                .source_book
                .clone()
                .or(lesson.source_book.clone()),
            title: lesson_ref
                .display_title
                .clone()
                .or(lesson.display_title.clone())
                .or(lesson_ref.source_chapter.clone())
                .or(lesson.source_chapter.clone())
                .unwrap_or_else(|| format!("第{}章", order)),
            subtitle: lesson_ref
                .source_chapter
                .clone()
                .or(lesson.source_chapter.clone()),
            segments,
        });
    }
    chapters.sort_by_key(|chapter| chapter.order);
    Ok(NormalizedDocument {
        dataset_id: manifest.id.clone(),
        title,
        author: None,
        dynasty: None,
        edition: None,
        pronunciation_mode: "reference".to_string(),
        chapters,
        warnings: vec![issue(
            "LEGACY_READER_SELECTED_FORMAT",
            "检测到阅读端精选内容包，已兼容为参考注音导入",
            None,
            None,
        )],
    })
}

fn normalize_segment_tokens(segment: &RawSegment) -> AppResult<Vec<NormalizedToken>> {
    let graphemes = UnicodeSegmentation::graphemes(segment.text.as_str(), true)
        .map(str::to_string)
        .collect::<Vec<_>>();
    let Some(raw_tokens) = segment.tokens.as_ref() else {
        let pinyin_source = segment
            .pinyin_numeric
            .as_deref()
            .or(segment.pinyin_tone_marks.as_deref());
        let syllables = pinyin_source
            .map(parse_pinyin)
            .transpose()?
            .unwrap_or_default();
        let han_count = graphemes
            .iter()
            .filter(|token| token.chars().any(text_service::is_han_character))
            .count();
        if han_count != syllables.len() {
            return Err(AppError::new(
                "ALIGNMENT_ERROR",
                format!(
                    "文字与拼音无法严格对齐：文字“{}”，拼音“{}”",
                    segment.text,
                    pinyin_source.unwrap_or("")
                ),
            ));
        }
        let mut cursor = 0;
        return Ok(graphemes
            .into_iter()
            .map(|text| {
                let pinyin = if text.chars().any(text_service::is_han_character) {
                    let value = syllables[cursor].clone();
                    cursor += 1;
                    Some(value)
                } else {
                    None
                };
                NormalizedToken { text, pinyin }
            })
            .collect());
    };

    let joined = raw_tokens
        .iter()
        .map(|token| token.text.as_str())
        .collect::<String>();
    if joined != segment.text {
        return Err(AppError::new(
            "TOKEN_TEXT_MISMATCH",
            format!("tokens 拼接结果与 text 不一致：{}", segment.text),
        ));
    }
    let mut result = Vec::with_capacity(raw_tokens.len());
    for raw in raw_tokens {
        if UnicodeSegmentation::graphemes(raw.text.as_str(), true).count() != 1 {
            return Err(AppError::new(
                "TOKEN_TEXT_MISMATCH",
                "每个 token 必须对应一个 Unicode Grapheme",
            ));
        }
        let pinyin_source = raw
            .pinyin_numeric
            .as_deref()
            .or(raw.pinyin_tone_marks.as_deref());
        let is_han = raw.text.chars().any(text_service::is_han_character);
        let pinyin = pinyin_source
            .map(parse_pinyin)
            .transpose()?
            .map(|values| {
                if values.len() == 1 {
                    Ok(values[0].clone())
                } else {
                    Err(AppError::new(
                        "PINYIN_TOKEN_COUNT_MISMATCH",
                        format!("token “{}” 必须对应一个拼音音节", raw.text),
                    ))
                }
            })
            .transpose()?;
        if !is_han && pinyin.is_some() {
            return Err(AppError::new(
                "INVALID_PUNCTUATION_PINYIN",
                format!("标点或空格 token “{}” 不应有拼音", raw.text),
            ));
        }
        result.push(NormalizedToken {
            text: raw.text.clone(),
            pinyin: if is_han { pinyin } else { None },
        });
    }
    Ok(result)
}

/// The original reader-selected package used an unmarked syllable such as
/// `de` for neutral tone. Keep the formal v1 importer strict, but normalize
/// this legacy representation at the adapter boundary instead of teaching
/// the rest of the pronunciation pipeline about the old format.
fn normalize_segment_tokens_with_legacy_neutral_tone(
    segment: &RawSegment,
) -> AppResult<Vec<NormalizedToken>> {
    let mut adapted = segment.clone();
    if let Some(tokens) = adapted.tokens.as_mut() {
        for token in tokens {
            if let Some(pinyin) = token.pinyin_numeric.as_mut() {
                if is_unmarked_ascii_syllable(pinyin) {
                    pinyin.push('5');
                }
            }
            if let Some(pinyin) = token.pinyin_tone_marks.as_mut() {
                if is_unmarked_ascii_syllable(pinyin) {
                    pinyin.push('5');
                }
            }
        }
    }
    normalize_segment_tokens(&adapted)
}

fn is_unmarked_ascii_syllable(value: &str) -> bool {
    !value.is_empty()
        && value
            .chars()
            .all(|character| character.is_ascii_lowercase() || character == 'v')
}

fn parse_pinyin(value: &str) -> AppResult<Vec<String>> {
    let mut syllables = Vec::new();
    let mut current = String::new();
    for character in value.trim().chars() {
        if character.is_ascii_digit() {
            if current.is_empty() {
                return Err(AppError::new("PINYIN_INVALID", "拼音声调前缺少音节"));
            }
            current.push(character);
            syllables.push(normalize_syllable(&current)?);
            current.clear();
        } else if character.is_ascii_alphabetic() || is_tone_mark(character) || character == 'ü' {
            current.push(character);
        } else if character.is_whitespace()
            || is_pinyin_separator(character)
            || text_service::is_han_character(character)
        {
            if !current.is_empty() {
                syllables.push(normalize_syllable(&current)?);
                current.clear();
            }
        } else {
            return Err(AppError::new("PINYIN_INVALID", "拼音包含无法识别的字符"));
        }
    }
    if !current.is_empty() {
        syllables.push(normalize_syllable(&current)?);
    }
    if syllables.is_empty() && !value.trim().is_empty() {
        return Err(AppError::new("PINYIN_INVALID", "拼音不能为空或无法解析"));
    }
    Ok(syllables)
}

fn normalize_syllable(value: &str) -> AppResult<String> {
    if value.is_empty() {
        return Err(AppError::new("PINYIN_INVALID", "拼音音节为空"));
    }
    if let Some(last) = value.chars().last() {
        if last.is_ascii_digit() {
            let letters = &value[..value.len() - last.len_utf8()];
            let tone = last.to_digit(10).unwrap_or_default() as u8;
            if (1..=5).contains(&tone)
                && !letters.is_empty()
                && letters
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte == b'v')
            {
                return Ok(format!("{letters}{tone}"));
            }
        }
    }
    let mut base = String::new();
    let mut tone = None;
    for character in value.chars() {
        if let Some((replacement, mark_tone)) = tone_mark_value(character) {
            base.push(replacement);
            tone = Some(mark_tone);
        } else if character == 'ü' {
            base.push('v');
        } else if character.is_ascii_lowercase() {
            base.push(character);
        } else {
            return Err(AppError::new(
                "PINYIN_INVALID",
                format!("无法识别拼音音节：{value}"),
            ));
        }
    }
    let tone = tone.ok_or_else(|| {
        AppError::new(
            "PINYIN_INVALID",
            format!("拼音必须带数字声调或声调符号：{value}"),
        )
    })?;
    Ok(format!("{base}{tone}"))
}

fn tone_mark_value(character: char) -> Option<(char, u8)> {
    Some(match character {
        'ā' => ('a', 1),
        'á' => ('a', 2),
        'ǎ' => ('a', 3),
        'à' => ('a', 4),
        'ē' => ('e', 1),
        'é' => ('e', 2),
        'ě' => ('e', 3),
        'è' => ('e', 4),
        'ī' => ('i', 1),
        'í' => ('i', 2),
        'ǐ' => ('i', 3),
        'ì' => ('i', 4),
        'ō' => ('o', 1),
        'ó' => ('o', 2),
        'ǒ' => ('o', 3),
        'ò' => ('o', 4),
        'ū' => ('u', 1),
        'ú' => ('u', 2),
        'ǔ' => ('u', 3),
        'ù' => ('u', 4),
        'ǖ' => ('v', 1),
        'ǘ' => ('v', 2),
        'ǚ' => ('v', 3),
        'ǜ' => ('v', 4),
        _ => return None,
    })
}

fn is_tone_mark(character: char) -> bool {
    tone_mark_value(character).is_some()
}

fn is_pinyin_separator(character: char) -> bool {
    character.is_ascii_punctuation()
        || matches!(
            character,
            '，' | '。'
                | '、'
                | '；'
                | '：'
                | '！'
                | '？'
                | '“'
                | '”'
                | '‘'
                | '’'
                | '（'
                | '）'
                | '《'
                | '》'
                | '…'
                | '—'
                | '·'
        )
}

fn effective_mode(mode: &str, trust: bool) -> AppResult<String> {
    match mode {
        "authoritative" => Ok("authoritative".to_string()),
        "reference" if trust => Ok("authoritative".to_string()),
        "reference" => Ok("reference".to_string()),
        _ => Err(AppError::new(
            "ANNOTATED_PRONUNCIATION_MODE_INVALID",
            "无效注音模式",
        )),
    }
}

fn preflight_from_document(
    document: &NormalizedDocument,
    trust_imported_pronunciation: bool,
) -> AnnotatedImportPreflight {
    let mut han_count = 0_i64;
    let mut covered = 0_i64;
    let mut segment_count = 0_i64;
    let mut translation_count = 0_i64;
    for chapter in &document.chapters {
        for segment in &chapter.segments {
            segment_count += 1;
            if segment.translation.is_some() {
                translation_count += 1;
            }
            for token in &segment.tokens {
                if token.text.chars().any(text_service::is_han_character) {
                    han_count += 1;
                    if token.pinyin.is_some() {
                        covered += 1;
                    }
                }
            }
        }
    }
    let percentage = if han_count == 0 {
        0.0
    } else {
        (covered as f64 / han_count as f64) * 100.0
    };
    let effective = effective_mode(&document.pronunciation_mode, trust_imported_pronunciation)
        .unwrap_or_else(|_| "reference".to_string());
    let mut warnings = document.warnings.clone();
    if document
        .chapters
        .iter()
        .flat_map(|chapter| chapter.segments.iter())
        .any(|segment| segment.translation.is_none())
    {
        warnings.push(issue(
            "TRANSLATION_MISSING",
            "部分 Segment 没有现代汉语解释",
            None,
            None,
        ));
    }
    if document.pronunciation_mode == "reference" && !trust_imported_pronunciation {
        warnings.push(issue(
            "REFERENCE_PRONUNCIATION",
            "当前注音为参考注音，不会自动强制进入 TTS",
            None,
            None,
        ));
    }
    let max_han = text_service::MAX_BOOK_HAN_CHARACTERS as i64;
    let can_import = han_count > 0 && han_count <= max_han;
    let mut errors = Vec::new();
    if han_count == 0 {
        errors.push(issue("TEXT_EMPTY", "导入文件没有汉字正文", None, None));
    }
    if han_count > max_han {
        errors.push(issue(
            "PROJECT_TEXT_LIMIT_EXCEEDED",
            &format!(
                "当前版本单个项目最多支持{}个汉字；格式本身没有此限制",
                text_service::MAX_BOOK_HAN_CHARACTERS
            ),
            None,
            None,
        ));
    }
    AnnotatedImportPreflight {
        can_import,
        format: FORMAT.to_string(),
        format_version: FORMAT_VERSION.to_string(),
        title: Some(document.title.clone()),
        chapter_count: document.chapters.len() as i64,
        segment_count,
        han_character_count: han_count,
        pinyin_covered_han_count: covered,
        pinyin_coverage_percent: percentage,
        translation_segment_count: translation_count,
        pronunciation_mode: document.pronunciation_mode.clone(),
        effective_pronunciation_mode: effective,
        warnings,
        errors,
    }
}

fn build_warnings(manifest: &RawManifest, legacy: bool) -> Vec<AnnotatedImportIssue> {
    let mut warnings = Vec::new();
    if legacy {
        warnings.push(issue(
            "LEGACY_FORMAT",
            "检测到旧版已注音数据，已按 reference 模式兼容导入",
            None,
            None,
        ));
    }
    if manifest.sources.is_none() {
        warnings.push(issue(
            "SOURCE_METADATA_MISSING",
            "未提供来源元数据",
            None,
            None,
        ));
    }
    warnings
}

fn read_source_files(path: &str) -> AppResult<SourceFiles> {
    let path_ref = Path::new(path);
    if !path_ref.is_file() {
        return Err(AppError::new("FILE_IO_ERROR", "已注音古籍文件不存在"));
    }
    let extension = path_ref
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if extension == "zip" {
        let file = File::open(path_ref).map_err(AppError::from)?;
        let compressed_size = file.metadata().map_err(AppError::from)?.len();
        if compressed_size > MAX_ZIP_COMPRESSED_BYTES {
            return Err(AppError::new("ANNOTATED_ZIP_TOO_LARGE", "ZIP 文件过大"));
        }
        let mut archive = ZipArchive::new(file)
            .map_err(|error| AppError::new("ANNOTATED_ZIP_INVALID", error.to_string()))?;
        if archive.len() > MAX_ZIP_FILES {
            return Err(AppError::new(
                "ANNOTATED_ZIP_TOO_MANY_FILES",
                "ZIP 文件数量过多",
            ));
        }
        let mut raw_files = HashMap::new();
        let mut expanded = 0_u64;
        for index in 0..archive.len() {
            let mut entry = archive
                .by_index(index)
                .map_err(|error| AppError::new("ANNOTATED_ZIP_INVALID", error.to_string()))?;
            let normalized = safe_relative_path(entry.name())?;
            if entry.is_dir() {
                continue;
            }
            if entry
                .unix_mode()
                .is_some_and(|mode| mode & 0o170000 == 0o120000)
            {
                return Err(AppError::new(
                    "ANNOTATED_ZIP_UNSAFE_PATH",
                    "ZIP 不允许符号链接",
                ));
            }
            expanded = expanded.saturating_add(entry.size());
            if expanded > MAX_ZIP_EXPANDED_BYTES {
                return Err(AppError::new("ANNOTATED_ZIP_TOO_LARGE", "ZIP 解压内容过大"));
            }
            let mut bytes = Vec::new();
            entry.read_to_end(&mut bytes).map_err(AppError::from)?;
            raw_files.insert(normalized, bytes);
        }
        Ok(SourceFiles {
            files: strip_common_root(raw_files)?,
        })
    } else if extension == "json" {
        let bytes = fs::read(path_ref).map_err(AppError::from)?;
        if bytes.len() as u64 > MAX_ZIP_EXPANDED_BYTES {
            return Err(AppError::new("ANNOTATED_JSON_TOO_LARGE", "JSON 文件过大"));
        }
        let mut files = HashMap::new();
        files.insert("manifest.json".to_string(), bytes);
        Ok(SourceFiles { files })
    } else {
        Err(AppError::new(
            "ANNOTATED_UNSUPPORTED_FILE",
            "已注音古籍只支持 .json 或 .zip",
        ))
    }
}

fn strip_common_root(files: HashMap<String, Vec<u8>>) -> AppResult<HashMap<String, Vec<u8>>> {
    if files.contains_key("manifest.json") {
        return Ok(files);
    }
    let manifest = files
        .keys()
        .find(|name| name.ends_with("/manifest.json"))
        .cloned()
        .ok_or_else(|| AppError::new("ANNOTATED_MANIFEST_MISSING", "ZIP 中缺少 manifest.json"))?;
    let prefix = manifest.trim_end_matches("manifest.json").to_string();
    Ok(files
        .into_iter()
        .filter_map(|(name, bytes)| {
            name.strip_prefix(&prefix)
                .map(|value| (value.to_string(), bytes))
        })
        .collect())
}

fn safe_relative_path(value: &str) -> AppResult<String> {
    if value.trim().is_empty() || value.contains('\\') || value.contains(':') {
        return Err(AppError::new(
            "ANNOTATED_UNSAFE_PATH",
            "资源路径不是安全的相对路径",
        ));
    }
    let path = Path::new(value);
    if path.is_absolute() {
        return Err(AppError::new("ANNOTATED_UNSAFE_PATH", "不允许绝对路径"));
    }
    let mut parts = Vec::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => parts.push(part.to_string_lossy().to_string()),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(AppError::new("ANNOTATED_UNSAFE_PATH", "不允许路径穿越"));
            }
        }
    }
    if parts.is_empty() {
        return Err(AppError::new("ANNOTATED_UNSAFE_PATH", "资源路径为空"));
    }
    Ok(parts.join("/"))
}

fn issue(
    code: &str,
    message: &str,
    chapter_id: Option<String>,
    segment_id: Option<String>,
) -> AnnotatedImportIssue {
    AnnotatedImportIssue {
        code: code.to_string(),
        message: message.to_string(),
        chapter_id,
        segment_id,
    }
}

fn now_rfc3339() -> AppResult<String> {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .map_err(|error| AppError::new("DB_ERROR", error.to_string()))
}

fn transaction_error(error: sqlx::Error) -> AppError {
    AppError::new("DB_TRANSACTION_FAILED", error.to_string())
}

#[cfg(test)]
mod tests {
    use super::{
        get_tts_preflight, import_book, normalize_segment_tokens, normalize_syllable, parse_pinyin,
        parse_source, preflight_from_document,
    };
    use crate::db::Database;
    use std::{fs, io::Write};
    use uuid::Uuid;

    #[test]
    fn converts_numeric_and_tone_mark_pinyin() {
        assert_eq!(
            parse_pinyin("xi1 zai4 黄帝，sheng1").unwrap(),
            vec!["xi1", "zai4", "sheng1"]
        );
        assert_eq!(normalize_syllable("wù").unwrap(), "wu4");
        assert_eq!(normalize_syllable("nǚ").unwrap(), "nv3");
        assert_eq!(normalize_syllable("nüè").unwrap(), "nve4");
        assert_eq!(normalize_syllable("lǜ").unwrap(), "lv4");
    }

    #[test]
    fn rejects_unmarked_or_uppercase_pinyin() {
        assert!(normalize_syllable("wu").is_err());
        assert!(normalize_syllable("Wu4").is_err());
    }

    #[test]
    fn validates_token_alignment_for_supplementary_cjk_and_punctuation() {
        let segment = super::RawSegment {
            text: "甲𫏋，乙".to_string(),
            tokens: Some(vec![
                super::RawToken {
                    text: "甲".to_string(),
                    pinyin_numeric: Some("jia3".to_string()),
                    pinyin_tone_marks: None,
                },
                super::RawToken {
                    text: "𫏋".to_string(),
                    pinyin_numeric: Some("jiao1".to_string()),
                    pinyin_tone_marks: None,
                },
                super::RawToken {
                    text: "，".to_string(),
                    pinyin_numeric: None,
                    pinyin_tone_marks: None,
                },
                super::RawToken {
                    text: "乙".to_string(),
                    pinyin_numeric: Some("yi3".to_string()),
                    pinyin_tone_marks: None,
                },
            ]),
            ..Default::default()
        };
        let tokens = normalize_segment_tokens(&segment).expect("token alignment");
        assert_eq!(tokens.len(), 4);
        assert_eq!(tokens[1].text, "𫏋");
        assert_eq!(tokens[2].pinyin, None);
        assert!(normalize_segment_tokens(&super::RawSegment {
            text: "甲乙".to_string(),
            pinyin_numeric: Some("jia3".to_string()),
            ..Default::default()
        })
        .is_err());
    }

    fn sample_document() -> String {
        r#"{
          "format":"ancient-annotated-book",
          "format_version":"1.0",
          "book":{"id":"generic-001","title":"通用古籍测试","author":null},
          "pronunciation":{"mode":"authoritative"},
          "chapters":[{
            "id":"chapter-one","order":1,"collection":"测试","title":"第一篇","subtitle":"小节",
            "segments":[{
              "id":"segment-one","order":1,"text":"恶寒。",
              "tokens":[
                {"text":"恶","pinyin_numeric":"wu4"},
                {"text":"寒","pinyin_numeric":"han2"},
                {"text":"。","pinyin_numeric":null}
              ],
              "translation":"怕冷。"
            }]
          }]
        }"#
        .to_string()
    }

    #[test]
    fn imports_single_json_as_authoritative_without_analyzer() {
        let path = std::env::temp_dir().join(format!("annotated-book-{}.json", Uuid::now_v7()));
        fs::write(&path, sample_document()).expect("annotated json");
        let database_path =
            std::env::temp_dir().join(format!("annotated-db-{}.sqlite", Uuid::now_v7()));
        let database =
            tauri::async_runtime::block_on(Database::open(&database_path)).expect("database");
        tauri::async_runtime::block_on(async {
            let result = import_book(&database, path.to_str().unwrap(), false)
                .await
                .expect("import");
            assert_eq!(result.segment_count, 1);
            let book = crate::services::book_service::get_book(&database, &result.book.book.id)
                .await
                .expect("book");
            assert_eq!(
                book.book.import_format.as_deref(),
                Some("ancient-annotated-book")
            );
            assert_eq!(
                book.book.import_pronunciation_mode.as_deref(),
                Some("authoritative")
            );
            let segments = crate::services::book_service::list_segments(
                &database,
                &result.chapters[0].id,
                0,
                10,
            )
            .await
            .expect("segments");
            assert_eq!(segments.items[0].translation.as_deref(), Some("怕冷。"));
            let annotations = crate::services::pronunciation_service::list_annotations(
                &database,
                &segments.items[0].id,
            )
            .await
            .expect("annotations");
            assert_eq!(annotations.len(), 2);
            assert!(annotations
                .iter()
                .all(|item| item.source.as_deref() == Some("imported_authoritative")));
            assert!(annotations
                .iter()
                .all(|item| item.review_status == "confirmed"));
            let forced =
                crate::services::pronunciation_service::build_effective_forced_pronunciations(
                    &database,
                    &segments.items[0].id,
                )
                .await
                .expect("forced pronunciation");
            assert_eq!(forced.len(), 1);
            assert_eq!(forced[0].surface_text, "恶寒");
            assert_eq!(forced[0].pinyin, "wu4 han2");
            let tts = get_tts_preflight(&database, &result.book.book.id)
                .await
                .expect("tts preflight");
            assert!(tts.can_generate_strict);
            assert_eq!(tts.missing_han_count, 0);
        });
        let _ = fs::remove_file(path);
        let _ = fs::remove_file(database_path);
    }

    #[test]
    fn imports_formal_zip_with_external_chapter_file() {
        let path = std::env::temp_dir().join(format!("annotated-formal-{}.zip", Uuid::now_v7()));
        let file = fs::File::create(&path).expect("zip");
        let mut writer = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default();
        writer
            .start_file("book/manifest.json", options)
            .expect("manifest entry");
        writer
            .write_all(
                r#"{
                  "format":"ancient-annotated-book",
                  "format_version":"1.0",
                  "book":{"id":"zip-book","title":"ZIP 测试"},
                  "pronunciation":{"mode":"reference"},
                  "chapters":[{"id":"zip-chapter","order":1,"title":"第一篇","file":"chapters/001.json"}]
                }"#
                .as_bytes(),
            )
            .expect("manifest");
        writer
            .start_file("book/chapters/001.json", options)
            .expect("chapter entry");
        writer
            .write_all(
                r#"{
                  "id":"zip-chapter","order":1,"title":"第一篇",
                  "segments":[{"id":"zip-segment","order":1,"text":"行","pinyin_numeric":"xing2"}]
                }"#
                .as_bytes(),
            )
            .expect("chapter");
        writer.finish().expect("finish zip");

        let document = parse_source(path.to_str().unwrap()).expect("formal zip");
        assert_eq!(document.title, "ZIP 测试");
        assert_eq!(document.pronunciation_mode, "reference");
        assert_eq!(document.chapters.len(), 1);
        assert_eq!(
            document.chapters[0].segments[0].tokens[0].pinyin.as_deref(),
            Some("xing2")
        );
        let _ = fs::remove_file(path);
    }

    #[test]
    fn adapts_reader_selected_content_zip_as_reference() {
        let path = std::env::temp_dir().join(format!("reader-selected-{}.zip", Uuid::now_v7()));
        let file = fs::File::create(&path).expect("zip");
        let mut writer = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default();
        writer
            .start_file("reader/manifest.json", options)
            .expect("manifest entry");
        writer
            .write_all(
                r#"{
                  "format":"ancient-medical-reader-selected-content",
                  "format_version":"1.0",
                  "id":"reader-selected-001","title":"精选古籍",
                  "lessons":[{"id":"day-01","day":1,"source_book":"素问","source_chapter":"上古天真论","display_title":"养生","file":"lessons/day-01.json"}]
                }"#
                .as_bytes(),
            )
            .expect("manifest");
        writer
            .start_file("reader/lessons/day-01.json", options)
            .expect("lesson entry");
        writer
            .write_all(
                r#"{
                  "id":"day-01","day":1,"source_book":"素问","source_chapter":"上古天真论","display_title":"养生",
                  "segments":[{"id":"day-01-seg-01","order":1,"text":"恶气。","tokens":[{"text":"恶","pinyin":"e4"},{"text":"气","pinyin":"qi4"},{"text":"。","pinyin":null}],"translation":"不良之气。"}]
                }"#
                .as_bytes(),
            )
            .expect("lesson");
        writer.finish().expect("finish zip");

        let document = parse_source(path.to_str().unwrap()).expect("reader selected zip");
        assert_eq!(document.title, "精选古籍");
        assert_eq!(document.pronunciation_mode, "reference");
        assert_eq!(document.chapters[0].collection.as_deref(), Some("素问"));
        assert_eq!(document.chapters[0].title, "养生");
        assert_eq!(
            document.chapters[0].segments[0].tokens[0].pinyin.as_deref(),
            Some("e4")
        );
        assert_eq!(
            document.chapters[0].segments[0].translation.as_deref(),
            Some("不良之气。")
        );
        assert_eq!(document.warnings[0].code, "LEGACY_READER_SELECTED_FORMAT");
        let _ = fs::remove_file(path);
    }

    #[test]
    #[ignore = "requires the real reader-selected dataset path"]
    fn validates_real_reader_selected_dataset_when_requested() {
        let path = std::env::var("ANCIENT_TTS_REAL_READER_SELECTED_DATASET")
            .expect("ANCIENT_TTS_REAL_READER_SELECTED_DATASET");
        let document = parse_source(&path).expect("reader-selected dataset");
        let preflight = preflight_from_document(&document, false);
        assert_eq!(document.title, "黄帝内经精选");
        assert_eq!(document.chapters.len(), 12);
        assert_eq!(preflight.segment_count, 43);
        assert_eq!(preflight.han_character_count, 1802);
        assert_eq!(preflight.pinyin_covered_han_count, 1802);
        assert_eq!(preflight.translation_segment_count, 43);
        assert_eq!(preflight.pinyin_coverage_percent, 100.0);
        assert_eq!(document.pronunciation_mode, "reference");
    }

    #[test]
    fn adapts_legacy_dataset_as_reference() {
        let path = std::env::temp_dir().join(format!("annotated-legacy-{}.zip", Uuid::now_v7()));
        let file = fs::File::create(&path).expect("zip");
        let mut writer = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default();
        writer
            .start_file("manifest.json", options)
            .expect("manifest entry");
        writer
            .write_all(
                r#"{
                  "dataset":"legacy-medical-text","version":"0.1.0","language":"zh-CN",
                  "chapters":[{"id":"legacy-chapter","order":1,"collection":"素问","title":"旧格式","reader_subtitle":"说明","file":"chapters/001.json"}]
                }"#
                .as_bytes(),
            )
            .expect("manifest");
        writer
            .start_file("chapters/001.json", options)
            .expect("chapter entry");
        writer
            .write_all(
                r#"{
                  "id":"legacy-chapter","order":1,"collection":"素问","title":"旧格式",
                  "segments":[{"id":"legacy-segment","order":1,"text":"行。","pinyin_tone_marks":"xíng。"}]
                }"#
                .as_bytes(),
            )
            .expect("chapter");
        writer.finish().expect("finish zip");

        let document = parse_source(path.to_str().unwrap()).expect("legacy zip");
        assert_eq!(document.title, "legacy-medical-text");
        assert_eq!(document.pronunciation_mode, "reference");
        assert_eq!(document.warnings[0].code, "LEGACY_FORMAT");
        assert_eq!(document.chapters[0].subtitle.as_deref(), Some("说明"));
        assert_eq!(
            document.chapters[0].segments[0].tokens[0].pinyin.as_deref(),
            Some("xing2")
        );
        let _ = fs::remove_file(path);
    }

    #[test]
    fn reference_json_requires_explicit_trust_to_force_tts() {
        let path =
            std::env::temp_dir().join(format!("annotated-reference-{}.json", Uuid::now_v7()));
        let reference = sample_document().replace("\"authoritative\"", "\"reference\"");
        fs::write(&path, reference).expect("annotated json");
        let database_path =
            std::env::temp_dir().join(format!("annotated-reference-db-{}.sqlite", Uuid::now_v7()));
        let database =
            tauri::async_runtime::block_on(Database::open(&database_path)).expect("database");
        tauri::async_runtime::block_on(async {
            let result = import_book(&database, path.to_str().unwrap(), false)
                .await
                .expect("import");
            let segments = crate::services::book_service::list_segments(
                &database,
                &result.chapters[0].id,
                0,
                10,
            )
            .await
            .expect("segments");
            let forced =
                crate::services::pronunciation_service::build_effective_forced_pronunciations(
                    &database,
                    &segments.items[0].id,
                )
                .await
                .expect("forced");
            assert!(forced.is_empty());
            assert_eq!(segments.items[0].status, "needs_review");
        });
        let _ = fs::remove_file(path);
        let _ = fs::remove_file(database_path);
    }

    #[test]
    fn rejects_zip_slip_before_import() {
        let path = std::env::temp_dir().join(format!("annotated-unsafe-{}.zip", Uuid::now_v7()));
        let file = fs::File::create(&path).expect("zip");
        let mut writer = zip::ZipWriter::new(file);
        writer
            .start_file("../manifest.json", zip::write::SimpleFileOptions::default())
            .expect("entry");
        writer.write_all(b"{}").expect("zip data");
        writer.finish().expect("finish zip");
        let error = parse_source(path.to_str().unwrap()).expect_err("unsafe zip");
        assert_eq!(error.code, "ANNOTATED_UNSAFE_PATH");
        let _ = fs::remove_file(path);
    }

    #[test]
    fn rejects_duplicate_segment_ids_across_chapters() {
        let path =
            std::env::temp_dir().join(format!("annotated-duplicate-{}.json", Uuid::now_v7()));
        fs::write(
            &path,
            r#"{
              "format":"ancient-annotated-book","format_version":"1.0",
              "book":{"id":"duplicate-book","title":"重复 ID 测试"},
              "pronunciation":{"mode":"reference"},
              "chapters":[
                {"id":"c1","order":1,"title":"第一章","segments":[{"id":"same","order":1,"text":"行","pinyin_numeric":"xing2"}]},
                {"id":"c2","order":2,"title":"第二章","segments":[{"id":"same","order":1,"text":"行","pinyin_numeric":"xing2"}]}
              ]
            }"#,
        )
        .expect("annotated json");
        let error = parse_source(path.to_str().unwrap()).expect_err("duplicate segment id");
        assert_eq!(error.code, "ANNOTATED_DUPLICATE_SEGMENT_ID");
        let _ = fs::remove_file(path);
    }

    #[test]
    #[ignore = "requires ANCIENT_TTS_REAL_ANNOTATED_DATASET to point to a real dataset archive"]
    fn validates_real_annotated_dataset_when_requested() {
        let Some(path) = std::env::var_os("ANCIENT_TTS_REAL_ANNOTATED_DATASET") else {
            return;
        };
        let path = path.to_string_lossy().to_string();
        let document = parse_source(&path).expect("real annotated dataset");
        let reference = preflight_from_document(&document, false);
        assert_eq!(
            reference.title.as_deref(),
            Some("huangdi-neijing-selected-v01")
        );
        assert_eq!(reference.chapter_count, 6);
        assert_eq!(reference.segment_count, 71);
        assert_eq!(reference.han_character_count, 3855);
        assert_eq!(reference.pinyin_covered_han_count, 3855);
        assert_eq!(reference.pinyin_coverage_percent, 100.0);
        assert_eq!(reference.translation_segment_count, 71);
        assert_eq!(reference.pronunciation_mode, "reference");
        assert_eq!(reference.effective_pronunciation_mode, "reference");
        assert!(reference.can_import);

        let trusted = preflight_from_document(&document, true);
        assert_eq!(trusted.effective_pronunciation_mode, "authoritative");

        let database_path =
            std::env::temp_dir().join(format!("annotated-real-db-{}.sqlite", Uuid::now_v7()));
        let database =
            tauri::async_runtime::block_on(Database::open(&database_path)).expect("database");
        tauri::async_runtime::block_on(async {
            let result = import_book(&database, &path, true)
                .await
                .expect("trusted real import");
            assert_eq!(result.segment_count, 71);
            let book = crate::services::book_service::get_book(&database, &result.book.book.id)
                .await
                .expect("book");
            assert_eq!(
                book.book.import_pronunciation_mode.as_deref(),
                Some("authoritative")
            );

            let first_page = crate::services::book_service::list_segments(
                &database,
                &result.chapters[0].id,
                0,
                100,
            )
            .await
            .expect("segments");
            assert_eq!(first_page.total, 17);
            assert_eq!(
                first_page.items[0].translation.as_deref(),
                Some("从前的黄帝，天资聪慧，幼年就表现出很强的理解和表达能力，成年后登上帝位。")
            );
            let first_annotations = crate::services::pronunciation_service::list_annotations(
                &database,
                &first_page.items[0].id,
            )
            .await
            .expect("annotations");
            assert!(first_annotations.iter().all(|annotation| {
                annotation.source.as_deref() == Some("imported_authoritative")
                    && annotation.review_status == "confirmed"
            }));

            let mut all_annotations = 0_i64;
            let mut chapter_page_count = 0;
            let mut found_forced_example = false;
            for chapter in &result.chapters {
                let page =
                    crate::services::book_service::list_segments(&database, &chapter.id, 0, 100)
                        .await
                        .expect("chapter segments");
                for segment in &page.items {
                    let annotations = crate::services::pronunciation_service::list_annotations(
                        &database,
                        &segment.id,
                    )
                    .await
                    .expect("segment annotations");
                    all_annotations += annotations.len() as i64;
                    if segment.original_text.contains("恶气") {
                        let forced = crate::services::pronunciation_service::build_effective_forced_pronunciations(
                            &database,
                            &segment.id,
                        )
                        .await
                        .expect("forced pronunciation");
                        assert!(forced.iter().any(|range| {
                            range.surface_text == "恶气不发" && range.pinyin == "e4 qi4 bu4 fa1"
                        }));
                        found_forced_example = true;
                    }
                }
                chapter_page_count += 1;
            }
            assert_eq!(all_annotations, 3855);
            assert_eq!(chapter_page_count, 6);
            assert!(found_forced_example);

            let tts = get_tts_preflight(&database, &result.book.book.id)
                .await
                .expect("tts preflight");
            assert!(tts.can_generate_strict);
            assert_eq!(tts.total_han_count, 3855);
            assert_eq!(tts.covered_han_count, 3855);
            assert_eq!(tts.missing_han_count, 0);
        });
        let _ = fs::remove_file(database_path);
    }
}
