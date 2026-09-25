use crate::{
    db::Database,
    error::{AppError, AppResult},
    models::{
        Annotation, GraphemeToken, PronunciationRule, PronunciationRuleApplyResult,
        PronunciationRuleMutation, Segment,
    },
    services::{book_service, pronunciation_service},
};
use sqlx::{Sqlite, Transaction};
use time::{format_description::well_known::Rfc3339, OffsetDateTime};
use uuid::Uuid;

const SCOPE_BOOK: &str = "book";
const SCOPE_GLOBAL: &str = "global";

#[derive(Clone, Debug)]
struct RuleRecord {
    id: String,
    scope: String,
    book_id: Option<String>,
    pattern_text: String,
    pattern_tokens: Vec<String>,
    target_pinyin: String,
    rule_type: String,
    source: Option<String>,
}

#[derive(Clone, Debug)]
struct RuleMatch {
    rule: RuleRecord,
    start_token: usize,
    end_token: usize,
}

#[derive(Clone, Debug)]
struct ExistingRuleAnnotation {
    id: String,
    start_token: usize,
    end_token: usize,
    review_status: String,
    target_pinyin: Option<String>,
    source_rule_id: Option<String>,
}

pub async fn create_rule(
    database: &Database,
    scope: &str,
    book_id: Option<&str>,
    pattern_text: &str,
    target_pinyin: &str,
    rule_type: &str,
    source: Option<&str>,
) -> AppResult<PronunciationRuleMutation> {
    let (pattern_text, target_pinyin, _pattern_tokens) =
        validate_rule_input(scope, book_id, pattern_text, target_pinyin)?;
    if scope == SCOPE_BOOK {
        let book_id = book_id.ok_or_else(|| {
            AppError::new(
                "INVALID_PRONUNCIATION_RULE_SCOPE",
                "book 规则必须有 book_id",
            )
        })?;
        book_service::get_book(database, book_id).await?;
    }
    let now = now_rfc3339()?;
    let mut transaction = database.pool().begin().await.map_err(transaction_error)?;
    let existing =
        find_duplicate_rule(&mut transaction, scope, book_id, &pattern_text, None).await?;
    let rule_id = if let Some(existing) = existing {
        if existing.target_pinyin != target_pinyin {
            return Err(AppError::new(
                "PRONUNCIATION_RULE_CONFLICT",
                format!("{} 已经存在不同目标读音的规则", pattern_text),
            ));
        }
        sqlx::query(
            "UPDATE pronunciation_rules SET target_pinyin = ?, rule_type = ?, source = ?, verified = 1, enabled = 1, updated_at = ? WHERE id = ?",
        )
        .bind(&target_pinyin)
        .bind(normalize_rule_type(rule_type))
        .bind(source)
        .bind(&now)
        .bind(&existing.id)
        .execute(&mut *transaction)
        .await
        .map_err(transaction_error)?;
        existing.id
    } else {
        let id = Uuid::now_v7().to_string();
        sqlx::query(
            "INSERT INTO pronunciation_rules (id, scope, book_id, pattern_text, target_pinyin, rule_type, source, verified, enabled, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, 1, 1, ?, ?)",
        )
        .bind(&id)
        .bind(scope)
        .bind(book_id)
        .bind(&pattern_text)
        .bind(&target_pinyin)
        .bind(normalize_rule_type(rule_type))
        .bind(source)
        .bind(&now)
        .bind(&now)
        .execute(&mut *transaction)
        .await
        .map_err(transaction_error)?;
        id
    };
    let mut apply_result = PronunciationRuleApplyResult::default();
    apply_scope_tx(&mut transaction, scope, book_id, &mut apply_result).await?;
    transaction.commit().await.map_err(transaction_error)?;
    let rule = get_rule(database, &rule_id).await?;
    Ok(PronunciationRuleMutation { rule, apply_result })
}

pub async fn update_rule(
    database: &Database,
    rule_id: &str,
    pattern_text: &str,
    target_pinyin: &str,
    rule_type: &str,
    source: Option<&str>,
) -> AppResult<PronunciationRuleMutation> {
    let current = load_rule(database, rule_id).await?;
    let (pattern_text, target_pinyin, _) = validate_rule_input(
        &current.scope,
        current.book_id.as_deref(),
        pattern_text,
        target_pinyin,
    )?;
    let now = now_rfc3339()?;
    let mut transaction = database.pool().begin().await.map_err(transaction_error)?;
    if let Some(existing) = find_duplicate_rule(
        &mut transaction,
        &current.scope,
        current.book_id.as_deref(),
        &pattern_text,
        Some(rule_id),
    )
    .await?
    {
        return Err(AppError::new(
            "PRONUNCIATION_RULE_CONFLICT",
            format!("{} 已经存在另一条发音规则", existing.pattern_text),
        ));
    }
    sqlx::query(
        "UPDATE pronunciation_rules SET pattern_text = ?, target_pinyin = ?, rule_type = ?, source = ?, updated_at = ? WHERE id = ?",
    )
    .bind(&pattern_text)
    .bind(&target_pinyin)
    .bind(normalize_rule_type(rule_type))
    .bind(source)
    .bind(&now)
    .bind(rule_id)
    .execute(&mut *transaction)
    .await
    .map_err(transaction_error)?;
    let mut apply_result = PronunciationRuleApplyResult::default();
    apply_scope_tx(
        &mut transaction,
        &current.scope,
        current.book_id.as_deref(),
        &mut apply_result,
    )
    .await?;
    transaction.commit().await.map_err(transaction_error)?;
    Ok(PronunciationRuleMutation {
        rule: get_rule(database, rule_id).await?,
        apply_result,
    })
}

