"""Deterministic, explainable pronunciation analysis for Chinese segments.

The worker deliberately knows nothing about the SQLite database. It receives
Grapheme Tokens from Rust and returns token ranges that Rust can validate and
persist. Dictionary data is versioned separately from this algorithm.
"""

from __future__ import annotations

import json
import unicodedata
from dataclasses import dataclass
from functools import lru_cache
from itertools import product
from pathlib import Path
from typing import Any, Iterable

from pypinyin import Style, pinyin

ANALYZER_VERSION = "0.3.0"
DOMAIN_LEXICON_VERSION = "0.3.0"
_PINYIN_STYLE = Style.TONE3
_DICTIONARY_DIR = Path(__file__).parents[1] / "dictionaries"
_MEDICAL_DICTIONARY_PATH = _DICTIONARY_DIR / "medical_lexicon_v3.json"
_CLASSICAL_DICTIONARY_PATH = _DICTIONARY_DIR / "classical_lexicon_v3.json"
_CONTEXT_RULE_PATH = _DICTIONARY_DIR / "context_pronunciation_rules.json"
_SEMANTIC_RULE_PATH = _DICTIONARY_DIR / "context_rules_v3.json"
_HIGH_RISK_PATH = _DICTIONARY_DIR / "high_risk_polyphones.json"
_VARIANT_PATH = _DICTIONARY_DIR / "variant_characters.json"
_RARE_PATH = _DICTIONARY_DIR / "known_rare_characters.json"


class AnalysisError(Exception):
    def __init__(self, code: str, message: str) -> None:
        self.code = code
        super().__init__(message)


@dataclass(frozen=True)
class Token:
    index: int
    text: str


@dataclass(frozen=True)
class MedicalTerm:
    text: str
    pinyin: tuple[str, ...]
    source: str
    category: str
    reason: str
    confidence: str


@dataclass(frozen=True)
class ClassicalTerm:
    text: str
    pinyin: tuple[str, ...]
    source: str
    category: str
    reason: str
    confidence: str


@dataclass(frozen=True)
class ContextRule:
    pattern: str
    pinyin: tuple[str, ...]
    source: str
    reason: str


@dataclass(frozen=True)
class SemanticContextRule:
    rule_id: str
    focus: str
    target: str
    category: str
    priority: int
    window: int
    left_terms: tuple[str, ...]
    right_terms: tuple[str, ...]
    nearby_terms: tuple[str, ...]
    reason: str
    confidence: str


@dataclass(frozen=True)
class VariantMapping:
    surface: str
    canonical: str
    reason: str
    context: str | None
    review_only: bool


@dataclass(frozen=True)
class RareCharacter:
    character: str
    reason: str


@dataclass(frozen=True)
class KnowledgeMatch:
    start: int
    end: int
    default_pinyin: str
    candidates: tuple[str, ...]
    risk_type: str
    source: str
    reason: str
    priority: int
    confidence: str
    rule_type: str
    conflict: bool = False


