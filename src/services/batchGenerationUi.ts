import type { BatchGenerationState } from "../types/library";

export function batchProgressPercent(state: BatchGenerationState): number {
  if (state.segments_requiring_generation <= 0) return state.status === "completed" ? 100 : 0;
  return Math.min(100, Math.round((state.processed / state.segments_requiring_generation) * 100));
}

export function batchIsActive(state: BatchGenerationState): boolean {
  return state.status === "running" || state.status === "cancelling";
}

