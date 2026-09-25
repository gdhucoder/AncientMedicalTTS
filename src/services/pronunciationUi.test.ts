import { describe, expect, it } from "vitest";
import { annotationForToken, annotationSourceLabel, confidenceLabel, isHanToken, isSingleHanGlobalRule, isTtsLockedAnnotation, normalizePinyinInput, realizedPronunciationForText, reviewStatusLabel, riskTypeLabel, ruleTypeLabel, ttsPronunciationMismatches } from "./pronunciationUi";
import type { Annotation, AudioVersion, GraphemeToken } from "../types/library";

describe("normalizePinyinInput", () => {
  it("normalizes input-method capitalization without changing tone numbers", () => {
    expect(normalizePinyinInput("Gong1 Xue2")).toBe("gong1 xue2");
    expect(normalizePinyinInput("lü4")).toBe("lü4");
  });

  it("only treats confirmed target pinyin as an input to TTS", () => {
    expect(isTtsLockedAnnotation({ ...annotation, review_status: "confirmed", target_pinyin: "shu4 xue2", source: "manual" })).toBe(true);
    expect(isTtsLockedAnnotation(annotation)).toBe(false);
    expect(isTtsLockedAnnotation({ ...annotation, review_status: "confirmed", target_pinyin: "shu4 xue2", source: "pypinyin" })).toBe(false);
  });

  it("reads realized pronunciation from the selected audio version", () => {
    const audio = { provider_metadata: { realized_pronunciation: [{ text: "恶", phoneme: "wu4", begin_ms: 1, end_ms: 2 }] } } as AudioVersion;
    expect(realizedPronunciationForText(audio, "恶")[0]?.phoneme).toBe("wu4");
    expect(realizedPronunciationForText(null, "恶")).toEqual([]);
  });

  it("marks provider pronunciation mismatches by grapheme token", () => {
    const tokens: GraphemeToken[] = [
      { index: 0, text: "恶" },
      { index: 1, text: "寒" },
    ];
    const audio = { provider_metadata: { realized_pronunciation: [
      { text: "恶", phoneme: "wu4", begin_ms: 1, end_ms: 2 },
      { text: "寒", phoneme: "han2", begin_ms: 2, end_ms: 3 },
    ] } } as AudioVersion;
    expect([...ttsPronunciationMismatches(tokens, ["e4", "han2"], audio)]).toEqual([0]);
  });

  it("accepts Tencent v spelling for standard ü pinyin", () => {
    const tokens: GraphemeToken[] = [{ index: 0, text: "穴" }];
    const audio = { provider_metadata: { realized_pronunciation: [{ text: "穴", phoneme: "xve2", begin_ms: 1, end_ms: 2 }] } } as AudioVersion;
    expect(ttsPronunciationMismatches(tokens, ["xue2"], audio)).toEqual(new Set());
  });

  it("does not compare punctuation or pause subtitles", () => {
    const tokens: GraphemeToken[] = [
      { index: 0, text: "恶" },
      { index: 1, text: "，" },
      { index: 2, text: "寒" },
    ];
    const audio = { provider_metadata: { realized_pronunciation: [
      { text: "恶", phoneme: "wu4", begin_ms: 1, end_ms: 2 },
      { text: "，", phoneme: "sil", begin_ms: 2, end_ms: 3 },
      { text: "寒", phoneme: "han2", begin_ms: 3, end_ms: 4 },
    ] } } as AudioVersion;
    expect([...ttsPronunciationMismatches(tokens, ["e4", null, "han2"], audio)]).toEqual([0]);
  });
});
const annotation: Annotation = {
  id: "a1", segment_id: "s1", start_token: 1, end_token: 3, surface_text: "腧穴",
  default_pinyin: "shu4 xue2", target_pinyin: null, candidate_pinyin: [], risk_type: "medical_term",
  reason: "medical term", review_status: "needs_review", analyzer_version: "0.1.0",
  source: "medical_lexicon",
  rule_type: "medical_term",
  confidence: "verified",
  source_rule_id: null,
  created_at: "now", updated_at: "now",
};

describe("pronunciation UI helpers", () => {
  it("finds a multi-token marker at its start token", () => {
    expect(annotationForToken([annotation], 1)).toEqual(annotation);
    expect(annotationForToken([annotation], 2)).toBeUndefined();
  });

  it("allows Han characters including supplementary-plane Han, but not punctuation", () => {
    expect(isHanToken("腧")).toBe(true);
    expect(isHanToken("𠮷")).toBe(true);
    expect(isHanToken("。")).toBe(false);
    expect(isHanToken("ab")).toBe(false);
  });

  it("warns only for a single-Han global rule", () => {
    expect(isSingleHanGlobalRule("说")).toBe(true);
    expect(isSingleHanGlobalRule(" 说 ")).toBe(true);
    expect(isSingleHanGlobalRule("不亦说乎")).toBe(false);
    expect(isSingleHanGlobalRule("。")).toBe(false);
  });

  it("labels review and risk states", () => {
    expect(reviewStatusLabel("needs_review")).toBe("待复核");
    expect(riskTypeLabel("medical_term")).toBe("医学词");
    expect(riskTypeLabel("context_pronunciation")).toBe("上下文读音");
    expect(annotationSourceLabel("variant_mapping")).toBe("异体映射");
    expect(riskTypeLabel("manual")).toBe("手工标注");
    expect(riskTypeLabel("knowledge_conflict")).toBe("知识冲突");
    expect(annotationSourceLabel("context_semantic")).toBe("上下文语义判断");
    expect(annotationSourceLabel("pypinyin")).toBe("拼音分析器");
    expect(ruleTypeLabel("manual")).toBe("手工标注");
    expect(ruleTypeLabel("context_semantic")).toBe("上下文语义判断");
    expect(confidenceLabel("high")).toBe("高");
  });
});
