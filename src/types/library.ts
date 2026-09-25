export type BookSummary = {
  id: string;
  title: string;
  source_file: string | null;
  chapter_count: number;
  segment_count: number;
  created_at: string;
};

export type Book = {
  id: string;
  title: string;
  author: string | null;
  dynasty: string | null;
  edition: string | null;
  source_file: string | null;
  created_at: string;
  updated_at: string;
};

export type BookDetail = {
  book: Book;
  chapter_count: number;
  segment_count: number;
};

export type Chapter = {
  id: string;
  book_id: string;
  title: string | null;
  order_index: number;
  created_at: string;
  updated_at: string;
};

export type ChapterSummary = {
  chapter: Chapter;
  segment_count: number;
};

export type Segment = {
  id: string;
  chapter_id: string;
  order_index: number;
  original_text: string;
  reading_text: string | null;
  speak_enabled: boolean;
  status: string;
  current_audio_id: string | null;
  created_at: string;
  updated_at: string;
};

export type SegmentEditResult = {
  segment: Segment;
  affected_segment_ids: string[];
  new_segment_ids: string[];
};

export type GraphemeToken = {
  index: number;
  text: string;
};

export type Annotation = {
  id: string;
  segment_id: string;
  start_token: number;
  end_token: number;
  surface_text: string;
  default_pinyin: string | null;
  target_pinyin: string | null;
  candidate_pinyin: string[];
  risk_type: string;
  reason: string | null;
  review_status: "needs_review" | "confirmed" | "ignored" | string;
  analyzer_version: string | null;
  source: string | null;
  rule_type: string | null;
  confidence: "verified" | "high" | "medium" | "low" | string | null;
  source_rule_id: string | null;
  created_at: string;
  updated_at: string;
};

export type PronunciationRule = {
  id: string;
  scope: "book" | "global" | string;
  book_id: string | null;
  pattern_text: string;
  target_pinyin: string;
  rule_type: string;
  source: string | null;
  verified: boolean;
  enabled: boolean;
  application_count: number;
  created_at: string;
  updated_at: string;
};

export type PronunciationRuleApplyResult = {
  matched_segments: number;
  created_annotations: number;
  updated_annotations: number;
  skipped_manual_overrides: number;
};

export type BookPronunciationAnalysisResult = {
  book_id: string;
  total_segments: number;
  analyzed_segments: number;
};

export type PronunciationRuleMutation = {
  rule: PronunciationRule;
  apply_result: PronunciationRuleApplyResult;
};

export type SegmentReader = {
  segment: Segment;
  tokens: GraphemeToken[];
  annotations: Annotation[];
  audio_versions: AudioVersion[];
};

export type AudioVersion = {
  id: string;
  segment_id: string;
  version_no: number;
  provider: string;
  voice_type: number;
  sample_rate: number;
  codec: string;
  speed: number;
  volume: number;
  ssml: string | null;
  audio_path: string;
  provider_metadata: AudioProviderMetadata | null;
  provider_request_id: string | null;
  provider_session_id: string | null;
  duration_ms: number | null;
  created_at: string;
};

export type TtsRealizedPronunciationItem = {
  text: string;
  phoneme: string | null;
  begin_ms: number | null;
  end_ms: number | null;
};

export type AudioProviderMetadata = {
  realized_pronunciation: TtsRealizedPronunciationItem[] | null;
};

export type CredentialStatus = {
  secret_id_configured: boolean;
  secret_key_configured: boolean;
};

export type TencentVoice = {
  id: number;
  name: string;
  category: string;
  description: string;
};

export type TtsSettings = {
  provider: string;
  voice_type: number;
  speed: number;
  volume: number;
  sample_rate: number;
  codec: string;
};

export type ApiUsageRange = "today" | "7d" | "30d" | "all";

export type ApiUsageVoiceSummary = {
  voice_type: number;
  characters: number;
};

export type ApiUsageSummary = {
  range: ApiUsageRange;
  tts: {
    requests: number;
    success: number;
    failed: number;
    characters: number;
    by_voice: ApiUsageVoiceSummary[];
  };
};

export type TtsPreviewResult = {
  audio_path: string;
  duration_ms: number | null;
  voice_type: number;
  speed: number;
  volume: number;
  sample_rate: number;
};

export type ReaderPinyinMode = "off" | "risky" | "all";

export type ReaderDisplaySettings = {
  font_size: number;
  pinyin_mode: ReaderPinyinMode;
};

export type SegmentDisplayPinyin = {
  token_pinyin: Array<string | null>;
};

export type PaginatedSegments = {
  items: Segment[];
  total: number;
  offset: number;
  limit: number;
};

export type ImportResult = {
  book: BookDetail;
  chapter: Chapter;
  chapters: Chapter[];
  segment_count: number;
};

export type BatchGenerationBlocker = {
  code: string;
  message: string;
  segment_id: string | null;
  chapter_title: string | null;
  segment_order: number | null;
  preview: string | null;
};

export type BookGenerationPreflight = {
  can_generate: boolean;
  han_character_count: number;
  total_segments: number;
  pending_segments: number;
  needs_review_segments: number;
  generated_and_reusable: number;
  need_generation: number;
  blockers: BatchGenerationBlocker[];
};

export type BatchFailedSegment = {
  segment_id: string;
  chapter_title: string | null;
  segment_order: number;
  preview: string;
  code: string;
  message: string;
};

export type BatchFatalError = {
  code: string;
  message: string;
};

export type BatchGenerationState = {
  book_id: string | null;
  status: "idle" | "running" | "cancelling" | "cancelled" | "completed" | "failed" | string;
  total_segments: number;
  segments_requiring_generation: number;
  processed: number;
  generated: number;
  skipped: number;
  failed: number;
  current_segment_id: string | null;
  current_segment_order: number | null;
  current_preview: string | null;
  has_failures: boolean;
  failed_segments: BatchFailedSegment[];
  fatal_error: BatchFatalError | null;
};

export type BookExportBlocker = {
  code: string;
  message: string;
  segment_id: string | null;
  chapter_title: string | null;
  segment_order: number | null;
  preview: string | null;
};

export type BookExportPreflight = {
  can_export: boolean;
  selection_mode: "book" | "selection" | string;
  han_character_count: number;
  total_segments: number;
  generated_segments: number;
  provider: string | null;
  voice_type: number | null;
  sample_rate: number | null;
  channels: number | null;
  bits_per_sample: number | null;
  speed: number | null;
  volume: number | null;
  blockers: BookExportBlocker[];
};

export type ExportState = {
  book_id: string | null;
  status: "idle" | "running" | "cancelling" | "completed" | "failed" | "cancelled" | string;
  format: "mp3" | "wav" | string | null;
  phase: "idle" | "preparing" | "merging" | "encoding_mp3" | "saving" | "cancelling" | "completed" | "failed" | "cancelled" | string;
  processed_segments: number;
  total_segments: number;
  output_path: string | null;
  error_code: string | null;
  error_message: string | null;
};

export type FfmpegStatus = {
  available: boolean;
  version: string | null;
};
