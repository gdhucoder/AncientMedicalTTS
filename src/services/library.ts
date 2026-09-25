import { invoke } from "@tauri-apps/api/core";
import type { Annotation, ApiUsageRange, ApiUsageSummary, BatchGenerationState, BookDetail, BookExportPreflight, BookGenerationPreflight, BookPronunciationAnalysisResult, BookSummary, ChapterSummary, CredentialStatus, ExportState, FfmpegStatus, ImportResult, PaginatedSegments, PronunciationRule, PronunciationRuleApplyResult, PronunciationRuleMutation, ReaderDisplaySettings, Segment, SegmentDisplayPinyin, SegmentEditResult, SegmentReader, TencentVoice, TtsPreviewResult, TtsSettings } from "../types/library";

export function importTxtBook(path: string, title?: string): Promise<ImportResult> {
  return invoke<ImportResult>("import_txt_book", { path, title: title ?? null });
}

export function listBooks(): Promise<BookSummary[]> {
  return invoke<BookSummary[]>("list_books");
}

export function getBook(bookId: string): Promise<BookDetail> {
  return invoke<BookDetail>("get_book", { bookId });
}

export function listChapters(bookId: string): Promise<ChapterSummary[]> {
  return invoke<ChapterSummary[]>("list_chapters", { bookId });
}

export function listSegments(chapterId: string, offset: number, limit: number): Promise<PaginatedSegments> {
  return invoke<PaginatedSegments>("list_segments", { chapterId, offset, limit });
}

export function getSegment(segmentId: string): Promise<Segment> {
  return invoke<Segment>("get_segment", { segmentId });
}

export function updateSegmentReadingText(segmentId: string, readingText: string | null): Promise<SegmentEditResult> {
  return invoke<SegmentEditResult>("update_segment_reading_text", { segmentId, readingText });
}

export function restoreSegmentReadingText(segmentId: string): Promise<SegmentEditResult> {
  return invoke<SegmentEditResult>("restore_segment_reading_text", { segmentId });
}

export function setSegmentSpeakEnabled(segmentId: string, speakEnabled: boolean): Promise<SegmentEditResult> {
  return invoke<SegmentEditResult>("set_segment_speak_enabled", { segmentId, speakEnabled });
}

export function splitSegment(segmentId: string, tokenIndex: number): Promise<SegmentEditResult> {
  return invoke<SegmentEditResult>("split_segment", { segmentId, tokenIndex });
}

export function mergeSegmentWithPrevious(segmentId: string): Promise<SegmentEditResult> {
  return invoke<SegmentEditResult>("merge_segment_with_previous", { segmentId });
}

export function mergeSegmentWithNext(segmentId: string): Promise<SegmentEditResult> {
  return invoke<SegmentEditResult>("merge_segment_with_next", { segmentId });
}

export function getSegmentReader(segmentId: string): Promise<SegmentReader> {
  return invoke<SegmentReader>("get_segment_reader", { segmentId });
}

export function getSegmentDisplayPinyin(segmentId: string): Promise<SegmentDisplayPinyin> {
  return invoke<SegmentDisplayPinyin>("get_segment_display_pinyin", { segmentId });
}

export function analyzeSegmentPronunciation(segmentId: string): Promise<SegmentReader> {
  return invoke<SegmentReader>("analyze_segment_pronunciation", { segmentId });
}

export function reanalyzeBookPronunciation(bookId: string): Promise<BookPronunciationAnalysisResult> {
  return invoke<BookPronunciationAnalysisResult>("reanalyze_book_pronunciation", { bookId });
}

export function listSegmentAnnotations(segmentId: string): Promise<Annotation[]> {
  return invoke<Annotation[]>("list_segment_annotations", { segmentId });
}

export function confirmAnnotation(annotationId: string, targetPinyin: string): Promise<SegmentReader> {
  return invoke<SegmentReader>("confirm_annotation", { annotationId, targetPinyin });
}

export function ignoreAnnotation(annotationId: string): Promise<SegmentReader> {
  return invoke<SegmentReader>("ignore_annotation", { annotationId });
}

export function resetAnnotation(annotationId: string): Promise<SegmentReader> {
  return invoke<SegmentReader>("reset_annotation", { annotationId });
}

export function createManualAnnotation(segmentId: string, tokenIndex: number, targetPinyin: string): Promise<SegmentReader> {
  return invoke<SegmentReader>("create_manual_annotation", { segmentId, tokenIndex, targetPinyin });
}

export function createPronunciationRule(input: { scope: "book" | "global"; bookId: string | null; patternText: string; targetPinyin: string; ruleType?: string; source?: string }): Promise<PronunciationRuleMutation> {
  return invoke<PronunciationRuleMutation>("create_pronunciation_rule", { scope: input.scope, bookId: input.bookId, patternText: input.patternText, targetPinyin: input.targetPinyin, ruleType: input.ruleType ?? "manual", source: input.source ?? "manual" });
}

export function createRuleFromAnnotation(annotationId: string, scope: "book" | "global"): Promise<PronunciationRuleMutation> {
  return invoke<PronunciationRuleMutation>("create_rule_from_annotation", { annotationId, scope });
}

export function updatePronunciationRule(rule: PronunciationRule, patternText: string, targetPinyin: string): Promise<PronunciationRuleMutation> {
  return invoke<PronunciationRuleMutation>("update_pronunciation_rule", { ruleId: rule.id, patternText, targetPinyin, ruleType: rule.rule_type, source: rule.source ?? "manual" });
}

