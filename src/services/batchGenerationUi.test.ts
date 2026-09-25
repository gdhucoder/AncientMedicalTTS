import { describe, expect, it } from "vitest";
import { batchIsActive, batchProgressPercent } from "./batchGenerationUi";
import type { BatchGenerationState } from "../types/library";

const state = (patch: Partial<BatchGenerationState> = {}): BatchGenerationState => ({
  book_id: "book-1",
  status: "running",
  total_segments: 100,
  segments_requiring_generation: 80,
  processed: 20,
  generated: 20,
  skipped: 20,
  failed: 0,
  current_segment_id: "segment-21",
  current_segment_order: 21,
  current_preview: "腧穴",
  has_failures: false,
  failed_segments: [],
  fatal_error: null,
  ...patch,
});

describe("batch generation UI state", () => {
  it("uses generation-required count rather than total count for progress", () => {
    expect(batchProgressPercent(state())).toBe(25);
  });

  it("recognizes cooperative cancellation as active", () => {
    expect(batchIsActive(state())).toBe(true);
    expect(batchIsActive(state({ status: "cancelled" }))).toBe(false);
  });

  it("marks an empty completed batch as complete", () => {
    expect(batchProgressPercent(state({ status: "completed", segments_requiring_generation: 0, processed: 0 }))).toBe(100);
  });
});
