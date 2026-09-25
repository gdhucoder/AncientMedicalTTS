use crate::{
    db::Database,
    error::{AppError, AppResult},
    models::{Annotation, GraphemeToken, PronunciationOverride, Segment, SegmentReader},
    services::{book_service, text_service},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::{sqlite::SqliteQueryResult, Transaction};
use time::{format_description::well_known::Rfc3339, OffsetDateTime};
use unicode_segmentation::UnicodeSegmentation;
use uuid::Uuid;

const REVIEW_NEEDS: &str = "needs_review";
const REVIEW_CONFIRMED: &str = "confirmed";
const REVIEW_IGNORED: &str = "ignored";
const RISK_TYPES: &[&str] = &[
    "polyphone",
    "rare_character",
    "medical_term",
    "context_pronunciation",
    "classical_term",
    "knowledge_conflict",
    "unknown_character",
    "textual_variant",
    "manual",
];

#[derive(sqlx::FromRow)]
struct AnnotationRow {
    id: String,
    segment_id: String,
    start_token: i64,
    end_token: i64,
    surface_text: String,
    default_pinyin: Option<String>,
    target_pinyin: Option<String>,
    candidates_json: String,
    risk_type: String,
    reason: Option<String>,
    review_status: String,
    analyzer_version: Option<String>,
    source: Option<String>,
    rule_type: Option<String>,
    confidence: Option<String>,
    source_rule_id: Option<String>,
    created_at: String,
    updated_at: String,
}

pub fn grapheme_tokens(text: &str) -> Vec<GraphemeToken> {
    UnicodeSegmentation::graphemes(text, true)
        .enumerate()
        .map(|(index, token)| GraphemeToken {
            index,
            text: token.to_string(),
        })
        .collect()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct EffectiveForcedPronunciation {
    pub start_token: usize,
    pub end_token: usize,
    pub surface_text: String,
    pub pinyin: String,
    pub source: String,
}

pub async fn get_segment_reader(database: &Database, segment_id: &str) -> AppResult<SegmentReader> {
    let segment = book_service::get_segment(database, segment_id).await?;
    let tokens = grapheme_tokens(segment.effective_text());
    let annotations = list_annotations(database, segment_id).await?;
    let audio_versions =
        crate::services::audio_service::list_audio_versions(database, segment_id).await?;
    Ok(SegmentReader {
        segment,
        tokens,
        annotations,
        audio_versions,
    })
}

pub async fn confirmed_overrides(
    database: &Database,
    segment_id: &str,
) -> AppResult<Vec<PronunciationOverride>> {
    Ok(build_effective_forced_pronunciations(database, segment_id)
        .await?
        .into_iter()
        .map(|item| PronunciationOverride {
            start_token: item.start_token,
            end_token: item.end_token,
            surface_text: item.surface_text,
            pinyin: item.pinyin,
        })
        .collect())
}

/// Resolves the only pronunciation annotations that are allowed to reach TTS.
/// Analyzer/reference annotations intentionally do not enter this result.
pub async fn build_effective_forced_pronunciations(
    database: &Database,
    segment_id: &str,
) -> AppResult<Vec<EffectiveForcedPronunciation>> {
    let annotations = list_annotations(database, segment_id).await?;
    let mut candidates: Vec<(EffectiveForcedPronunciation, u8)> = Vec::new();
    for annotation in annotations {
        if annotation.review_status != REVIEW_CONFIRMED {
            continue;
        }
        let Some(pinyin) = annotation.target_pinyin else {
            continue;
        };
        if let Some(rule_id) = annotation.source_rule_id.as_deref() {
            let rule = sqlx::query_as::<_, (String, i64)>(
                "SELECT scope, enabled FROM pronunciation_rules WHERE id = ?",
            )
            .bind(rule_id)
            .fetch_optional(database.pool())
            .await?;
            let Some((scope, enabled)) = rule else {
                continue;
            };
            if enabled != 1 || !matches!(scope.as_str(), "book" | "global") {
                continue;
            }
            let priority = if scope == "book" { 2 } else { 1 };
            candidates.push((
                EffectiveForcedPronunciation {
                    start_token: annotation.start_token,
                    end_token: annotation.end_token,
                    surface_text: annotation.surface_text,
                    pinyin,
                    source: if scope == "book" {
                        "book_rule".to_string()
                    } else {
                        "global_rule".to_string()
                    },
                },
                priority,
            ));
        } else if annotation.source.as_deref() == Some("manual") {
            candidates.push((
                EffectiveForcedPronunciation {
                    start_token: annotation.start_token,
                    end_token: annotation.end_token,
                    surface_text: annotation.surface_text,
                    pinyin,
                    source: "manual_confirmed".to_string(),
                },
                3,
            ));
        }
    }

    candidates.sort_by(|(left, left_priority), (right, right_priority)| {
        left.start_token
            .cmp(&right.start_token)
            .then_with(|| {
                right
                    .end_token
                    .saturating_sub(right.start_token)
                    .cmp(&left.end_token.saturating_sub(left.start_token))
            })
            .then_with(|| right_priority.cmp(left_priority))
            .then_with(|| left.pinyin.cmp(&right.pinyin))
    });
    let mut selected = Vec::new();
    for (candidate, _) in candidates {
        if selected
            .iter()
            .any(|existing: &EffectiveForcedPronunciation| {
                ranges_overlap(
                    candidate.start_token,
                    candidate.end_token,
                    existing.start_token,
                    existing.end_token,
                )
            })
        {
            continue;
        }
        selected.push(candidate);
    }
    selected.sort_by_key(|item| (item.start_token, item.end_token));
    Ok(selected)
}

pub async fn list_annotations(database: &Database, segment_id: &str) -> AppResult<Vec<Annotation>> {
    let _ = book_service::get_segment(database, segment_id).await?;
    let rows = sqlx::query_as::<_, AnnotationRow>(
        "SELECT id, segment_id, start_token, end_token, surface_text, default_pinyin, target_pinyin,
                candidates_json, risk_type, reason, review_status, analyzer_version, source, rule_type, confidence, source_rule_id, created_at, updated_at
         FROM segment_annotations WHERE segment_id = ? ORDER BY start_token, end_token, created_at")
        .bind(segment_id).fetch_all(database.pool()).await?;
    rows.into_iter().map(annotation_from_row).collect()
}

pub async fn apply_analysis(
    database: &Database,
    segment: &Segment,
    tokens: &[GraphemeToken],
    analysis: Value,
) -> AppResult<SegmentReader> {
    let new_annotations = parse_analysis_response(&analysis, &segment.id, tokens)?;
    let before_signature = effective_signature(database, &segment.id).await?;
    let now = now_rfc3339()?;
    let mut transaction = database.pool().begin().await.map_err(transaction_error)?;

    let protected = load_protected_annotations(&mut transaction, &segment.id).await?;
    sqlx::query("DELETE FROM segment_annotations WHERE segment_id = ? AND review_status = ?")
        .bind(&segment.id)
        .bind(REVIEW_NEEDS)
        .execute(&mut *transaction)
        .await
        .map_err(transaction_error)?;

    let mut inserted_ranges: Vec<(usize, usize)> = Vec::new();
    for annotation in new_annotations {
        if protected.iter().any(|existing| {
            ranges_overlap(
                annotation.start_token,
                annotation.end_token,
                existing.start_token,
                existing.end_token,
            )
        }) {
            continue;
        }
        if inserted_ranges.iter().any(|(start, end)| {
            ranges_overlap(annotation.start_token, annotation.end_token, *start, *end)
        }) {
            return Err(AppError::new(
                "INVALID_ANALYSIS_RESPONSE",
                "自动分析返回了重叠 Annotation",
            ));
        }
        insert_annotation(&mut transaction, &annotation, &now).await?;
        inserted_ranges.push((annotation.start_token, annotation.end_token));
    }

    crate::services::pronunciation_rule_service::apply_active_rules_to_segment_tx(
        &mut transaction,
        segment,
        tokens,
        &mut Default::default(),
    )
    .await?;
    let effective_changed =
        before_signature != effective_signature_tx(&mut transaction, &segment.id).await?;
    recalculate_segment_status_tx(&mut transaction, &segment.id, effective_changed).await?;
    transaction.commit().await.map_err(transaction_error)?;
    get_segment_reader(database, &segment.id).await
}

pub async fn confirm_annotation(
    database: &Database,
    annotation_id: &str,
    target_pinyin: &str,
) -> AppResult<SegmentReader> {
    let annotation = get_annotation(database, annotation_id).await?;
    let segment = book_service::get_segment(database, &annotation.segment_id).await?;
    let tokens = grapheme_tokens(segment.effective_text());
    validate_annotation_range(&annotation, &tokens)?;
    let normalized = normalize_and_validate_pinyin(target_pinyin)?;
    validate_pinyin_token_count(
        &tokens,
        annotation.start_token,
        annotation.end_token,
        &normalized,
    )?;
    let before_signature = effective_signature(database, &annotation.segment_id).await?;
    let now = now_rfc3339()?;
    let mut transaction = database.pool().begin().await.map_err(transaction_error)?;
    let result = sqlx::query("UPDATE segment_annotations SET target_pinyin = ?, review_status = ?, source = 'manual', rule_type = 'manual', confidence = 'verified', source_rule_id = NULL, updated_at = ? WHERE id = ?")
        .bind(normalized).bind(REVIEW_CONFIRMED).bind(&now).bind(annotation_id).execute(&mut *transaction).await.map_err(transaction_error)?;
    ensure_annotation_updated(result)?;
    let effective_changed = before_signature
        != effective_signature_tx(&mut transaction, &annotation.segment_id).await?;
    recalculate_segment_status_tx(&mut transaction, &annotation.segment_id, effective_changed)
        .await?;
    transaction.commit().await.map_err(transaction_error)?;
    get_segment_reader(database, &annotation.segment_id).await
}

pub async fn ignore_annotation(
    database: &Database,
    annotation_id: &str,
) -> AppResult<SegmentReader> {
    update_review_status(database, annotation_id, REVIEW_IGNORED).await
}

pub async fn reset_annotation(
    database: &Database,
    annotation_id: &str,
) -> AppResult<SegmentReader> {
    update_review_status(database, annotation_id, REVIEW_NEEDS).await
}

pub async fn create_manual_annotation(
    database: &Database,
    segment_id: &str,
    token_index: usize,
    target_pinyin: &str,
) -> AppResult<SegmentReader> {
    let segment = book_service::get_segment(database, segment_id).await?;
    let tokens = grapheme_tokens(segment.effective_text());
    let token = tokens
        .get(token_index)
        .ok_or_else(|| AppError::new("INVALID_ANALYSIS_RESPONSE", "token_index 超出范围"))?;
    if !is_han_token(&token.text) {
        return Err(AppError::new(
            "PINYIN_INVALID",
            "标点和非汉字 token 不能添加发音标注",
        ));
    }
    let normalized = normalize_and_validate_pinyin(target_pinyin)?;
    validate_pinyin_token_count(&tokens, token_index, token_index + 1, &normalized)?;
    let existing = list_annotations(database, segment_id).await?;
    if existing.iter().any(|annotation| {
        ranges_overlap(
            token_index,
            token_index + 1,
            annotation.start_token,
            annotation.end_token,
        )
    }) {
        return Err(AppError::new(
            "ANNOTATION_OVERLAP",
            "该 token 已经存在 Annotation",
        ));
    }
    let before_signature = effective_signature(database, segment_id).await?;
    let now = now_rfc3339()?;
    let mut transaction = database.pool().begin().await.map_err(transaction_error)?;
    let annotation = Annotation {
        id: Uuid::now_v7().to_string(),
        segment_id: segment_id.to_string(),
        start_token: token_index,
        end_token: token_index + 1,
        surface_text: token.text.clone(),
        default_pinyin: None,
        target_pinyin: Some(normalized),
        candidate_pinyin: Vec::new(),
        risk_type: "manual".to_string(),
        reason: Some("用户手工添加".to_string()),
        review_status: REVIEW_CONFIRMED.to_string(),
        analyzer_version: None,
        source: Some("manual".to_string()),
        rule_type: Some("manual".to_string()),
        confidence: Some("verified".to_string()),
        source_rule_id: None,
        created_at: now.clone(),
        updated_at: now.clone(),
    };
    insert_annotation(&mut transaction, &annotation, &now).await?;
    let effective_changed =
        before_signature != effective_signature_tx(&mut transaction, segment_id).await?;
    recalculate_segment_status_tx(&mut transaction, segment_id, effective_changed).await?;
    transaction.commit().await.map_err(transaction_error)?;
    get_segment_reader(database, segment_id).await
}

fn parse_analysis_response(
    value: &Value,
    segment_id: &str,
    tokens: &[GraphemeToken],
) -> AppResult<Vec<Annotation>> {
    let analyzer_version = value
        .get("analyzer_version")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            AppError::new("INVALID_ANALYSIS_RESPONSE", "分析结果缺少 analyzer_version")
        })?;
    let items = value
        .get("items")
        .and_then(Value::as_array)
        .ok_or_else(|| AppError::new("INVALID_ANALYSIS_RESPONSE", "分析结果缺少 items"))?;
    let mut annotations = Vec::with_capacity(items.len());
    for item in items {
        let start = item
            .get("start_token")
            .and_then(Value::as_u64)
            .map(|value| value as usize)
            .ok_or_else(|| AppError::new("INVALID_ANALYSIS_RESPONSE", "start_token 无效"))?;
        let end = item
            .get("end_token")
            .and_then(Value::as_u64)
            .map(|value| value as usize)
            .ok_or_else(|| AppError::new("INVALID_ANALYSIS_RESPONSE", "end_token 无效"))?;
        if start >= end || end > tokens.len() {
            return Err(AppError::new(
                "INVALID_ANALYSIS_RESPONSE",
                "Annotation token 范围无效",
            ));
        }
        let surface_text = item
            .get("surface_text")
            .and_then(Value::as_str)
            .ok_or_else(|| AppError::new("INVALID_ANALYSIS_RESPONSE", "surface_text 缺失"))?;
        let expected_surface = tokens[start..end]
            .iter()
            .map(|token| token.text.as_str())
            .collect::<String>();
        if surface_text != expected_surface {
            return Err(AppError::new(
                "INVALID_ANALYSIS_RESPONSE",
                "surface_text 与 token 范围不一致",
            ));
        }
        let risk_type = item
            .get("risk_type")
            .and_then(Value::as_str)
            .ok_or_else(|| AppError::new("INVALID_ANALYSIS_RESPONSE", "risk_type 缺失"))?;
        if !RISK_TYPES.contains(&risk_type) || risk_type == "manual" {
            return Err(AppError::new(
                "INVALID_ANALYSIS_RESPONSE",
                "分析结果 risk_type 不受支持",
            ));
        }
        let default_pinyin = optional_pinyin(item.get("default_pinyin"))?;
        let candidates = parse_candidates(item.get("candidate_pinyin"))?;
        let reason = item
            .get("reason")
            .and_then(Value::as_str)
            .map(str::to_string);
        let source = item
            .get("source")
            .and_then(Value::as_str)
            .map(str::to_string)
            .or_else(|| Some("pypinyin".to_string()));
        let rule_type = item
            .get("rule_type")
            .and_then(Value::as_str)
            .map(str::to_string);
        let confidence = item
            .get("confidence")
            .and_then(Value::as_str)
            .map(str::to_string);
        if confidence
            .as_deref()
            .is_some_and(|value| !matches!(value, "verified" | "high" | "medium" | "low"))
        {
            return Err(AppError::new(
                "INVALID_ANALYSIS_RESPONSE",
                "分析结果 confidence 不受支持",
            ));
        }
        validate_pinyin_token_count(tokens, start, end, default_pinyin.as_deref().unwrap_or(""))?;
        for candidate in &candidates {
            validate_pinyin_token_count(tokens, start, end, candidate)?;
        }
        annotations.push(Annotation {
            id: Uuid::now_v7().to_string(),
            segment_id: segment_id.to_string(),
            start_token: start,
            end_token: end,
            surface_text: surface_text.to_string(),
            default_pinyin,
            target_pinyin: None,
            candidate_pinyin: candidates,
            risk_type: risk_type.to_string(),
            reason,
            review_status: REVIEW_NEEDS.to_string(),
            analyzer_version: Some(analyzer_version.to_string()),
            source,
            rule_type,
            confidence,
            source_rule_id: None,
            created_at: String::new(),
            updated_at: String::new(),
        });
    }
    for left in 0..annotations.len() {
        for right in left + 1..annotations.len() {
            if ranges_overlap(
                annotations[left].start_token,
                annotations[left].end_token,
                annotations[right].start_token,
                annotations[right].end_token,
            ) {
                return Err(AppError::new(
                    "INVALID_ANALYSIS_RESPONSE",
                    "分析结果包含重叠 Annotation",
                ));
            }
        }
    }
    Ok(annotations)
}

fn parse_candidates(value: Option<&Value>) -> AppResult<Vec<String>> {
    let values = value
        .and_then(Value::as_array)
        .ok_or_else(|| AppError::new("INVALID_ANALYSIS_RESPONSE", "candidate_pinyin 缺失"))?;
    let mut candidates = Vec::with_capacity(values.len());
    for value in values {
        let candidate = value.as_str().ok_or_else(|| {
            AppError::new("INVALID_ANALYSIS_RESPONSE", "candidate_pinyin 包含无效值")
        })?;
        candidates.push(normalize_and_validate_pinyin(candidate)?);
    }
    candidates.sort();
    candidates.dedup();
    Ok(candidates)
}

fn optional_pinyin(value: Option<&Value>) -> AppResult<Option<String>> {
    match value {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) => Ok(Some(normalize_and_validate_pinyin(value)?)),
        _ => Err(AppError::new(
            "INVALID_ANALYSIS_RESPONSE",
            "pinyin 字段类型无效",
        )),
    }
}

