import { beforeEach, describe, expect, it } from "vitest";
import { useLibraryStore } from "./libraryStore";

describe("library navigation state", () => {
  beforeEach(() => {
    useLibraryStore.setState({ view: "books", bookId: null, chapterId: null, segmentId: null });
  });

  it("starts with an empty book selection", () => {
    expect(useLibraryStore.getState().view).toBe("books");
    expect(useLibraryStore.getState().bookId).toBeNull();
  });

  it("opens an imported book and drills into chapter and segment", () => {
    useLibraryStore.getState().openBook("book-1");
    useLibraryStore.getState().selectChapter("chapter-1");
    useLibraryStore.getState().selectSegment("segment-1");

    expect(useLibraryStore.getState()).toMatchObject({ view: "reader", bookId: "book-1", chapterId: "chapter-1", segmentId: "segment-1" });
  });

  it("closes the reader and clears the current selection", () => {
    useLibraryStore.getState().openBook("book-1");
    useLibraryStore.getState().closeReader();

    expect(useLibraryStore.getState()).toMatchObject({ view: "books", bookId: null, chapterId: null, segmentId: null });
  });
});
