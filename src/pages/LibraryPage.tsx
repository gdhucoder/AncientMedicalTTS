import { getCurrentWebview } from "@tauri-apps/api/webview";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { open } from "@tauri-apps/plugin-dialog";
import { useCallback, useEffect, useMemo, useState } from "react";
import type { KeyboardEvent, MouseEvent } from "react";
import { friendlyErrorMessage } from "../services/errors";
import { deleteBook, getAnnotatedBookImportPreflight, getBookGenerationPreflight, importAnnotatedBook, importTxtBook, listBooks } from "../services/library";
import { bookCoverGlyph, bookMatchesQuery, fileStem, isSupportedTxtPath, relativeImportLabel, sortLibraryBooks, sourceFileName } from "../services/libraryUi";
import type { LibrarySort } from "../services/libraryUi";
import type { AnnotatedImportPreflight, BookSummary } from "../types/library";

type LibraryViewMode = "cards" | "list";

type ImportFeedback =
  | { kind: "importing"; title: string }
  | { kind: "success"; title: string; bookId: string; segmentCount: number; chapterCount: number; hanCharacterCount: number | null }
  | null;

type LibraryPageProps = {
  operationsLocked: boolean;
  onOpenBook: (bookId: string) => void;
};

const AUTO_OPEN_PREFERENCE = "ancient-medical-tts.library.auto-open-import";

async function restoreLibraryWindowFocus() {
  if (!("__TAURI_INTERNALS__" in window)) return;
  try {
    const currentWindow = getCurrentWindow();
    await currentWindow.show();
    await currentWindow.setFocus();
  } catch {
    // The browser preview and older Tauri runtimes may not expose window focus APIs.
  }
}

function shouldAutoOpenImport(): boolean {
  return window.localStorage.getItem(AUTO_OPEN_PREFERENCE) !== "false";
}

function ImportDropzone({ active, disabled, importing, autoOpen, onAutoOpenChange, onChoose, onChooseAnnotated }: { active: boolean; disabled: boolean; importing: boolean; autoOpen: boolean; onAutoOpenChange: (value: boolean) => void; onChoose: () => void; onChooseAnnotated: () => void }) {
  return (
    <section className={`import-dropzone${active ? " drag-active" : ""}${disabled ? " disabled" : ""}`} aria-label="导入古籍 TXT">
      <div className="import-document-icon" aria-hidden="true"><span /></div>
      <div className="import-dropzone-copy">
        <strong>{importing ? "正在导入古籍…" : active ? "松开即可导入" : "将古籍 TXT 文件拖到这里"}</strong>
        <span className="import-or">或</span>
        <div className="import-choice-row"><button className="import-choose-button" type="button" onClick={onChoose} disabled={disabled || importing}>选择 TXT 文件</button><button className="import-secondary-button" type="button" onClick={onChooseAnnotated} disabled={disabled || importing}>导入已注音古籍</button></div>
        <small>支持 UTF-8 TXT · 单个文档最多 5000 个汉字</small>
        <label className="import-auto-open"><input type="checkbox" checked={autoOpen} onChange={(event) => onAutoOpenChange(event.target.checked)} />导入后自动打开</label>
      </div>
      <div className="import-capabilities" aria-hidden="true">
        <span>导入后可进行</span>
        <strong>发音分析</strong>
        <strong>人工校对</strong>
        <strong>生成朗读音频</strong>
      </div>
    </section>
  );
}