pub fn normalize_and_validate_pinyin(value: &str) -> AppResult<String> {
    let trimmed = value.trim();
    if trimmed
        .chars()
        .any(|character| character.is_ascii_uppercase())
    {
        return Err(AppError::new(
            "PINYIN_INVALID",
            "拼音必须使用小写 ASCII 字母加数字声调，例如 shu4",
        ));
    }
    let normalized = trimmed.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.is_empty() {
        return Err(AppError::new("PINYIN_INVALID", "拼音不能为空"));
    }
    for syllable in normalized.split_whitespace() {
        let (letters, tone) = syllable.split_at(syllable.len().saturating_sub(1));
        if letters.is_empty()
            || !letters.bytes().all(|byte| byte.is_ascii_lowercase())
            || !matches!(tone, "1" | "2" | "3" | "4" | "5")
        {
            return Err(AppError::new(
                "PINYIN_INVALID",
                "拼音必须使用 ASCII 字母加数字声调，例如 shu4",
            ));
        }
    }
    Ok(normalized)
}

pub(crate) fn validate_pinyin_token_count(
    tokens: &[GraphemeToken],
    start: usize,
    end: usize,
    pinyin: &str,
) -> AppResult<()> {
    if pinyin.is_empty() {
        return Ok(());
    }
    let pronunciation_tokens = tokens[start..end]
        .iter()
        .filter(|token| is_han_token(&token.text))
        .count();
    let pinyin_count = pinyin.split_whitespace().count();
    if pronunciation_tokens != pinyin_count {
        return Err(AppError::new(
            "PINYIN_TOKEN_COUNT_MISMATCH",
            "拼音音节数量与汉字 token 数量不一致",
        ));
    }
    Ok(())
}