def analyze(params: Any) -> dict[str, Any]:
    if not isinstance(params, dict):
        raise AnalysisError("PRONUNCIATION_ANALYSIS_FAILED", "params 必须是对象")
    segment_id = params.get("segment_id")
    text = params.get("text")
    raw_tokens = params.get("tokens")
    if not isinstance(segment_id, str) or not isinstance(text, str) or not isinstance(raw_tokens, list):
        raise AnalysisError("PRONUNCIATION_ANALYSIS_FAILED", "segment_id、text、tokens 参数不完整")
    tokens = _parse_tokens(raw_tokens)
    if "".join(token.text for token in tokens) != text:
        raise AnalysisError("PRONUNCIATION_ANALYSIS_FAILED", "tokens 与 text 不一致")

    defaults = _default_pinyin_by_token(text, tokens)
    variants = _load_variants()
    canonical_tokens = _canonical_tokens(tokens, text, variants)
    exact_context_matches = _context_matches(tokens, canonical_tokens)
    medical_matches = _medical_matches(tokens, canonical_tokens)
    semantic_matches = _semantic_context_matches(tokens, canonical_tokens)
    classical_matches = _classical_matches(tokens, canonical_tokens)
    selected_matches, warnings = _select_knowledge_matches(
        exact_context_matches,
        medical_matches,
        semantic_matches,
        classical_matches,
        tokens,
    )

    items: list[dict[str, Any]] = []
    covered: set[int] = set()
    for match in selected_matches:
        match_tokens = tokens[match.start:match.end]
        items.append(_item(
            match_tokens,
            default_pinyin=None if match.conflict else match.default_pinyin,
            candidates=list(match.candidates),
            risk_type=match.risk_type,
            source=match.source,
            reason=match.reason,
            confidence=match.confidence,
            rule_type=match.rule_type,
        ))
        covered.update(token.index for token in match_tokens)

    # Phrase-level textual variants are review signals. They never rewrite the
    # original surface text or silently replace the user's text.
    for start, end, canonical, reason in _phrase_variant_matches(tokens, text, variants):
        indexes = {tokens[index].index for index in range(start, end)}
        if covered.intersection(indexes):
            continue
        phrase_tokens = tokens[start:end]
        original_default = _phrase_default(defaults, start, end)
        canonical_default = _phrase_pinyin(canonical)
        candidates = _unique([value for value in (original_default, canonical_default) if value])
        items.append(_item(
            phrase_tokens,
            default_pinyin=original_default,
            candidates=candidates,
            risk_type="textual_variant",
            source="variant_mapping",
            reason=reason,
            confidence="medium",
            rule_type="variant_mapping",
        ))
        covered.update(indexes)

    high_risk = _load_high_risk()
    rare_characters = _load_rare_characters()
    for position, token in enumerate(tokens):
        if token.index in covered or not _is_han(token.text):
            continue
        canonical = canonical_tokens[position]
        candidates = _candidates_for_token(token.text)
        if canonical != token.text:
            candidates = _unique([*candidates, *_candidates_for_token(canonical)])
            items.append(_item(
                [token],
                default_pinyin=_first_pinyin(canonical) or defaults[position],
                candidates=candidates,
                risk_type="textual_variant",
                source="variant_mapping",
                reason=f"异体字分析映射：{token.text} → {canonical}；保留原文表面，不自动改写",
                confidence="medium",
                rule_type="variant_mapping",
            ))
        elif canonical in high_risk:
            items.append(_item(
                [token], default_pinyin=defaults[position], candidates=candidates,
                risk_type="polyphone", source="high_risk_polyphone",
                reason=high_risk[canonical],
                confidence="low", rule_type="high_risk_polyphone",
            ))
        elif canonical in rare_characters:
            items.append(_item(
                [token], default_pinyin=defaults[position], candidates=candidates,
                risk_type="rare_character", source="rare_character",
                reason=rare_characters[canonical].reason,
                confidence="high", rule_type="rare_classical_lexicon",
            ))
        elif not candidates or not _is_valid_pinyin(defaults[position]):
            items.append(_item(
                [token], default_pinyin=None, candidates=candidates,
                risk_type="unknown_character", source="pypinyin",
                reason="无法从 pypinyin 得到有效拼音",
                confidence="low", rule_type="unknown_character",
            ))

    items.sort(key=lambda item: (item["start_token"], item["end_token"]))
    result: dict[str, Any] = {
        "analyzer_version": ANALYZER_VERSION,
        "domain_lexicon_version": DOMAIN_LEXICON_VERSION,
        "items": items,
    }
    if warnings:
        result["warnings"] = warnings
    return result


