import { useEffect, useRef, useState } from "react";
import type { SyntheticEvent } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { save } from "@tauri-apps/plugin-dialog";
import { analyzeSegmentPronunciation, applyPronunciationRulesToBook, cancelBookAudioGeneration, cancelExport, confirmAnnotation, createManualAnnotation, createPronunciationRule, createRuleFromAnnotation, deleteTencentCredentials, disablePronunciationRule, enablePronunciationRule, exportBookAudio, generateSegmentAudio, generateTtsPreview, getApiUsageSummary, getBatchGenerationState, getBook, getBookExportPreflight, getBookGenerationPreflight, getExportState, getReaderDisplaySettings, getSegmentDisplayPinyin, getSegmentReader, getTencentVoices, getTtsCredentialStatus, getTtsSettings, ignoreAnnotation, listBooks, listBookPronunciationRules, listChapters, listGlobalPronunciationRules, listSegments, mergeSegmentWithNext, mergeSegmentWithPrevious, reanalyzeBookPronunciation, resetAnnotation, restoreSegmentReadingText, saveReaderDisplaySettings, saveTencentCredentials, saveTtsSettings, selectAudioVersion, setSegmentSpeakEnabled, splitSegment, startBookAudioGeneration, testTtsConnection, updatePronunciationRule, updateSegmentReadingText } from "./services/library";
import { friendlyErrorMessage } from "./services/errors";
import { exportPhaseLabel, sanitizeExportFilename } from "./services/exportUi";
import { batchIsActive, batchProgressPercent } from "./services/batchGenerationUi";
import { annotationForToken, annotationSourceLabel, confidenceLabel, isHanToken, isSingleHanGlobalRule, isTtsLockedAnnotation, normalizePinyinInput, reviewStatusLabel, riskTypeLabel, ruleTypeLabel } from "./services/pronunciationUi";
import { pinyinToToneMarks } from "./services/readerDisplay";
import { graphemeIndexAtCaret } from "./services/segmentEditingUi";
import { LibraryPage } from "./pages/LibraryPage";
import { useLibraryStore } from "./stores/libraryStore";
import { useStatusStore } from "./stores/statusStore";
import type { Annotation, ApiUsageRange, ApiUsageSummary, BatchGenerationState, BookDetail, BookExportPreflight, BookGenerationPreflight, BookSummary, ChapterSummary, CredentialStatus, ExportState, PronunciationRule, ReaderDisplaySettings, ReaderPinyinMode, Segment, SegmentReader, TencentVoice, TtsPreviewResult, TtsSettings } from "./types/library";
import type { ComponentStatus, WorkerStatus } from "./types/status";

const idleBatchState: BatchGenerationState = {
  book_id: null,
  status: "idle",
  total_segments: 0,
  segments_requiring_generation: 0,
  processed: 0,
  generated: 0,
  skipped: 0,
  failed: 0,
  current_segment_id: null,
  current_segment_order: null,
  current_preview: null,
  has_failures: false,
  failed_segments: [],
  fatal_error: null,
};

function useBatchGenerationState(): BatchGenerationState {
  const [state, setState] = useState<BatchGenerationState>(idleBatchState);
  useEffect(() => {
    let active = true;
    let unlisten: (() => void) | undefined;
    void getBatchGenerationState().then((nextState) => { if (active) setState(nextState); }).catch(() => undefined);
    void listen<BatchGenerationState>("batch-generation-progress", (event) => {
      if (active) setState(event.payload);
    }).then((cleanup) => {
      if (active) unlisten = cleanup;
      else cleanup();
    });
    return () => {
      active = false;
      unlisten?.();
    };
  }, []);
  return state;
}

const idleExportState: ExportState = {
  book_id: null,
  status: "idle",
  format: null,
  phase: "idle",
  processed_segments: 0,
  total_segments: 0,
  output_path: null,
  error_code: null,
  error_message: null,
};

function useExportState(): ExportState {
  const [state, setState] = useState<ExportState>(idleExportState);
  useEffect(() => {
    let active = true;
    let unlisten: (() => void) | undefined;
    void getExportState().then((nextState) => { if (active) setState(nextState); }).catch(() => undefined);
    void listen<ExportState>("export-progress", (event) => {
      if (active) setState(event.payload);
    }).then((cleanup) => {
      if (active) unlisten = cleanup;
      else cleanup();
    });
    return () => { active = false; unlisten?.(); };
  }, []);
  return state;
}

function exportIsActive(state: ExportState): boolean {
  return state.status === "running" || state.status === "cancelling";
}

function Indicator({ status }: { status: ComponentStatus }) {
  return (
    <span className={`indicator ${status.ok ? "ok" : "error"}`}>
      <span aria-hidden="true">{status.ok ? "✓" : "!"}</span>
      {status.message}
    </span>
  );
}

function WorkerCard({ status }: { status: WorkerStatus }) {
  return (
    <section className="status-card worker-card">
      <div><p className="eyebrow">后台服务</p><h2>发音服务</h2></div>
      <Indicator status={status} />
      <div className="worker-details"><span>运行状态</span><strong>{status.running ? "是" : "否"}</strong><span>版本</span><strong>{status.version ?? "—"}</strong></div>
    </section>
  );
}

async function listAllChapterSegments(chapterId: string): Promise<Segment[]> {
  const firstPage = await listSegments(chapterId, 0, 100);
  if (firstPage.total <= firstPage.items.length) return firstPage.items;
  const remainingPages = await Promise.all(
    Array.from(
      { length: Math.ceil(firstPage.total / 100) - 1 },
      (_, index) => listSegments(chapterId, (index + 1) * 100, 100),
    ),
  );
  return [firstPage, ...remainingPages].flatMap((page) => page.items);
}

function AppHeader() {
  const { view, setView, closeReader } = useLibraryStore();
  return (
    <header className="topbar">
      <div className="brand-mark" aria-hidden="true">古</div>
      <div className="brand-copy"><p className="eyebrow">中医古籍朗读</p><h1>古籍朗读</h1></div>
      <nav className="topnav" aria-label="主导航">
        <button className={view === "books" ? "nav-button active" : "nav-button"} type="button" onClick={() => { closeReader(); setView("books"); }}>我的古籍</button>
        <button className={view === "settings" ? "nav-button active" : "nav-button"} type="button" onClick={() => setView("settings")}>语音设置</button>
        <button className={view === "rules" ? "nav-button active" : "nav-button"} type="button" onClick={() => setView("rules")}>发音词典</button>
      </nav>
      <span className="version">v0.1.0-rc1</span>
    </header>
  );
}

function BooksPage() {
  const batchState = useBatchGenerationState();
  const batchRunning = batchIsActive(batchState);
  const exportState = useExportState();
  const exportRunning = exportIsActive(exportState);
  const openBook = useLibraryStore((state) => state.openBook);
  return <LibraryPage operationsLocked={batchRunning || exportRunning} onOpenBook={openBook} />;
}