pub async fn set_rule_enabled(
    database: &Database,
    rule_id: &str,
    enabled: bool,
) -> AppResult<PronunciationRuleMutation> {
    let current = load_rule(database, rule_id).await?;
    let now = now_rfc3339()?;
    let mut transaction = database.pool().begin().await.map_err(transaction_error)?;
    sqlx::query("UPDATE pronunciation_rules SET enabled = ?, updated_at = ? WHERE id = ?")
        .bind(if enabled { 1_i64 } else { 0_i64 })
        .bind(&now)
        .bind(rule_id)
        .execute(&mut *transaction)
        .await
        .map_err(transaction_error)?;
    let mut apply_result = PronunciationRuleApplyResult::default();
    apply_scope_tx(
        &mut transaction,
        &current.scope,
        current.book_id.as_deref(),
        &mut apply_result,
    )
    .await?;
    transaction.commit().await.map_err(transaction_error)?;
    Ok(PronunciationRuleMutation {
        rule: get_rule(database, rule_id).await?,
        apply_result,
    })
}

pub async fn apply_pronunciation_rules_to_book(
    database: &Database,
    book_id: &str,
) -> AppResult<PronunciationRuleApplyResult> {
    book_service::get_book(database, book_id).await?;
    let mut transaction = database.pool().begin().await.map_err(transaction_error)?;
    let mut result = PronunciationRuleApplyResult::default();
    apply_book_tx(&mut transaction, book_id, &mut result).await?;
    transaction.commit().await.map_err(transaction_error)?;
    Ok(result)
}

pub async fn create_rule_from_annotation(
    database: &Database,
    annotation_id: &str,
    scope: &str,
) -> AppResult<PronunciationRuleMutation> {
    let annotation = pronunciation_service::get_annotation(database, annotation_id).await?;
    if annotation.review_status != "confirmed" {
        return Err(AppError::new(
            "PRONUNCIATION_RULE_REQUIRES_CONFIRMED",
            "只有已确认的 Annotation 才能创建发音规则",
        ));
    }
    let target_pinyin = annotation.target_pinyin.as_deref().ok_or_else(|| {
        AppError::new(
            "PRONUNCIATION_RULE_REQUIRES_CONFIRMED",
            "Annotation 缺少目标拼音",
        )
    })?;
    let book_id = book_service::get_book_id_for_segment(database, &annotation.segment_id).await?;
    let scoped_book_id = if scope == SCOPE_BOOK {
        Some(book_id.as_str())
    } else {
        None
    };
    create_rule(
        database,
        scope,
        scoped_book_id,
        &annotation.surface_text,
        target_pinyin,
        if annotation.risk_type == "manual" {
            "manual"
        } else {
            &annotation.risk_type
        },
        Some("manual"),
    )
    .await
}

pub async fn list_book_rules(
    database: &Database,
    book_id: &str,
) -> AppResult<Vec<PronunciationRule>> {
    book_service::get_book(database, book_id).await?;
    list_rules(database, Some(book_id), false).await
}

pub async fn list_global_rules(database: &Database) -> AppResult<Vec<PronunciationRule>> {
    list_rules(database, None, true).await
}

pub async fn get_rule(database: &Database, rule_id: &str) -> AppResult<PronunciationRule> {
    let row = sqlx::query_as::<
        _,
        (
            String,
            String,
            Option<String>,
            String,
            String,
            String,
            Option<String>,
            i64,
            i64,
            i64,
            String,
            String,
        ),
    >(
        "SELECT r.id, r.scope, r.book_id, r.pattern_text, r.target_pinyin, r.rule_type, r.source,
                r.verified, r.enabled,
                (SELECT COUNT(*) FROM segment_annotations a WHERE a.source_rule_id = r.id),
                r.created_at, r.updated_at
         FROM pronunciation_rules r WHERE r.id = ?",
    )
    .bind(rule_id)
    .fetch_optional(database.pool())
    .await?;
    row.map(rule_from_row)
        .transpose()?
        .ok_or_else(|| AppError::new("PRONUNCIATION_RULE_NOT_FOUND", "发音规则不存在"))
}

async fn list_rules(
    database: &Database,
    book_id: Option<&str>,
    global: bool,
) -> AppResult<Vec<PronunciationRule>> {
    let rows = if global {
        sqlx::query_as::<
            _,
            (
                String,
                String,
                Option<String>,
                String,
                String,
                String,
                Option<String>,
                i64,
                i64,
                i64,
                String,
                String,
            ),
        >(
            "SELECT r.id, r.scope, r.book_id, r.pattern_text, r.target_pinyin, r.rule_type, r.source,
                    r.verified, r.enabled,
                    (SELECT COUNT(*) FROM segment_annotations a WHERE a.source_rule_id = r.id),
                    r.created_at, r.updated_at
             FROM pronunciation_rules r WHERE r.scope = 'global' ORDER BY r.pattern_text, r.id",
        )
        .fetch_all(database.pool())
        .await?
    } else {
        sqlx::query_as::<
            _,
            (
                String,
                String,
                Option<String>,
                String,
                String,
                String,
                Option<String>,
                i64,
                i64,
                i64,
                String,
                String,
            ),
        >(
            "SELECT r.id, r.scope, r.book_id, r.pattern_text, r.target_pinyin, r.rule_type, r.source,
                    r.verified, r.enabled,
                    (SELECT COUNT(*) FROM segment_annotations a WHERE a.source_rule_id = r.id),
                    r.created_at, r.updated_at
             FROM pronunciation_rules r WHERE r.scope = 'book' AND r.book_id = ? ORDER BY r.pattern_text, r.id",
        )
        .bind(book_id)
        .fetch_all(database.pool())
        .await?
    };
    rows.into_iter().map(rule_from_row).collect()
}