function AnnotatedImportDialog({ preflight, trust, busy, onTrustChange, onCancel, onImport }: { preflight: AnnotatedImportPreflight; trust: boolean; busy: boolean; onTrustChange: (value: boolean) => void; onCancel: () => void; onImport: () => void }) {
  const authoritative = preflight.pronunciation_mode === "authoritative" || trust;
  return <div className="modal-backdrop" role="presentation"><section className="modal-card annotated-import-dialog" role="dialog" aria-modal="true" aria-labelledby="annotated-import-title"><button className="modal-close" type="button" onClick={onCancel} disabled={busy} aria-label="关闭">×</button><h2 id="annotated-import-title">导入已注音古籍</h2><p className="modal-lead">先检查文件内容，再决定注音是否直接用于语音合成。</p><div className="annotated-import-summary"><strong>{preflight.title ?? "未命名古籍"}</strong><span>{preflight.chapter_count} 个章节 · {preflight.segment_count} 个段落 · {preflight.han_character_count.toLocaleString("zh-CN")} 个汉字</span><span>拼音覆盖率：{preflight.pinyin_coverage_percent.toFixed(1)}% · 译文：{preflight.translation_segment_count} 段</span><span>注音模式：{authoritative ? "最终发音" : "参考注音"}</span></div><label className="annotated-trust-option"><input type="checkbox" checked={authoritative} disabled={preflight.pronunciation_mode === "authoritative"} onChange={(event) => onTrustChange(event.target.checked)} />将文件中的注音作为最终发音</label><p className="annotated-import-note">{authoritative ? "导入后可直接按文件注音生成 TTS；之后仍可用人工校音或发音规则覆盖。" : "当前只保存为参考注音，不会自动强制进入 TTS；如确认文件注音可靠，请勾选上方选项。"}</p>{preflight.errors.length > 0 && <div className="error-banner"><strong>无法导入</strong><span>{preflight.errors[0].message}</span></div>}{preflight.warnings.length > 0 && <div className="annotated-import-warnings"><strong>提示</strong>{preflight.warnings.slice(0, 3).map((warning) => <span key={`${warning.code}-${warning.message}`}>{warning.message}</span>)}</div>}<div className="modal-actions"><button type="button" className="import-secondary-button" onClick={onCancel} disabled={busy}>取消</button><button type="button" className="import-choose-button" onClick={onImport} disabled={busy || !preflight.can_import}>{busy ? "正在导入…" : "确认导入"}</button></div></section></div>;
}

function ImportStatusToast({ feedback, onOpen }: { feedback: ImportFeedback; onOpen: (bookId: string) => void }) {
  if (!feedback) return null;
  if (feedback.kind === "importing") {
    return <div className="import-toast importing" role="status"><span className="toast-spinner" aria-hidden="true" /><div><strong>正在导入《{feedback.title}》…</strong><small>正在创建篇章与段落</small></div></div>;
  }
  return <div className="import-toast success" role="status"><span className="toast-success" aria-hidden="true">✓</span><div><strong>《{feedback.title}》导入完成</strong><small>{feedback.hanCharacterCount === null ? `${feedback.chapterCount} 个篇章` : `${feedback.hanCharacterCount.toLocaleString("zh-CN")} 汉字`} · {feedback.segmentCount} 个段落 · 即将打开</small></div><button type="button" onClick={() => onOpen(feedback.bookId)}>现在打开</button></div>;
}

function LibraryToolbar({ query, onQueryChange, sort, onSortChange, viewMode, onViewModeChange }: { query: string; onQueryChange: (value: string) => void; sort: LibrarySort; onSortChange: (value: LibrarySort) => void; viewMode: LibraryViewMode; onViewModeChange: (value: LibraryViewMode) => void }) {
  return (
    <div className="library-toolbar">
      <label className="library-search"><span aria-hidden="true">⌕</span><span className="sr-only">搜索古籍</span><input value={query} onChange={(event) => onQueryChange(event.target.value)} placeholder="搜索书名或文件名…" /></label>
      <label className="library-sort"><span className="sr-only">书库排序</span><select value={sort} onChange={(event) => onSortChange(event.target.value as LibrarySort)}><option value="recent">最近导入</option><option value="title">书名</option></select></label>
      <div className="library-view-toggle" role="group" aria-label="书库视图">
        <button className={viewMode === "cards" ? "active" : ""} type="button" onClick={() => onViewModeChange("cards")} aria-pressed={viewMode === "cards"}><span aria-hidden="true">▦</span>卡片</button>
        <button className={viewMode === "list" ? "active" : ""} type="button" onClick={() => onViewModeChange("list")} aria-pressed={viewMode === "list"}><span aria-hidden="true">☷</span>列表</button>
      </div>
    </div>
  );
}

