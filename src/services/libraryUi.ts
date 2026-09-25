import type { BookSummary } from "../types/library";

export type LibrarySort = "recent" | "title";

export function isSupportedTxtPath(path: string): boolean {
  return path.trim().toLocaleLowerCase().endsWith(".txt");
}

export function sourceFileName(path: string | null): string {
  if (!path) return "本地 TXT";
  const parts = path.split(/[\\/]/);
  return parts.at(-1) || path;
}

export function fileStem(path: string): string {
  return sourceFileName(path).replace(/\.txt$/i, "");
}

export function bookCoverGlyph(title: string): string {
  for (const character of Array.from(title.trim())) {
    const point = character.codePointAt(0) ?? 0;
    const isHan = (point >= 0x3400 && point <= 0x9fff)
      || (point >= 0x20000 && point <= 0x2fa1f);
    if (isHan || /[A-Za-z0-9]/.test(character)) return character.toLocaleUpperCase();
  }
  return "古";
}

export function bookMatchesQuery(book: BookSummary, query: string): boolean {
  const normalized = query.trim().toLocaleLowerCase();
  if (!normalized) return true;
  return book.title.toLocaleLowerCase().includes(normalized)
    || (book.source_file ?? "").toLocaleLowerCase().includes(normalized);
}

export function sortLibraryBooks(books: BookSummary[], sort: LibrarySort): BookSummary[] {
  return [...books].sort((left, right) => {
    if (sort === "title") return left.title.localeCompare(right.title, "zh-CN");
    return Date.parse(right.created_at) - Date.parse(left.created_at);
  });
}

export function relativeImportLabel(createdAt: string, now = Date.now()): string {
  const created = Date.parse(createdAt);
  if (!Number.isFinite(created)) return "导入时间未知";
  const elapsedMinutes = Math.max(0, Math.floor((now - created) / 60_000));
  if (elapsedMinutes < 1) return "刚刚导入";
  if (elapsedMinutes < 60) return `${elapsedMinutes} 分钟前导入`;
  const elapsedHours = Math.floor(elapsedMinutes / 60);
  if (elapsedHours < 24) return `${elapsedHours} 小时前导入`;
  const elapsedDays = Math.floor(elapsedHours / 24);
  if (elapsedDays < 7) return `${elapsedDays} 天前导入`;
  return `${new Date(created).toLocaleDateString("zh-CN")} 导入`;
}