async fn apply_scope_tx(
    transaction: &mut Transaction<'_, Sqlite>,
    scope: &str,
    book_id: Option<&str>,
    result: &mut PronunciationRuleApplyResult,
) -> AppResult<()> {
    if scope == SCOPE_BOOK {
        let book_id = book_id.ok_or_else(|| {
            AppError::new(
                "INVALID_PRONUNCIATION_RULE_SCOPE",
                "book 规则必须有 book_id",
            )
        })?;
        apply_book_tx(transaction, book_id, result).await
    } else {
        let book_ids = sqlx::query_scalar::<_, String>("SELECT id FROM books ORDER BY id")
            .fetch_all(&mut **transaction)
            .await
            .map_err(transaction_error)?;
        for book_id in book_ids {
            apply_book_tx(transaction, &book_id, result).await?;
        }
        Ok(())
    }
}

async fn apply_book_tx(
    transaction: &mut Transaction<'_, Sqlite>,
    book_id: &str,
    result: &mut PronunciationRuleApplyResult,
) -> AppResult<()> {
    let rules = active_rules_for_book_tx(transaction, book_id).await?;
    let segments = sqlx::query_as::<_, (String, String, i64, String, Option<String>, i64, String, Option<String>, String, String)>(
        "SELECT s.id, s.chapter_id, s.order_index, s.original_text, s.reading_text, s.speak_enabled, s.status, s.current_audio_id, s.created_at, s.updated_at
         FROM segments s JOIN chapters c ON c.id = s.chapter_id
         WHERE c.book_id = ? AND s.status <> 'superseded' ORDER BY c.order_index, s.order_index",
    )
    .bind(book_id)
    .fetch_all(&mut **transaction)
    .await
    .map_err(transaction_error)?;
    for (
        id,
        chapter_id,
        order_index,
        original_text,
        reading_text,
        speak_enabled,
        status,
        current_audio_id,
        created_at,
        updated_at,
    ) in segments
    {
        let segment = Segment {
            id,
            chapter_id,
            order_index,
            original_text,
            reading_text,
            speak_enabled: speak_enabled != 0,
            status,
            current_audio_id,
            created_at,
            updated_at,
        };
        let tokens = pronunciation_service::grapheme_tokens(segment.effective_text());
        let before =
            pronunciation_service::effective_signature_tx(transaction, &segment.id).await?;
        apply_rules_to_segment_with_rules_tx(transaction, &segment, &tokens, &rules, result)
            .await?;
        let after = pronunciation_service::effective_signature_tx(transaction, &segment.id).await?;
        pronunciation_service::recalculate_segment_status_tx(
            transaction,
            &segment.id,
            before != after,
        )
        .await?;
    }
    Ok(())
}

pub(crate) async fn apply_active_rules_to_segment_tx(
    transaction: &mut Transaction<'_, Sqlite>,
    segment: &Segment,
    tokens: &[GraphemeToken],
    result: &mut PronunciationRuleApplyResult,
) -> AppResult<()> {
    let book_id = book_service::get_book_id_for_segment_tx(transaction, &segment.id).await?;
    let rules = active_rules_for_book_tx(transaction, &book_id).await?;
    apply_rules_to_segment_with_rules_tx(transaction, segment, tokens, &rules, result).await
}

async fn apply_rules_to_segment_with_rules_tx(
    transaction: &mut Transaction<'_, Sqlite>,
    segment: &Segment,
    tokens: &[GraphemeToken],
    rules: &[RuleRecord],
    result: &mut PronunciationRuleApplyResult,
) -> AppResult<()> {
    let existing = load_segment_annotations_tx(transaction, &segment.id).await?;
    let manual_ranges: Vec<(usize, usize)> = existing
        .iter()
        .filter(|annotation| {
            annotation.source_rule_id.is_none()
                && matches!(annotation.review_status.as_str(), "confirmed" | "ignored")
        })
        .map(|annotation| (annotation.start_token, annotation.end_token))
        .collect();
    let matches = find_matches(tokens, rules);
    if !matches.is_empty() {
        result.matched_segments += 1;
    }
    let mut desired = Vec::new();
    for matched in matches {
        if manual_ranges.iter().any(|(start, end)| {
            pronunciation_service::ranges_overlap(
                matched.start_token,
                matched.end_token,
                *start,
                *end,
            )
        }) {
            result.skipped_manual_overrides += 1;
            continue;
        }
        desired.push(matched);
    }

    for annotation in existing
        .iter()
        .filter(|annotation| annotation.source_rule_id.is_some())
    {
        let keep = desired.iter().any(|matched| {
            annotation.source_rule_id.as_deref() == Some(matched.rule.id.as_str())
                && annotation.start_token == matched.start_token
                && annotation.end_token == matched.end_token
        });
        if !keep {
            sqlx::query("DELETE FROM segment_annotations WHERE id = ?")
                .bind(&annotation.id)
                .execute(&mut **transaction)
                .await
                .map_err(transaction_error)?;
        }
    }

    let pending = existing
        .iter()
        .filter(|annotation| {
            annotation.source_rule_id.is_none() && annotation.review_status == "needs_review"
        })
        .filter(|annotation| {
            desired.iter().any(|matched| {
                pronunciation_service::ranges_overlap(
                    annotation.start_token,
                    annotation.end_token,
                    matched.start_token,
                    matched.end_token,
                )
            })
        })
        .map(|annotation| annotation.id.clone())
        .collect::<Vec<_>>();
    for annotation_id in pending {
        sqlx::query("DELETE FROM segment_annotations WHERE id = ?")
            .bind(annotation_id)
            .execute(&mut **transaction)
            .await
            .map_err(transaction_error)?;
    }

    for matched in desired {
        let current = existing.iter().find(|annotation| {
            annotation.source_rule_id.as_deref() == Some(matched.rule.id.as_str())
                && annotation.start_token == matched.start_token
                && annotation.end_token == matched.end_token
        });
        let now = pronunciation_service::now_rfc3339()?;
        if let Some(current) = current {
            if current.target_pinyin.as_deref() != Some(matched.rule.target_pinyin.as_str()) {
                sqlx::query("UPDATE segment_annotations SET target_pinyin = ?, candidates_json = ?, risk_type = ?, reason = ?, review_status = 'confirmed', analyzer_version = NULL, source = 'pronunciation_rule', rule_type = ?, confidence = 'verified', updated_at = ? WHERE id = ?")
                    .bind(&matched.rule.target_pinyin)
                    .bind(serde_json::to_string(&vec![matched.rule.target_pinyin.clone()]).map_err(|error| AppError::new("DB_TRANSACTION_FAILED", error.to_string()))?)
                    .bind(&matched.rule.rule_type)
                    .bind(rule_reason(&matched.rule))
                    .bind(&matched.rule.rule_type)
                    .bind(&now)
                    .bind(&current.id)
                    .execute(&mut **transaction)
                    .await
                    .map_err(transaction_error)?;
                result.updated_annotations += 1;
            }
        } else {
            let annotation = Annotation {
                id: Uuid::now_v7().to_string(),
                segment_id: segment.id.clone(),
                start_token: matched.start_token,
                end_token: matched.end_token,
                surface_text: tokens[matched.start_token..matched.end_token]
                    .iter()
                    .map(|token| token.text.as_str())
                    .collect(),
                default_pinyin: None,
                target_pinyin: Some(matched.rule.target_pinyin.clone()),
                candidate_pinyin: vec![matched.rule.target_pinyin.clone()],
                risk_type: matched.rule.rule_type.clone(),
                reason: Some(rule_reason(&matched.rule)),
                review_status: "confirmed".to_string(),
                analyzer_version: None,
                source: Some("pronunciation_rule".to_string()),
                rule_type: Some(matched.rule.rule_type.clone()),
                confidence: Some("verified".to_string()),
                source_rule_id: Some(matched.rule.id.clone()),
                created_at: now.clone(),
                updated_at: now.clone(),
            };
            pronunciation_service::insert_annotation(transaction, &annotation, &now).await?;
            result.created_annotations += 1;
        }
    }
    Ok(())
}

