import { describe, expect, it } from "vitest";
import type { BookSummary } from "../types/library";
import { bookCoverGlyph, bookMatchesQuery, fileStem, isSupportedTxtPath, relativeImportLabel, sortLibraryBooks, sourceFileName } from "./libraryUi";

const books: BookSummary[] = [
  { id: "old", title: "伤寒论", source_file: "C:\\Books\\shanghan.txt", chapter_count: 2, segment_count: 12, created_at: "2025-01-01T00:00:00Z" },
  { id: "new", title: "黄帝内经", source_file: "/Users/test/huangdi.txt", chapter_count: 4, segment_count: 84, created_at: "2025-02-01T00:00:00Z" },
];

describe("library UI helpers", () => {
  it("accepts only txt paths and handles Windows/macOS filenames", () => {
    expect(isSupportedTxtPath("/Users/test/古籍.TXT")).toBe(true);
    expect(isSupportedTxtPath("C:\\Books\\古籍.pdf")).toBe(false);
    expect(sourceFileName("C:\\Books\\伤寒论.txt")).toBe("伤寒论.txt");
    expect(fileStem("/Users/test/黄帝内经.txt")).toBe("黄帝内经");
  });

  it("searches title and source file case-insensitively", () => {
    expect(bookMatchesQuery(books[0], "伤寒")).toBe(true);
    expect(bookMatchesQuery(books[0], "SHANGHAN.TXT")).toBe(true);
    expect(bookMatchesQuery(books[0], "金匮")).toBe(false);
  });

  it("sorts recent imports without mutating the source list", () => {
    expect(sortLibraryBooks(books, "recent").map((book) => book.id)).toEqual(["new", "old"]);
    expect(sortLibraryBooks(books, "title").map((book) => book.title)).toEqual(["黄帝内经", "伤寒论"]);
    expect(books.map((book) => book.id)).toEqual(["old", "new"]);
  });

  it("derives stable cover glyphs and relative import labels", () => {
    expect(bookCoverGlyph("《黄帝内经》")).toBe("黄");
    expect(bookCoverGlyph("  classic text")).toBe("C");
    expect(bookCoverGlyph("《》")).toBe("古");
    expect(relativeImportLabel("2025-01-01T00:00:00Z", Date.parse("2025-01-01T00:10:00Z"))).toBe("10 分钟前导入");
  });

  it("filters a 20-book library and preserves long titles", () => {
    const manyBooks = Array.from({ length: 20 }, (_, index): BookSummary => ({
      id: `book-${index}`,
      title: index === 12 ? "一部用于验证书库长标题截断与搜索行为的中医古籍" : `古籍 ${index + 1}`,
      source_file: index === 12 ? "very_long_english_source_filename_for_classical_medical_text.txt" : `book_${index + 1}.txt`,
      chapter_count: 1,
      segment_count: index + 1,
      created_at: new Date(Date.UTC(2025, 0, index + 1)).toISOString(),
    }));
    expect(sortLibraryBooks(manyBooks, "recent")).toHaveLength(20);
    expect(manyBooks.filter((book) => bookMatchesQuery(book, "long_english"))).toHaveLength(1);
    expect(manyBooks.filter((book) => bookMatchesQuery(book, "长标题"))[0].id).toBe("book-12");
  });
});