function ReaderPage() {
  const bookId = useLibraryStore((state) => state.bookId);
  const chapterId = useLibraryStore((state) => state.chapterId);
  const segmentId = useLibraryStore((state) => state.segmentId);
  const selectChapter = useLibraryStore((state) => state.selectChapter);
  const selectSegment = useLibraryStore((state) => state.selectSegment);
  const closeReader = useLibraryStore((state) => state.closeReader);
  const setView = useLibraryStore((state) => state.setView);
  const [book, setBook] = useState<BookDetail | null>(null);
  const [chapters, setChapters] = useState<ChapterSummary[]>([]);
  const [segments, setSegments] = useState<Segment[]>([]);
  const [navigatorSegments, setNavigatorSegments] = useState<Segment[]>([]);
  const [reader, setReader] = useState<SegmentReader | null>(null);
  const [displaySettings, setDisplaySettings] = useState<ReaderDisplaySettings>({ font_size: 25, pinyin_mode: "risky" });
  const [displayPinyin, setDisplayPinyin] = useState<Array<string | null> | null>(null);
  const [displayPinyinLoading, setDisplayPinyinLoading] = useState(false);
  const [selectedAnnotationId, setSelectedAnnotationId] = useState<string | null>(null);
  const [inspectorOpen, setInspectorOpen] = useState(false);
  const [manualTokenIndex, setManualTokenIndex] = useState<number | null>(null);
  const [cursorTokenIndex, setCursorTokenIndex] = useState<number | null>(null);
  const [readingTextDraft, setReadingTextDraft] = useState("");
  const [segmentEditBusy, setSegmentEditBusy] = useState(false);
  const [editMode, setEditMode] = useState(false);
  const [segmentFilter, setSegmentFilter] = useState<"all" | "needs_review" | "not_generated">("all");
  const [selectedSegmentIds, setSelectedSegmentIds] = useState<string[]>([]);
  const [pendingSegmentId, setPendingSegmentId] = useState<string | null>(null);
  const [moreOpen, setMoreOpen] = useState(false);
  const [targetPinyin, setTargetPinyin] = useState("");
  const [manualPinyin, setManualPinyin] = useState("");
  const [analysisBusy, setAnalysisBusy] = useState(false);
  const [bookAnalysisBusy, setBookAnalysisBusy] = useState(false);
  const [audioBusy, setAudioBusy] = useState(false);
  const [offset, setOffset] = useState(0);
  const [total, setTotal] = useState(0);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [message, setMessage] = useState<string | null>(null);
  const batchState = useBatchGenerationState();
  const exportState = useExportState();
  const [preflight, setPreflight] = useState<BookGenerationPreflight | null>(null);
  const pageSize = 100;
  const displaySettingsSaveTimer = useRef<ReturnType<typeof window.setTimeout> | null>(null);
  const pendingDisplaySettings = useRef<ReaderDisplaySettings | null>(null);

  useEffect(() => {
    return () => {
      if (displaySettingsSaveTimer.current !== null) window.clearTimeout(displaySettingsSaveTimer.current);
      const pending = pendingDisplaySettings.current;
      if (pending) void saveReaderDisplaySettings(pending).catch(() => undefined);
    };
  }, []);

  const batchRunning = batchIsActive(batchState);
  const batchForBook = batchState.book_id === bookId;
  const exportRunning = exportIsActive(exportState);
  const exportForBook = exportState.book_id === bookId;

  useEffect(() => {
    let active = true;
    getReaderDisplaySettings().then((settings) => {
      if (active) setDisplaySettings(settings);
    }).catch((reason: unknown) => { if (active) setError(friendlyErrorMessage(reason)); });
    return () => { active = false; };
  }, []);

  useEffect(() => {
    if (!segmentId || displaySettings.pinyin_mode !== "all") {
      setDisplayPinyin(null);
      setDisplayPinyinLoading(false);
      return;
    }
    let active = true;
    setDisplayPinyin(null);
    setDisplayPinyinLoading(true);
    getSegmentDisplayPinyin(segmentId).then((result) => {
      if (active) setDisplayPinyin(result.token_pinyin);
    }).catch((reason: unknown) => { if (active) setError(friendlyErrorMessage(reason)); })
      .finally(() => { if (active) setDisplayPinyinLoading(false); });
    return () => { active = false; };
  }, [displaySettings.pinyin_mode, segmentId]);

  useEffect(() => {
    if (!bookId) return;
    setSelectedSegmentIds([]);
    let active = true;
    setLoading(true);
    Promise.all([getBook(bookId), listChapters(bookId)]).then(async ([nextBook, nextChapters]) => {
      if (!active) return;
      const nextNavigatorSegments = (await Promise.all(nextChapters.map((summary) => listAllChapterSegments(summary.chapter.id)))).flat();
      if (!active) return;
      setBook(nextBook); setChapters(nextChapters); setNavigatorSegments(nextNavigatorSegments); setError(null);
      if (nextChapters.length > 0) selectChapter(nextChapters[0].chapter.id);
    }).catch((reason: unknown) => { if (active) setError(friendlyErrorMessage(reason)); })
      .finally(() => { if (active) setLoading(false); });
    return () => { active = false; };
  }, [bookId, selectChapter]);

  useEffect(() => {
    if (!chapterId) return;
    let active = true;
    setLoading(true);
    listSegments(chapterId, offset, pageSize).then((page) => {
      if (!active) return;
      setSegments(page.items); setTotal(page.total);
      const requestedSegment = pendingSegmentId ? page.items.find((item) => item.id === pendingSegmentId) : undefined;
      const currentSegment = segmentId ? page.items.find((item) => item.id === segmentId) : undefined;
      const nextSegmentId = requestedSegment?.id ?? currentSegment?.id ?? page.items[0]?.id;
      if (nextSegmentId) selectSegment(nextSegmentId);
      if (pendingSegmentId) setPendingSegmentId(null);
    }).catch((reason: unknown) => { if (active) setError(friendlyErrorMessage(reason)); })
      .finally(() => { if (active) setLoading(false); });
    return () => { active = false; };
  }, [chapterId, offset, pendingSegmentId, selectSegment]);

  useEffect(() => {
    if (!segmentId) { setReader(null); return; }
    let active = true;
    setReader(null);
    setSelectedAnnotationId(null);
    setManualTokenIndex(null);
    setCursorTokenIndex(null);
    setEditMode(false);
    getSegmentReader(segmentId).then((nextReader) => { if (active) { setReader(nextReader); setReadingTextDraft(nextReader.segment.reading_text ?? nextReader.segment.original_text); } })
      .catch((reason: unknown) => { if (active) setError(friendlyErrorMessage(reason)); });
    return () => { active = false; };
  }, [segmentId]);

  useEffect(() => {
    if (!segmentId || !batchForBook || !["completed", "cancelled", "failed"].includes(batchState.status)) return;
    void getSegmentReader(segmentId).then((nextReader) => { setReader(nextReader); setReadingTextDraft(nextReader.segment.reading_text ?? nextReader.segment.original_text); }).catch(() => undefined);
  }, [batchForBook, batchState.status, segmentId]);

  const selectedAnnotation = reader?.annotations.find((annotation) => annotation.id === selectedAnnotationId) ?? null;
  const manualToken = reader?.tokens.find((token) => token.index === manualTokenIndex) ?? null;

  const selectAnnotation = (annotation: Annotation) => {
    setSelectedAnnotationId(annotation.id);
    setManualTokenIndex(null);
    setCursorTokenIndex(annotation.start_token);
    setTargetPinyin(annotation.target_pinyin ?? annotation.default_pinyin ?? annotation.candidate_pinyin[0] ?? "");
    setInspectorOpen(true);
  };

  const selectManualToken = (tokenIndex: number) => {
    setSelectedAnnotationId(null);
    setManualTokenIndex(tokenIndex);
    setCursorTokenIndex(tokenIndex);
    setManualPinyin("");
    setInspectorOpen(true);
  };

  const updateReader = (nextReader: SegmentReader, annotationId?: string) => {
    setReader(nextReader);
    setReadingTextDraft(nextReader.segment.reading_text ?? nextReader.segment.original_text);
    setSegments((current) => current.map((segment) => segment.id === nextReader.segment.id ? nextReader.segment : segment));
    setNavigatorSegments((current) => current.map((segment) => segment.id === nextReader.segment.id ? nextReader.segment : segment));
    setManualTokenIndex(null);
    if (annotationId) {
      setSelectedAnnotationId(annotationId);
      const nextAnnotation = nextReader.annotations.find((annotation) => annotation.id === annotationId);
      setTargetPinyin(nextAnnotation?.target_pinyin ?? nextAnnotation?.default_pinyin ?? nextAnnotation?.candidate_pinyin[0] ?? "");
    } else {
      setSelectedAnnotationId(null);
    }
  };

  const updateDisplaySettings = (patch: Partial<ReaderDisplaySettings>) => {
    const nextSettings = { ...displaySettings, ...patch };
    setDisplaySettings(nextSettings);
    pendingDisplaySettings.current = nextSettings;
    if (displaySettingsSaveTimer.current !== null) window.clearTimeout(displaySettingsSaveTimer.current);
    displaySettingsSaveTimer.current = window.setTimeout(() => {
      const pending = pendingDisplaySettings.current;
      pendingDisplaySettings.current = null;
      displaySettingsSaveTimer.current = null;
      if (pending) void saveReaderDisplaySettings(pending).catch((reason: unknown) => setError(friendlyErrorMessage(reason)));
    }, 180);
  };

  const refreshSegmentPage = async (preferredSegmentId: string) => {
    if (!chapterId) return;
    const page = await listSegments(chapterId, offset, pageSize);
    setSegments(page.items);
    setTotal(page.total);
    selectSegment(preferredSegmentId);
  };

  const refreshNavigator = async () => {
    if (chapters.length === 0) return;
    const refreshed = (await Promise.all(chapters.map((summary) => listAllChapterSegments(summary.chapter.id)))).flat();
    setNavigatorSegments(refreshed);
    setSelectedSegmentIds((current) => current.filter((id) => refreshed.some((segment) => segment.id === id && segment.speak_enabled)));
  };

  const reanalyzeEditedSegments = async (result: { segment: Segment; new_segment_ids: string[] }) => {
    for (const editedSegmentId of result.new_segment_ids) {
      await analyzeSegmentPronunciation(editedSegmentId);
    }
    await refreshSegmentPage(result.segment.id);
    await refreshNavigator();
    updateReader(await getSegmentReader(result.segment.id));
    setEditMode(false);
    setMessage("段落已更新，并已重新进行发音分析；现有音频需要重新生成。");
  };

  const handleSaveReadingText = async () => {
    if (!reader || segmentEditBusy || batchRunning || exportRunning) return;
    setSegmentEditBusy(true); setError(null); setMessage(null);
    try {
      await reanalyzeEditedSegments(await updateSegmentReadingText(reader.segment.id, readingTextDraft));
    } catch (reason: unknown) { setError(friendlyErrorMessage(reason)); }
    finally { setSegmentEditBusy(false); }
  };

  const handleRestoreReadingText = async () => {
    if (!reader || segmentEditBusy || batchRunning || exportRunning) return;
    setSegmentEditBusy(true); setError(null); setMessage(null);
    try {
      await reanalyzeEditedSegments(await restoreSegmentReadingText(reader.segment.id));
    } catch (reason: unknown) { setError(friendlyErrorMessage(reason)); }
    finally { setSegmentEditBusy(false); }
  };

  const handleSpeakEnabledChange = async (speakEnabled: boolean) => {
    if (!reader || segmentEditBusy || batchRunning || exportRunning) return;
    setSegmentEditBusy(true); setError(null); setMessage(null);
    try {
      const result = await setSegmentSpeakEnabled(reader.segment.id, speakEnabled);
      if (!speakEnabled) setSelectedSegmentIds((current) => current.filter((id) => id !== reader.segment.id));
      updateReader(await getSegmentReader(result.segment.id));
      await refreshSegmentPage(result.segment.id);
      setMessage(speakEnabled ? "段落已恢复参与朗读。" : "段落已设置为不参与朗读。");
    } catch (reason: unknown) { setError(friendlyErrorMessage(reason)); }
    finally { setSegmentEditBusy(false); }
  };

  const handleSplit = async () => {
    if (!reader || cursorTokenIndex === null || segmentEditBusy || batchRunning || exportRunning) return;
    setSegmentEditBusy(true); setError(null); setMessage(null);
    try {
      const currentEffectiveText = reader.segment.reading_text ?? reader.segment.original_text;
      if (readingTextDraft !== currentEffectiveText) {
        await updateSegmentReadingText(reader.segment.id, readingTextDraft);
      }
      await reanalyzeEditedSegments(await splitSegment(reader.segment.id, cursorTokenIndex));
    } catch (reason: unknown) { setError(friendlyErrorMessage(reason)); }
    finally { setSegmentEditBusy(false); }
  };

  const handleMerge = async (direction: "previous" | "next") => {
    if (!reader || segmentEditBusy || batchRunning || exportRunning) return;
    setSegmentEditBusy(true); setError(null); setMessage(null);
    try {
      const result = direction === "previous"
        ? await mergeSegmentWithPrevious(reader.segment.id)
        : await mergeSegmentWithNext(reader.segment.id);
      await reanalyzeEditedSegments(result);
    } catch (reason: unknown) { setError(friendlyErrorMessage(reason)); }
    finally { setSegmentEditBusy(false); }
  };

  const handleAnalyze = async () => {
    if (!segmentId || exportRunning) return;
    setAnalysisBusy(true);
    setError(null); setMessage(null);
    try { updateReader(await analyzeSegmentPronunciation(segmentId)); }
    catch (reason: unknown) { setError(friendlyErrorMessage(reason)); }
    finally { setAnalysisBusy(false); }
  };

  const handleReanalyzeBook = async () => {
    if (!bookId || batchRunning || exportRunning || bookAnalysisBusy) return;
    setBookAnalysisBusy(true); setError(null); setMessage(null);
    try {
      const result = await reanalyzeBookPronunciation(bookId);
      const [nextBook, nextChapters, nextReader] = await Promise.all([
        getBook(bookId),
        listChapters(bookId),
        segmentId ? getSegmentReader(segmentId) : Promise.resolve(null),
      ]);
      setBook(nextBook); setChapters(nextChapters);
      setNavigatorSegments((await Promise.all(nextChapters.map((summary) => listAllChapterSegments(summary.chapter.id)))).flat());
      if (nextReader) updateReader(nextReader);
      setMessage(`已重新分析全书发音：${result.analyzed_segments} / ${result.total_segments} 个段落。`);
    } catch (reason: unknown) { setError(friendlyErrorMessage(reason)); }
    finally { setBookAnalysisBusy(false); }
  };

  const handleConfirm = async () => {
    if (!selectedAnnotation || exportRunning) return;
    setAnalysisBusy(true);
    setError(null);
    try { updateReader(await confirmAnnotation(selectedAnnotation.id, targetPinyin), selectedAnnotation.id); }
    catch (reason: unknown) { setError(friendlyErrorMessage(reason)); }
    finally { setAnalysisBusy(false); }
  };

  const handleAnnotationStatus = async (action: "ignore" | "reset") => {
    if (!selectedAnnotation || exportRunning) return;
    setAnalysisBusy(true);
    setError(null);
    try {
      const nextReader = action === "ignore"
        ? await ignoreAnnotation(selectedAnnotation.id)
        : await resetAnnotation(selectedAnnotation.id);
      updateReader(nextReader, selectedAnnotation.id);
    } catch (reason: unknown) { setError(friendlyErrorMessage(reason)); }
    finally { setAnalysisBusy(false); }
  };

  const handleCreateRule = async (scope: "book" | "global") => {
    if (!selectedAnnotation || selectedAnnotation.review_status !== "confirmed" || !selectedAnnotation.target_pinyin || !bookId || exportRunning) return;
    if (scope === "global" && isSingleHanGlobalRule(selectedAnnotation.surface_text)) {
      const confirmed = window.confirm(`这是单字全局发音规则。\n它会应用到以后所有书籍中出现的“${selectedAnnotation.surface_text}”。\n建议优先建立词组或上下文规则。\n\n确认继续？`);
      if (!confirmed) return;
    }
    setAnalysisBusy(true);
    setError(null);
    try {
      const result = await createRuleFromAnnotation(selectedAnnotation.id, scope);
      const nextReader = await getSegmentReader(segmentId ?? selectedAnnotation.segment_id);
      updateReader(nextReader, selectedAnnotation.id);
      const appliedAnnotations = result.apply_result.created_annotations + result.apply_result.updated_annotations;
      const scopeLabel = scope === "book" ? "本书" : "全局";
      const skippedMessage = result.apply_result.skipped_manual_overrides > 0
        ? `，已有 ${result.apply_result.skipped_manual_overrides} 处本处人工确认按优先级未覆盖`
        : "";
      setMessage(`已创建${scopeLabel}规则“${result.rule.pattern_text} → ${result.rule.target_pinyin}”：应用 ${appliedAnnotations} 条标注，匹配 ${result.apply_result.matched_segments} 个段落${skippedMessage}。`);
    } catch (reason: unknown) { setError(friendlyErrorMessage(reason)); }
    finally { setAnalysisBusy(false); }
  };

  const handleManualAnnotation = async () => {
    if (!segmentId || manualTokenIndex === null || !manualToken || !isHanToken(manualToken.text) || exportRunning) return;
    setAnalysisBusy(true);
    setError(null);
    try {
      const nextReader = await createManualAnnotation(segmentId, manualTokenIndex, manualPinyin);
      const nextAnnotation = nextReader.annotations.find((annotation) => annotation.start_token === manualTokenIndex);
      updateReader(nextReader, nextAnnotation?.id);
    } catch (reason: unknown) { setError(friendlyErrorMessage(reason)); }
    finally { setAnalysisBusy(false); }
  };

  const handleGenerateAudio = async () => {
    if (!segmentId || exportRunning) return;
    setAudioBusy(true); setError(null);
    try { updateReader(await generateSegmentAudio(segmentId)); }
    catch (reason: unknown) { setError(friendlyErrorMessage(reason)); }
    finally { setAudioBusy(false); }
  };

  const handleSelectAudio = async (audioId: string) => {
    if (exportRunning) return;
    setAudioBusy(true); setError(null);
    try { updateReader(await selectAudioVersion(audioId)); }
    catch (reason: unknown) { setError(friendlyErrorMessage(reason)); }
    finally { setAudioBusy(false); }
  };

  const handleStartBatch = async () => {
    if (!bookId || batchRunning || exportRunning) return;
    setError(null);
    try {
      const nextPreflight = await getBookGenerationPreflight(bookId);
      setPreflight(nextPreflight);
      if (!nextPreflight.can_generate) {
        setError(nextPreflight.blockers.map((blocker) => blocker.message).join("；"));
        return;
      }
      const confirmed = window.confirm(`将按当前语音设置串行生成《${book?.book.title ?? "当前古籍"}》的 ${nextPreflight.need_generation} 个段落。\n可复用的已有语音：${nextPreflight.generated_and_reusable} 个。\n\n过程中不会自动生成批量以外的音频，是否开始？`);
      if (!confirmed) return;
      await startBookAudioGeneration(bookId);
    } catch (reason: unknown) { setError(friendlyErrorMessage(reason)); }
  };

  const handleCancelBatch = async () => {
    try { await cancelBookAudioGeneration(); }
    catch (reason: unknown) { setError(friendlyErrorMessage(reason)); }
  };

  const handleExport = async (format: "mp3" | "wav", preflight: BookExportPreflight, segmentIds: string[]) => {
    if (!bookId || exportRunning || !preflight.can_export) return;
    const selected = segmentIds.length > 0 ? segmentIds : null;
    const exportTitle = selected ? `${book?.book.title ?? "当前古籍"}-选中${segmentIds.length}段` : (book?.book.title ?? "ancient-medical-tts");
    const destination = await save({
      defaultPath: sanitizeExportFilename(exportTitle, format),
      filters: [{ name: format === "mp3" ? "MP3 Audio" : "WAV Audio", extensions: [format] }],
    });
    if (typeof destination !== "string") return;
    try {
      await exportBookAudio(bookId, destination, format, false, selected);
    } catch (reason: unknown) {
      const errorValue = reason as { code?: string; message?: string };
      if (errorValue.code === "EXPORT_OUTPUT_EXISTS" && window.confirm("目标文件已存在，是否覆盖？")) {
        try { await exportBookAudio(bookId, destination, format, true, selected); }
        catch (retryReason: unknown) { setError(friendlyErrorMessage(retryReason)); }
      } else {
        setError(friendlyErrorMessage(reason));
      }
    }
  };

  const handleCancelExport = async () => {
    try { await cancelExport(); }
    catch (reason: unknown) { setError(friendlyErrorMessage(reason)); }
  };

  const handleCancelEdit = () => {
    if (reader) setReadingTextDraft(reader.segment.reading_text ?? reader.segment.original_text);
    setEditMode(false);
  };

  const orderedSegments = navigatorSegments.length > 0 ? navigatorSegments : segments;
  const currentGlobalIndex = segmentId ? orderedSegments.findIndex((segment) => segment.id === segmentId) : -1;
  const currentPosition = currentGlobalIndex >= 0 ? currentGlobalIndex + 1 : (reader?.segment.order_index ?? 0) + 1;
  const needsReviewCount = orderedSegments.filter((segment) => segment.status === "needs_review").length;
  const generatedCount = orderedSegments.filter((segment) => segment.status === "generated").length;
  const notGeneratedCount = orderedSegments.filter((segment) => segment.status !== "generated").length;
  const filteredNavigatorSegments = orderedSegments.filter((segment) => {
    if (segmentFilter === "needs_review") return segment.status === "needs_review";
    if (segmentFilter === "not_generated") return segment.status !== "generated";
    return true;
  });

  const toggleSegmentSelection = (nextSegmentId: string) => {
    setSelectedSegmentIds((current) => current.includes(nextSegmentId)
      ? current.filter((segmentId) => segmentId !== nextSegmentId)
      : [...current, nextSegmentId]);
  };

  const selectVisibleSegments = () => {
    setSelectedSegmentIds((current) => Array.from(new Set([...current, ...filteredNavigatorSegments.filter((segment) => segment.speak_enabled).map((segment) => segment.id)])));
  };

  const navigateToSegment = (nextSegment: Segment) => {
    if (nextSegment.chapter_id !== chapterId) {
      setPendingSegmentId(nextSegment.id);
      setOffset(0);
      selectChapter(nextSegment.chapter_id);
      return;
    }
    if (!segments.some((segment) => segment.id === nextSegment.id)) {
      setPendingSegmentId(nextSegment.id);
      setOffset(Math.floor(nextSegment.order_index / pageSize) * pageSize);
      return;
    }
    selectSegment(nextSegment.id);
  };

  const navigateRelative = (direction: -1 | 1) => {
    if (currentGlobalIndex < 0) return;
    const nextSegment = orderedSegments[currentGlobalIndex + direction];
    if (nextSegment) navigateToSegment(nextSegment);
  };

  const navigateToNextReview = () => {
    const start = currentGlobalIndex >= 0 ? currentGlobalIndex + 1 : 0;
    const nextReview = [...orderedSegments.slice(start), ...orderedSegments.slice(0, start)].find((segment) => segment.status === "needs_review");
    if (nextReview) navigateToSegment(nextReview);
  };

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (editMode || event.isComposing) return;
      const target = event.target as HTMLElement | null;
      if (target && ["INPUT", "TEXTAREA", "SELECT", "BUTTON"].includes(target.tagName)) return;
      if (event.key === "ArrowLeft") { event.preventDefault(); navigateRelative(-1); }
      if (event.key === "ArrowRight") { event.preventDefault(); navigateRelative(1); }
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [editMode, currentGlobalIndex, orderedSegments, chapterId, segments]);

  if (!bookId) return null;
  const activeChapterId = chapterId ?? chapters[0]?.chapter.id;
  return (
    <main className="reader-shell">
      <header className="reader-book-toolbar">
        <button className="reader-back-button" type="button" onClick={closeReader} aria-label="返回我的古籍">‹</button>
        <div className="reader-book-title"><span className="brand-mark small" aria-hidden="true">古</span><div><h1>{book?.book.title ?? "正在加载…"}</h1><p>逐段校音工作台</p></div></div>
        <div className="reader-book-stats"><span>{book?.segment_count ?? 0} 段</span><span className="stat-review"><i />{needsReviewCount} 待确认</span><span className="stat-done"><i />{generatedCount} 已完成</span><span className="stat-muted"><i />{notGeneratedCount} 未生成</span></div>
        <div className="reader-book-actions">
          <button className="toolbar-button primary" type="button" onClick={() => void handleReanalyzeBook()} disabled={bookAnalysisBusy || batchRunning || exportRunning}>{bookAnalysisBusy ? "分析中…" : "全文分析"}</button>
          <button className="toolbar-button" type="button" onClick={() => void (batchRunning ? handleCancelBatch() : handleStartBatch())} disabled={bookAnalysisBusy || exportRunning}>{batchRunning ? "停止生成" : "生成全文"}</button>
          <BookExportPanel bookId={bookId} bookTitle={book?.book.title ?? "当前古籍"} selectedSegmentIds={selectedSegmentIds} state={exportForBook ? exportState : idleExportState} blockedByOtherExport={exportRunning && !exportForBook} onExport={(format, nextPreflight, segmentIds) => void handleExport(format, nextPreflight, segmentIds)} onCancel={() => void handleCancelExport()} compact />
          <button className="toolbar-button more-button" type="button" onClick={() => setMoreOpen((open) => !open)} aria-expanded={moreOpen}>更多…</button>
          {moreOpen && <div className="more-menu"><button type="button" onClick={() => { setMoreOpen(false); setView("books"); closeReader(); }}>我的古籍</button><button type="button" onClick={() => { setMoreOpen(false); setView("settings"); }}>语音设置</button><button type="button" onClick={() => { setMoreOpen(false); setView("rules"); }}>发音词典</button></div>}
        </div>
      </header>
      {error && <div className="error-banner reader-error" role="alert">{error}</div>}
      {message && <div className="success-banner reader-error" role="status">{message}</div>}
      <BookBatchPanel bookTitle={book?.book.title ?? "当前古籍"} state={batchForBook ? batchState : idleBatchState} preflight={preflight} running={batchRunning && batchForBook} analyzing={bookAnalysisBusy} onCancel={() => void handleCancelBatch()} />
      <div className="reader-layout">
          <aside className="segment-navigator">
            <div className="navigator-heading"><div><p className="eyebrow">段落导航</p><h2>章节与段落</h2></div><span>{book?.segment_count ?? 0}</span></div>
            <div className="segment-filters"><button className={segmentFilter === "all" ? "active" : ""} type="button" onClick={() => setSegmentFilter("all")}>全部 <b>{book?.segment_count ?? orderedSegments.length}</b></button><button className={segmentFilter === "needs_review" ? "active" : ""} type="button" onClick={() => setSegmentFilter("needs_review")}>待确认 <b>{needsReviewCount}</b></button><button className={segmentFilter === "not_generated" ? "active" : ""} type="button" onClick={() => setSegmentFilter("not_generated")}>未生成 <b>{notGeneratedCount}</b></button></div>
            <div className="navigator-selection"><span>已选 {selectedSegmentIds.length} 段</span><button type="button" onClick={selectVisibleSegments} disabled={filteredNavigatorSegments.length === 0}>全选当前列表</button><button type="button" onClick={() => setSelectedSegmentIds([])} disabled={selectedSegmentIds.length === 0}>清空</button></div>
            <div className="navigator-list">{chapters.map((summary) => { const chapterSegments = filteredNavigatorSegments.filter((segment) => segment.chapter_id === summary.chapter.id); return <div className="navigator-chapter" key={summary.chapter.id}><button className={summary.chapter.id === activeChapterId ? "navigator-chapter-title active" : "navigator-chapter-title"} type="button" onClick={() => { setOffset(0); selectChapter(summary.chapter.id); }}><span>{summary.chapter.title ?? `第 ${summary.chapter.order_index + 1} 章`}</span><small>{summary.segment_count}</small></button>{chapterSegments.map((segment) => <div className={`navigator-segment-row${segment.id === segmentId ? " active" : ""}`} key={segment.id}><input className="navigator-select" type="checkbox" checked={selectedSegmentIds.includes(segment.id)} onChange={() => toggleSegmentSelection(segment.id)} disabled={!segment.speak_enabled} aria-label={`选择第 ${orderedSegments.findIndex((item) => item.id === segment.id) + 1} 段`} /><button className="navigator-segment" type="button" onClick={() => navigateToSegment(segment)}><span className="navigator-index">{String(orderedSegments.findIndex((item) => item.id === segment.id) + 1).padStart(2, "0")}</span><span className="status-dot" data-status={segment.status} aria-label={reviewStatusLabel(segment.status)} /><span className="navigator-preview">{segment.reading_text ?? segment.original_text}</span>{!segment.speak_enabled && <small className="navigator-muted">不朗读</small>}</button></div>)}</div>; })}{loading && <div className="subtle-empty">正在读取段落…</div>}{!loading && filteredNavigatorSegments.length === 0 && <div className="subtle-empty">没有符合条件的段落。</div>}</div>
          </aside>
          <section className="reader-workspace">
            <div className="workspace-navigation"><button className="text-navigation-button" type="button" onClick={() => navigateRelative(-1)} disabled={currentGlobalIndex <= 0}>← 上一段</button><span>第 <strong>{currentPosition}</strong> / {book?.segment_count ?? orderedSegments.length} 段</span><button className="text-navigation-button" type="button" onClick={() => navigateRelative(1)} disabled={currentGlobalIndex < 0 || currentGlobalIndex >= orderedSegments.length - 1}>下一段 →</button></div>
            <div className="workspace-toolbar"><div className="font-controls"><button type="button" onClick={() => updateDisplaySettings({ font_size: Math.max(20, displaySettings.font_size - 1) })} aria-label="减小字号">A−</button><output>{displaySettings.font_size}px</output><button type="button" onClick={() => updateDisplaySettings({ font_size: Math.min(40, displaySettings.font_size + 1) })} aria-label="增大字号">A+</button></div><div className="pinyin-controls"><span>拼音：</span>{(["off", "risky", "all"] as const).map((mode) => <button key={mode} className={displaySettings.pinyin_mode === mode ? "active" : ""} type="button" onClick={() => updateDisplaySettings({ pinyin_mode: mode })}>{mode === "off" ? "关闭" : mode === "risky" ? "疑难" : "全文"}</button>)}</div><button className="toolbar-button edit-trigger" type="button" onClick={() => { if (editMode) handleCancelEdit(); else { setCursorTokenIndex(null); setEditMode(true); } }} disabled={!reader || batchRunning || exportRunning}>{editMode ? "取消编辑" : "编辑文本"}</button><button className="inspector-toggle" type="button" onClick={() => setInspectorOpen((open) => !open)} aria-expanded={inspectorOpen}>{inspectorOpen ? "收起检查器" : "发音检查器"}</button></div>
            {reader ? <>
              {editMode ? <SegmentEditingPanel reader={reader} draft={readingTextDraft} onDraftChange={setReadingTextDraft} onCaretChange={(text, caretOffset) => setCursorTokenIndex(graphemeIndexAtCaret(text, caretOffset))} cursorTokenIndex={cursorTokenIndex} onSplit={() => void handleSplit()} onMergePrevious={() => void handleMerge("previous")} onMergeNext={() => void handleMerge("next")} onSave={() => void handleSaveReadingText()} onCancel={handleCancelEdit} onRestore={() => void handleRestoreReadingText()} onSpeakEnabledChange={(enabled) => void handleSpeakEnabledChange(enabled)} busy={segmentEditBusy || analysisBusy || batchRunning || exportRunning} canMergePrevious={reader.segment.order_index > 0} canMergeNext={reader.segment.order_index + 1 < total} /> : <>
                {displaySettings.pinyin_mode === "all" && displayPinyinLoading && <p className="reader-display-loading">正在读取全文拼音…</p>}
                <div className={displaySettings.pinyin_mode === "off" ? "reader-text" : "reader-text pinyin-enabled"} style={{ fontSize: `${displaySettings.font_size}px` }} aria-label="分词原文">{reader.tokens.map((token) => {
              const annotation = annotationForToken(reader.annotations, token.index);
              if (annotation) {
                const text = reader.tokens.slice(annotation.start_token, annotation.end_token).map((part) => part.text).join("");
                const ttsLocked = isTtsLockedAnnotation(annotation);
                const annotationPinyin = annotation.review_status === "ignored" ? null : annotation.target_pinyin ?? annotation.default_pinyin ?? annotation.candidate_pinyin[0] ?? null;
                const fallbackPinyin = displaySettings.pinyin_mode === "all" ? displayPinyin?.slice(annotation.start_token, annotation.end_token).filter((value): value is string => value !== null).join(" ") ?? null : null;
                const shownPinyin = annotationPinyin ?? fallbackPinyin;
                const pronunciationLabel = ttsLocked ? "已锁定读音 · 将用于 TTS" : "预览读音 · 仅用于页面显示";
                const content = shownPinyin ? <ruby className={ttsLocked ? "pronunciation-ruby locked" : "pronunciation-ruby preview"}><span>{text}</span><rt>{pinyinToToneMarks(shownPinyin)}</rt></ruby> : text;
                return <button key={annotation.id} className={`annotation-token status-${annotation.review_status}${selectedAnnotationId === annotation.id ? " selected" : ""}${cursorTokenIndex === annotation.start_token ? " cursor-token" : ""}`} type="button" onClick={() => selectAnnotation(annotation)} title={`${riskTypeLabel(annotation.risk_type)} · ${reviewStatusLabel(annotation.review_status)} · ${pronunciationLabel}`}>{content}</button>;
              }
              const covered = reader.annotations.some((item) => token.index > item.start_token && token.index < item.end_token);
              if (covered) return null;
              const tokenPinyin = displaySettings.pinyin_mode === "all" ? displayPinyin?.[token.index] ?? null : null;
              const content = tokenPinyin ? <ruby className="pronunciation-ruby preview"><span>{token.text}</span><rt>{pinyinToToneMarks(tokenPinyin)}</rt></ruby> : token.text;
              return <button key={token.index} className={`${manualTokenIndex === token.index ? "plain-token selected" : "plain-token"}${cursorTokenIndex === token.index ? " cursor-token" : ""}`} type="button" onClick={() => selectManualToken(token.index)}>{content}</button>;
                })}</div>
                {displaySettings.pinyin_mode !== "off" && <div className="pronunciation-legend" aria-label="读音来源说明"><span className="pronunciation-legend-item locked"><i aria-hidden="true" />已锁定读音 <small>将用于语音合成</small></span><span className="pronunciation-legend-item preview"><i aria-hidden="true" />预览读音 <small>仅用于页面显示</small></span></div>}
                <p className="reader-help">点击标注查看读音详情；点击普通汉字可添加手工发音。只有已确认或规则生成的读音会进入语音合成。</p>
              </>}
              <AudioPanel reader={reader} busy={audioBusy || batchRunning || exportRunning} onGenerate={() => void handleGenerateAudio()} onSelect={(audioId) => void handleSelectAudio(audioId)} />
            </> : <div className="subtle-empty">选择一条段落查看正文。</div>}
          </section>
          <aside className={`inspector-panel${inspectorOpen ? " open" : ""}`}><div className="inspector-heading"><div><p className="eyebrow">发音检查器</p><h2>发音检查器</h2></div><button className="inspector-close" type="button" onClick={() => setInspectorOpen(false)}>收起</button><span className="inspector-mark" aria-hidden="true">⌁</span></div><PronunciationInspector reader={reader} annotation={selectedAnnotation} manualToken={manualToken} targetPinyin={targetPinyin} setTargetPinyin={setTargetPinyin} manualPinyin={manualPinyin} setManualPinyin={setManualPinyin} busy={analysisBusy || batchRunning || exportRunning} onAnalyze={() => void handleAnalyze()} onConfirm={() => void handleConfirm()} onIgnore={() => void handleAnnotationStatus("ignore")} onReset={() => void handleAnnotationStatus("reset")} onCreateRule={(scope) => void handleCreateRule(scope)} onManualAnnotation={() => void handleManualAnnotation()} onNextReview={navigateToNextReview} /></aside>
      </div>
    </main>
  );
}