def display_pinyin(params: Any) -> dict[str, Any]:
    """Return best-effort ASCII tone-number pinyin for every Han token.

    This is a display-only projection. It does not apply dictionaries or
    produce annotations, so it cannot alter pronunciation review or TTS data.
    """
    if not isinstance(params, dict):
        raise AnalysisError("PRONUNCIATION_DISPLAY_FAILED", "params 必须是对象")
    text = params.get("text")
    raw_tokens = params.get("tokens")
    if not isinstance(text, str) or not isinstance(raw_tokens, list):
        raise AnalysisError("PRONUNCIATION_DISPLAY_FAILED", "text、tokens 参数不完整")
    tokens = _parse_tokens(raw_tokens)
    if "".join(token.text for token in tokens) != text:
        raise AnalysisError("PRONUNCIATION_DISPLAY_FAILED", "tokens 与 text 不一致")
    defaults = _default_pinyin_by_token(text, tokens)
    return {
        "token_pinyin": [
            value if _is_han(token.text) and _is_valid_pinyin(value) else None
            for token, value in zip(tokens, defaults)
        ]
    }


def _parse_tokens(raw_tokens: list[Any]) -> list[Token]:
    tokens: list[Token] = []
    for expected_index, raw_token in enumerate(raw_tokens):
        if not isinstance(raw_token, dict) or not isinstance(raw_token.get("index"), int) or not isinstance(raw_token.get("text"), str):
            raise AnalysisError("PRONUNCIATION_ANALYSIS_FAILED", "tokens 包含无效 token")
        if raw_token["index"] != expected_index or not raw_token["text"]:
            raise AnalysisError("PRONUNCIATION_ANALYSIS_FAILED", "tokens index 必须连续并从 0 开始")
        tokens.append(Token(index=raw_token["index"], text=raw_token["text"]))
    return tokens


def _default_pinyin_by_token(text: str, tokens: list[Token]) -> list[str | None]:
    try:
        full_result = pinyin(text, style=_PINYIN_STYLE, heteronym=False, errors="default")
        if len(full_result) == len(tokens):
            defaults: list[str | None] = []
            for token, values in zip(tokens, full_result):
                candidate = _canonical(values[0]) if values else None
                # pypinyin may return an untoned value in heteronym=False mode
                # (for example, "子" -> "zi"). Never let that invalid value
                # reach rare-character detection when a valid candidate exists.
                defaults.append(
                    candidate if _is_valid_pinyin(candidate) else _first_pinyin(token.text)
                )
            return defaults
    except Exception:
        pass
    return [_first_pinyin(token.text) for token in tokens]


def _first_pinyin(text: str) -> str | None:
    try:
        result = pinyin(text, style=_PINYIN_STYLE, heteronym=False, errors="default")
        for values in result:
            for value in values:
                candidate = _canonical(value)
                if _is_valid_pinyin(candidate):
                    return candidate

        # Some pypinyin entries only expose a tone-number reading when
        # heteronym=True. This is a generic normalization fallback, not a
        # pronunciation choice: it uses the first valid result in pypinyin's
        # own deterministic order.
        heteronyms = pinyin(text, style=_PINYIN_STYLE, heteronym=True, errors="default")
        for values in heteronyms:
            for value in values:
                candidate = _canonical(value)
                if _is_valid_pinyin(candidate):
                    return candidate
        return None
    except Exception:
        return None


def _candidates_for_token(text: str) -> list[str]:
    try:
        result = pinyin(text, style=_PINYIN_STYLE, heteronym=True, errors="default")
        candidates = [_canonical(value) for value in (result[0] if result else [])]
        return _unique(value for value in candidates if _is_valid_pinyin(value))
    except Exception:
        return []


def _phrase_candidates(tokens: list[Token]) -> list[str]:
    choices = [_candidates_for_token(token.text) for token in tokens if _is_han(token.text)]
    if not choices or any(not values for values in choices):
        return []
    return [" ".join(parts) for parts in list(product(*choices))[:256]]


def _phrase_default(defaults: list[str | None], start: int, end: int) -> str | None:
    values = [value for value in defaults[start:end] if value]
    return " ".join(values) if len(values) == end - start else None


def _phrase_pinyin(text: str) -> str | None:
    han_count = sum(1 for character in text if _is_han(character))
    values = [_first_pinyin(character) for character in text if _is_han(character)]
    return " ".join(value for value in values if value) if len(values) == han_count else None


def _canonical(value: str) -> str:
    return value.strip().lower().replace("ü", "v")