export function enablePronunciationRule(ruleId: string): Promise<PronunciationRuleMutation> {
  return invoke<PronunciationRuleMutation>("enable_pronunciation_rule", { ruleId });
}

export function disablePronunciationRule(ruleId: string): Promise<PronunciationRuleMutation> {
  return invoke<PronunciationRuleMutation>("disable_pronunciation_rule", { ruleId });
}

export function getPronunciationRule(ruleId: string): Promise<PronunciationRule> {
  return invoke<PronunciationRule>("get_pronunciation_rule", { ruleId });
}

export function listBookPronunciationRules(bookId: string): Promise<PronunciationRule[]> {
  return invoke<PronunciationRule[]>("list_book_pronunciation_rules", { bookId });
}

export function listGlobalPronunciationRules(): Promise<PronunciationRule[]> {
  return invoke<PronunciationRule[]>("list_global_pronunciation_rules");
}

export function applyPronunciationRulesToBook(bookId: string): Promise<PronunciationRuleApplyResult> {
  return invoke<PronunciationRuleApplyResult>("apply_pronunciation_rules_to_book", { bookId });
}

export function getTtsSettings(): Promise<TtsSettings> {
  return invoke<TtsSettings>("get_tts_settings");
}

export type TtsSettingsCommandPayload = {
  provider: string;
  voiceType: number;
  speed: number;
  volume: number;
  sampleRate: number;
};

export function ttsSettingsCommandPayload(settings: TtsSettings): TtsSettingsCommandPayload {
  return {
    provider: settings.provider,
    voiceType: settings.voice_type,
    speed: settings.speed,
    volume: settings.volume,
    sampleRate: settings.sample_rate,
  };
}

export function saveTtsSettings(settings: TtsSettings): Promise<TtsSettings> {
  return invoke<TtsSettings>("save_tts_settings", ttsSettingsCommandPayload(settings));
}

export function getReaderDisplaySettings(): Promise<ReaderDisplaySettings> {
  return invoke<ReaderDisplaySettings>("get_reader_display_settings");
}

export function saveReaderDisplaySettings(settings: ReaderDisplaySettings): Promise<ReaderDisplaySettings> {
  return invoke<ReaderDisplaySettings>("save_reader_display_settings", {
    fontSize: settings.font_size,
    pinyinMode: settings.pinyin_mode,
  });
}

export function getTencentVoices(): Promise<TencentVoice[]> {
  return invoke<TencentVoice[]>("get_tencent_voices");
}

export function getTtsCredentialStatus(): Promise<CredentialStatus> {
  return invoke<CredentialStatus>("get_tts_credential_status");
}

export function saveTencentCredentials(secretId: string, secretKey: string): Promise<CredentialStatus> {
  return invoke<CredentialStatus>("save_tencent_credentials", { secretId, secretKey });
}

export function deleteTencentCredentials(): Promise<CredentialStatus> {
  return invoke<CredentialStatus>("delete_tencent_credentials");
}

export function testTtsConnection(voiceType: number, sampleRate: number): Promise<Record<string, unknown>> {
  return invoke<Record<string, unknown>>("test_tts_connection", { voiceType, sampleRate });
}

export function getApiUsageSummary(range: ApiUsageRange): Promise<ApiUsageSummary> {
  return invoke<ApiUsageSummary>("get_api_usage_summary", { range });
}

export function generateTtsPreview(text: string, settings: TtsSettings): Promise<TtsPreviewResult> {
  return invoke<TtsPreviewResult>("generate_tts_preview", {
    text,
    provider: settings.provider,
    voiceType: settings.voice_type,
    speed: settings.speed,
    volume: settings.volume,
    sampleRate: settings.sample_rate,
  });
}

export function generateSegmentAudio(segmentId: string): Promise<SegmentReader> {
  return invoke<SegmentReader>("generate_segment_audio", { segmentId });
}

export function selectAudioVersion(audioId: string): Promise<SegmentReader> {
  return invoke<SegmentReader>("select_audio_version", { audioId });
}

export function deleteBook(bookId: string): Promise<void> {
  return invoke<void>("delete_book", { bookId });
}

export function getBookGenerationPreflight(bookId: string): Promise<BookGenerationPreflight> {
  return invoke<BookGenerationPreflight>("get_book_generation_preflight", { bookId });
}

export function startBookAudioGeneration(bookId: string): Promise<void> {
  return invoke<void>("start_book_audio_generation", { bookId });
}

export function cancelBookAudioGeneration(): Promise<BatchGenerationState> {
  return invoke<BatchGenerationState>("cancel_book_audio_generation");
}

export function getBatchGenerationState(): Promise<BatchGenerationState> {
  return invoke<BatchGenerationState>("get_batch_generation_state");
}

export function checkFfmpeg(): Promise<FfmpegStatus> {
  return invoke<FfmpegStatus>("check_ffmpeg");
}

export function getBookExportPreflight(bookId: string, segmentIds: string[] | null = null): Promise<BookExportPreflight> {
  return invoke<BookExportPreflight>("get_book_export_preflight", { bookId, segmentIds });
}

export function exportBookAudio(bookId: string, destinationPath: string, format: "mp3" | "wav", overwrite = false, segmentIds: string[] | null = null): Promise<void> {
  return invoke<void>("export_book_audio", { bookId, destinationPath, format, overwrite, segmentIds });
}

export function cancelExport(): Promise<ExportState> {
  return invoke<ExportState>("cancel_export");
}

export function getExportState(): Promise<ExportState> {
  return invoke<ExportState>("get_export_state");
}