function SegmentEditingPanel({ reader, draft, onDraftChange, onCaretChange, cursorTokenIndex, onSplit, onMergePrevious, onMergeNext, onSave, onCancel, onRestore, onSpeakEnabledChange, busy, canMergePrevious, canMergeNext }: { reader: SegmentReader; draft: string; onDraftChange: (value: string) => void; onCaretChange: (text: string, caretOffset: number) => void; cursorTokenIndex: number | null; onSplit: () => void; onMergePrevious: () => void; onMergeNext: () => void; onSave: () => void; onCancel: () => void; onRestore: () => void; onSpeakEnabledChange: (enabled: boolean) => void; busy: boolean; canMergePrevious: boolean; canMergeNext: boolean }) {
  const isEdited = reader.segment.reading_text !== null;
  const updateCaret = (event: SyntheticEvent<HTMLTextAreaElement>) => {
    const textarea = event.currentTarget;
    onCaretChange(textarea.value, textarea.selectionStart);
  };
  return <div className="segment-edit-panel" aria-label="段落编辑"><div className="segment-edit-heading"><div><p className="eyebrow">手工编辑</p><strong>朗读文本与分段</strong></div><label className="speak-toggle"><input type="checkbox" checked={reader.segment.speak_enabled} onChange={(event) => onSpeakEnabledChange(event.target.checked)} disabled={busy} />参与朗读</label></div><p className="segment-original-preview">导入原文（不可编辑）：{reader.segment.original_text}</p><label className="segment-edit-label">朗读文本 <span>原文不可变；可增加或删除标点、空格</span><textarea className="segment-edit-textarea" value={draft} onChange={(event) => { onDraftChange(event.target.value); onCaretChange(event.target.value, event.currentTarget.selectionStart); }} onSelect={updateCaret} onClick={updateCaret} onKeyUp={updateCaret} disabled={busy} rows={3} /></label><div className="segment-edit-actions"><button className="primary-button" type="button" onClick={onSave} disabled={busy || draft.trim().length === 0}>{busy ? "处理中…" : "保存朗读文本"}</button><button className="secondary-button" type="button" onClick={onCancel} disabled={busy}>取消</button><button className="secondary-button" type="button" onClick={onRestore} disabled={busy || !isEdited}>恢复为原文</button></div><div className="segment-structure-actions"><span className="segment-cursor-hint">{cursorTokenIndex === null ? "请在朗读文本中放置光标" : `将在第 ${cursorTokenIndex + 1} 个字词前分段`}</span><button className="secondary-button" type="button" onClick={onSplit} disabled={busy || cursorTokenIndex === null}>在光标处分段</button><button className="secondary-button" type="button" onClick={onMergePrevious} disabled={busy || !canMergePrevious}>与上一段合并</button><button className="secondary-button" type="button" onClick={onMergeNext} disabled={busy || !canMergeNext}>与下一段合并</button></div><p className="segment-edit-note">修改文本或分段后会清除当前发音分析并重新分析；已有语音版本保留，但需要重新生成。</p></div>;
}

