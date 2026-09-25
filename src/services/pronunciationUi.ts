import type { Annotation, AudioVersion, GraphemeToken, TtsRealizedPronunciationItem } from "../types/library";

export function annotationForToken(annotations: Annotation[], tokenIndex: number): Annotation | undefined {
  return annotations.find((annotation) => annotation.start_token === tokenIndex);
}

export function isHanToken(text: string): boolean {
  const codePoints = Array.from(text);
  if (codePoints.length !== 1) return false;
  const codePoint = codePoints[0].codePointAt(0) ?? 0;
  return (codePoint >= 0x3400 && codePoint <= 0x4dbf)
    || (codePoint >= 0x4e00 && codePoint <= 0x9fff)
    || (codePoint >= 0x20000 && codePoint <= 0x2fa1f);
}

export function isSingleHanGlobalRule(patternText: string): boolean {
  return isHanToken(patternText.trim());
}

/** ASCII tone-number pinyin is stored and validated in lowercase form. */
export function normalizePinyinInput(value: string): string {
  return value.toLowerCase();
}

export function isTtsLockedAnnotation(annotation: Annotation): boolean {
  return annotation.review_status === "confirmed"
    && annotation.target_pinyin !== null
    && (annotation.source_rule_id !== null || annotation.source === "manual");
}

export function realizedPronunciationForText(audio: AudioVersion | null, surfaceText: string): TtsRealizedPronunciationItem[] {
  const items = audio?.provider_metadata?.realized_pronunciation ?? [];
  const exact = items.filter((item) => item.text === surfaceText && item.phoneme);
  if (exact.length > 0) return exact;
  return items.filter((item) => item.text.includes(surfaceText) && item.phoneme);
}

function canonicalPinyin(value: string): string {
  const normalized = value.trim().toLowerCase().replace(/u:/g, "ü").replace(/v/g, "ü");
  const match = /^([a-zü]+)([1-5])$/.exec(normalized);
  if (!match) return normalized;
  let body = match[1];
  // Tencent may use v for ü and spell ü after j/q/x/y explicitly. Treat
  // those provider spellings as equivalent to the UI's standard pinyin.
  if (/^[jqxy]/.test(body)) body = body.replace(/ü/g, "u");
  return `${body}${match[2]}`;
}

function findTokenSpan(tokens: GraphemeToken[], text: string, fromIndex: number): [number, number] | null {
  for (let start = fromIndex; start < tokens.length; start += 1) {
    let combined = "";
    for (let end = start; end < tokens.length; end += 1) {
      combined += tokens[end].text;
      if (combined === text) return [start, end + 1];
      if (!text.startsWith(combined)) break;
    }
  }
  return null;
}

/** Returns token indexes where the provider's realized pinyin differs from the UI pinyin. */
export function ttsPronunciationMismatches(
  tokens: GraphemeToken[],
  expectedPinyin: Array<string | null>,
  audio: AudioVersion | null,
): Set<number> {
  const mismatches = new Set<number>();
  const items = audio?.provider_metadata?.realized_pronunciation ?? [];
  let cursor = 0;

  for (const item of items) {
    const text = item.text.trim();
    const phoneme = item.phoneme?.trim();
    if (!text || !phoneme || text.toUpperCase() === "SIL") continue;
    // Tencent subtitles may contain punctuation and pause entries. They do
    // not have a page pinyin value and must never become mismatches.
    if (!Array.from(text).every((character) => isHanToken(character))) continue;

    const span = findTokenSpan(tokens, text, cursor);
    if (!span) continue;
    const [start, end] = span;
    const actual = phoneme.split(/\s+/).filter(Boolean);
    const expected = expectedPinyin.slice(start, end);
    const mismatch = expected.length !== actual.length
      || expected.some((value, index) => value === null || canonicalPinyin(value) !== canonicalPinyin(actual[index] ?? ""));
    if (mismatch) {
      for (let index = start; index < end; index += 1) mismatches.add(tokens[index].index);
    }
    cursor = end;
  }
  return mismatches;
}

export function reviewStatusLabel(status: string): string {
  if (status === "needs_review") return "待复核";
  if (status === "confirmed") return "已确认";
  if (status === "ignored") return "已忽略";
  return status;
}

export function riskTypeLabel(riskType: string): string {
  if (riskType === "polyphone") return "多音字";
  if (riskType === "rare_character") return "生僻字";
  if (riskType === "medical_term") return "医学词";
  if (riskType === "context_pronunciation") return "上下文读音";
  if (riskType === "textual_variant") return "文本异体";
  if (riskType === "classical_term") return "古籍词";
  if (riskType === "knowledge_conflict") return "知识冲突";
  if (riskType === "unknown_character") return "未知字";
  if (riskType === "manual") return "手工标注";
  return riskType;
}

export function annotationSourceLabel(source: string | null): string {
  if (source === "medical_lexicon" || source === "medical_lexicon_v3") return "中医领域词典";
  if (source === "context_rule_v2" || source === "context_exact") return "精确上下文规则";
  if (source === "context_semantic") return "上下文语义判断";
  if (source === "rare_classical_lexicon") return "古籍词典";
  if (source === "knowledge_conflict") return "知识冲突检查";
  if (source === "high_risk_polyphone") return "高风险多音字表";
  if (source === "rare_character") return "生僻字表";
  if (source === "variant_mapping") return "异体映射";
  if (source === "pypinyin") return "拼音分析器";
  if (source === "manual") return "手工标注";
  if (source === "pronunciation_rule") return "发音规则";
  return source ?? "分析器";
}

export function ruleTypeLabel(ruleType: string | null): string {
  if (!ruleType) return "—";
  const labels: Record<string, string> = {
    manual: "手工标注",
    medical_term: "医学术语",
    classical_term: "古籍术语",
    classical_medical_term: "古籍医学术语",
    pronunciation_rule: "发音规则",
    high_risk_polyphone: "高风险多音字",
    context_rule: "上下文规则",
    context_rule_v2: "上下文规则",
    context_exact: "精确上下文规则",
    context_semantic: "上下文语义判断",
    variant_mapping: "异体字映射",
    rare_character: "生僻字",
  };
  return labels[ruleType] ?? ruleType;
}

export function confidenceLabel(confidence: string | null): string {
  if (confidence === "verified") return "已核验";
  if (confidence === "high") return "高";
  if (confidence === "medium") return "中";
  if (confidence === "low") return "低";
  return confidence ?? "未提供";
}