def _is_valid_pinyin(value: str | None) -> bool:
    if not value:
        return False
    for syllable in value.split():
        if not syllable or not syllable[-1].isdigit() or syllable[-1] not in "12345":
            return False
        if not syllable[:-1].isalpha() or not syllable[:-1].isascii():
            return False
    return True


def _unique(values: Iterable[str]) -> list[str]:
    result: list[str] = []
    for value in values:
        if value and value not in result:
            result.append(value)
    return result


def _is_han(text: str) -> bool:
    return any(
        "CJK UNIFIED IDEOGRAPH" in unicodedata.name(character, "")
        or "CJK COMPATIBILITY IDEOGRAPH" in unicodedata.name(character, "")
        for character in text
    )


def _load_json(path: Path) -> Any:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise AnalysisError("PRONUNCIATION_ANALYSIS_FAILED", f"词典读取失败: {path.name}: {error}") from error


@lru_cache(maxsize=1)
def _load_terms() -> tuple[MedicalTerm, ...]:
    terms: list[MedicalTerm] = []
    document = _load_json(_MEDICAL_DICTIONARY_PATH)
    raw_terms = document.get("entries", []) if isinstance(document, dict) else []
    for raw_term in raw_terms:
        if not isinstance(raw_term, dict) or not isinstance(raw_term.get("text"), str) or not isinstance(raw_term.get("pinyin"), list):
            continue
        values = tuple(_canonical(value) for value in raw_term["pinyin"] if isinstance(value, str) and _is_valid_pinyin(value))
        if raw_term["text"] and values and sum(_is_han(character) for character in raw_term["text"]) == len(values):
            source = str(raw_term.get("source", "中医术语资源"))
            category = str(raw_term.get("category", "other"))
            terms.append(MedicalTerm(
                raw_term["text"], values, source, category,
                str(raw_term.get("reason", f"命中 v3 中医领域词典（{category}；来源：{source}）")),
                str(raw_term.get("confidence", "verified")),
            ))
    return tuple(sorted(terms, key=lambda term: (-len(list(term.text)), term.text)))


@lru_cache(maxsize=1)
def _load_classical_terms() -> tuple[ClassicalTerm, ...]:
    terms: list[ClassicalTerm] = []
    document = _load_json(_CLASSICAL_DICTIONARY_PATH)
    raw_terms = document.get("entries", []) if isinstance(document, dict) else []
    for raw_term in raw_terms:
        if not isinstance(raw_term, dict) or not isinstance(raw_term.get("text"), str) or not isinstance(raw_term.get("pinyin"), list):
            continue
        values = tuple(_canonical(value) for value in raw_term["pinyin"] if isinstance(value, str) and _is_valid_pinyin(value))
        if raw_term["text"] and values and sum(_is_han(character) for character in raw_term["text"]) == len(values):
            source = str(raw_term.get("source", "古籍辞书资源"))
            category = str(raw_term.get("category", "classical_term"))
            terms.append(ClassicalTerm(
                raw_term["text"], values, source, category,
                str(raw_term.get("reason", f"命中 v3 古籍词典（来源：{source}）")),
                str(raw_term.get("confidence", "high")),
            ))
    return tuple(sorted(terms, key=lambda term: (-len(list(term.text)), term.text)))


@lru_cache(maxsize=1)
def _load_context_rules() -> tuple[ContextRule, ...]:
    rules: list[ContextRule] = []
    raw_rules = _load_json(_CONTEXT_RULE_PATH)
    for raw_rule in raw_rules if isinstance(raw_rules, list) else []:
        if not isinstance(raw_rule, dict) or not isinstance(raw_rule.get("pattern"), str) or not isinstance(raw_rule.get("pinyin"), list):
            continue
        values = tuple(_canonical(value) for value in raw_rule["pinyin"] if isinstance(value, str) and _is_valid_pinyin(value))
        if raw_rule["pattern"] and values and sum(_is_han(character) for character in raw_rule["pattern"]) == len(values):
            rules.append(ContextRule(
                raw_rule["pattern"], values, str(raw_rule.get("source", "context_rule_v2")),
                str(raw_rule.get("reason", "命中古籍语境读音规则")),
            ))
    return tuple(sorted(rules, key=lambda rule: (-len(list(rule.pattern)), rule.pattern)))