function ReaderDisplaySettingsPanel({ settings, onChange }: { settings: ReaderDisplaySettings; onChange: (patch: Partial<ReaderDisplaySettings>) => void }) {
  return <div className="reader-display-settings" aria-label="阅读显示设置"><div><p className="eyebrow">阅读显示</p><strong>阅读显示</strong></div><label>正文字号 <output>{settings.font_size}px</output><input type="range" min="20" max="40" step="1" value={settings.font_size} onChange={(event) => onChange({ font_size: Number(event.target.value) })} aria-label="正文字号" /></label><label>注音<select value={settings.pinyin_mode} onChange={(event) => onChange({ pinyin_mode: event.target.value as ReaderPinyinMode })} aria-label="注音模式"><option value="off">关闭</option><option value="risky">仅标注字词</option><option value="all">全文汉字</option></select></label></div>;
}

function BookBatchPanel({ bookTitle, state, preflight, running, analyzing, onCancel }: { bookTitle: string; state: BatchGenerationState; preflight: BookGenerationPreflight | null; running: boolean; analyzing: boolean; onCancel: () => void }) {
  const isTerminal = ["completed", "cancelled", "failed"].includes(state.status);
  const denominator = state.segments_requiring_generation;
  const progress = batchProgressPercent(state);
  if (!running && !analyzing && !isTerminal) return null;
  return <div className="task-strip">{analyzing && <div><strong>正在分析《{bookTitle}》</strong><span>只更新发音标注，不会生成语音。</span></div>}{running && <div className="task-progress"><div><strong>正在生成《{bookTitle}》</strong><span>{state.processed} / {denominator} 段</span></div><div className="progress-track"><div className="progress-value" style={{ width: `${progress}%` }} /></div><small>已生成 {state.generated} · 已复用 {state.skipped} · 失败 {state.failed}</small></div>}{isTerminal && state.book_id && <div><strong>{state.status === "completed" ? "全文生成完成" : state.status === "cancelled" ? "已停止全文生成" : "全文生成失败"}</strong><span>生成 {state.generated} · 复用 {state.skipped} · 失败 {state.failed}</span>{state.fatal_error && <p className="stale-warning">{state.fatal_error.message}</p>}</div>}{(running || analyzing) && <button className="task-cancel" type="button" onClick={onCancel} disabled={!running || state.status === "cancelling"}>{state.status === "cancelling" ? "正在停止…" : "停止"}</button>}{preflight && !running && state.status === "idle" && preflight.blockers.length > 0 && <span className="batch-blocked">预检未通过</span>}</div>;
}