pub(crate) fn is_han_token(text: &str) -> bool {
    text.chars().any(text_service::is_han_character)
}

pub(crate) fn ranges_overlap(a_start: usize, a_end: usize, b_start: usize, b_end: usize) -> bool {
    a_start < b_end && b_start < a_end
}

pub(crate) async fn insert_annotation(
    connection: &mut Transaction<'_, sqlx::Sqlite>,
    annotation: &Annotation,
    now: &str,
) -> AppResult<()> {
    let candidates_json = serde_json::to_string(&annotation.candidate_pinyin)
        .map_err(|error| AppError::new("DB_TRANSACTION_FAILED", error.to_string()))?;
    sqlx::query("INSERT INTO segment_annotations (id, segment_id, start_token, end_token, surface_text, default_pinyin, target_pinyin, candidates_json, risk_type, reason, review_status, analyzer_version, source, rule_type, confidence, source_rule_id, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)")
        .bind(&annotation.id).bind(&annotation.segment_id).bind(annotation.start_token as i64).bind(annotation.end_token as i64)
        .bind(&annotation.surface_text).bind(&annotation.default_pinyin).bind(&annotation.target_pinyin).bind(candidates_json)
        .bind(&annotation.risk_type).bind(&annotation.reason).bind(&annotation.review_status).bind(&annotation.analyzer_version).bind(&annotation.source).bind(&annotation.rule_type).bind(&annotation.confidence).bind(&annotation.source_rule_id)
        .bind(now).bind(now).execute(&mut **connection).await.map_err(transaction_error)?;
    Ok(())
}