async fn active_rules_for_book_tx(
    transaction: &mut Transaction<'_, Sqlite>,
    book_id: &str,
) -> AppResult<Vec<RuleRecord>> {
    let rows = sqlx::query_as::<_, (String, String, Option<String>, String, String, String, Option<String>, i64, i64, String, String)>(
        "SELECT id, scope, book_id, pattern_text, target_pinyin, rule_type, source, verified, enabled, created_at, updated_at
         FROM pronunciation_rules WHERE enabled = 1 AND
         (scope = 'global' OR (scope = 'book' AND book_id = ?))
         ORDER BY pattern_text, scope, id",
    )
    .bind(book_id)
    .fetch_all(&mut **transaction)
    .await
    .map_err(transaction_error)?;
    rows.into_iter().map(rule_record_from_row).collect()
}

fn find_matches(tokens: &[GraphemeToken], rules: &[RuleRecord]) -> Vec<RuleMatch> {
    let mut matches = Vec::new();
    let mut position = 0;
    while position < tokens.len() {
        let mut candidates = rules
            .iter()
            .filter(|rule| {
                position + rule.pattern_tokens.len() <= tokens.len()
                    && rule
                        .pattern_tokens
                        .iter()
                        .enumerate()
                        .all(|(offset, text)| tokens[position + offset].text == *text)
            })
            .collect::<Vec<_>>();
        candidates.sort_by(|left, right| {
            right
                .pattern_tokens
                .len()
                .cmp(&left.pattern_tokens.len())
                .then_with(|| scope_priority(&right.scope).cmp(&scope_priority(&left.scope)))
                .then_with(|| left.id.cmp(&right.id))
        });
        if let Some(rule) = candidates.first() {
            let end = position + rule.pattern_tokens.len();
            matches.push(RuleMatch {
                rule: (*rule).clone(),
                start_token: position,
                end_token: end,
            });
            position = end;
        } else {
            position += 1;
        }
    }
    matches
}

async fn load_segment_annotations_tx(
    transaction: &mut Transaction<'_, Sqlite>,
    segment_id: &str,
) -> AppResult<Vec<ExistingRuleAnnotation>> {
    Ok(
        sqlx::query_as::<_, (String, i64, i64, String, Option<String>, Option<String>)>(
            "SELECT id, start_token, end_token, review_status, target_pinyin, source_rule_id
         FROM segment_annotations WHERE segment_id = ? ORDER BY start_token, end_token, id",
        )
        .bind(segment_id)
        .fetch_all(&mut **transaction)
        .await
        .map_err(transaction_error)?
        .into_iter()
        .map(
            |(id, start_token, end_token, review_status, target_pinyin, source_rule_id)| {
                ExistingRuleAnnotation {
                    id,
                    start_token: start_token as usize,
                    end_token: end_token as usize,
                    target_pinyin,
                    source_rule_id,
                    review_status,
                }
            },
        )
        .collect(),
    )
}

fn validate_rule_input(
    scope: &str,
    book_id: Option<&str>,
    pattern_text: &str,
    target_pinyin: &str,
) -> AppResult<(String, String, Vec<String>)> {
    match scope {
        SCOPE_BOOK if book_id.is_some_and(|value| !value.trim().is_empty()) => {}
        SCOPE_GLOBAL if book_id.is_none() => {}
        SCOPE_BOOK => {
            return Err(AppError::new(
                "INVALID_PRONUNCIATION_RULE_SCOPE",
                "book 规则必须指定 book_id",
            ))
        }
        SCOPE_GLOBAL => {
            return Err(AppError::new(
                "INVALID_PRONUNCIATION_RULE_SCOPE",
                "global 规则不能指定 book_id",
            ))
        }
        _ => {
            return Err(AppError::new(
                "INVALID_PRONUNCIATION_RULE_SCOPE",
                "规则 scope 只能是 book 或 global",
            ))
        }
    }
    let pattern_text = pattern_text.trim().to_string();
    let pattern_tokens = pronunciation_service::grapheme_tokens(&pattern_text);
    if pattern_tokens.is_empty()
        || !pattern_tokens
            .iter()
            .any(|token| pronunciation_service::is_han_token(&token.text))
    {
        return Err(AppError::new(
            "INVALID_PRONUNCIATION_RULE_PATTERN",
            "规则 pattern 必须至少包含一个汉字",
        ));
    }
    let target_pinyin = pronunciation_service::normalize_and_validate_pinyin(target_pinyin)?;
    pronunciation_service::validate_pinyin_token_count(
        &pattern_tokens,
        0,
        pattern_tokens.len(),
        &target_pinyin,
    )?;
    Ok((
        pattern_text,
        target_pinyin,
        pattern_tokens.into_iter().map(|token| token.text).collect(),
    ))
}