function BookExportPanel({ bookId, bookTitle, selectedSegmentIds, state, blockedByOtherExport, onExport, onCancel, compact = false }: { bookId: string; bookTitle: string; selectedSegmentIds: string[]; state: ExportState; blockedByOtherExport: boolean; onExport: (format: "mp3" | "wav", preflight: BookExportPreflight, segmentIds: string[]) => void; onCancel: () => void; compact?: boolean }) {
  const [format, setFormat] = useState<"mp3" | "wav">("mp3");
  const [preflight, setPreflight] = useState<BookExportPreflight | null>(null);
  const [dialogSegmentIds, setDialogSegmentIds] = useState<string[]>([]);
  const [dialogOpen, setDialogOpen] = useState(false);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const running = exportIsActive(state);
  const refreshPreflight = async () => {
    const nextSegmentIds = [...selectedSegmentIds];
    setLoading(true); setError(null);
    try { setDialogSegmentIds(nextSegmentIds); setPreflight(await getBookExportPreflight(bookId, nextSegmentIds.length > 0 ? nextSegmentIds : null)); setDialogOpen(true); }
    catch (reason: unknown) { setError(friendlyErrorMessage(reason)); }
    finally { setLoading(false); }
  };
  const selectionCount = selectedSegmentIds.length;
  const exportLabel = selectionCount > 0 ? `导出选中（${selectionCount}段）` : "导出整本";
  const dialogTitle = dialogSegmentIds.length > 0 ? `导出选中的 ${dialogSegmentIds.length} 段` : `导出《${bookTitle}》`;
  return <div className={compact ? "export-control" : "export-control standalone"}><button className="toolbar-button" type="button" onClick={() => void (running ? onCancel() : refreshPreflight())} disabled={blockedByOtherExport || loading}>{blockedByOtherExport ? "其他导出进行中" : running ? "取消导出" : loading ? "检查中…" : exportLabel}</button>{error && <span className="export-error">{error}</span>}{running && <div className="export-task"><strong>{dialogSegmentIds.length > 0 ? `正在导出选中的 ${dialogSegmentIds.length} 段` : `正在导出《${bookTitle}》`}</strong><span>{exportPhaseLabel(state)} · {state.total_segments} 段</span></div>}{state.status === "completed" && state.output_path && <div className="export-task success"><strong>导出完成</strong><span>{state.output_path}</span></div>}{state.status === "failed" && state.error_message && <div className="export-task error"><strong>导出失败</strong><span>{state.error_message}</span></div>}{dialogOpen && <div className="export-dialog"><div><p className="detail-label">{dialogTitle}</p>{preflight && !preflight.can_export ? <div className="export-blockers"><strong>当前不能导出</strong>{preflight.blockers.slice(0, 5).map((blocker) => <p key={`${blocker.code}-${blocker.segment_id ?? "book"}`}>{blocker.message}{blocker.preview ? ` · ${blocker.preview}` : ""}</p>)}</div> : <><label><input type="radio" checked={format === "mp3"} onChange={() => setFormat("mp3")} /> MP3（96 kbps）</label><label><input type="radio" checked={format === "wav"} onChange={() => setFormat("wav")} /> WAV（无损）</label>{preflight && <p className="rule-hint">段落：{preflight.total_segments} · 音频：{preflight.sample_rate ?? "—"}Hz / {preflight.channels ?? "—"} 声道</p>}</>}</div><div className="settings-actions"><button className="secondary-button" type="button" onClick={() => setDialogOpen(false)}>取消</button>{preflight?.can_export && <button className="primary-button" type="button" onClick={() => { setDialogOpen(false); onExport(format, preflight, dialogSegmentIds); }}>选择保存位置并导出</button>}</div></div>}</div>;
}