@lru_cache(maxsize=1)
def _load_semantic_rules() -> tuple[SemanticContextRule, ...]:
    document = _load_json(_SEMANTIC_RULE_PATH)
    raw_rules = document.get("rules", []) if isinstance(document, dict) else []
    rules: list[SemanticContextRule] = []
    for raw_rule in raw_rules:
        if not isinstance(raw_rule, dict):
            continue
        focus = raw_rule.get("focus")
        target = raw_rule.get("target")
        rule_id = raw_rule.get("rule_id")
        if not isinstance(focus, str) or not focus or not isinstance(target, str) or not _is_valid_pinyin(target) or not isinstance(rule_id, str):
            continue
        rules.append(SemanticContextRule(
            rule_id=rule_id,
            focus=focus,
            target=_canonical(target),
            category=str(raw_rule.get("category", "classical_semantic_context")),
            priority=int(raw_rule.get("priority", 100)),
            window=max(1, int(raw_rule.get("window", 3))),
            left_terms=tuple(value for value in raw_rule.get("left_terms", []) if isinstance(value, str) and value),
            right_terms=tuple(value for value in raw_rule.get("right_terms", []) if isinstance(value, str) and value),
            nearby_terms=tuple(value for value in raw_rule.get("nearby_terms", []) if isinstance(value, str) and value),
            reason=str(raw_rule.get("reason_template", "命中可解释语义上下文规则")),
            confidence=str(raw_rule.get("confidence", "medium")),
        ))
    return tuple(sorted(rules, key=lambda rule: (-rule.priority, rule.rule_id)))


@lru_cache(maxsize=1)
def _load_high_risk() -> dict[str, str]:
    raw = _load_json(_HIGH_RISK_PATH)
    return {
        item["character"]: item["reason"]
        for item in raw if isinstance(raw, list)
        and isinstance(item, dict)
        and isinstance(item.get("character"), str)
        and isinstance(item.get("reason"), str)
    }


@lru_cache(maxsize=1)
def _load_variants() -> tuple[VariantMapping, ...]:
    raw = _load_json(_VARIANT_PATH)
    result: list[VariantMapping] = []
    for item in raw if isinstance(raw, list) else []:
        if isinstance(item, dict) and isinstance(item.get("surface"), str) and isinstance(item.get("canonical"), str):
            result.append(VariantMapping(
                item["surface"], item["canonical"], str(item.get("reason", "文本异体映射，保留原文")),
                item.get("context") if isinstance(item.get("context"), str) else None,
                bool(item.get("review_only", False)),
            ))
    return tuple(result)


@lru_cache(maxsize=1)
def _load_rare_characters() -> dict[str, RareCharacter]:
    raw = _load_json(_RARE_PATH)
    result: dict[str, RareCharacter] = {}
    for item in raw if isinstance(raw, list) else []:
        if isinstance(item, dict) and isinstance(item.get("character"), str):
            result[item["character"]] = RareCharacter(
                item["character"], str(item.get("reason", "词典标记的生僻/古籍字，建议复核"))
            )
    return result


def _canonical_tokens(tokens: list[Token], text: str, variants: tuple[VariantMapping, ...]) -> list[str]:
    result: list[str] = []
    for token in tokens:
        replacement = token.text
        for mapping in variants:
            if len(mapping.surface) == 1 and mapping.surface == token.text and (mapping.context is None or mapping.context in text):
                replacement = mapping.canonical
                break
        result.append(replacement)
    return result


def _find_pattern(tokens: list[Token], canonical_tokens: list[str], pattern: str) -> list[tuple[int, int]]:
    pattern_tokens = list(pattern)
    matches: list[tuple[int, int]] = []
    for start in range(0, len(tokens) - len(pattern_tokens) + 1):
        surface = [token.text for token in tokens[start:start + len(pattern_tokens)]]
        canonical = canonical_tokens[start:start + len(pattern_tokens)]
        if surface == pattern_tokens or canonical == pattern_tokens:
            matches.append((start, start + len(pattern_tokens)))
    return matches