fn normalize_rule_type(rule_type: &str) -> String {
    if rule_type.trim().is_empty() {
        "manual".to_string()
    } else {
        rule_type.trim().to_string()
    }
}

fn rule_reason(rule: &RuleRecord) -> String {
    format!("发音规则：{}", rule.source.as_deref().unwrap_or("用户词典"))
}

fn scope_priority(scope: &str) -> u8 {
    if scope == SCOPE_BOOK {
        2
    } else {
        1
    }
}

async fn find_duplicate_rule(
    transaction: &mut Transaction<'_, Sqlite>,
    scope: &str,
    book_id: Option<&str>,
    pattern_text: &str,
    exclude_id: Option<&str>,
) -> AppResult<Option<RuleRecord>> {
    let mut rules = if scope == SCOPE_GLOBAL {
        sqlx::query_as::<_, (String, String, Option<String>, String, String, String, Option<String>, i64, i64, String, String)>(
            "SELECT id, scope, book_id, pattern_text, target_pinyin, rule_type, source, verified, enabled, created_at, updated_at
             FROM pronunciation_rules WHERE scope = 'global' AND book_id IS NULL AND pattern_text = ?",
        )
        .bind(pattern_text)
        .fetch_all(&mut **transaction)
        .await
        .map_err(transaction_error)?
    } else {
        sqlx::query_as::<_, (String, String, Option<String>, String, String, String, Option<String>, i64, i64, String, String)>(
            "SELECT id, scope, book_id, pattern_text, target_pinyin, rule_type, source, verified, enabled, created_at, updated_at
             FROM pronunciation_rules WHERE scope = 'book' AND book_id = ? AND pattern_text = ?",
        )
        .bind(book_id)
        .bind(pattern_text)
        .fetch_all(&mut **transaction)
        .await
        .map_err(transaction_error)?
    };
    if let Some(exclude_id) = exclude_id {
        rules.retain(|row| row.0 != exclude_id);
    }
    rules
        .into_iter()
        .next()
        .map(rule_record_from_row)
        .transpose()
}

async fn load_rule(database: &Database, rule_id: &str) -> AppResult<RuleRecord> {
    let row = sqlx::query_as::<_, (String, String, Option<String>, String, String, String, Option<String>, i64, i64, String, String)>(
        "SELECT id, scope, book_id, pattern_text, target_pinyin, rule_type, source, verified, enabled, created_at, updated_at
         FROM pronunciation_rules WHERE id = ?",
    )
    .bind(rule_id)
    .fetch_optional(database.pool())
    .await?;
    row.map(rule_record_from_row)
        .transpose()?
        .ok_or_else(|| AppError::new("PRONUNCIATION_RULE_NOT_FOUND", "发音规则不存在"))
}

fn rule_record_from_row(
    row: (
        String,
        String,
        Option<String>,
        String,
        String,
        String,
        Option<String>,
        i64,
        i64,
        String,
        String,
    ),
) -> AppResult<RuleRecord> {
    let (
        id,
        scope,
        book_id,
        pattern_text,
        target_pinyin,
        rule_type,
        source,
        _verified,
        _enabled,
        _created_at,
        _updated_at,
    ) = row;
    let pattern_tokens = pronunciation_service::grapheme_tokens(&pattern_text)
        .into_iter()
        .map(|token| token.text)
        .collect();
    Ok(RuleRecord {
        id,
        scope,
        book_id,
        pattern_text,
        pattern_tokens,
        target_pinyin,
        rule_type,
        source,
    })
}