function PronunciationInspector({ reader, annotation, manualToken, targetPinyin, setTargetPinyin, manualPinyin, setManualPinyin, busy, onAnalyze, onConfirm, onIgnore, onReset, onCreateRule, onManualAnnotation, onNextReview }: { reader: SegmentReader | null; annotation: Annotation | null; manualToken: { text: string } | null; targetPinyin: string; setTargetPinyin: (value: string) => void; manualPinyin: string; setManualPinyin: (value: string) => void; busy: boolean; onAnalyze: () => void; onConfirm: () => void; onIgnore: () => void; onReset: () => void; onCreateRule: (scope: "book" | "global") => void; onManualAnnotation: () => void; onNextReview: () => void }) {
  if (!reader) return <div className="inspector-empty">选择一个段落开始校音。</div>;
  if (annotation) {
    const ttsLocked = isTtsLockedAnnotation(annotation);
    const pronunciationSourceLabel = ttsLocked ? "已锁定读音 · 将用于语音合成" : "预览读音 · 仅用于页面显示";
    return <div className="inspector-detail"><div className="inspector-token-heading"><div><span className="inspector-surface">{annotation.surface_text}</span><span className="inspector-tone">{annotation.target_pinyin ? pinyinToToneMarks(annotation.target_pinyin) : annotation.default_pinyin ? pinyinToToneMarks(annotation.default_pinyin) : "—"}</span></div><span className={`review-badge status-${annotation.review_status}`}>{reviewStatusLabel(annotation.review_status)}</span></div><div className={`pronunciation-source-badge ${ttsLocked ? "locked" : "preview"}`}><i aria-hidden="true" />{pronunciationSourceLabel}</div><div className="inspector-context"><span>上下文</span><p>{reader.segment.reading_text ?? reader.segment.original_text}</p></div><dl className="inspector-fields"><dt>默认读音</dt><dd>{annotation.default_pinyin ? pinyinToToneMarks(annotation.default_pinyin) : "无有效拼音"}<code>{annotation.default_pinyin ?? "—"}</code></dd><dt>建议读音</dt><dd>{annotation.target_pinyin ? pinyinToToneMarks(annotation.target_pinyin) : "待确认"}</dd><dt>来源</dt><dd>{annotation.source_rule_id ? "本书/全局发音规则" : annotationSourceLabel(annotation.source)}</dd><dt>规则</dt><dd>{ruleTypeLabel(annotation.rule_type)}</dd><dt>置信</dt><dd>{confidenceLabel(annotation.confidence)}</dd><dt>风险</dt><dd>{riskTypeLabel(annotation.risk_type)}</dd></dl>{annotation.candidate_pinyin.length > 0 && <div className="inspector-candidates"><p className="detail-label">候选读音</p>{annotation.candidate_pinyin.map((candidate) => <button key={candidate} className={candidate === targetPinyin ? "candidate-button selected" : "candidate-button"} type="button" onClick={() => setTargetPinyin(candidate)}>{pinyinToToneMarks(candidate)} <code>{candidate}</code></button>)}</div>}<label className="inspector-input-label">确认读音<input className="pinyin-input" value={targetPinyin} onChange={(event) => setTargetPinyin(normalizePinyinInput(event.target.value))} autoCapitalize="none" autoCorrect="off" autoComplete="off" spellCheck={false} inputMode="text" placeholder="输入 ASCII 数字拼音，例如 shu4" aria-label="目标拼音" /></label>{annotation.reason && <p className="detail-reason">{annotation.reason}</p>}<div className="inspector-actions"><button className="detail-action primary-detail-action" type="button" onClick={onConfirm} disabled={busy}>确认</button>{annotation.review_status === "needs_review" ? <button className="detail-action" type="button" onClick={onIgnore} disabled={busy}>忽略</button> : <button className="detail-action" type="button" onClick={onReset} disabled={busy}>恢复待确认</button>}</div>{annotation.review_status === "confirmed" && annotation.target_pinyin && <div className="inspector-scope"><p className="detail-label">应用范围</p><div className="scope-current">仅本处（确认后立即生效）</div><div className="inspector-scope-actions"><button className="detail-action" type="button" onClick={() => onCreateRule("book")} disabled={busy}>应用到本书</button><button className="detail-action" type="button" onClick={() => onCreateRule("global")} disabled={busy}>加入全局词典</button></div></div>}</div>;
  }
  if (manualToken) {
    return <div className="inspector-detail manual-details"><p className="detail-label">手工标注</p><h3>为“{manualToken.text}”添加发音</h3>{isHanToken(manualToken.text) ? <><p className="manual-hint">正文上方的注音仅用于阅读显示；点击“添加手工发音”后，读音才会进入 TTS 输入。</p><input className="pinyin-input" value={manualPinyin} onChange={(event) => setManualPinyin(normalizePinyinInput(event.target.value))} autoCapitalize="none" autoCorrect="off" autoComplete="off" spellCheck={false} inputMode="text" placeholder="例如 gui3" aria-label="手工拼音" disabled={busy} /><button className="detail-action primary-detail-action" type="button" onClick={onManualAnnotation} disabled={busy || manualPinyin.trim().length === 0}>添加手工发音</button></> : <p className="manual-hint">标点或非汉字 token 不支持手工发音。</p>}</div>;
  }
  const reviewCount = reader.annotations.filter((item) => item.review_status === "needs_review").length;
  const confirmedCount = reader.annotations.filter((item) => item.review_status === "confirmed").length;
  return <div className="inspector-summary"><div className="inspector-summary-row"><span>本段待确认</span><strong className={reviewCount > 0 ? "review-number" : ""}>{reviewCount}</strong></div><div className="inspector-summary-row"><span>本段已确认</span><strong>{confirmedCount}</strong></div><button className="next-review-button" type="button" onClick={onNextReview} disabled={busy || reviewCount === 0}>下一个待确认 →</button><button className="inspect-analyze-button" type="button" onClick={onAnalyze} disabled={busy}>{busy ? "处理中…" : "分析本段"}</button><p>点击正文中的黄色、绿色标注查看详情；也可以点击普通汉字添加手工读音。</p></div>;
}

function AnnotationDetails({ annotation, targetPinyin, setTargetPinyin, busy, onConfirm, onIgnore, onReset, onCreateRule }: { annotation: Annotation | null; targetPinyin: string; setTargetPinyin: (value: string) => void; busy: boolean; onConfirm: () => void; onIgnore: () => void; onReset: () => void; onCreateRule: (scope: "book" | "global") => void }) {
  if (!annotation) return null;
  return <div className="annotation-details"><div className="detail-heading"><div><p className="eyebrow">读音标注</p><h3>{annotation.surface_text}</h3></div><span className={`review-badge status-${annotation.review_status}`}>{reviewStatusLabel(annotation.review_status)}</span></div><div className="detail-grid"><span>风险</span><strong>{riskTypeLabel(annotation.risk_type)}</strong><span>来源</span><strong>{annotation.source_rule_id ? "发音词典规则" : annotationSourceLabel(annotation.source)}</strong><span>规则</span><strong>{ruleTypeLabel(annotation.rule_type)}</strong><span>置信</span><strong>{confidenceLabel(annotation.confidence)}</strong><span>默认</span><strong>{annotation.default_pinyin ?? "无有效拼音"}</strong></div>{annotation.candidate_pinyin.length > 0 && <div><p className="detail-label">候选读音</p><div className="candidate-list">{annotation.candidate_pinyin.map((candidate) => <button key={candidate} className={candidate === targetPinyin ? "candidate-button selected" : "candidate-button"} type="button" onClick={() => setTargetPinyin(candidate)}>{candidate}</button>)}</div></div>}<input className="pinyin-input" value={targetPinyin} onChange={(event) => setTargetPinyin(normalizePinyinInput(event.target.value))} autoCapitalize="none" autoCorrect="off" autoComplete="off" spellCheck={false} inputMode="text" placeholder="输入 ASCII 数字拼音，例如 shu4" aria-label="目标拼音" />{annotation.reason && <p className="detail-reason">{annotation.reason}</p>}<div className="detail-actions"><button className="detail-action primary-detail-action" type="button" onClick={onConfirm} disabled={busy}>确认读音</button>{annotation.review_status === "needs_review" ? <button className="detail-action" type="button" onClick={onIgnore} disabled={busy}>忽略</button> : <button className="detail-action" type="button" onClick={onReset} disabled={busy}>重置复核</button>}</div>{annotation.review_status === "confirmed" && annotation.target_pinyin && <div className="rule-actions"><p className="detail-label">复用这条确认</p><div className="detail-actions"><button className="detail-action" type="button" onClick={() => onCreateRule("book")} disabled={busy}>应用到本书</button><button className="detail-action" type="button" onClick={() => onCreateRule("global")} disabled={busy}>加入全局词典</button></div></div>}</div>;
}

