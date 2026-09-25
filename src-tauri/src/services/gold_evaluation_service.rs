use crate::{
    db::Database,
    error::{AppError, AppResult},
    models::{Annotation, Segment},
    services::{book_service, pronunciation_service},
};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fs, path::Path};

#[derive(Debug, Deserialize)]
struct GoldFile {
    entries: Vec<GoldEntry>,
}

#[derive(Debug, Deserialize)]
struct GoldEntry {
    id: String,
    chapter: String,
    match_text: String,
    gold_pinyin: String,
    status: String,
    occurrence_count_in_chapter: i64,
}

#[derive(Debug, Serialize)]
struct BenchmarkSnapshot {
    benchmark_version: String,
    book_id: String,
    chapters: Vec<BenchmarkChapterSnapshot>,
}

#[derive(Debug, Serialize)]
struct BenchmarkChapterSnapshot {
    title: String,
    segments: Vec<BenchmarkSegmentSnapshot>,
}

#[derive(Debug, Serialize)]
struct BenchmarkSegmentSnapshot {
    segment_id: String,
    order_index: i64,
    text: String,
    tokens: Vec<crate::models::GraphemeToken>,
    predictions: Vec<Annotation>,
}

#[derive(Debug, Serialize, Clone)]
pub struct GoldEvaluationIssue {
    pub entry_id: String,
    pub chapter: String,
    pub match_text: String,
    pub segment_id: Option<String>,
    pub segment_order: Option<i64>,
    pub reason: String,
    pub observed_pinyin: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
pub struct GoldEvaluationReport {
    pub gold_entries: i64,
    pub hard_gold_entries: i64,
    pub review_required_entries: i64,
    pub expected_hard_occurrences: i64,
    pub detected_hard_occurrences: i64,
    pub default_exact_occurrences: i64,
    pub candidate_contains_gold_occurrences: i64,
    pub confirmed_target_exact_occurrences: i64,
    pub expected_review_occurrences: i64,
    pub detected_review_occurrences: i64,
    pub unmatched_annotation_count: i64,
    pub occurrence_count_mismatches: Vec<GoldEvaluationIssue>,
    pub issues: Vec<GoldEvaluationIssue>,
}

#[derive(Clone)]
struct SegmentSnapshot {
    segment: Segment,
    annotations: Vec<Annotation>,
}

pub async fn evaluate_book(
    database: &Database,
    book_id: &str,
    gold_path: &Path,
) -> AppResult<GoldEvaluationReport> {
    let chapters = book_service::list_chapters(database, book_id).await?;
    let mut snapshots_by_chapter = HashMap::new();
    let mut ordered_snapshots = Vec::new();
    for chapter_summary in chapters {
        let chapter_id = chapter_summary.chapter.id.clone();
        let mut snapshots = Vec::new();
        let mut offset = 0_i64;
        loop {
            let page = book_service::list_segments(database, &chapter_id, offset, 100).await?;
            let page_item_count = page.items.len();
            for segment in page.items {
                let annotations =
                    pronunciation_service::list_annotations(database, &segment.id).await?;
                snapshots.push(SegmentSnapshot {
                    segment,
                    annotations,
                });
            }
            offset += page_item_count as i64;
            if offset >= page.total {
                break;
            }
        }
        let title = chapter_summary.chapter.title.unwrap_or_default();
        ordered_snapshots.push((title.clone(), snapshots.clone()));
        snapshots_by_chapter.insert(title, snapshots);
    }

    write_snapshot_if_requested(book_id, &ordered_snapshots)?;
    // The production prediction snapshot is deliberately written before Gold
    // is loaded. This keeps hold-out prediction generation independent from
    // the evaluator and makes it harder to accidentally leak Gold data into
    // the production analysis path.
    if std::env::var("ANCIENT_TTS_BENCHMARK_SNAPSHOT_ONLY").as_deref() == Ok("1") {
        return Ok(GoldEvaluationReport {
            gold_entries: 0,
            hard_gold_entries: 0,
            review_required_entries: 0,
            expected_hard_occurrences: 0,
            detected_hard_occurrences: 0,
            default_exact_occurrences: 0,
            candidate_contains_gold_occurrences: 0,
            confirmed_target_exact_occurrences: 0,
            expected_review_occurrences: 0,
            detected_review_occurrences: 0,
            unmatched_annotation_count: 0,
            occurrence_count_mismatches: Vec::new(),
            issues: Vec::new(),
        });
    }
    let gold = load_gold(gold_path)?;

    let mut report = GoldEvaluationReport {
        gold_entries: gold.entries.len() as i64,
        hard_gold_entries: 0,
        review_required_entries: 0,
        expected_hard_occurrences: 0,
        detected_hard_occurrences: 0,
        default_exact_occurrences: 0,
        candidate_contains_gold_occurrences: 0,
        confirmed_target_exact_occurrences: 0,
        expected_review_occurrences: 0,
        detected_review_occurrences: 0,
        unmatched_annotation_count: 0,
        occurrence_count_mismatches: Vec::new(),
        issues: Vec::new(),
    };

    let mut matched_annotation_ids = HashMap::new();
    for entry in gold.entries {
        let Some(snapshots) = snapshots_by_chapter.get(&entry.chapter) else {
            return Err(AppError::new(
                "GOLD_CHAPTER_NOT_FOUND",
                format!(
                    "Gold 条目 {} 对应的 Chapter 不存在: {}",
                    entry.id, entry.chapter
                ),
            ));
        };
        let occurrences = find_occurrences(snapshots, &entry.match_text);
        if occurrences.len() as i64 != entry.occurrence_count_in_chapter {
            report
                .occurrence_count_mismatches
                .push(GoldEvaluationIssue {
                    entry_id: entry.id.clone(),
                    chapter: entry.chapter.clone(),
                    match_text: entry.match_text.clone(),
                    segment_id: None,
                    segment_order: None,
                    reason: format!(
                        "Gold 声明 {} 次，当前正文找到 {} 次",
                        entry.occurrence_count_in_chapter,
                        occurrences.len()
                    ),
                    observed_pinyin: None,
                });
        }

        let is_hard_gold = entry.status == "gold";
        if is_hard_gold {
            report.hard_gold_entries += 1;
            report.expected_hard_occurrences += occurrences.len() as i64;
        } else {
            report.review_required_entries += 1;
            report.expected_review_occurrences += occurrences.len() as i64;
        }

        for occurrence in occurrences {
            let annotation = occurrence.annotation;
            if is_hard_gold {
                if annotation.is_some() {
                    report.detected_hard_occurrences += 1;
                }
            } else if annotation.is_some() {
                report.detected_review_occurrences += 1;
            }

            let Some(annotation) = annotation else {
                report.issues.push(GoldEvaluationIssue {
                    entry_id: entry.id.clone(),
                    chapter: entry.chapter.clone(),
                    match_text: entry.match_text.clone(),
                    segment_id: Some(occurrence.segment_id),
                    segment_order: Some(occurrence.segment_order),
                    reason: "未找到同范围 Annotation".to_string(),
                    observed_pinyin: None,
                });
                continue;
            };
            matched_annotation_ids.insert(annotation.id.clone(), ());
            if !is_hard_gold {
                continue;
            }
            if annotation.default_pinyin.as_deref() == Some(entry.gold_pinyin.as_str()) {
                report.default_exact_occurrences += 1;
            }
            if annotation
                .candidate_pinyin
                .iter()
                .any(|candidate| candidate == &entry.gold_pinyin)
            {
                report.candidate_contains_gold_occurrences += 1;
            }
            if annotation.target_pinyin.as_deref() == Some(entry.gold_pinyin.as_str()) {
                report.confirmed_target_exact_occurrences += 1;
            } else if annotation.target_pinyin.is_some()
                || annotation.default_pinyin.as_deref() != Some(entry.gold_pinyin.as_str())
            {
                report.issues.push(GoldEvaluationIssue {
                    entry_id: entry.id.clone(),
                    chapter: entry.chapter.clone(),
                    match_text: entry.match_text.clone(),
                    segment_id: Some(occurrence.segment_id),
                    segment_order: Some(occurrence.segment_order),
                    reason: "Annotation 拼音与 Gold 不一致或尚未确认".to_string(),
                    observed_pinyin: annotation
                        .target_pinyin
                        .clone()
                        .or(annotation.default_pinyin.clone()),
                });
            }
        }
    }

    report.unmatched_annotation_count = snapshots_by_chapter
        .values()
        .flat_map(|snapshots| snapshots.iter())
        .flat_map(|snapshot| snapshot.annotations.iter())
        .filter(|annotation| !matched_annotation_ids.contains_key(&annotation.id))
        .count() as i64;
    Ok(report)
}

fn write_snapshot_if_requested(
    book_id: &str,
    chapters: &[(String, Vec<SegmentSnapshot>)],
) -> AppResult<()> {
    let Some(output) = std::env::var_os("ANCIENT_TTS_BENCHMARK_SNAPSHOT") else {
        return Ok(());
    };
    let snapshot = BenchmarkSnapshot {
        benchmark_version: std::env::var("ANCIENT_TTS_BENCHMARK_LABEL")
            .map(|label| {
                let benchmark_name = std::env::var("ANCIENT_TTS_BENCHMARK_NAME")
                    .unwrap_or_else(|_| "huangdi_neijing_v01".to_string());
                format!("{benchmark_name}-{label}")
            })
            .unwrap_or_else(|_| "huangdi_neijing_v01-baseline".to_string()),
        book_id: book_id.to_string(),
        chapters: chapters
            .iter()
            .map(|(title, segments)| BenchmarkChapterSnapshot {
                title: title.clone(),
                segments: segments
                    .iter()
                    .map(|snapshot| BenchmarkSegmentSnapshot {
                        segment_id: snapshot.segment.id.clone(),
                        order_index: snapshot.segment.order_index,
                        text: snapshot.segment.effective_text().to_string(),
                        tokens: pronunciation_service::grapheme_tokens(
                            snapshot.segment.effective_text(),
                        ),
                        predictions: snapshot.annotations.clone(),
                    })
                    .collect(),
            })
            .collect(),
    };
    let output = Path::new(&output);
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            AppError::new(
                "GOLD_EVALUATION_ERROR",
                format!("创建 benchmark 输出目录失败: {error}"),
            )
        })?;
    }
    let contents = serde_json::to_vec_pretty(&snapshot).map_err(|error| {
        AppError::new(
            "GOLD_EVALUATION_ERROR",
            format!("序列化 benchmark snapshot 失败: {error}"),
        )
    })?;
    fs::write(output, contents).map_err(|error| {
        AppError::new(
            "GOLD_EVALUATION_ERROR",
            format!("写入 benchmark snapshot 失败: {error}"),
        )
    })?;
    Ok(())
}