function BookOverflowMenu({ book, open: menuOpen, disabled, onToggle, onOpen, onDelete }: { book: BookSummary; open: boolean; disabled: boolean; onToggle: (event: MouseEvent<HTMLButtonElement>) => void; onOpen: () => void; onDelete: () => void }) {
  return <div className="book-menu-wrap" onPointerDown={(event) => event.stopPropagation()} onClick={(event) => event.stopPropagation()}><button className="book-menu-button" type="button" onClick={onToggle} disabled={disabled} aria-label={`更多《${book.title}》操作`} aria-haspopup="menu" aria-expanded={menuOpen}>•••</button>{menuOpen && <div className="book-overflow-menu" role="menu"><button type="button" role="menuitem" onClick={onOpen}>打开</button><div className="book-menu-divider" /><button className="danger" type="button" role="menuitem" onClick={onDelete}>删除</button></div>}</div>;
}

function BookCard({ book, viewMode, menuOpen, disabled, onToggleMenu, onOpen, onDelete }: { book: BookSummary; viewMode: LibraryViewMode; menuOpen: boolean; disabled: boolean; onToggleMenu: (event: MouseEvent<HTMLButtonElement>) => void; onOpen: () => void; onDelete: () => void }) {
  const handleKeyDown = (event: KeyboardEvent<HTMLElement>) => {
    if (event.currentTarget !== event.target || (event.key !== "Enter" && event.key !== " ")) return;
    event.preventDefault();
    onOpen();
  };
  return (
    <article className={`library-book-card ${viewMode}`} role="button" tabIndex={0} onClick={onOpen} onKeyDown={handleKeyDown} aria-label={`打开《${book.title}》`}>
      <div className="book-cover" aria-hidden="true"><span>{bookCoverGlyph(book.title)}</span></div>
      <div className="book-card-copy">
        <h3 title={book.title}>{book.title}</h3>
        <p className="book-source" title={book.source_file ?? undefined}>{sourceFileName(book.source_file)}</p>
        <div className="book-metadata"><span>{book.chapter_count} 篇章</span><span>{book.segment_count} 段</span></div>
        <p className="book-import-time"><span aria-hidden="true">◷</span>{relativeImportLabel(book.created_at)}</p>
      </div>
      <BookOverflowMenu book={book} open={menuOpen} disabled={disabled} onToggle={onToggleMenu} onOpen={onOpen} onDelete={onDelete} />
    </article>
  );
}

function EmptyLibraryState({ filtered, onImport }: { filtered: boolean; onImport: () => void }) {
  if (filtered) return <div className="library-empty compact"><div className="empty-library-mark" aria-hidden="true">⌕</div><h3>没有匹配的古籍</h3><p>换一个书名或文件名试试。</p></div>;
  return <div className="library-empty"><div className="empty-library-mark" aria-hidden="true">古</div><h3>尚未导入古籍</h3><p>导入 TXT 后，即可进行发音分析、人工校音和语音生成。</p><button className="import-choose-button" type="button" onClick={onImport}>选择 TXT 文件</button></div>;
}