function AudioPanel({ reader, busy, onGenerate, onSelect }: { reader: SegmentReader; busy: boolean; onGenerate: () => void; onSelect: (audioId: string) => void }) {
  const { segment, audio_versions: versions } = reader;
  const current = versions.find((version) => version.id === segment.current_audio_id) ?? null;
  const canGenerate = segment.speak_enabled && ["analyzed", "ready", "generated"].includes(segment.status);
  const stale = current !== null && segment.status !== "generated";
  return <div className="audio-toolbar"><div className="audio-playback">{current ? <audio className="audio-player" controls src={convertFileSrc(current.audio_path)} aria-label="当前段落语音" /> : <span className="audio-empty">尚未生成语音</span>}{stale && <span className="audio-stale">基于旧发音</span>}</div><span className={`audio-state ${segment.status}`}>{!segment.speak_enabled ? "不朗读" : segment.status === "needs_review" ? "待确认" : segment.status === "pending" ? "未分析" : segment.status === "generated" ? "已生成" : "可生成"}</span><button className="secondary-button audio-generate-button" type="button" onClick={onGenerate} disabled={!canGenerate || busy}>{busy ? "处理中…" : "重新生成"}</button>{versions.length > 1 && <details className="audio-versions"><summary>版本 {versions.length}</summary><div>{versions.map((version) => <button key={version.id} className={version.id === segment.current_audio_id ? "audio-version current" : "audio-version"} type="button" onClick={() => onSelect(version.id)} disabled={busy}>第 {version.version_no} 版 · {version.id === segment.current_audio_id ? "当前" : "试听"}</button>)}</div></details>}</div>;
}

function SettingsPage() {
  const batchState = useBatchGenerationState();
  const batchRunning = batchIsActive(batchState);
  const exportState = useExportState();
  const exportRunning = exportIsActive(exportState);
  const [settings, setSettings] = useState<TtsSettings | null>(null);
  const [voices, setVoices] = useState<TencentVoice[]>([]);
  const [credentials, setCredentials] = useState<CredentialStatus | null>(null);
  const [secretId, setSecretId] = useState("");
  const [secretKey, setSecretKey] = useState("");
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [usageRange, setUsageRange] = useState<ApiUsageRange>("30d");
  const [usage, setUsage] = useState<ApiUsageSummary | null>(null);
  const [usageLoading, setUsageLoading] = useState(false);
  const [previewText, setPreviewText] = useState("上古之人，其知道者，法于阴阳，和于术数。");
  const [preview, setPreview] = useState<TtsPreviewResult | null>(null);
  const [previewBusy, setPreviewBusy] = useState(false);
  const [previewError, setPreviewError] = useState<string | null>(null);

  useEffect(() => {
    void Promise.all([getTtsSettings(), getTencentVoices(), getTtsCredentialStatus()])
      .then(([nextSettings, nextVoices, nextCredentials]) => {
        setSettings(nextSettings);
        setVoices(nextVoices);
        setCredentials(nextCredentials);
      })
      .catch((reason: unknown) => setError(friendlyErrorMessage(reason)));
  }, []);

  const refreshUsage = async (range = usageRange) => {
    setUsageLoading(true);
    try {
      setUsage(await getApiUsageSummary(range));
    } catch (reason: unknown) {
      setError(friendlyErrorMessage(reason));
    } finally {
      setUsageLoading(false);
    }
  };

  useEffect(() => { void refreshUsage(usageRange); }, [usageRange]);

  if (!settings) return <main className="content"><div className="empty-panel">正在读取 TTS 设置…</div></main>;
  const update = (patch: Partial<TtsSettings>) => setSettings({ ...settings, ...patch });
  const run = async (action: () => Promise<void>) => { setBusy(true); setError(null); setMessage(null); try { await action(); } catch (reason: unknown) { setError(friendlyErrorMessage(reason)); } finally { setBusy(false); } };
  const voiceGroups = ["大模型·阅读", "精品·通用"];
  const selectedVoice = voices.find((voice) => voice.id === settings.voice_type);
  const selectedVoiceName = selectedVoice?.name ?? `音色 ${settings.voice_type}`;
  const credentialsReady = Boolean(credentials?.secret_id_configured && credentials.secret_key_configured);
  const previewDisabled = busy || batchRunning || exportRunning || previewBusy || !credentialsReady || previewText.trim().length === 0 || previewText.length > 200;
  const usageRangeLabels: Array<[ApiUsageRange, string]> = [["today", "今天"], ["7d", "近 7 天"], ["30d", "近 30 天"], ["all", "全部"]];
  const handleConnectionTest = () => void run(async () => {
    try {
      await testTtsConnection(settings.voice_type, settings.sample_rate);
      setMessage("连接测试成功。");
    } finally {
      await refreshUsage();
    }
  });
  const handlePreview = async () => {
    if (previewDisabled) return;
    setPreviewBusy(true);
    setPreviewError(null);
    setError(null);
    try {
      setPreview(await generateTtsPreview(previewText, settings));
      await refreshUsage();
    } catch (reason: unknown) {
      setPreviewError(friendlyErrorMessage(reason));
    } finally {
      setPreviewBusy(false);
    }
  };

  return <main className="content settings-content">
    <div className="page-heading"><div><p className="kicker">设置 · 语音合成</p><h2>腾讯云语音合成</h2><p>凭证保存在应用数据目录的受控配置文件中，不进入 SQLite、日志或前端持久化。</p>{(batchRunning || exportRunning) && <p className="batch-lock-note">语音任务进行中，语音设置暂时锁定。</p>}</div></div>
    {error && <div className="error-banner" role="alert">{error}</div>}
    {message && <div className="success-banner" role="status">{message}</div>}
    <div className="settings-grid">
      <fieldset className="settings-card settings-fieldset" disabled={busy || batchRunning || exportRunning}>
        <p className="eyebrow">云服务凭证</p><h3>腾讯云凭证</h3>
        <label>SecretId<input className="settings-input" value={secretId} onChange={(event) => setSecretId(event.target.value)} placeholder={credentials?.secret_id_configured ? "已配置，输入新值可覆盖" : "请输入 SecretId"} /></label>
        <label>SecretKey<input className="settings-input" type="password" value={secretKey} onChange={(event) => setSecretKey(event.target.value)} placeholder={credentials?.secret_key_configured ? "已配置，输入新值可覆盖" : "请输入 SecretKey"} /></label>
        <p className="credential-status">状态：{credentialsReady ? "✓ 已配置" : "未配置"}</p>
        <div className="settings-actions"><button className="primary-button" type="button" disabled={busy || !secretId.trim() || !secretKey.trim()} onClick={() => void run(async () => { setCredentials(await saveTencentCredentials(secretId, secretKey)); setSecretId(""); setSecretKey(""); setMessage("凭据已保存，Worker 已重启并完成 ping。"); })}>保存凭证</button><button className="secondary-button" type="button" disabled={busy || !credentials?.secret_id_configured} onClick={() => void run(async () => { setCredentials(await deleteTencentCredentials()); setMessage("凭据已删除。"); })}>删除凭据</button></div>
      </fieldset>

      <fieldset className="settings-card settings-fieldset" disabled={busy || batchRunning || exportRunning}>
        <p className="eyebrow">声音参数</p><h3>合成参数</h3>
        <label>服务商<select className="settings-input" value={settings.provider} onChange={(event) => update({ provider: event.target.value })}><option value="tencent">腾讯云</option></select></label>
        <label>音色<select className="settings-input" value={settings.voice_type} onChange={(event) => update({ voice_type: Number(event.target.value) })}>{voiceGroups.map((group) => <optgroup key={group} label={group}>{voices.filter((voice) => voice.category === group).map((voice) => <option key={voice.id} value={voice.id}>{voice.name} · {voice.description} ({voice.id})</option>)}</optgroup>)}</select></label>
        <label>语速 <span className="field-hint">腾讯参数：-2 ～ 6，0 为正常速度</span><input className="settings-input" type="number" min="-2" max="6" step="0.1" value={settings.speed} onChange={(event) => update({ speed: Number(event.target.value) })} /></label>
        <label>音量 <span className="field-hint">-10 ～ 10</span><input className="settings-input" type="number" min="-10" max="10" step="1" value={settings.volume} onChange={(event) => update({ volume: Number(event.target.value) })} /></label>
        <p className="fixed-setting">采样率：16000Hz · 格式：WAV</p>
        <div className="settings-actions"><button className="primary-button" type="button" disabled={busy} onClick={() => void run(async () => { setSettings(await saveTtsSettings(settings)); setMessage("语音设置已保存。"); })}>保存设置</button><button className="secondary-button" type="button" disabled={busy || !credentialsReady} onClick={handleConnectionTest}>测试连接</button></div>
      </fieldset>

      <section className="settings-card usage-card">
        <p className="eyebrow">本地统计</p><h3>API 调用用量</h3><p className="settings-card-intro">仅统计本应用发起的腾讯云语音请求，不包含费用估算。</p>
        <div className="usage-range" role="tablist" aria-label="用量统计范围">{usageRangeLabels.map(([range, label]) => <button key={range} className={usageRange === range ? "active" : ""} type="button" onClick={() => setUsageRange(range)}>{label}</button>)}</div>
        {usageLoading && !usage ? <p className="usage-loading">正在读取用量…</p> : <><div className="usage-metrics"><div><strong>{usage?.tts.requests ?? 0}</strong><span>请求次数</span></div><div><strong>{usage?.tts.success ?? 0}</strong><span>成功</span></div><div><strong>{usage?.tts.failed ?? 0}</strong><span>失败</span></div><div><strong>{usage?.tts.characters ?? 0}</strong><span>成功字符</span></div></div><div className="usage-voices"><p className="detail-label">常用音色（按成功字符）</p>{usage?.tts.by_voice.length ? usage.tts.by_voice.map((item) => <div className="usage-voice-row" key={item.voice_type}><span>{voices.find((voice) => voice.id === item.voice_type)?.name ?? `音色 ${item.voice_type}`}</span><strong>{item.characters} 字</strong></div>) : <span className="usage-empty">暂无记录</span>}</div></>}
      </section>

      <section className="settings-card preview-card">
        <p className="eyebrow">无需保存即可试听</p><h3>音色试听 · {selectedVoiceName}</h3><p className="settings-card-intro">当前选择会立即同步；只有点击“保存设置”后才会成为正式合成参数。</p>
        <div className="preview-settings"><span>{selectedVoice?.description ?? "腾讯云音色"}</span><span>语速 {settings.speed}</span><span>音量 {settings.volume}</span><span>{settings.sample_rate}Hz</span></div>
        <label className="preview-label">试听文本<textarea className="preview-textarea" maxLength={200} value={previewText} onChange={(event) => setPreviewText(event.target.value)} placeholder="输入不超过 200 字的试听文本" /></label><div className="preview-count">{previewText.length} / 200</div>
        <button className="primary-button preview-button" type="button" disabled={previewDisabled} onClick={() => void handlePreview()}>{previewBusy ? "正在生成试听…" : "生成试听"}</button>
        {!credentialsReady && <p className="preview-hint">请先保存腾讯云凭证。</p>}
        {previewError && <p className="preview-error" role="alert">{previewError}</p>}
        {preview && <div className="preview-player"><audio controls src={convertFileSrc(preview.audio_path)} aria-label={`${voices.find((voice) => voice.id === preview.voice_type)?.name ?? `音色 ${preview.voice_type}`}试听音频`} /><span>{voices.find((voice) => voice.id === preview.voice_type)?.name ?? `音色 ${preview.voice_type}`}试听音频已生成</span></div>}
      </section>
    </div>
  </main>;
}