struct GoldOccurrence {
    segment_id: String,
    segment_order: i64,
    annotation: Option<Annotation>,
}

fn find_occurrences(snapshots: &[SegmentSnapshot], pattern: &str) -> Vec<GoldOccurrence> {
    let pattern_tokens = pronunciation_service::grapheme_tokens(pattern);
    if pattern_tokens.is_empty() {
        return Vec::new();
    }
    snapshots
        .iter()
        .flat_map(|snapshot| {
            let tokens = pronunciation_service::grapheme_tokens(snapshot.segment.effective_text());
            tokens
                .windows(pattern_tokens.len())
                .enumerate()
                .filter_map(|(start, window)| {
                    let surface = window
                        .iter()
                        .map(|token| token.text.as_str())
                        .collect::<String>();
                    if surface != pattern {
                        return None;
                    }
                    let end = start + pattern_tokens.len();
                    let annotation = snapshot
                        .annotations
                        .iter()
                        .find(|annotation| {
                            annotation.start_token == start
                                && annotation.end_token == end
                                && annotation.surface_text == pattern
                        })
                        .cloned();
                    Some(GoldOccurrence {
                        segment_id: snapshot.segment.id.clone(),
                        segment_order: snapshot.segment.order_index,
                        annotation,
                    })
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

fn load_gold(path: &Path) -> AppResult<GoldFile> {
    let contents = fs::read_to_string(path).map_err(|error| {
        AppError::new(
            "GOLD_EVALUATION_ERROR",
            format!("读取 Gold JSON 失败: {error}"),
        )
    })?;
    serde_json::from_str(&contents).map_err(|error| {
        AppError::new(
            "GOLD_EVALUATION_ERROR",
            format!("Gold JSON 格式无效: {error}"),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::evaluate_book;
    use crate::{
        db::Database,
        services::{book_service, pronunciation_service},
        worker::WorkerClient,
    };
    use serde_json::json;
    use std::{
        path::{Path, PathBuf},
        process::Command,
        time::Duration,
    };
    use uuid::Uuid;

    fn temp_db() -> Database {
        let path =
            std::env::temp_dir().join(format!("ancient-medical-gold-{}.sqlite", Uuid::now_v7()));
        tauri::async_runtime::block_on(Database::open(path)).expect("database should initialize")
    }

    fn spawn_worker() -> Option<WorkerClient> {
        let project_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
        let worker_script = project_root.join("worker/main.py");
        let worker_project = project_root.join("worker");
        let log_path =
            std::env::temp_dir().join(format!("ancient-medical-gold-{}.log", Uuid::now_v7()));
        if Command::new("uv").arg("--version").output().is_ok() {
            return WorkerClient::spawn_with_env(
                Path::new("uv"),
                &[
                    PathBuf::from("run"),
                    PathBuf::from("--project"),
                    worker_project,
                    PathBuf::from("python"),
                    worker_script,
                ],
                &log_path,
                Duration::from_secs(10),
                &[],
            )
            .ok();
        }
        None
    }

    #[test]
    fn evaluates_realbook_annotations_against_gold_json() {
        let Some(mut worker) = spawn_worker() else {
            eprintln!("Gold evaluation skipped because uv is unavailable");
            return;
        };
        let database = temp_db();
        let text_path = std::env::var_os("ANCIENT_TTS_BENCHMARK_TEXT")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../realbooks/huangdi_neijing_test_v01.txt")
            });
        let gold_path = std::env::var_os("ANCIENT_TTS_BENCHMARK_GOLD")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../realbooks/huangdi_neijing_gold_v01.json")
            });
        tauri::async_runtime::block_on(async {
            let text_path_string = text_path.to_string_lossy().to_string();
            let imported = book_service::import_txt_book(&database, &text_path_string, None)
                .await
                .expect("realbook import");
            for chapter in book_service::list_chapters(&database, &imported.book.book.id)
                .await
                .expect("chapters")
            {
                let page = book_service::list_segments(&database, &chapter.chapter.id, 0, 100)
                    .await
                    .expect("segments");
                for segment in page.items {
                    let tokens = pronunciation_service::grapheme_tokens(segment.effective_text());
                    let analysis = worker
                        .call(
                            "pronunciation.analyze",
                            json!({
                                "segment_id": segment.id.clone(),
                                "text": segment.effective_text(),
                                "tokens": tokens.clone(),
                            }),
                        )
                        .expect("worker analysis");
                    pronunciation_service::apply_analysis(&database, &segment, &tokens, analysis)
                        .await
                        .expect("persist analysis");
                }
            }
            let report = evaluate_book(&database, &imported.book.book.id, &gold_path)
                .await
                .expect("gold evaluation");
            println!(
                "gold benchmark snapshot ready: hard_occurrences={}, occurrence_mismatches={}",
                report.expected_hard_occurrences,
                report.occurrence_count_mismatches.len()
            );
            if std::env::var_os("ANCIENT_TTS_BENCHMARK_TEXT").is_none()
                && std::env::var_os("ANCIENT_TTS_BENCHMARK_GOLD").is_none()
            {
                assert_eq!(report.gold_entries, 78);
                assert_eq!(report.hard_gold_entries, 74);
                assert_eq!(report.review_required_entries, 4);
                assert_eq!(report.expected_hard_occurrences, 102);
                assert!(report.detected_hard_occurrences > 0);
            }
            assert!(report.occurrence_count_mismatches.is_empty());
        });
    }
}