def _medical_matches(tokens: list[Token], canonical_tokens: list[str]) -> list[KnowledgeMatch]:
    matches: list[KnowledgeMatch] = []
    for term in _load_terms():
        for start, end in _find_pattern(tokens, canonical_tokens, term.text):
            phrase_candidates = _phrase_candidates(tokens[start:end])
            candidates = _unique([" ".join(term.pinyin), *phrase_candidates])
            matches.append(KnowledgeMatch(
                start, end, " ".join(term.pinyin), tuple(candidates), "medical_term", "medical_lexicon_v3",
                term.reason, 300, term.confidence, term.category,
            ))
    return matches


def _classical_matches(tokens: list[Token], canonical_tokens: list[str]) -> list[KnowledgeMatch]:
    matches: list[KnowledgeMatch] = []
    for term in _load_classical_terms():
        for start, end in _find_pattern(tokens, canonical_tokens, term.text):
            phrase_candidates = _phrase_candidates(tokens[start:end])
            candidates = _unique([" ".join(term.pinyin), *phrase_candidates])
            matches.append(KnowledgeMatch(
                start, end, " ".join(term.pinyin), tuple(candidates), "classical_term",
                "rare_classical_lexicon", term.reason, 100, term.confidence, term.category,
            ))
    return matches


def _context_matches(tokens: list[Token], canonical_tokens: list[str]) -> list[KnowledgeMatch]:
    matches: list[KnowledgeMatch] = []
    for rule in _load_context_rules():
        for start, end in _find_pattern(tokens, canonical_tokens, rule.pattern):
            phrase_candidates = _phrase_candidates(tokens[start:end])
            candidates = _unique([" ".join(rule.pinyin), *phrase_candidates])
            matches.append(KnowledgeMatch(
                start, end, " ".join(rule.pinyin), tuple(candidates), "context_pronunciation", "context_exact",
                rule.reason, 400, "high", "context_exact",
            ))
    return matches


def _semantic_context_matches(tokens: list[Token], canonical_tokens: list[str]) -> list[KnowledgeMatch]:
    matches: list[KnowledgeMatch] = []
    for rule in _load_semantic_rules():
        for start, end in _find_pattern(tokens, canonical_tokens, rule.focus):
            left = "".join(canonical_tokens[max(0, start - rule.window):start])
            right = "".join(canonical_tokens[end:min(len(tokens), end + rule.window)])
            nearby = "".join(canonical_tokens[max(0, start - rule.window):min(len(tokens), end + rule.window)])
            if rule.left_terms and not any(term in left for term in rule.left_terms):
                continue
            if rule.right_terms and not any(term in right for term in rule.right_terms):
                continue
            if rule.nearby_terms and not any(term in nearby for term in rule.nearby_terms):
                continue
            focus_tokens = tokens[start:end]
            candidates = _unique([rule.target, *_phrase_candidates(focus_tokens)])
            matches.append(KnowledgeMatch(
                start, end, rule.target, tuple(candidates), "context_pronunciation",
                "context_semantic", rule.reason, 100 + min(rule.priority, 99),
                rule.confidence, rule.category,
            ))
    return matches