fn rule_from_row(
    row: (
        String,
        String,
        Option<String>,
        String,
        String,
        String,
        Option<String>,
        i64,
        i64,
        i64,
        String,
        String,
    ),
) -> AppResult<PronunciationRule> {
    let (
        id,
        scope,
        book_id,
        pattern_text,
        target_pinyin,
        rule_type,
        source,
        verified,
        enabled,
        application_count,
        created_at,
        updated_at,
    ) = row;
    Ok(PronunciationRule {
        id,
        scope,
        book_id,
        pattern_text,
        target_pinyin,
        rule_type,
        source,
        verified: verified != 0,
        enabled: enabled != 0,
        application_count,
        created_at,
        updated_at,
    })
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
        apply_pronunciation_rules_to_book, create_rule, get_rule, set_rule_enabled, update_rule,
    };
    use crate::{
        db::Database,
        models::Segment,
        services::{book_service, pronunciation_service},
    };
    use uuid::Uuid;

    fn temp_db() -> Database {
        let path = std::env::temp_dir().join(format!(
            "ancient-pronunciation-rules-{}.sqlite",
            Uuid::now_v7()
        ));
        tauri::async_runtime::block_on(Database::open(path)).expect("database should initialize")
    }

    async fn seed_book(database: &Database, texts: &[&str]) -> (String, Vec<Segment>) {
        let now = "2026-01-01T00:00:00Z";
        let book_id = Uuid::now_v7().to_string();
        let chapter_id = Uuid::now_v7().to_string();
        sqlx::query("INSERT INTO books (id, title, created_at, updated_at) VALUES (?, ?, ?, ?)")
            .bind(&book_id)
            .bind("规则测试书")
            .bind(now)
            .bind(now)
            .execute(database.pool())
            .await
            .expect("book");
        sqlx::query("INSERT INTO chapters (id, book_id, title, order_index, created_at, updated_at) VALUES (?, ?, ?, 0, ?, ?)")
            .bind(&chapter_id).bind(&book_id).bind("正文").bind(now).bind(now)
            .execute(database.pool()).await.expect("chapter");
        let mut segments = Vec::new();
        for (index, text) in texts.iter().enumerate() {
            let segment_id = Uuid::now_v7().to_string();
            sqlx::query("INSERT INTO segments (id, chapter_id, order_index, original_text, status, created_at, updated_at) VALUES (?, ?, ?, ?, 'pending', ?, ?)")
                .bind(&segment_id).bind(&chapter_id).bind(index as i64).bind(text).bind(now).bind(now)
                .execute(database.pool()).await.expect("segment");
            segments.push(
                book_service::get_segment(database, &segment_id)
                    .await
                    .expect("segment"),
            );
        }
        (book_id, segments)
    }

    fn annotation<'a>(
        reader: &'a crate::models::SegmentReader,
        surface: &str,
    ) -> &'a crate::models::Annotation {
        reader
            .annotations
            .iter()
            .find(|item| item.surface_text == surface)
            .expect("annotation")
    }

    #[test]
    fn exact_match_uses_grapheme_tokens_longest_match_and_scope_priority() {
        let database = temp_db();
        tauri::async_runtime::block_on(async {
            let (book_id, segments) = seed_book(&database, &["腧穴说𠮷"]).await;
            let global_say = create_rule(
                &database,
                "global",
                None,
                "说",
                "shuo1",
                "manual",
                Some("test"),
            )
            .await
            .expect("global rule")
            .rule;
            let global_supplementary = create_rule(
                &database,
                "global",
                None,
                "𠮷",
                "ji2",
                "manual",
                Some("test"),
            )
            .await
            .expect("supplementary rule")
            .rule;
            let book_single = create_rule(
                &database,
                "book",
                Some(&book_id),
                "腧",
                "shu4",
                "manual",
                Some("test"),
            )
            .await
            .expect("short rule")
            .rule;
            let book_say = create_rule(
                &database,
                "book",
                Some(&book_id),
                "说",
                "yue4",
                "manual",
                Some("test"),
            )
            .await
            .expect("book say rule")
            .rule;
            let book_word = create_rule(
                &database,
                "book",
                Some(&book_id),
                "腧穴",
                "shu4 xue2",
                "manual",
                Some("test"),
            )
            .await
            .expect("long rule")
            .rule;
            let reader = pronunciation_service::get_segment_reader(&database, &segments[0].id)
                .await
                .expect("reader");
            let word = annotation(&reader, "腧穴");
            assert_eq!(word.start_token, 0);
            assert_eq!(word.end_token, 2);
            assert_eq!(word.target_pinyin.as_deref(), Some("shu4 xue2"));
            assert_eq!(word.source_rule_id.as_deref(), Some(book_word.id.as_str()));
            assert!(!reader
                .annotations
                .iter()
                .any(|item| item.surface_text == "腧"));
            assert_eq!(
                annotation(&reader, "说").target_pinyin.as_deref(),
                Some("yue4")
            );
            assert_eq!(
                annotation(&reader, "说").source_rule_id.as_deref(),
                Some(book_say.id.as_str())
            );
            assert_eq!(
                annotation(&reader, "𠮷").source_rule_id.as_deref(),
                Some(global_supplementary.id.as_str())
            );
            assert_eq!(global_say.target_pinyin, "shuo1");
            assert_eq!(book_single.target_pinyin, "shu4");
        });
    }

    #[test]
    fn manual_confirmed_and_ignored_annotations_are_never_overwritten() {
        let database = temp_db();
        tauri::async_runtime::block_on(async {
            let (book_id, segments) = seed_book(&database, &["说", "行"]).await;
            let manual_seed = create_rule(
                &database,
                "global",
                None,
                "说",
                "shuo1",
                "manual",
                Some("test"),
            )
            .await
            .expect("seed rule")
            .rule;
            let initial = pronunciation_service::get_segment_reader(&database, &segments[0].id)
                .await
                .expect("reader");
            let manual = pronunciation_service::confirm_annotation(
                &database,
                &initial.annotations[0].id,
                "shui4",
            )
            .await
            .expect("manual override");
            let book_rule = create_rule(
                &database,
                "book",
                Some(&book_id),
                "说",
                "yue4",
                "manual",
                Some("test"),
            )
            .await
            .expect("book rule");
            let after = pronunciation_service::get_segment_reader(&database, &segments[0].id)
                .await
                .expect("reader");
            assert_eq!(after.annotations.len(), 1);
            assert_eq!(after.annotations[0].target_pinyin.as_deref(), Some("shui4"));
            assert!(after.annotations[0].source_rule_id.is_none());
            assert!(book_rule.apply_result.skipped_manual_overrides > 0);
            assert_eq!(
                manual.annotations[0].target_pinyin.as_deref(),
                Some("shui4")
            );

            let ignored_seed = pronunciation_service::create_manual_annotation(
                &database,
                &segments[1].id,
                0,
                "hang2",
            )
            .await
            .expect("manual");
            pronunciation_service::ignore_annotation(&database, &ignored_seed.annotations[0].id)
                .await
                .expect("ignore");
            let ignored_rule = create_rule(
                &database,
                "global",
                None,
                "行",
                "xing2",
                "manual",
                Some("test"),
            )
            .await
            .expect("global row rule");
            let ignored_reader =
                pronunciation_service::get_segment_reader(&database, &segments[1].id)
                    .await
                    .expect("reader");
            assert_eq!(ignored_reader.annotations[0].review_status, "ignored");
            assert!(ignored_reader.annotations[0].target_pinyin.is_none());
            assert!(ignored_reader.annotations[0].source_rule_id.is_none());
            assert!(ignored_rule.apply_result.skipped_manual_overrides > 0);
            let _ = manual_seed;
        });
    }

    #[test]
    fn updating_a_rule_only_updates_annotations_still_bound_to_it() {
        let database = temp_db();
        tauri::async_runtime::block_on(async {
            let texts = ["腧穴"; 10];
            let (book_id, segments) = seed_book(&database, &texts).await;
            let created = create_rule(
                &database,
                "book",
                Some(&book_id),
                "腧穴",
                "shu4 xue2",
                "manual",
                Some("test"),
            )
            .await
            .expect("rule");
            let before = apply_pronunciation_rules_to_book(&database, &book_id)
                .await
                .expect("apply");
            assert_eq!(before.matched_segments, 10);
            let first_reader =
                pronunciation_service::get_segment_reader(&database, &segments[0].id)
                    .await
                    .expect("reader");
            pronunciation_service::confirm_annotation(
                &database,
                &first_reader.annotations[0].id,
                "yu4 xue2",
            )
            .await
            .expect("detach");
            let updated = update_rule(
                &database,
                &created.rule.id,
                "腧穴",
                "yu4 xue2",
                "manual",
                Some("test"),
            )
            .await
            .expect("update");
            assert_eq!(updated.apply_result.updated_annotations, 9);
            let detached = pronunciation_service::get_segment_reader(&database, &segments[0].id)
                .await
                .expect("reader");
            assert_eq!(
                detached.annotations[0].target_pinyin.as_deref(),
                Some("yu4 xue2")
            );
            assert!(detached.annotations[0].source_rule_id.is_none());
            let bound = pronunciation_service::get_segment_reader(&database, &segments[1].id)
                .await
                .expect("reader");
            assert_eq!(
                bound.annotations[0].source_rule_id.as_deref(),
                Some(created.rule.id.as_str())
            );
            assert_eq!(
                bound.annotations[0].target_pinyin.as_deref(),
                Some("yu4 xue2")
            );
        });
    }

    #[test]
    fn disabling_book_rule_reveals_global_rule_and_stales_changed_audio() {
        let database = temp_db();
        tauri::async_runtime::block_on(async {
            let (book_id, segments) = seed_book(&database, &["腧穴", "说"]).await;
            let global = create_rule(
                &database,
                "global",
                None,
                "腧穴",
                "shu4 xue2",
                "manual",
                Some("test"),
            )
            .await
            .expect("global")
            .rule;
            let book = create_rule(
                &database,
                "book",
                Some(&book_id),
                "腧穴",
                "yu4 xue2",
                "manual",
                Some("test"),
            )
            .await
            .expect("book")
            .rule;
            let book_reader = pronunciation_service::get_segment_reader(&database, &segments[0].id)
                .await
                .expect("reader");
            assert_eq!(
                book_reader.annotations[0].source_rule_id.as_deref(),
                Some(book.id.as_str())
            );
            sqlx::query("UPDATE segments SET status = 'generated', current_audio_id = 'audio-kept' WHERE id = ?").bind(&segments[0].id).execute(database.pool()).await.expect("generated");
            let updated = update_rule(
                &database,
                &book.id,
                "腧穴",
                "lu4 xue2",
                "manual",
                Some("test"),
            )
            .await
            .expect("update");
            assert!(updated.apply_result.updated_annotations > 0);
            let stale = book_service::get_segment(&database, &segments[0].id)
                .await
                .expect("segment");
            assert_eq!(stale.status, "ready");
            assert_eq!(stale.current_audio_id.as_deref(), Some("audio-kept"));
            let fallback = set_rule_enabled(&database, &book.id, false)
                .await
                .expect("disable");
            let after_disable =
                pronunciation_service::get_segment_reader(&database, &segments[0].id)
                    .await
                    .expect("reader");
            assert_eq!(
                after_disable.annotations[0].source_rule_id.as_deref(),
                Some(global.id.as_str())
            );
            assert_eq!(
                after_disable.annotations[0].target_pinyin.as_deref(),
                Some("shu4 xue2")
            );
            assert!(fallback.apply_result.created_annotations > 0);
        });
    }

    #[test]
    fn reanalysis_filters_rule_overlap_without_calling_a_second_analyzer() {
        let database = temp_db();
        tauri::async_runtime::block_on(async {
            let (book_id, segments) = seed_book(&database, &["腧穴"]).await;
            let rule = create_rule(
                &database,
                "book",
                Some(&book_id),
                "腧穴",
                "shu4 xue2",
                "manual",
                Some("test"),
            )
            .await
            .expect("rule")
            .rule;
            let segment = &segments[0];
            let tokens = pronunciation_service::grapheme_tokens(segment.effective_text());
            let analysis = serde_json::json!({
                "analyzer_version": "0.1.0",
                "items": [{"start_token": 0, "end_token": 2, "surface_text": "腧穴", "default_pinyin": "shu4 xue2", "candidate_pinyin": ["shu4 xue2"], "risk_type": "medical_term", "reason": "analyzer"}]
            });
            let reader =
                pronunciation_service::apply_analysis(&database, segment, &tokens, analysis)
                    .await
                    .expect("reanalyze");
            assert_eq!(reader.annotations.len(), 1);
            assert_eq!(
                reader.annotations[0].source_rule_id.as_deref(),
                Some(rule.id.as_str())
            );
            assert_eq!(reader.annotations[0].review_status, "confirmed");
            assert_eq!(reader.segment.status, "ready");
        });
    }

    #[test]
    fn rule_input_rejects_invalid_pattern_count_and_conflict() {
        let database = temp_db();
        tauri::async_runtime::block_on(async {
            let (book_id, _) = seed_book(&database, &["腧穴"]).await;
            let error = create_rule(
                &database,
                "book",
                Some(&book_id),
                "。",
                "ju4",
                "manual",
                None,
            )
            .await
            .expect_err("punctuation");
            assert_eq!(error.code, "INVALID_PRONUNCIATION_RULE_PATTERN");
            let error = create_rule(
                &database,
                "book",
                Some(&book_id),
                "腧穴",
                "shu4",
                "manual",
                None,
            )
            .await
            .expect_err("count");
            assert_eq!(error.code, "PINYIN_TOKEN_COUNT_MISMATCH");
            create_rule(
                &database,
                "book",
                Some(&book_id),
                "腧穴",
                "shu4 xue2",
                "manual",
                None,
            )
            .await
            .expect("first");
            let error = create_rule(
                &database,
                "book",
                Some(&book_id),
                "腧穴",
                "yu4 xue2",
                "manual",
                None,
            )
            .await
            .expect_err("conflict");
            assert_eq!(error.code, "PRONUNCIATION_RULE_CONFLICT");
            let rules = super::list_book_rules(&database, &book_id)
                .await
                .expect("rules");
            assert_eq!(rules.len(), 1);
            assert!(get_rule(&database, &rules[0].id).await.is_ok());
        });
    }

    #[test]
    fn huangdi_neijing_fixture_rule_acceptance_metrics() {
        let database = temp_db();
        tauri::async_runtime::block_on(async {
            let path = format!(
                "{}/../tests/fixtures/pronunciation_classic.txt",
                env!("CARGO_MANIFEST_DIR")
            );
            let imported = book_service::import_txt_book(
                &database,
                &path,
                Some("黄帝内经·素问测试节选".to_string()),
            )
            .await
            .expect("fixture import");
            let terms = [
                ("天癸", "tian1 gui3"),
                ("肾气", "shen4 qi4"),
                ("筋骨", "jin1 gu3"),
                ("经脉", "jing1 mai4"),
                ("百岁", "bai3 sui4"),
            ];
            let segments = book_service::list_segments(&database, &imported.chapter.id, 0, 100)
                .await
                .expect("fixture segments");
            for (index, (pattern, _)) in terms.iter().enumerate() {
                let pattern_tokens = pronunciation_service::grapheme_tokens(pattern);
                let occurrence = segments
                    .items
                    .iter()
                    .find_map(|segment| {
                        let tokens =
                            pronunciation_service::grapheme_tokens(segment.effective_text());
                        tokens.windows(pattern_tokens.len()).enumerate().find_map(
                            |(start, window)| {
                                (window
                                    .iter()
                                    .map(|token| token.text.as_str())
                                    .collect::<String>()
                                    == *pattern)
                                    .then_some((segment, start, start + pattern_tokens.len()))
                            },
                        )
                    })
                    .expect("fixture term occurrence");
                sqlx::query("INSERT INTO segment_annotations (id, segment_id, start_token, end_token, surface_text, candidates_json, risk_type, reason, review_status, analyzer_version, created_at, updated_at) VALUES (?, ?, ?, ?, ?, '[]', 'medical_term', 'fixture analyzer', 'needs_review', '0.1.0', ?, ?)")
                    .bind(Uuid::now_v7().to_string())
                    .bind(&occurrence.0.id)
                    .bind(occurrence.1 as i64)
                    .bind(occurrence.2 as i64)
                    .bind(pattern)
                    .bind("2026-01-01T00:00:00Z")
                    .bind("2026-01-01T00:00:00Z")
                    .execute(database.pool())
                    .await
                    .unwrap_or_else(|error| panic!("fixture annotation {index}: {error}"));
            }
            let needs_before: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM segment_annotations WHERE review_status = 'needs_review'",
            )
            .fetch_one(database.pool())
            .await
            .expect("needs review before");
            let mut matched_segments = 0_i64;
            let mut applied_annotations = 0_i64;
            for (pattern, pinyin) in terms {
                let mutation = create_rule(
                    &database,
                    "book",
                    Some(&imported.book.book.id),
                    pattern,
                    pinyin,
                    "medical_term",
                    Some("classic_fixture_manual"),
                )
                .await
                .expect("fixture rule");
                matched_segments += mutation.apply_result.matched_segments;
                applied_annotations += mutation.rule.application_count;
                assert!(
                    mutation.rule.application_count >= 2,
                    "{pattern} should repeat in the classic fixture"
                );
            }
            let overrides: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM segment_annotations WHERE source_rule_id IS NULL AND review_status IN ('confirmed', 'ignored')")
                .fetch_one(database.pool()).await.expect("override count");
            let needs_review: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM segment_annotations WHERE review_status = 'needs_review'",
            )
            .fetch_one(database.pool())
            .await
            .expect("needs review count");
            println!("classic fixture acceptance: rules={}, matched_segments={}, applied_annotations={}, manual_overrides={}, needs_review_reduction={}", terms.len(), matched_segments, applied_annotations, overrides, needs_before - needs_review);
            assert_eq!(overrides, 0);
            assert_eq!(needs_before, 5);
            assert_eq!(needs_review, 0);
        });
    }

    #[test]
    fn applies_a_rule_to_three_thousand_segments() {
        let database = temp_db();
        tauri::async_runtime::block_on(async {
            let texts = vec!["腧穴"; 3000];
            let (book_id, _) = seed_book(&database, &texts).await;
            create_rule(
                &database,
                "book",
                Some(&book_id),
                "腧穴",
                "shu4 xue2",
                "manual",
                Some("performance_test"),
            )
            .await
            .expect("rule");
            let start = std::time::Instant::now();
            let result = apply_pronunciation_rules_to_book(&database, &book_id)
                .await
                .expect("apply");
            println!(
                "rule performance: segments=3000, matched_segments={}, elapsed_ms={}",
                result.matched_segments,
                start.elapsed().as_millis()
            );
            assert_eq!(result.matched_segments, 3000);
        });
    }
}