export function LibraryPage({ operationsLocked, onOpenBook }: LibraryPageProps) {
  const [books, setBooks] = useState<BookSummary[]>([]);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [dragActive, setDragActive] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [feedback, setFeedback] = useState<ImportFeedback>(null);
  const [query, setQuery] = useState("");
  const [sort, setSort] = useState<LibrarySort>("recent");
  const [viewMode, setViewMode] = useState<LibraryViewMode>(() => window.localStorage.getItem("ancient-medical-tts.library.view") === "list" ? "list" : "cards");
  const [autoOpenAfterImport, setAutoOpenAfterImport] = useState(shouldAutoOpenImport);
  const [menuBookId, setMenuBookId] = useState<string | null>(null);
  const [annotatedPreflight, setAnnotatedPreflight] = useState<AnnotatedImportPreflight | null>(null);
  const [annotatedPath, setAnnotatedPath] = useState<string | null>(null);
  const [trustAnnotated, setTrustAnnotated] = useState(false);
  const [annotatedBusy, setAnnotatedBusy] = useState(false);

  const refresh = useCallback(async () => {
    setLoading(true);
    if (!("__TAURI_INTERNALS__" in window)) {
      setBooks([]); setError(null); setLoading(false);
      return;
    }
    try { setBooks(await listBooks()); setError(null); }
    catch (reason: unknown) { setError(friendlyErrorMessage(reason)); }
    finally { setLoading(false); }
  }, []);

  useEffect(() => { void refresh(); }, [refresh]);
  useEffect(() => { window.localStorage.setItem("ancient-medical-tts.library.view", viewMode); }, [viewMode]);
  useEffect(() => { window.localStorage.setItem(AUTO_OPEN_PREFERENCE, String(autoOpenAfterImport)); }, [autoOpenAfterImport]);
  useEffect(() => {
    if (!menuBookId) return;
    const close = () => setMenuBookId(null);
    window.addEventListener("pointerdown", close);
    return () => window.removeEventListener("pointerdown", close);
  }, [menuBookId]);
  useEffect(() => {
    if (feedback?.kind !== "success" || !autoOpenAfterImport) return;
    const timer = window.setTimeout(() => onOpenBook(feedback.bookId), 700);
    return () => window.clearTimeout(timer);
  }, [autoOpenAfterImport, feedback, onOpenBook]);

  const importPath = useCallback(async (path: string) => {
    if (operationsLocked || busy) return;
    if (!isSupportedTxtPath(path)) {
      setError("当前仅支持 UTF-8 TXT 文件。");
      setFeedback(null);
      return;
    }
    const pendingTitle = fileStem(path) || "古籍";
    setBusy(true); setError(null); setFeedback({ kind: "importing", title: pendingTitle });
    try {
      const result = await importTxtBook(path);
      const preflight = await getBookGenerationPreflight(result.book.book.id).catch(() => null);
      await refresh();
      setFeedback({ kind: "success", title: result.book.book.title, bookId: result.book.book.id, segmentCount: result.segment_count, chapterCount: result.chapters.length, hanCharacterCount: preflight?.han_character_count ?? null });
    } catch (reason: unknown) {
      setFeedback(null);
      setError(friendlyErrorMessage(reason));
    } finally { setBusy(false); }
  }, [busy, operationsLocked, refresh]);

  useEffect(() => {
    let active = true;
    let unlisten: (() => void) | undefined;
    try {
      void getCurrentWebview().onDragDropEvent((event) => {
        if (!active) return;
        const payload = event.payload;
        if (payload.type === "enter" || payload.type === "over") setDragActive(true);
        if (payload.type === "leave") setDragActive(false);
        if (payload.type === "drop") {
          setDragActive(false);
          if (payload.paths.length !== 1) {
            setError(payload.paths.length === 0 ? "当前仅支持 UTF-8 TXT 文件。" : "每次只能导入一个 TXT 文件。");
            return;
          }
          void importPath(payload.paths[0]);
        }
      }).then((cleanup) => {
        if (active) unlisten = cleanup;
        else cleanup();
      }).catch(() => undefined);
    } catch {
      // Browser preview has no Tauri webview; the file picker remains available.
    }
    return () => { active = false; unlisten?.(); };
  }, [importPath]);

  const chooseFile = useCallback(async () => {
    if (operationsLocked || busy) return;
    const selected = await open({ multiple: false, directory: false, filters: [{ name: "TXT 文档", extensions: ["txt"] }] });
    if (typeof selected === "string") await importPath(selected);
  }, [busy, importPath, operationsLocked]);

  const chooseAnnotatedFile = useCallback(async () => {
    if (operationsLocked || busy || annotatedBusy) return;
    const selected = await open({ multiple: false, directory: false, filters: [{ name: "已注音古籍", extensions: ["zip", "json"] }] });
    if (typeof selected !== "string") return;
    await restoreLibraryWindowFocus();
    setAnnotatedPath(selected);
    setTrustAnnotated(false);
    setError(null);
    try {
      const result = await getAnnotatedBookImportPreflight(selected, false);
      await restoreLibraryWindowFocus();
      setAnnotatedPreflight(result);
    } catch (reason: unknown) {
      setAnnotatedPath(null);
      setError(friendlyErrorMessage(reason));
    }
  }, [annotatedBusy, busy, operationsLocked]);

  const confirmAnnotatedImport = useCallback(async () => {
    if (!annotatedPath || !annotatedPreflight || annotatedBusy || !annotatedPreflight.can_import) return;
    setAnnotatedBusy(true); setError(null);
    try {
      const result = await importAnnotatedBook(annotatedPath, trustAnnotated);
      await refresh();
      setAnnotatedPreflight(null); setAnnotatedPath(null);
      setFeedback({ kind: "success", title: result.book.book.title, bookId: result.book.book.id, segmentCount: result.segment_count, chapterCount: result.chapters.length, hanCharacterCount: annotatedPreflight.han_character_count });
    } catch (reason: unknown) {
      setError(friendlyErrorMessage(reason));
    } finally { setAnnotatedBusy(false); }
  }, [annotatedBusy, annotatedPath, annotatedPreflight, refresh, trustAnnotated]);

  const removeBook = async (book: BookSummary) => {
    setMenuBookId(null);
    if (!window.confirm(`确认删除《${book.title}》？\n\n该操作会删除当前项目中的文本数据。`)) return;
    setBusy(true); setError(null);
    try { await deleteBook(book.id); await refresh(); }
    catch (reason: unknown) { setError(friendlyErrorMessage(reason)); }
    finally { setBusy(false); }
  };

  const visibleBooks = useMemo(
    () => sortLibraryBooks(books.filter((book) => bookMatchesQuery(book, query)), sort),
    [books, query, sort],
  );
  const locked = operationsLocked || busy;

  return (
    <main className="content library-content">
      <section className="library-heading"><div><h2>我的古籍</h2><p>管理本地古籍、校对读音，并生成朗读音频。</p></div><strong>{books.length} 本古籍</strong></section>
      <ImportDropzone active={dragActive} disabled={operationsLocked} importing={busy && feedback?.kind === "importing"} autoOpen={autoOpenAfterImport} onAutoOpenChange={setAutoOpenAfterImport} onChoose={() => void chooseFile()} onChooseAnnotated={() => void chooseAnnotatedFile()} />
      {annotatedPreflight && <AnnotatedImportDialog preflight={annotatedPreflight} trust={trustAnnotated} busy={annotatedBusy} onTrustChange={setTrustAnnotated} onCancel={() => { setAnnotatedPreflight(null); setAnnotatedPath(null); }} onImport={() => void confirmAnnotatedImport()} />}
      <ImportStatusToast feedback={feedback} onOpen={onOpenBook} />
      {operationsLocked && <p className="library-lock-note">全文生成或导出进行中，暂时不能导入或删除古籍。</p>}
      {error && <div className="error-banner library-error" role="alert">{error}</div>}
      <LibraryToolbar query={query} onQueryChange={setQuery} sort={sort} onSortChange={setSort} viewMode={viewMode} onViewModeChange={setViewMode} />
      <div className="library-section-title"><h3>本地书库</h3><span>{visibleBooks.length === books.length ? `${books.length} 本` : `${visibleBooks.length} / ${books.length} 本`}</span></div>
      {loading ? <div className="library-loading">正在读取书库…</div> : visibleBooks.length === 0 ? <EmptyLibraryState filtered={books.length > 0} onImport={() => void chooseFile()} /> : (
        <div className={`book-grid ${viewMode}`}>
          {visibleBooks.map((book) => <BookCard key={book.id} book={book} viewMode={viewMode} menuOpen={menuBookId === book.id} disabled={locked} onToggleMenu={(event) => { event.stopPropagation(); setMenuBookId((current) => current === book.id ? null : book.id); }} onOpen={() => onOpenBook(book.id)} onDelete={() => void removeBook(book)} />)}
          <button className={`library-import-card ${viewMode}`} type="button" onClick={() => void chooseFile()} disabled={locked}><span aria-hidden="true">＋</span><strong>导入古籍</strong><small>支持 TXT 格式</small></button>
        </div>
      )}
    </main>
  );
}