function RuleManagerPage() {
  const batchState = useBatchGenerationState();
  const batchRunning = batchIsActive(batchState);
  const exportState = useExportState();
  const exportRunning = exportIsActive(exportState);
  const currentBookId = useLibraryStore((state) => state.bookId);
  const [books, setBooks] = useState<BookSummary[]>([]);
  const [selectedBookId, setSelectedBookId] = useState(currentBookId ?? "");
  const [tab, setTab] = useState<"book" | "global">("book");
  const [rules, setRules] = useState<PronunciationRule[]>([]);
  const [patternText, setPatternText] = useState("");
  const [targetPinyin, setTargetPinyin] = useState("");
  const [editingId, setEditingId] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [message, setMessage] = useState<string | null>(null);

  const refresh = async (nextTab = tab, nextBookId = selectedBookId) => {
    setLoading(true);
    try {
      const nextRules = nextTab === "global"
        ? await listGlobalPronunciationRules()
        : nextBookId ? await listBookPronunciationRules(nextBookId) : [];
      setRules(nextRules);
      setError(null);
    } catch (reason: unknown) { setError(friendlyErrorMessage(reason)); }
    finally { setLoading(false); }
  };

  useEffect(() => {
    listBooks().then((nextBooks) => {
      setBooks(nextBooks);
      if (!selectedBookId && nextBooks.length > 0) setSelectedBookId(nextBooks[0].id);
    }).catch((reason: unknown) => setError(friendlyErrorMessage(reason)));
  }, []);

  useEffect(() => { void refresh(tab, selectedBookId); }, [tab, selectedBookId]);

  const resetForm = () => { setEditingId(null); setPatternText(""); setTargetPinyin(""); };
  const submit = async () => {
    if (exportRunning) return;
    if (!patternText.trim() || !targetPinyin.trim() || (tab === "book" && !selectedBookId)) return;
    if (tab === "global" && isSingleHanGlobalRule(patternText) && !window.confirm(`这是单字全局发音规则。\n它会应用到以后所有书籍中出现的“${patternText.trim()}”。\n建议优先建立词组或上下文规则。\n\n确认继续？`)) return;
    const editingRule = editingId ? rules.find((rule) => rule.id === editingId) : undefined;
    if (editingId && !editingRule) {
      setError("当前规则不在已加载的列表中，请重新选择后再编辑。");
      return;
    }
    setBusy(true); setError(null); setMessage(null);
    try {
      const result = editingRule
        ? await updatePronunciationRule(editingRule, patternText, targetPinyin)
        : await createPronunciationRule({ scope: tab, bookId: tab === "book" ? selectedBookId : null, patternText, targetPinyin });
      setMessage(`规则已保存，匹配 ${result.apply_result.matched_segments} 个段落。`);
      resetForm(); await refresh();
    } catch (reason: unknown) { setError(friendlyErrorMessage(reason)); }
    finally { setBusy(false); }
  };

  const toggle = async (rule: PronunciationRule) => {
    if (exportRunning) return;
    setBusy(true); setError(null); setMessage(null);
    try {
      const result = rule.enabled ? await disablePronunciationRule(rule.id) : await enablePronunciationRule(rule.id);
      setMessage(`${rule.enabled ? "规则已禁用" : "规则已启用"}，匹配 ${result.apply_result.matched_segments} 个段落。`);
      await refresh();
    } catch (reason: unknown) { setError(friendlyErrorMessage(reason)); }
    finally { setBusy(false); }
  };

  const applyBook = async () => {
    if (!selectedBookId || exportRunning) return;
    setBusy(true); setError(null); setMessage(null);
    try {
      const result = await applyPronunciationRulesToBook(selectedBookId);
      setMessage(`规则应用完成：匹配 ${result.matched_segments} 个段落，新增 ${result.created_annotations} 条标注，跳过 ${result.skipped_manual_overrides} 处人工覆盖。`);
      await refresh();
    } catch (reason: unknown) { setError(friendlyErrorMessage(reason)); }
    finally { setBusy(false); }
  };

  return <main className="content rules-content"><div className="page-heading"><div><p className="kicker">发音词典 · 规则管理</p><h2>发音词典</h2><p>精确匹配已确认的词语发音；规则只会影响发音标注，不会自动生成语音。</p>{batchRunning && <p className="batch-lock-note">批量生成进行中，发音词典暂时锁定。</p>}</div></div>{error && <div className="error-banner" role="alert">{error}</div>}{message && <div className="success-banner" role="status">{message}</div>}<fieldset disabled={busy || batchRunning}><div className="rule-tabs"><button className={tab === "book" ? "rule-tab active" : "rule-tab"} type="button" onClick={() => setTab("book")}>本书词典</button><button className={tab === "global" ? "rule-tab active" : "rule-tab"} type="button" onClick={() => setTab("global")}>全局词典</button></div>{tab === "book" && <label className="rule-book-select">当前书<select className="settings-input" value={selectedBookId} onChange={(event) => setSelectedBookId(event.target.value)}><option value="">请选择书籍</option>{books.map((book) => <option key={book.id} value={book.id}>{book.title}</option>)}</select></label>}<section className="rule-form"><div className="rule-form-heading"><div><p className="eyebrow">{editingId ? "编辑规则" : "新增规则"}</p><h3>{editingId ? "编辑规则" : "新增规则"}</h3></div>{editingId && <button className="secondary-button" type="button" onClick={resetForm}>取消编辑</button>}</div><div className="rule-form-grid"><label>词语<input className="settings-input" value={patternText} onChange={(event) => setPatternText(event.target.value)} placeholder="例如 腧穴" /></label><label>拼音<input className="settings-input pinyin-entry" value={targetPinyin} onChange={(event) => setTargetPinyin(normalizePinyinInput(event.target.value))} autoCapitalize="none" autoCorrect="off" autoComplete="off" spellCheck={false} inputMode="text" placeholder="例如 shu4 xue2" /></label></div><p className="rule-hint">拼音使用 ASCII 数字声调；词语按 Grapheme Token 精确匹配。</p><button className="primary-button" type="button" onClick={() => void submit()} disabled={busy || !patternText.trim() || !targetPinyin.trim() || (tab === "book" && !selectedBookId)}>{busy ? "处理中…" : editingId ? "保存修改" : "新增规则"}</button></section><div className="rule-toolbar"><div><p className="eyebrow">规则列表</p><h3>{tab === "book" ? "本书规则" : "全局规则"}</h3></div>{tab === "book" && <button className="secondary-button" type="button" onClick={() => void applyBook()} disabled={busy || !selectedBookId}>立即应用到本书</button>}</div>{loading ? <div className="empty-panel">正在读取规则…</div> : rules.length === 0 ? <div className="empty-panel"><h3>还没有规则</h3><p>确认一条发音后，可以从详情页复用为规则。</p></div> : <div className="rule-list">{rules.map((rule) => <article className={rule.enabled ? "rule-row" : "rule-row disabled"} key={rule.id}><div className="rule-main"><strong>{rule.pattern_text}</strong><code>{rule.target_pinyin}</code><span>{ruleTypeLabel(rule.rule_type)}</span></div><div className="rule-meta"><span>{rule.enabled ? "启用" : "已禁用"}</span><span>应用 {rule.application_count} 次</span><span>{rule.source ?? "手工"}</span></div><div className="rule-actions"><button className="secondary-button" type="button" onClick={() => { setEditingId(rule.id); setPatternText(rule.pattern_text); setTargetPinyin(rule.target_pinyin); }} disabled={busy}>编辑</button><button className="secondary-button" type="button" onClick={() => void toggle(rule)} disabled={busy}>{rule.enabled ? "禁用" : "启用"}</button></div></article>)}</div>}</fieldset></main>;
}

function DeveloperStatusPage() {
  const { status, lastPing, loading, error, refresh, ping } = useStatusStore();
  useEffect(() => { void refresh(); }, [refresh]);
  return <main className="content"><div className="page-heading"><div><p className="kicker">运行状态</p><h2>应用运行状态</h2><p>应用、数据库与发音服务的基础运行状态。</p></div></div>{error && <div className="error-banner" role="alert">{error}</div>}<div className="status-grid"><section className="status-card"><p className="eyebrow">应用</p><h2>应用程序</h2><Indicator status={status?.application ?? { ok: true, message: "应用正在运行" }} /></section><section className="status-card"><p className="eyebrow">数据</p><h2>数据库</h2><Indicator status={status?.database ?? { ok: false, message: "检查中…" }} /></section><WorkerCard status={status?.worker ?? { ok: false, running: false, version: null, message: "检查中…" }} /></div><section className="actions-card"><div><p className="eyebrow">通信检查</p><h2>发音服务响应</h2><p className="muted">通过应用内部通信检查发音服务。</p></div><button className="primary-button" type="button" onClick={() => void ping()} disabled={loading}>{loading ? "检查中…" : "检查服务"}</button>{lastPing && <div className="ping-result"><span>响应正常</span><strong>{lastPing.elapsed_ms} 毫秒</strong><small>版本 {lastPing.version}</small></div>}</section></main>;
}

export default function App() {
  const view = useLibraryStore((state) => state.view);
  return <>{view !== "reader" && <AppHeader />}{view === "books" && <BooksPage />}{view === "reader" && <ReaderPage />}{view === "settings" && <SettingsPage />}{view === "rules" && <RuleManagerPage />}{view === "status" && <DeveloperStatusPage />}</>;
}