async fn load_protected_annotations(
    connection: &mut Transaction<'_, sqlx::Sqlite>,
    segment_id: &str,
) -> AppResult<Vec<Annotation>> {
    let rows = sqlx::query_as::<_, AnnotationRow>(
        "SELECT id, segment_id, start_token, end_token, surface_text, default_pinyin, target_pinyin, candidates_json, risk_type, reason, review_status, analyzer_version, source, rule_type, confidence, source_rule_id, created_at, updated_at
         FROM segment_annotations WHERE segment_id = ? AND review_status IN (?, ?) AND source_rule_id IS NULL")
        .bind(segment_id).bind(REVIEW_CONFIRMED).bind(REVIEW_IGNORED).fetch_all(&mut **connection).await.map_err(transaction_error)?;
    rows.into_iter().map(annotation_from_row).collect()
}

pub(crate) async fn recalculate_segment_status_tx(
    connection: &mut Transaction<'_, sqlx::Sqlite>,
    segment_id: &str,
    effective_changed: bool,
) -> AppResult<String> {
    let statuses = sqlx::query_as::<_, (String,)>(
        "SELECT review_status FROM segment_annotations WHERE segment_id = ?",
    )
    .bind(segment_id)
    .fetch_all(&mut **connection)
    .await
    .map_err(transaction_error)?;
    let current_status: String = sqlx::query_scalar("SELECT status FROM segments WHERE id = ?")
        .bind(segment_id)
        .fetch_optional(&mut **connection)
        .await
        .map_err(transaction_error)?
        .ok_or_else(|| AppError::new("SEGMENT_NOT_FOUND", "Segment 不存在"))?;
    let current_audio_matches = if effective_changed {
        current_audio_matches_pronunciation_signature_tx(connection, segment_id).await?
    } else {
        false
    };
    let status = if statuses.iter().any(|(status,)| status == REVIEW_NEEDS) {
        "needs_review"
    } else if effective_changed && current_audio_matches {
        "generated"
    } else if effective_changed {
        "ready"
    } else if current_status == "generated" {
        "generated"
    } else if statuses.is_empty() {
        "analyzed"
    } else {
        "ready"
    };
    let result = sqlx::query("UPDATE segments SET status = ?, updated_at = ? WHERE id = ?")
        .bind(status)
        .bind(now_rfc3339()?)
        .bind(segment_id)
        .execute(&mut **connection)
        .await
        .map_err(transaction_error)?;
    if result.rows_affected() == 0 {
        return Err(AppError::new("SEGMENT_NOT_FOUND", "Segment 不存在"));
    }
    Ok(status.to_string())
}

async fn current_audio_matches_pronunciation_signature_tx(
    connection: &mut Transaction<'_, sqlx::Sqlite>,
    segment_id: &str,
) -> AppResult<bool> {
    let stored_signature: Option<String> = sqlx::query_scalar(
        "SELECT a.pronunciation_signature
         FROM segments s
         JOIN audio_versions a ON a.id = s.current_audio_id
         WHERE s.id = ?",
    )
    .bind(segment_id)
    .fetch_optional(&mut **connection)
    .await
    .map_err(transaction_error)?;
    let Some(stored_signature) = stored_signature else {
        return Ok(false);
    };
    let current_signature = effective_signature_tx(connection, segment_id).await?;
    Ok(stored_signature == serialize_pronunciation_signature(&current_signature)?)
}

async fn update_review_status(
    database: &Database,
    annotation_id: &str,
    status: &str,
) -> AppResult<SegmentReader> {
    let annotation = get_annotation(database, annotation_id).await?;
    let before_signature = effective_signature(database, &annotation.segment_id).await?;
    let now = now_rfc3339()?;
    let mut transaction = database.pool().begin().await.map_err(transaction_error)?;
    let result = sqlx::query("UPDATE segment_annotations SET target_pinyin = NULL, review_status = ?, source = 'manual', rule_type = 'manual', confidence = 'verified', source_rule_id = NULL, updated_at = ? WHERE id = ?")
        .bind(status).bind(&now).bind(annotation_id).execute(&mut *transaction).await.map_err(transaction_error)?;
    ensure_annotation_updated(result)?;
    let effective_changed = before_signature
        != effective_signature_tx(&mut transaction, &annotation.segment_id).await?;
    recalculate_segment_status_tx(&mut transaction, &annotation.segment_id, effective_changed)
        .await?;
    transaction.commit().await.map_err(transaction_error)?;
    get_segment_reader(database, &annotation.segment_id).await
}

pub(crate) async fn get_annotation(
    database: &Database,
    annotation_id: &str,
) -> AppResult<Annotation> {
    let row = sqlx::query_as::<_, AnnotationRow>(
        "SELECT id, segment_id, start_token, end_token, surface_text, default_pinyin, target_pinyin, candidates_json, risk_type, reason, review_status, analyzer_version, source, rule_type, confidence, source_rule_id, created_at, updated_at FROM segment_annotations WHERE id = ?")
        .bind(annotation_id).fetch_optional(database.pool()).await?;
    row.map(annotation_from_row)
        .transpose()?
        .ok_or_else(|| AppError::new("ANNOTATION_NOT_FOUND", "Annotation 不存在"))
}

fn annotation_from_row(row: AnnotationRow) -> AppResult<Annotation> {
    let candidate_pinyin = serde_json::from_str::<Vec<String>>(&row.candidates_json)
        .map_err(|error| AppError::new("DB_ERROR", format!("candidates_json 无效: {error}")))?;
    Ok(Annotation {
        id: row.id,
        segment_id: row.segment_id,
        start_token: row.start_token as usize,
        end_token: row.end_token as usize,
        surface_text: row.surface_text,
        default_pinyin: row.default_pinyin,
        target_pinyin: row.target_pinyin,
        candidate_pinyin,
        risk_type: row.risk_type,
        reason: row.reason,
        review_status: row.review_status,
        analyzer_version: row.analyzer_version,
        source: row.source,
        rule_type: row.rule_type,
        confidence: row.confidence,
        source_rule_id: row.source_rule_id,
        created_at: row.created_at,
        updated_at: row.updated_at,
    })
}

fn validate_annotation_range(annotation: &Annotation, tokens: &[GraphemeToken]) -> AppResult<()> {
    if annotation.start_token >= annotation.end_token || annotation.end_token > tokens.len() {
        return Err(AppError::new(
            "INVALID_ANALYSIS_RESPONSE",
            "Annotation 范围无效",
        ));
    }
    let surface = tokens[annotation.start_token..annotation.end_token]
        .iter()
        .map(|token| token.text.as_str())
        .collect::<String>();
    if surface != annotation.surface_text {
        return Err(AppError::new(
            "INVALID_ANALYSIS_RESPONSE",
            "Annotation surface_text 不一致",
        ));
    }
    Ok(())
}

fn ensure_annotation_updated(result: SqliteQueryResult) -> AppResult<()> {
    if result.rows_affected() == 0 {
        Err(AppError::new("ANNOTATION_NOT_FOUND", "Annotation 不存在"))
    } else {
        Ok(())
    }
}

pub(crate) fn now_rfc3339() -> AppResult<String> {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .map_err(|error| AppError::new("DB_ERROR", error.to_string()))
}

fn transaction_error(error: sqlx::Error) -> AppError {
    AppError::new("DB_TRANSACTION_FAILED", error.to_string())
}

pub(crate) async fn effective_signature(
    database: &Database,
    segment_id: &str,
) -> AppResult<Vec<(i64, i64, String)>> {
    Ok(sqlx::query_as::<_, (i64, i64, String)>(
        "SELECT a.start_token, a.end_token, a.target_pinyin FROM segment_annotations a
         LEFT JOIN pronunciation_rules r ON r.id = a.source_rule_id
         WHERE a.segment_id = ? AND a.review_status = 'confirmed' AND a.target_pinyin IS NOT NULL
           AND ((a.source_rule_id IS NULL AND a.source = 'manual')
                OR (r.id IS NOT NULL AND r.enabled = 1))
         ORDER BY a.start_token, a.end_token, a.target_pinyin",
    )
    .bind(segment_id)
    .fetch_all(database.pool())
    .await?)
}

pub(crate) fn serialize_pronunciation_signature(
    signature: &[(i64, i64, String)],
) -> AppResult<String> {
    serde_json::to_string(signature)
        .map_err(|error| AppError::new("DB_ERROR", format!("发音快照序列化失败: {error}")))
}

pub(crate) async fn effective_signature_tx(
    connection: &mut Transaction<'_, sqlx::Sqlite>,
    segment_id: &str,
) -> AppResult<Vec<(i64, i64, String)>> {
    Ok(sqlx::query_as::<_, (i64, i64, String)>(
        "SELECT a.start_token, a.end_token, a.target_pinyin FROM segment_annotations a
         LEFT JOIN pronunciation_rules r ON r.id = a.source_rule_id
         WHERE a.segment_id = ? AND a.review_status = 'confirmed' AND a.target_pinyin IS NOT NULL
           AND ((a.source_rule_id IS NULL AND a.source = 'manual')
                OR (r.id IS NOT NULL AND r.enabled = 1))
         ORDER BY a.start_token, a.end_token, a.target_pinyin",
    )
    .bind(segment_id)
    .fetch_all(&mut **connection)
    .await
    .map_err(transaction_error)?)
}

#[cfg(test)]
mod tests {
    use super::{
        apply_analysis, build_effective_forced_pronunciations, create_manual_annotation,
        get_segment_reader, grapheme_tokens, normalize_and_validate_pinyin, ranges_overlap,
    };
    use crate::{db::Database, models::Segment};
    use serde_json::json;
    use uuid::Uuid;

    fn temp_db() -> Database {
        let path = std::env::temp_dir().join(format!(
            "ancient-medical-pronunciation-{}.sqlite",
            Uuid::now_v7()
        ));
        tauri::async_runtime::block_on(Database::open(path)).expect("database should initialize")
    }

    async fn seed_segment(database: &Database, text: &str) -> Segment {
        let now = "2026-01-01T00:00:00Z";
        let book_id = Uuid::now_v7().to_string();
        let chapter_id = Uuid::now_v7().to_string();
        let segment_id = Uuid::now_v7().to_string();
        sqlx::query("INSERT INTO books (id, title, created_at, updated_at) VALUES (?, ?, ?, ?)")
            .bind(&book_id)
            .bind("test")
            .bind(now)
            .bind(now)
            .execute(database.pool())
            .await
            .expect("book");
        sqlx::query("INSERT INTO chapters (id, book_id, title, order_index, created_at, updated_at) VALUES (?, ?, ?, 0, ?, ?)").bind(&chapter_id).bind(&book_id).bind("正文").bind(now).bind(now).execute(database.pool()).await.expect("chapter");
        sqlx::query("INSERT INTO segments (id, chapter_id, order_index, original_text, status, created_at, updated_at) VALUES (?, ?, 0, ?, 'pending', ?, ?)").bind(&segment_id).bind(&chapter_id).bind(text).bind(now).bind(now).execute(database.pool()).await.expect("segment");
        crate::services::book_service::get_segment(database, &segment_id)
            .await
            .expect("segment")
    }

    #[test]
    fn validates_grapheme_tokens_and_pinyin() {
        let tokens = grapheme_tokens("A𠮷B");
        assert_eq!(tokens.len(), 3);
        assert_eq!(tokens[1].text, "𠮷");
        assert_eq!(
            normalize_and_validate_pinyin("  lv4   ").expect("valid pinyin"),
            "lv4"
        );
        assert!(normalize_and_validate_pinyin("LV4").is_err());
        assert!(normalize_and_validate_pinyin("shuo").is_err());
        assert!(ranges_overlap(1, 3, 2, 4));
        assert!(!ranges_overlap(1, 2, 2, 3));
    }

    #[test]
    fn analysis_writes_annotations_and_reanalysis_keeps_confirmed() {
        let database = temp_db();
        tauri::async_runtime::block_on(async {
            let segment = seed_segment(&database, "不亦说乎").await;
            let tokens = grapheme_tokens(&segment.original_text);
            let analysis = json!({"analyzer_version":"0.3.0","domain_lexicon_version":"0.3.0","items":[{"start_token":2,"end_token":3,"surface_text":"说","default_pinyin":"shuo1","candidate_pinyin":["shuo1","yue4"],"risk_type":"polyphone","reason":"multiple","source":"high_risk_polyphone","rule_type":"high_risk_polyphone","confidence":"low"}]});
            let first = apply_analysis(&database, &segment, &tokens, analysis)
                .await
                .expect("analysis");
            assert_eq!(first.annotations.len(), 1);
            assert_eq!(
                first.annotations[0].source.as_deref(),
                Some("high_risk_polyphone")
            );
            assert_eq!(
                first.annotations[0].rule_type.as_deref(),
                Some("high_risk_polyphone")
            );
            assert_eq!(first.annotations[0].confidence.as_deref(), Some("low"));
            let annotation_id = first.annotations[0].id.clone();
            let confirmed = super::confirm_annotation(&database, &annotation_id, "yue4")
                .await
                .expect("confirm");
            assert_eq!(confirmed.segment.status, "ready");
            let second_analysis = json!({"analyzer_version":"0.1.0","items":[{"start_token":2,"end_token":3,"surface_text":"说","default_pinyin":"shuo1","candidate_pinyin":["shuo1","yue4"],"risk_type":"polyphone","reason":"multiple","confidence":null},{"start_token":0,"end_token":1,"surface_text":"不","default_pinyin":"bu4","candidate_pinyin":["bu4","fou3"],"risk_type":"polyphone","reason":"multiple","confidence":null}]});
            let second = apply_analysis(&database, &segment, &tokens, second_analysis)
                .await
                .expect("reanalyze");
            assert_eq!(
                second
                    .annotations
                    .iter()
                    .filter(|annotation| annotation.review_status == "confirmed")
                    .count(),
                1
            );
            assert_eq!(
                second
                    .annotations
                    .iter()
                    .filter(|annotation| annotation.review_status == "needs_review")
                    .count(),
                1
            );
        });
    }

    #[test]
    fn reference_pinyin_is_not_sent_to_tts_until_manually_confirmed() {
        let database = temp_db();
        tauri::async_runtime::block_on(async {
            let segment = seed_segment(&database, "恶寒").await;
            let tokens = grapheme_tokens(segment.effective_text());
            let analysis = json!({
                "analyzer_version": "0.3.0",
                "items": [{
                    "start_token": 0,
                    "end_token": 1,
                    "surface_text": "恶",
                    "default_pinyin": "e4",
                    "candidate_pinyin": ["e4", "wu4"],
                    "risk_type": "context_pronunciation",
                    "reason": "context",
                    "source": "context_exact",
                    "rule_type": "context_exact",
                    "confidence": "high"
                }]
            });
            let analyzed = apply_analysis(&database, &segment, &tokens, analysis)
                .await
                .expect("analysis");
            assert!(
                build_effective_forced_pronunciations(&database, &segment.id)
                    .await
                    .expect("forced policy")
                    .is_empty()
            );

            let confirmed = super::confirm_annotation(&database, &analyzed.annotations[0].id, "e4")
                .await
                .expect("confirm");
            let forced = build_effective_forced_pronunciations(&database, &segment.id)
                .await
                .expect("forced policy");
            assert_eq!(forced.len(), 1);
            assert_eq!(forced[0].pinyin, "e4");
            assert_eq!(forced[0].source, "manual_confirmed");
            assert_eq!(confirmed.annotations[0].source.as_deref(), Some("manual"));

            let reference_only = seed_segment(&database, "恶").await;
            let reference_tokens = grapheme_tokens(reference_only.effective_text());
            let reference_analysis = json!({
                "analyzer_version": "0.3.0",
                "items": [{
                    "start_token": 0,
                    "end_token": 1,
                    "surface_text": "恶",
                    "default_pinyin": "e4",
                    "candidate_pinyin": ["e4", "wu4"],
                    "risk_type": "polyphone",
                    "reason": "reference",
                    "source": "pypinyin",
                    "rule_type": "polyphone",
                    "confidence": "low"
                }]
            });
            let reference_reader = apply_analysis(
                &database,
                &reference_only,
                &reference_tokens,
                reference_analysis,
            )
            .await
            .expect("reference analysis");
            sqlx::query("UPDATE segment_annotations SET review_status = 'confirmed', target_pinyin = 'e4' WHERE id = ?")
                .bind(&reference_reader.annotations[0].id)
                .execute(database.pool())
                .await
                .expect("seed non-authoritative confirmation");
            assert!(
                build_effective_forced_pronunciations(&database, &reference_only.id)
                    .await
                    .expect("forced policy")
                    .is_empty()
            );
        });
    }

    #[test]
    fn restoring_audio_pronunciation_signature_reuses_current_audio() {
        let database = temp_db();
        tauri::async_runtime::block_on(async {
            let segment = seed_segment(&database, "说").await;
            let tokens = grapheme_tokens(&segment.original_text);
            let analysis = json!({"analyzer_version":"0.3.0","items":[{"start_token":0,"end_token":1,"surface_text":"说","default_pinyin":"shuo1","candidate_pinyin":["shuo1","yue4"],"risk_type":"polyphone","reason":"multiple"}]});
            let analyzed = apply_analysis(&database, &segment, &tokens, analysis)
                .await
                .expect("analysis");
            let annotation_id = analyzed.annotations[0].id.clone();
            let confirmed = super::confirm_annotation(&database, &annotation_id, "yue4")
                .await
                .expect("confirm");
            let signature =
                super::serialize_pronunciation_signature(&vec![(0, 1, "yue4".to_string())])
                    .expect("signature");
            let audio_id = Uuid::now_v7().to_string();
            sqlx::query("INSERT INTO audio_versions (id, segment_id, version_no, provider, voice_type, sample_rate, codec, speed, volume, pronunciation_signature, audio_path, created_at) VALUES (?, ?, 1, 'tencent', 501000, 16000, 'wav', 0, 0, ?, '/tmp/test.wav', 'now')")
                .bind(&audio_id)
                .bind(&segment.id)
                .bind(&signature)
                .execute(database.pool())
                .await
                .expect("audio version");
            sqlx::query(
                "UPDATE segments SET current_audio_id = ?, status = 'generated' WHERE id = ?",
            )
            .bind(&audio_id)
            .bind(&segment.id)
            .execute(database.pool())
            .await
            .expect("current audio");

            let changed = super::confirm_annotation(&database, &annotation_id, "shuo1")
                .await
                .expect("change pronunciation");
            assert_eq!(changed.segment.status, "ready");
            let restored = super::confirm_annotation(&database, &annotation_id, "yue4")
                .await
                .expect("restore pronunciation");
            assert_eq!(restored.segment.status, "generated");
            assert_eq!(
                restored.segment.current_audio_id.as_deref(),
                Some(audio_id.as_str())
            );
            assert_eq!(confirmed.segment.status, "ready");
        });
    }

    #[test]
    fn manual_annotation_status_can_be_reset_and_ignored() {
        let database = temp_db();
        tauri::async_runtime::block_on(async {
            let segment = seed_segment(&database, "说").await;
            let created = create_manual_annotation(&database, &segment.id, 0, "yue4")
                .await
                .expect("manual annotation");
            let annotation_id = created.annotations[0].id.clone();
            assert_eq!(created.segment.status, "ready");
            let reset = super::reset_annotation(&database, &annotation_id)
                .await
                .expect("reset");
            assert_eq!(reset.segment.status, "needs_review");
            let ignored = super::ignore_annotation(&database, &annotation_id)
                .await
                .expect("ignore");
            assert_eq!(ignored.segment.status, "ready");
        });
    }

    #[test]
    fn reader_returns_tokens_and_annotations() {
        let database = temp_db();
        tauri::async_runtime::block_on(async {
            let segment = seed_segment(&database, "𠮷").await;
            let reader = get_segment_reader(&database, &segment.id)
                .await
                .expect("reader");
            assert_eq!(reader.tokens[0].text, "𠮷");
            assert!(reader.annotations.is_empty());
        });
    }

    #[test]
    fn invalid_analysis_range_or_surface_is_rejected() {
        let database = temp_db();
        tauri::async_runtime::block_on(async {
            let segment = seed_segment(&database, "说").await;
            let tokens = grapheme_tokens(&segment.original_text);
            let out_of_range = json!({"analyzer_version":"0.1.0","items":[{"start_token":0,"end_token":2,"surface_text":"说","default_pinyin":"shuo1","candidate_pinyin":["shuo1"],"risk_type":"polyphone","reason":"test"}]});
            let error = apply_analysis(&database, &segment, &tokens, out_of_range)
                .await
                .expect_err("range should fail");
            assert_eq!(error.code, "INVALID_ANALYSIS_RESPONSE");

            let wrong_surface = json!({"analyzer_version":"0.1.0","items":[{"start_token":0,"end_token":1,"surface_text":"行","default_pinyin":"shuo1","candidate_pinyin":["shuo1"],"risk_type":"polyphone","reason":"test"}]});
            let error = apply_analysis(&database, &segment, &tokens, wrong_surface)
                .await
                .expect_err("surface should fail");
            assert_eq!(error.code, "INVALID_ANALYSIS_RESPONSE");
        });
    }

    #[test]
    fn empty_analysis_is_analyzed_and_ignored_survives_reanalysis() {
        let database = temp_db();
        tauri::async_runtime::block_on(async {
            let segment = seed_segment(&database, "说").await;
            let tokens = grapheme_tokens(&segment.original_text);
            let empty = json!({"analyzer_version":"0.1.0","items":[]});
            let analyzed = apply_analysis(&database, &segment, &tokens, empty.clone())
                .await
                .expect("empty analysis");
            assert_eq!(analyzed.segment.status, "analyzed");

            let annotation = json!({"analyzer_version":"0.1.0","items":[{"start_token":0,"end_token":1,"surface_text":"说","default_pinyin":"shuo1","candidate_pinyin":["shuo1","yue4"],"risk_type":"polyphone","reason":"test"}]});
            let first = apply_analysis(&database, &segment, &tokens, annotation.clone())
                .await
                .expect("analysis");
            let ignored = super::ignore_annotation(&database, &first.annotations[0].id)
                .await
                .expect("ignore");
            assert_eq!(ignored.segment.status, "ready");
            let second = apply_analysis(&database, &segment, &tokens, annotation)
                .await
                .expect("reanalysis");
            assert_eq!(second.annotations.len(), 1);
            assert_eq!(second.annotations[0].review_status, "ignored");
            assert_eq!(second.segment.status, "ready");
        });
    }

    #[test]
    fn manual_annotation_rejects_punctuation() {
        let database = temp_db();
        tauri::async_runtime::block_on(async {
            let segment = seed_segment(&database, "。").await;
            let error = create_manual_annotation(&database, &segment.id, 0, "ju4")
                .await
                .expect_err("punctuation should fail");
            assert_eq!(error.code, "PINYIN_INVALID");
        });
    }
}
