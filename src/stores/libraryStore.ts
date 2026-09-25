import { create } from "zustand";

export type LibraryView = "books" | "reader" | "status" | "settings" | "rules";

type LibraryState = {
  view: LibraryView;
  bookId: string | null;
  chapterId: string | null;
  segmentId: string | null;
  setView: (view: LibraryView) => void;
  openBook: (bookId: string) => void;
  selectChapter: (chapterId: string) => void;
  selectSegment: (segmentId: string) => void;
  closeReader: () => void;
};

export const useLibraryStore = create<LibraryState>((set) => ({
  view: "books",
  bookId: null,
  chapterId: null,
  segmentId: null,
  setView: (view) => set({ view }),
  openBook: (bookId) => set({ view: "reader", bookId, chapterId: null, segmentId: null }),
  selectChapter: (chapterId) => set({ chapterId, segmentId: null }),
  selectSegment: (segmentId) => set({ segmentId }),
  closeReader: () => set({ view: "books", bookId: null, chapterId: null, segmentId: null }),
}));
