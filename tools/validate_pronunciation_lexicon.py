#!/usr/bin/env python3
"""Validate versioned pronunciation knowledge resources without loading the app."""

from __future__ import annotations

import json
import re
import sys
import unicodedata
from dataclasses import dataclass
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
DICTIONARY_DIR = ROOT / "worker" / "dictionaries"
LEXICON_FILES = (
    DICTIONARY_DIR / "medical_lexicon_v3.json",
    DICTIONARY_DIR / "classical_lexicon_v3.json",
)
SEMANTIC_RULE_FILE = DICTIONARY_DIR / "context_rules_v3.json"
VARIANT_FILE = DICTIONARY_DIR / "variant_characters.json"
PINYIN = re.compile(r"^[a-z]+[1-5]$")
LEXICON_CATEGORIES = {
    "disease", "symptom", "pulse", "meridian", "organ", "formula", "herb",
    "acupoint", "treatment", "classical_term", "other",
}
RULE_CATEGORIES = {
    "pulse_context", "organ_context", "meridian_context", "formula_context",
    "disease_context", "acupoint_context", "classical_semantic_context",
}
CONFIDENCE_VALUES = {"verified", "high", "medium", "low"}


@dataclass(frozen=True)
class ValidationResult:
    errors: tuple[str, ...]
    lexicon_entries: int
    semantic_rules: int
    variants: int


def _load(path: Path) -> Any:
    return json.loads(path.read_text(encoding="utf-8"))


def _is_han(character: str) -> bool:
    name = unicodedata.name(character, "")
    return "CJK UNIFIED IDEOGRAPH" in name or "CJK COMPATIBILITY IDEOGRAPH" in name


def validate() -> ValidationResult:
    errors: list[str] = []
    seen_terms: dict[str, tuple[tuple[str, ...], str]] = {}
    lexicon_entries = 0

    for path in LEXICON_FILES:
        document = _load(path)
        if not isinstance(document, dict) or document.get("version") != "0.3.0":
            errors.append(f"{path.name}: version 必须是 0.3.0")
            continue
        entries = document.get("entries")
        if not isinstance(entries, list):
            errors.append(f"{path.name}: entries 必须是数组")
            continue
        for index, entry in enumerate(entries):
            lexicon_entries += 1
            prefix = f"{path.name}[{index}]"
            if not isinstance(entry, dict):
                errors.append(f"{prefix}: entry 必须是对象")
                continue
            text = entry.get("text")
            raw_pinyin = entry.get("pinyin")
            category = entry.get("category")
            source = entry.get("source")
            confidence = entry.get("confidence")
            if not isinstance(text, str) or not text.strip():
                errors.append(f"{prefix}: text 不能为空")
                continue
            if not isinstance(raw_pinyin, list) or not raw_pinyin:
                errors.append(f"{prefix}: pinyin 必须是非空数组")
                continue
            values = tuple(value for value in raw_pinyin if isinstance(value, str))
            if len(values) != len(raw_pinyin) or any(not PINYIN.fullmatch(value) for value in values):
                errors.append(f"{prefix}: pinyin 必须使用小写 ASCII tone-number")
            han_count = sum(1 for character in text if _is_han(character))
            if han_count != len(values):
                errors.append(f"{prefix}: 汉字数 {han_count} 与拼音音节数 {len(values)} 不一致")
            if category not in LEXICON_CATEGORIES:
                errors.append(f"{prefix}: unknown category {category!r}")
            if not isinstance(source, str) or not source.strip():
                errors.append(f"{prefix}: source 缺失")
            if confidence not in CONFIDENCE_VALUES:
                errors.append(f"{prefix}: confidence 无效")
            previous = seen_terms.get(text)
            if previous is not None:
                previous_pinyin, previous_file = previous
                if previous_pinyin != values:
                    errors.append(f"{prefix}: {text} 与 {previous_file} 存在冲突读音")
                else:
                    errors.append(f"{prefix}: duplicate term {text}")
            else:
                seen_terms[text] = (values, path.name)

    rule_document = _load(SEMANTIC_RULE_FILE)
    raw_rules = rule_document.get("rules", []) if isinstance(rule_document, dict) else []
    seen_rule_ids: set[str] = set()
    for index, rule in enumerate(raw_rules):
        prefix = f"{SEMANTIC_RULE_FILE.name}[{index}]"
        if not isinstance(rule, dict):
            errors.append(f"{prefix}: rule 必须是对象")
            continue
        rule_id = rule.get("rule_id")
        focus = rule.get("focus")
        target = rule.get("target")
        if not isinstance(rule_id, str) or not rule_id:
            errors.append(f"{prefix}: rule_id 缺失")
        elif rule_id in seen_rule_ids:
            errors.append(f"{prefix}: duplicate rule_id {rule_id}")
        else:
            seen_rule_ids.add(rule_id)
        if not isinstance(focus, str) or not focus or not any(_is_han(character) for character in focus):
            errors.append(f"{prefix}: focus 必须包含汉字")
        if not isinstance(target, str) or not PINYIN.fullmatch(target):
            errors.append(f"{prefix}: target 必须是一个 tone-number 音节")
        if rule.get("category") not in RULE_CATEGORIES:
            errors.append(f"{prefix}: unknown rule category {rule.get('category')!r}")
        if rule.get("confidence") not in CONFIDENCE_VALUES:
            errors.append(f"{prefix}: confidence 无效")
        context_fields = ("left_terms", "right_terms", "nearby_terms", "required_nearby_terms")
        if not any(isinstance(rule.get(field), list) and rule[field] for field in context_fields):
            errors.append(f"{prefix}: semantic rule 至少需要一个上下文条件")

    raw_variants = _load(VARIANT_FILE)
    seen_surfaces: dict[str, str] = {}
    for index, variant in enumerate(raw_variants if isinstance(raw_variants, list) else []):
        prefix = f"{VARIANT_FILE.name}[{index}]"
        if not isinstance(variant, dict):
            errors.append(f"{prefix}: variant 必须是对象")
            continue
        surface = variant.get("surface")
        canonical = variant.get("canonical")
        if not isinstance(surface, str) or not surface or not isinstance(canonical, str) or not canonical:
            errors.append(f"{prefix}: surface/canonical 缺失")
            continue
        if surface in seen_surfaces:
            kind = "conflicting" if seen_surfaces[surface] != canonical else "duplicate"
            errors.append(f"{prefix}: {kind} variant {surface}")
        else:
            seen_surfaces[surface] = canonical

    return ValidationResult(
        errors=tuple(errors),
        lexicon_entries=lexicon_entries,
        semantic_rules=len(raw_rules),
        variants=len(raw_variants) if isinstance(raw_variants, list) else 0,
    )


def main() -> int:
    result = validate()
    if result.errors:
        for error in result.errors:
            print(error, file=sys.stderr)
        return 1
    print(
        f"Pronunciation resources valid: lexicon_entries={result.lexicon_entries}, "
        f"semantic_rules={result.semantic_rules}, variants={result.variants}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
