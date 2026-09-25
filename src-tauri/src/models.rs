use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Clone)]
pub struct ComponentStatus {
    pub ok: bool,
    pub message: String,
}

#[derive(Debug, Serialize, Clone)]
pub struct WorkerStatus {
    pub ok: bool,
    pub running: bool,
    pub version: Option<String>,
    pub message: String,
}

#[derive(Debug, Serialize, Clone)]
pub struct DeveloperStatus {
    pub application: ComponentStatus,
    pub database: ComponentStatus,
    pub worker: WorkerStatus,
}

#[derive(Debug, Serialize, Clone)]
pub struct WorkerPing {
    pub version: String,
    pub elapsed_ms: u128,
}

#[derive(Debug, Serialize, Clone)]
pub struct CredentialStatus {
    pub secret_id_configured: bool,
    pub secret_key_configured: bool,
}

#[derive(Debug, Serialize, Clone)]
pub struct TencentVoice {
    pub id: i64,
    pub name: String,
    pub category: String,
    pub description: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TtsSettings {
    pub provider: String,
    pub voice_type: i64,
    pub speed: f64,
    pub volume: f64,
    pub sample_rate: i64,
    pub codec: String,
}

#[derive(Debug, Serialize, Clone)]
pub struct ApiUsageVoiceSummary {
    pub voice_type: i64,
    pub characters: i64,
}

#[derive(Debug, Serialize, Clone)]
pub struct ApiUsageTtsSummary {
    pub requests: i64,
    pub success: i64,
    pub failed: i64,
    pub characters: i64,
    pub by_voice: Vec<ApiUsageVoiceSummary>,
}

#[derive(Debug, Serialize, Clone)]
pub struct ApiUsageSummary {
    pub range: String,
    pub tts: ApiUsageTtsSummary,
}

#[derive(Debug, Serialize, Clone)]
pub struct TtsPreviewResult {
    pub audio_path: String,
    pub duration_ms: Option<i64>,
    pub voice_type: i64,
    pub speed: f64,
    pub volume: f64,
    pub sample_rate: i64,
}

#[derive(Debug, Serialize, Clone, PartialEq, Eq)]
pub struct ReaderDisplaySettings {
    pub font_size: i64,
    pub pinyin_mode: String,
}

#[derive(Debug, Serialize, Clone)]
pub struct SegmentDisplayPinyin {
    pub token_pinyin: Vec<Option<String>>,
}

#[derive(Debug, Serialize, Clone)]
pub struct BookTextStats {
    pub han_character_count: i64,
    pub chapter_count: i64,
    pub segment_count: i64,
}

#[derive(Debug, Serialize, Clone)]
pub struct BatchGenerationBlocker {
    pub code: String,
    pub message: String,
    pub segment_id: Option<String>,
    pub chapter_title: Option<String>,
    pub segment_order: Option<i64>,
    pub preview: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
pub struct BookGenerationPreflight {
    pub can_generate: bool,
    pub han_character_count: i64,
    pub total_segments: i64,
    pub pending_segments: i64,
    pub needs_review_segments: i64,
    pub generated_and_reusable: i64,
    pub need_generation: i64,
    pub blockers: Vec<BatchGenerationBlocker>,
}

#[derive(Debug, Serialize, Clone)]
pub struct BatchFailedSegment {
    pub segment_id: String,
    pub chapter_title: Option<String>,
    pub segment_order: i64,
    pub preview: String,
    pub code: String,
    pub message: String,
}

#[derive(Debug, Serialize, Clone)]
pub struct BatchFatalError {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Serialize, Clone)]
pub struct BatchGenerationState {
    pub book_id: Option<String>,
    pub status: String,
    pub total_segments: i64,
    pub segments_requiring_generation: i64,
    pub processed: i64,
    pub generated: i64,
    pub skipped: i64,
    pub failed: i64,
    pub current_segment_id: Option<String>,
    pub current_segment_order: Option<i64>,
    pub current_preview: Option<String>,
    pub has_failures: bool,
    pub failed_segments: Vec<BatchFailedSegment>,
    pub fatal_error: Option<BatchFatalError>,
}

#[derive(Debug, Serialize, Clone)]
pub struct BookExportBlocker {
    pub code: String,
    pub message: String,
    pub segment_id: Option<String>,
    pub chapter_title: Option<String>,
    pub segment_order: Option<i64>,
    pub preview: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
pub struct BookExportPreflight {
    pub can_export: bool,
    pub han_character_count: i64,
    pub total_segments: i64,
    pub generated_segments: i64,
    pub provider: Option<String>,
    pub voice_type: Option<i64>,
    pub sample_rate: Option<i64>,
    pub channels: Option<i64>,
    pub bits_per_sample: Option<i64>,
    pub speed: Option<f64>,
    pub volume: Option<f64>,
    pub blockers: Vec<BookExportBlocker>,
}

#[derive(Debug, Serialize, Clone)]
pub struct ExportState {
    pub book_id: Option<String>,
    pub status: String,
    pub format: Option<String>,
    pub phase: String,
    pub processed_segments: i64,
    pub total_segments: i64,
    pub output_path: Option<String>,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
pub struct FfmpegStatus {
    pub available: bool,
    pub version: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
pub struct Book {
    pub id: String,
    pub title: String,
    pub author: Option<String>,
    pub dynasty: Option<String>,
    pub edition: Option<String>,
    pub source_file: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, Clone)]
pub struct BookSummary {
    pub id: String,
    pub title: String,
    pub source_file: Option<String>,
    pub chapter_count: i64,
    pub segment_count: i64,
    pub created_at: String,
}

#[derive(Debug, Serialize, Clone)]
pub struct BookDetail {
    pub book: Book,
    pub chapter_count: i64,
    pub segment_count: i64,
}

#[derive(Debug, Serialize, Clone)]
pub struct Chapter {
    pub id: String,
    pub book_id: String,
    pub title: Option<String>,
    pub order_index: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, Clone)]
pub struct ChapterSummary {
    pub chapter: Chapter,
    pub segment_count: i64,
}

#[derive(Debug, Serialize, Clone)]
pub struct Segment {
    pub id: String,
    pub chapter_id: String,
    pub order_index: i64,
    pub original_text: String,
    pub reading_text: Option<String>,
    pub speak_enabled: bool,
    pub status: String,
    pub current_audio_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl Segment {
    pub fn effective_text(&self) -> &str {
        self.reading_text
            .as_deref()
            .unwrap_or(self.original_text.as_str())
    }
}

#[derive(Debug, Serialize, Clone)]
pub struct SegmentEditResult {
    pub segment: Segment,
    pub affected_segment_ids: Vec<String>,
    pub new_segment_ids: Vec<String>,
}

#[derive(Debug, Serialize, Clone)]
pub struct GraphemeToken {
    pub index: usize,
    pub text: String,
}

#[derive(Debug, Serialize, Clone)]
pub struct PronunciationOverride {
    pub start_token: usize,
    pub end_token: usize,
    pub surface_text: String,
    pub pinyin: String,
}

#[derive(Debug, Serialize, Clone)]
pub struct Annotation {
    pub id: String,
    pub segment_id: String,
    pub start_token: usize,
    pub end_token: usize,
    pub surface_text: String,
    pub default_pinyin: Option<String>,
    pub target_pinyin: Option<String>,
    pub candidate_pinyin: Vec<String>,
    pub risk_type: String,
    pub reason: Option<String>,
    pub review_status: String,
    pub analyzer_version: Option<String>,
    pub source: Option<String>,
    pub rule_type: Option<String>,
    pub confidence: Option<String>,
    pub source_rule_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, Clone)]
pub struct PronunciationRule {
    pub id: String,
    pub scope: String,
    pub book_id: Option<String>,
    pub pattern_text: String,
    pub target_pinyin: String,
    pub rule_type: String,
    pub source: Option<String>,
    pub verified: bool,
    pub enabled: bool,
    pub application_count: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, Clone, Default)]
pub struct PronunciationRuleApplyResult {
    pub matched_segments: i64,
    pub created_annotations: i64,
    pub updated_annotations: i64,
    pub skipped_manual_overrides: i64,
}

#[derive(Debug, Serialize, Clone)]
pub struct BookPronunciationAnalysisResult {
    pub book_id: String,
    pub total_segments: i64,
    pub analyzed_segments: i64,
}

#[derive(Debug, Serialize, Clone)]
pub struct PronunciationRuleMutation {
    pub rule: PronunciationRule,
    pub apply_result: PronunciationRuleApplyResult,
}

#[derive(Debug, Serialize, Clone)]
pub struct SegmentReader {
    pub segment: Segment,
    pub tokens: Vec<GraphemeToken>,
    pub annotations: Vec<Annotation>,
    pub audio_versions: Vec<AudioVersion>,
}

#[derive(Debug, Serialize, Clone)]
pub struct AudioVersion {
    pub id: String,
    pub segment_id: String,
    pub version_no: i64,
    pub provider: String,
    pub voice_type: i64,
    pub sample_rate: i64,
    pub codec: String,
    pub speed: f64,
    pub volume: f64,
    pub ssml: Option<String>,
    pub pronunciation_signature: Option<String>,
    pub audio_path: String,
    pub provider_request_id: Option<String>,
    pub provider_session_id: Option<String>,
    pub duration_ms: Option<i64>,
    pub created_at: String,
}

#[derive(Debug, Serialize, Clone)]
pub struct PaginatedSegments {
    pub items: Vec<Segment>,
    pub total: i64,
    pub offset: i64,
    pub limit: i64,
}

#[derive(Debug, Serialize, Clone)]
pub struct ImportResult {
    pub book: BookDetail,
    pub chapter: Chapter,
    pub chapters: Vec<Chapter>,
    pub segment_count: i64,
}