def _select_knowledge_matches(
    exact_context: list[KnowledgeMatch],
    medical: list[KnowledgeMatch],
    semantic: list[KnowledgeMatch],
    classical: list[KnowledgeMatch],
    tokens: list[Token],
) -> tuple[list[KnowledgeMatch], list[dict[str, str]]]:
    warnings: list[dict[str, str]] = []
    grouped: dict[tuple[int, int], list[KnowledgeMatch]] = {}
    for match in [*exact_context, *medical, *semantic, *classical]:
        grouped.setdefault((match.start, match.end), []).append(match)

    merged: list[KnowledgeMatch] = []
    for (start, end), matches in grouped.items():
        pinyin_values = {match.default_pinyin for match in matches}
        sources = {match.source for match in matches}
        conflict_required = len(pinyin_values) > 1 and (
            "medical_lexicon_v3" in sources or "context_exact" in sources
        )
        if conflict_required:
            candidates = _unique(value for match in matches for value in match.candidates)
            warnings.append({
                "code": "ANALYZER_KNOWLEDGE_CONFLICT",
                "message": f"知识词典在“{''.join(token.text for token in tokens[start:end])}”的读音不一致",
            })
            merged.append(KnowledgeMatch(
                start, end, "", tuple(candidates), "knowledge_conflict", "knowledge_conflict",
                "领域词典与上下文规则结果不一致，需要人工确认。", 500,
                "low", "knowledge_conflict", True,
            ))
        else:
            merged.append(max(matches, key=lambda match: (match.priority, match.source, match.rule_type)))

    conflict_ranges = {(match.start, match.end) for match in merged if match.conflict}
    for medical_match in medical:
        if (medical_match.start, medical_match.end) in conflict_ranges:
            continue
        medical_parts = medical_match.default_pinyin.split()
        for semantic_match in semantic:
            if semantic_match.start < medical_match.start or semantic_match.end > medical_match.end:
                continue
            left = semantic_match.start - medical_match.start
            right = semantic_match.end - medical_match.start
            if right > len(medical_parts):
                continue
            medical_focus = " ".join(medical_parts[left:right])
            if medical_focus == semantic_match.default_pinyin:
                continue
            replacement = [*medical_parts]
            replacement[left:right] = semantic_match.default_pinyin.split()
            candidates = _unique([
                medical_match.default_pinyin,
                " ".join(replacement),
                *medical_match.candidates,
            ])
            merged.append(KnowledgeMatch(
                medical_match.start, medical_match.end, "", tuple(candidates),
                "knowledge_conflict", "knowledge_conflict",
                "领域词典与上下文规则结果不一致，需要人工确认。", 500,
                "low", "knowledge_conflict", True,
            ))
            warnings.append({
                "code": "ANALYZER_KNOWLEDGE_CONFLICT",
                "message": f"领域词典与语义上下文在“{''.join(token.text for token in tokens[medical_match.start:medical_match.end])}”的读音不一致",
            })
            conflict_ranges.add((medical_match.start, medical_match.end))
            break

    selected: list[KnowledgeMatch] = []
    deduplicated = {
        (match.start, match.end, match.source, match.default_pinyin, match.rule_type): match
        for match in merged
        if not (
            (match.start, match.end) in conflict_ranges
            and not match.conflict
        )
    }
    for match in sorted(deduplicated.values(), key=lambda item: (item.start, -(item.end - item.start), -item.priority, item.source, item.rule_type)):
        if any(_ranges_overlap(match.start, match.end, selected_match.start, selected_match.end) for selected_match in selected):
            continue
        selected.append(match)
    return selected, warnings


def _phrase_variant_matches(
    tokens: list[Token], text: str, variants: tuple[VariantMapping, ...]
) -> list[tuple[int, int, str, str]]:
    matches: list[tuple[int, int, str, str]] = []
    for mapping in variants:
        if len(mapping.surface) <= 1 or (mapping.context is not None and mapping.context not in text):
            continue
        for start, end in _find_pattern(tokens, [token.text for token in tokens], mapping.surface):
            matches.append((start, end, mapping.canonical, mapping.reason))
    return matches


def _ranges_overlap(a_start: int, a_end: int, b_start: int, b_end: int) -> bool:
    return a_start < b_end and b_start < a_end


def _item(
    tokens: list[Token], *, default_pinyin: str | None, candidates: list[str],
    risk_type: str, source: str, reason: str, confidence: str, rule_type: str,
) -> dict[str, Any]:
    return {
        "start_token": tokens[0].index,
        "end_token": tokens[-1].index + 1,
        "surface_text": "".join(token.text for token in tokens),
        "default_pinyin": default_pinyin,
        "candidate_pinyin": _unique(candidates),
        "risk_type": risk_type,
        "source": source,
        "reason": reason,
        "confidence": confidence,
        "rule_type": rule_type,
    }
