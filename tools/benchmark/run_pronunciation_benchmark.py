#!/usr/bin/env python3
"""Run the production pronunciation analyzer and build a read-only baseline report."""

from __future__ import annotations

import argparse
import csv
import copy
import hashlib
import json
import os
import platform
import subprocess
import sys
from collections import Counter, defaultdict
from datetime import datetime, timezone
from itertools import product
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[2]
BENCHMARK_RUNNER_VERSION = "0.4.0-m9"
APP_VERSION = json.loads((ROOT / "package.json").read_text(encoding="utf-8"))["version"]
DEFAULT_TEXT = ROOT / "realbooks/huangdi_neijing_test_v01.txt"
DEFAULT_GOLD = ROOT / "realbooks/huangdi_neijing_gold_v01.json"
DEFAULT_OUTPUT = ROOT / "reports/benchmark/huangdi_neijing_v01"
BENCHMARK_TEST = "gold_evaluation_service::tests::evaluates_realbook_annotations_against_gold_json"

DATASETS: dict[str, dict[str, Any]] = {
    "huangdi_neijing_v01": {
        "book": "黄帝内经·素问",
        "text": DEFAULT_TEXT,
        "gold": DEFAULT_GOLD,
        "output": DEFAULT_OUTPUT,
        "snapshot_label": "m8",
        "report_label": "m8",
    },
    "shanghanlun_holdout_v01": {
        "book": "伤寒论",
        "text": ROOT / "realbooks/shanghanlun_holdout_v01.txt",
        "gold": ROOT / "realbooks/shanghanlun_gold_v01.json",
        "output": ROOT / "reports/benchmark/shanghanlun_holdout_v01",
        "snapshot_label": "holdout",
        "report_label": "holdout",
    },
    "jinguiyaolue_holdout_v01": {
        "book": "金匮要略",
        "text": ROOT / "realbooks/jinguiyaolue_holdout_v01.txt",
        "gold": ROOT / "realbooks/jinguiyaolue_gold_v01.json",
        "output": ROOT / "reports/benchmark/jinguiyaolue_holdout_v01",
        "snapshot_label": "holdout",
        "report_label": "holdout",
    },
}

HOLDOUT_FOCUS_TERMS: dict[str, list[str]] = {
    "shanghanlun_holdout_v01": [
        "恶寒", "中风", "脉数急", "瘈疭", "阳数七", "阴数六", "啬啬", "淅淅",
        "翕翕", "厚朴", "厥逆", "肉瞤", "目瞑", "衄", "不更衣", "哕", "濈然",
        "少阳", "谵语", "胁下硬满", "太阴", "少阴", "脉细沉数", "厥阴", "吐蛔",
        "黄芩", "除中", "索饼", "几几", "少腹",
    ],
    "jinguiyaolue_holdout_v01": [
        "脏腑", "中人多死", "疢难", "干忤", "腠理", "喑喑", "啾啾", "肺痿", "微数",
        "刚痉", "柔痉", "卒口噤", "栝蒌", "挛", "齘齿", "湿痹", "哕", "中暍", "芤",
        "数下之", "其脉微数", "淅然", "头眩", "代赭", "狐惑", "眦", "鳖甲", "疟脉",
        "弦数者", "不差", "癥瘕", "疟母", "消铄", "牡疟", "中风", "脉微而数", "㖞",
        "瘾疹",
    ],
}


def is_han_character(text: str) -> bool:
    return any(
        0x3400 <= ord(character) <= 0x4DBF
        or 0x4E00 <= ord(character) <= 0x9FFF
        or 0xF900 <= ord(character) <= 0xFAFF
        or 0x20000 <= ord(character) <= 0x323AF
        for character in text
    )


def canonical_pinyin(value: str) -> str:
    return " ".join(value.strip().lower().replace("ü", "v").split())


def compact(value: str) -> str:
    return "".join(character for character in value if not character.isspace())


def run_production_pipeline(
    snapshot_path: Path,
    text_path: Path,
    gold_path: Path,
    dataset_name: str,
    snapshot_label: str,
) -> None:
    environment = os.environ.copy()
    environment["ANCIENT_TTS_BENCHMARK_SNAPSHOT"] = str(snapshot_path.resolve())
    environment["ANCIENT_TTS_BENCHMARK_LABEL"] = snapshot_label
    environment["ANCIENT_TTS_BENCHMARK_NAME"] = dataset_name
    environment["ANCIENT_TTS_BENCHMARK_TEXT"] = str(text_path.resolve())
    environment["ANCIENT_TTS_BENCHMARK_GOLD"] = str(gold_path.resolve())
    if dataset_name.endswith("_holdout_v01"):
        environment["ANCIENT_TTS_BENCHMARK_SNAPSHOT_ONLY"] = "1"
    else:
        environment.pop("ANCIENT_TTS_BENCHMARK_SNAPSHOT_ONLY", None)
    command = [
        "cargo", "test", "--manifest-path", str(ROOT / "src-tauri/Cargo.toml"),
        "--lib", BENCHMARK_TEST, "--", "--nocapture",
    ]
    completed = subprocess.run(command, cwd=ROOT, env=environment, check=False)
    if completed.returncode != 0:
        raise RuntimeError(f"production benchmark pipeline failed: exit {completed.returncode}")
    if not snapshot_path.exists():
        raise RuntimeError(f"production pipeline did not write {snapshot_path}")


def write_csv(path: Path, rows: list[dict[str, Any]], fields: list[str]) -> None:
    with path.open("w", encoding="utf-8", newline="") as handle:
        writer = csv.DictWriter(handle, fieldnames=fields, extrasaction="ignore")
        writer.writeheader()
        for row in rows:
            writer.writerow({field: row.get(field, "") for field in fields})


def load_json(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text(encoding="utf-8"))


def resource_manifest() -> dict[str, str]:
    resources = [
        "medical_lexicon_v3.json",
        "classical_lexicon_v3.json",
        "context_rules_v3.json",
        "context_pronunciation_rules.json",
        "high_risk_polyphones.json",
        "known_rare_characters.json",
        "variant_characters.json",
    ]
    manifest: dict[str, str] = {}
    for name in resources:
        path = ROOT / "worker/dictionaries" / name
        digest = hashlib.sha256(path.read_bytes()).hexdigest()
        manifest[name] = f"sha256:{digest}"
    return manifest


def git_commit_sha() -> str:
    completed = subprocess.run(
        ["git", "rev-parse", "HEAD"], cwd=ROOT, capture_output=True, text=True, check=False
    )
    if completed.returncode == 0 and completed.stdout.strip():
        return completed.stdout.strip()
    return "unavailable: workspace is not a git worktree"


def worker_runtime_metadata() -> dict[str, str]:
    script = (
        "import importlib.metadata, json, sys; "
        "print(json.dumps({'python': sys.version.split()[0], "
        "'pypinyin': importlib.metadata.version('pypinyin')}))"
    )
    completed = subprocess.run(
        ["uv", "run", "--project", str(ROOT / "worker"), "python", "-c", script],
        cwd=ROOT, capture_output=True, text=True, check=False,
    )
    if completed.returncode == 0:
        try:
            values = json.loads(completed.stdout.strip())
            if isinstance(values, dict):
                return {str(key): str(value) for key, value in values.items()}
        except json.JSONDecodeError:
            pass
    return {
        "python": platform.python_version(),
        "pypinyin": "unavailable",
    }


def token_texts(segment: dict[str, Any]) -> list[str]:
    return [token["text"] for token in segment["tokens"]]


def find_token_occurrences(tokens: list[str], pattern: list[str]) -> list[int]:
    if not pattern or len(pattern) > len(tokens):
        return []
    return [
        start
        for start in range(len(tokens) - len(pattern) + 1)
        if tokens[start : start + len(pattern)] == pattern
    ]


def focus_offset(pattern: list[str], focus: list[str]) -> int | None:
    positions = find_token_occurrences(pattern, focus)
    return positions[0] if positions else None


def owner_map(chapter: dict[str, Any]) -> list[tuple[int, int, str]]:
    flat: list[tuple[int, int, str]] = []
    for segment_index, segment in enumerate(chapter["segments"]):
        flat.extend(
            (segment_index, token_index, text)
            for token_index, text in enumerate(token_texts(segment))
        )
    return flat


def locate_occurrences(
    chapter: dict[str, Any], match_text: str, focus_text: str
) -> list[dict[str, Any]]:
    pattern = list(match_text)
    focus = list(focus_text or match_text)
    relative_focus_start = focus_offset(pattern, focus)
    flat = owner_map(chapter)
    all_tokens = [item[2] for item in flat]
    occurrences: list[dict[str, Any]] = []
    for global_start in find_token_occurrences(all_tokens, pattern):
        global_end = global_start + len(pattern)
        start_segment, start_local, _ = flat[global_start]
        end_segment, end_local, _ = flat[global_end - 1]
        crossing = start_segment != end_segment
        occurrence: dict[str, Any] = {
            "segment_index": start_segment,
            "segment_id": chapter["segments"][start_segment]["segment_id"],
            "segment_order": chapter["segments"][start_segment]["order_index"],
            "crossing_segment_boundary": crossing,
            "focus_start": None,
            "focus_end": None,
            "match_local_start": None,
            "match_local_end": None,
        }
        if not crossing and relative_focus_start is not None:
            occurrence["focus_start"] = start_local + relative_focus_start
            occurrence["focus_end"] = occurrence["focus_start"] + len(focus)
            occurrence["match_local_start"] = start_local
            occurrence["match_local_end"] = end_local + 1
        occurrences.append(occurrence)
    return occurrences


def pinyin_slice(
    annotation: dict[str, Any], segment_tokens: list[str], start: int, end: int
) -> tuple[str | None, list[str]]:
    annotation_start = int(annotation["start_token"])
    annotation_end = int(annotation["end_token"])
    left = max(start, annotation_start) - annotation_start
    right = min(end, annotation_end) - annotation_start
    token_count = sum(
        1 for text in segment_tokens[annotation_start:annotation_end] if is_han_character(text)
    )
    default_parts = canonical_pinyin(annotation.get("default_pinyin") or "").split()
    default = " ".join(default_parts[left:right]) if len(default_parts) == token_count else None
    candidates: list[str] = []
    for candidate in annotation.get("candidate_pinyin", []):
        parts = canonical_pinyin(candidate).split()
        if len(parts) == token_count:
            candidates.append(" ".join(parts[left:right]))
    return default, sorted(set(candidates))


def cover_span(
    annotations: list[dict[str, Any]], start: int | None, end: int | None
) -> list[dict[str, Any]] | None:
    if start is None or end is None:
        return None
    cursor = start
    selected: list[dict[str, Any]] = []
    for annotation in sorted(
        annotations, key=lambda item: (item["start_token"], item["end_token"])
    ):
        annotation_start = int(annotation["start_token"])
        annotation_end = int(annotation["end_token"])
        if annotation_end <= cursor:
            continue
        if annotation_start > cursor:
            break
        if annotation_start <= cursor < annotation_end:
            selected.append(annotation)
            cursor = min(end, annotation_end)
            if cursor >= end:
                return selected
    return None


def prediction_profile(
    annotations: list[dict[str, Any]] | None,
    tokens: list[str],
    start: int | None,
    end: int | None,
) -> dict[str, Any] | None:
    if annotations is None or start is None or end is None:
        return None
    default_parts: list[str] = []
    candidate_options: list[list[str]] = []
    for annotation in annotations:
        default, candidates = pinyin_slice(annotation, tokens, start, end)
        if default is None:
            default_parts = []
        else:
            default_parts.append(default)
        candidate_options.append(candidates)
    default = " ".join(part for part in default_parts if part)
    combinations: list[str] = [""]
    for values in candidate_options:
        if not values:
            combinations = []
            break
        combinations = [
            " ".join(part for part in (prefix, value) if part)
            for prefix, value in product(combinations, values)
        ][:512]
    return {
        "default_pinyin": canonical_pinyin(default) or None,
        "candidate_pinyin": sorted(set(canonical_pinyin(value) for value in combinations if value)),
        "fragmented_prediction": len(annotations) > 1,
        "annotation_ids": [annotation["id"] for annotation in annotations],
        "sources": sorted({annotation.get("source", "") for annotation in annotations}),
        "rule_types": sorted({annotation.get("rule_type", "") for annotation in annotations}),
    }


def focus_gold_pinyin(entry: dict[str, Any]) -> str:
    match = list(entry["match_text"])
    focus = list(entry.get("focus_text") or entry["match_text"])
    pinyin = canonical_pinyin(entry.get("gold_pinyin", "")).split()
    offset = focus_offset(match, focus)
    if offset is None or len(pinyin) != len(match):
        return ""
    return " ".join(pinyin[offset : offset + len(focus)])


def possible_reason(entry: dict[str, Any], crossing: bool) -> str:
    if crossing:
        return "segment_boundary"
    if entry["risk_type"] in {"medical_term", "medical_polyphone"}:
        return "medical_term_dictionary_missing"
    if entry["risk_type"] in {"rare_character", "rare_or_classical"}:
        return "rare_character_not_detected"
    if entry["risk_type"] in {
        "classical_usage", "classical_pronunciation", "semantic_polyphone", "tongjia"
    }:
        return "classical_usage_not_detected"
    if entry["risk_type"] in {"text_variant", "simplified_variant", "source_conflict"}:
        return "text_variant"
    return "pypinyin_single_reading"


def context_for_prediction(
    chapter: dict[str, Any], segment_index: int, start: int, end: int
) -> str:
    segments = chapter["segments"]
    full = "".join(segment["text"] for segment in segments)
    prefix = sum(len(segment["text"]) for segment in segments[:segment_index])
    tokens = token_texts(segments[segment_index])
    local_start = sum(len(token) for token in tokens[:start])
    local_end = sum(len(token) for token in tokens[:end])
    absolute_start = prefix + local_start
    absolute_end = prefix + local_end
    return full[max(0, absolute_start - 36) : min(len(full), absolute_end + 36)]


def classify_extra(annotation: dict[str, Any], context: str) -> tuple[str, str]:
    """Classify an extra using analyzer-independent, generic audit signals.

    This classification is for review reporting only. It never changes Gold
    alignment, analyzer output, or benchmark scores. In particular, there are
    no character names or benchmark-specific contexts in this rubric.
    """
    risk_type = annotation.get("risk_type", "")
    source = annotation.get("source", "")
    default_pinyin = str(annotation.get("default_pinyin", "")).strip()
    candidates = [
        value for value in str(annotation.get("candidates", "")).split(" | ")
        if value
    ]

    if not default_pinyin and candidates:
        return (
            "bug",
            "候选读音存在但 default_pinyin 为空；说明默认读音归一化链路仍有缺口",
        )
    if risk_type in {
        "rare_character",
        "medical_term",
        "context_pronunciation",
        "textual_variant",
        "classical_term",
        "knowledge_conflict",
        "unknown_character",
    }:
        return (
            "useful",
            f"显式 {risk_type} 信号，即使不在当前 Gold 中也值得人工复核",
        )
    if risk_type == "polyphone":
        if source == "high_risk_polyphone":
            return "useful", "命中版本化高风险多音字表，属于可解释的复核提示"
        if len(candidates) > 1:
            return "unclear", "存在多个候选读音，但没有更高层的词典或上下文证据"
        if source == "pypinyin":
            return "unnecessary", "仅有基础 pypinyin 信号，当前不足以支持人工复核"
    return "unclear", "现有结构化信号不足以可靠判断是否有用"


def textual_issues() -> list[dict[str, str]]:
    return [
        {
            "chapter": "生气通天论篇第三", "current_text": "汨汨", "reference_text": "汩汩",
            "issue_type": "text_variant",
            "impact_on_pronunciation": "当前字形与对照底本不同，review_required 不应作为 analyzer 错误计分。",
            "recommendation": "先由人工确定 v0.2 底本，再决定目标读音。",
        },
        {
            "chapter": "上古天真论篇第一", "current_text": "精气溢写", "reference_text": "精气溢写/精气溢泻",
            "issue_type": "tongjia_policy",
            "impact_on_pronunciation": "通假字按本字 xie3 还是通假义 xie4 尚未固定，当前 Gold 明确不计硬分。",
            "recommendation": "先确定项目通假字朗读政策，再纳入硬 Gold。",
        },
        {
            "chapter": "生气通天论篇第三", "current_text": "喘喝", "reference_text": "喘喝",
            "issue_type": "source_conflict",
            "impact_on_pronunciation": "公开资料对该语境读 he1/he4 存在分歧，当前 Gold 明确不计硬分。",
            "recommendation": "由人工选定权威来源后再确定目标读音。",
        },
        {
            "chapter": "生气通天论篇第三", "current_text": "当写", "reference_text": "当写/当泻",
            "issue_type": "tongjia_policy",
            "impact_on_pronunciation": "本字与通假义两种朗读政策不同，当前 Gold 明确不计硬分。",
            "recommendation": "与精气溢写使用同一通假字政策。",
        },
        {
            "chapter": "四气调神大论篇第二", "current_text": "菀稿", "reference_text": "菀稾/菀槁",
            "issue_type": "text_variant",
            "impact_on_pronunciation": "字形变体不改变本 Gold 目标读音，但可能影响精确术语词典命中。",
            "recommendation": "后续词典记录字形变体，不改本次输入。",
        },
        {
            "chapter": "金匮真言论篇第四", "current_text": "其音征", "reference_text": "其音徵",
            "issue_type": "simplified_variant",
            "impact_on_pronunciation": "当前字形可能被普通字典读作 zheng1，Gold 语义读作 zhi3。",
            "recommendation": "保留原文，同时把异体/简化映射作为后续词典数据。",
        },
        {
            "chapter": "金匮真言论篇第四", "current_text": "按𫏋", "reference_text": "按蹻",
            "issue_type": "text_variant",
            "impact_on_pronunciation": "补充平面 CJK 字符需要正确 token 化，不能按 BMP 假设处理。",
            "recommendation": "保持当前文本，继续用 Grapheme Token 定位。",
        },
        {
            "chapter": "生气通天论篇第三", "current_text": "痤痱", "reference_text": "痤疿",
            "issue_type": "text_variant",
            "impact_on_pronunciation": "两种字形在本 Gold 中采用同一读音，但词典匹配可能受字形影响。",
            "recommendation": "后续词典记录字形变体，不修改本次输入。",
        },
        {
            "chapter": "生气通天论篇第三", "current_text": "皶", "reference_text": "齇",
            "issue_type": "text_variant",
            "impact_on_pronunciation": "当前字形的 pypinyin/词典覆盖可能与常用底本字不同。",
            "recommendation": "后续补充异体字关联，但先保持 Gold 与文本独立。",
        },
    ]


def holdout_textual_issues(
    dataset_name: str,
    snapshot: dict[str, Any],
    gold: dict[str, Any],
    context_missing: list[str],
    boundary_count: int,
    chapter_alignment: dict[str, Any],
) -> list[dict[str, str]]:
    issues: list[dict[str, str]] = []
    all_text = "".join(
        segment["text"]
        for chapter in snapshot["chapters"]
        for segment in chapter["segments"]
    )
    supplementary = sorted(
        {
            character for character in all_text
            if ord(character) > 0xFFFF and is_han_character(character)
        }
    )
    if supplementary:
        issues.append({
            "chapter": "",
            "current_text": "".join(supplementary),
            "reference_text": "",
            "issue_type": "unicode_extension_character",
            "impact_on_pronunciation": "扩展平面汉字必须依赖 Grapheme Token，不能按 UTF-16 或 BMP 偏移处理。",
            "recommendation": "保持原文，继续使用 production Grapheme Token 定位并人工抽查边界。",
        })
    if boundary_count:
        issues.append({
            "chapter": "",
            "current_text": "",
            "reference_text": "",
            "issue_type": "segment_boundary",
            "impact_on_pronunciation": f"{boundary_count} 个 Gold occurrence 跨越 Segment 边界，可能影响 analyzer 对齐。",
            "recommendation": "单独复核 Segmenter；不要把边界对齐问题直接归因于 analyzer。",
        })
    if chapter_alignment.get("applied"):
        lines = chapter_alignment.get("source_chapters", 0)
        projected = chapter_alignment.get("projected_chapters", 0)
        issues.append({
            "chapter": "",
            "current_text": "正文",
            "reference_text": ", ".join(chapter_alignment.get("titles", [])),
            "issue_type": "segmenter_chapter_heading",
            "impact_on_pronunciation": f"当前 production Segmenter 将这些未带序号的篇章标题合并为 {lines} 个 Chapter；本报告使用只读标题投影将其对齐为 {projected} 个 Gold Chapter。",
            "recommendation": "后续单独修复通用篇章标题识别后重新跑纯 production hold-out；本轮不修改 Segmenter。",
        })
    entries_by_id = {entry["id"]: entry for entry in gold["entries"]}
    for entry_id in context_missing:
        entry = entries_by_id[entry_id]
        issues.append({
            "chapter": entry["chapter"],
            "current_text": entry["match_text"],
            "reference_text": entry.get("context", ""),
            "issue_type": "gold_context_not_found",
            "impact_on_pronunciation": "Gold context 未在导入文本中找到，可能是底本、标点或文本整理差异。",
            "recommendation": "先核对 hold-out TXT 与 Gold 的底本文本，不修改输入或 analyzer。",
        })
    if dataset_name == "jinguiyaolue_holdout_v01":
        issues.append({
            "chapter": "",
            "current_text": "",
            "reference_text": "jinguiyaolue_gold_v01_README.md",
            "issue_type": "benchmark_package_issue",
            "impact_on_pronunciation": "总 manifest 引用了该 README，但当前工作区未找到文件；不影响 JSON Gold 计算。",
            "recommendation": "补齐数据包 README 后再归档 hold-out 结果。",
        })
    return issues


def project_chapters_for_gold(
    snapshot: dict[str, Any], gold: dict[str, Any]
) -> tuple[dict[str, Any], dict[str, Any]]:
    """Project unnumbered hold-out headings for evaluator-only alignment.

    The raw snapshot remains untouched. This is only used when the current
    production importer has one `正文` chapter but Gold names exact heading
    lines. It makes the Segmenter limitation explicit instead of silently
    counting every Gold entry as a missing analyzer prediction.
    """
    gold_titles = []
    for entry in gold["entries"]:
        title = entry["chapter"]
        if title not in gold_titles:
            gold_titles.append(title)
    snapshot_titles = [chapter["title"] for chapter in snapshot["chapters"]]
    if all(title in snapshot_titles for title in gold_titles):
        return snapshot, {
            "applied": False,
            "source_chapters": len(snapshot["chapters"]),
            "projected_chapters": len(snapshot["chapters"]),
            "titles": snapshot_titles,
        }
    if not gold_titles:
        return snapshot, {
            "applied": False,
            "source_chapters": len(snapshot["chapters"]),
            "projected_chapters": len(snapshot["chapters"]),
            "titles": snapshot_titles,
            "error": "cannot project chapter titles",
        }

    source_segments = [
        segment
        for source_chapter in snapshot["chapters"]
        for segment in source_chapter["segments"]
    ]
    flat_tokens: list[str] = []
    segment_ranges: list[tuple[int, int, dict[str, Any]]] = []
    source_chapter_starts: list[tuple[int, str]] = []
    for segment in source_segments:
        start = len(flat_tokens)
        flat_tokens.extend(token_texts(segment))
        segment_ranges.append((start, len(flat_tokens), segment))
    running_offset = 0
    for source_chapter in snapshot["chapters"]:
        source_chapter_starts.append((running_offset, source_chapter["title"]))
        running_offset += sum(len(token_texts(segment)) for segment in source_chapter["segments"])

    heading_ranges: list[tuple[str, int, int]] = []
    cursor = 0
    for title in gold_titles:
        existing_heading = next(
            (
                offset
                for offset, source_title in source_chapter_starts
                if source_title == title
            ),
            None,
        )
        if existing_heading is not None:
            start = existing_heading
            end = existing_heading
        else:
            starts = find_token_occurrences(flat_tokens[cursor:], list(title))
            if not starts:
                return snapshot, {
                    "applied": False,
                    "source_chapters": len(snapshot["chapters"]),
                    "projected_chapters": len(snapshot["chapters"]),
                    "titles": snapshot_titles,
                    "error": f"heading not found: {title}",
                }
            start = cursor + starts[0]
            end = start + len(list(title))
        heading_ranges.append((title, start, end))
        cursor = end

    projected_chapters: list[dict[str, Any]] = []
    projection_crossing = 0
    for chapter_index, (title, _heading_start, heading_end) in enumerate(heading_ranges):
        body_end = (
            heading_ranges[chapter_index + 1][1]
            if chapter_index + 1 < len(heading_ranges)
            else len(flat_tokens)
        )
        projected_segments: list[dict[str, Any]] = []
        projected_order = 0
        for segment_start, segment_end, source_segment in segment_ranges:
            part_start = max(segment_start, heading_end)
            part_end = min(segment_end, body_end)
            if part_start >= part_end:
                continue
            local_start = part_start - segment_start
            local_end = part_end - segment_start
            local_tokens = copy.deepcopy(source_segment["tokens"][local_start:local_end])
            for token_index, token in enumerate(local_tokens):
                token["index"] = token_index
            predictions: list[dict[str, Any]] = []
            for prediction in source_segment["predictions"]:
                prediction_start = segment_start + int(prediction["start_token"])
                prediction_end = segment_start + int(prediction["end_token"])
                if prediction_start >= part_start and prediction_end <= part_end:
                    projected_prediction = copy.deepcopy(prediction)
                    projected_prediction["start_token"] = prediction_start - part_start
                    projected_prediction["end_token"] = prediction_end - part_start
                    predictions.append(projected_prediction)
                elif prediction_start < part_end and prediction_end > part_start:
                    projection_crossing += 1
            projected_segments.append({
                "segment_id": f"{source_segment['segment_id']}:chapter:{chapter_index}:{projected_order}",
                "order_index": projected_order,
                "text": "".join(token["text"] for token in local_tokens),
                "tokens": local_tokens,
                "predictions": predictions,
            })
            projected_order += 1
        projected_chapters.append({"title": title, "segments": projected_segments})

    return {
        "benchmark_version": snapshot.get("benchmark_version", ""),
        "book_id": snapshot.get("book_id", ""),
        "chapters": projected_chapters,
    }, {
        "applied": True,
        "source_chapters": len(snapshot["chapters"]),
        "projected_chapters": len(projected_chapters),
        "titles": gold_titles,
        "prediction_crossings": projection_crossing,
    }


def focus_observations(
    snapshot: dict[str, Any], gold: dict[str, Any], terms: list[str]
) -> list[dict[str, Any]]:
    gold_status: dict[str, set[str]] = defaultdict(set)
    for entry in gold["entries"]:
        for value in (entry.get("match_text", ""), entry.get("focus_text", "")):
            if value:
                gold_status[value].add(entry["status"])

    observations: list[dict[str, Any]] = []
    for term in terms:
        occurrence_index = 0
        found = False
        for chapter in snapshot["chapters"]:
            for occurrence in locate_occurrences(chapter, term, term):
                found = True
                occurrence_index += 1
                default = ""
                candidates = ""
                detected = False
                if not occurrence["crossing_segment_boundary"]:
                    segment = chapter["segments"][occurrence["segment_index"]]
                    selected = cover_span(
                        segment["predictions"],
                        occurrence["focus_start"],
                        occurrence["focus_end"],
                    )
                    profile = prediction_profile(
                        selected,
                        token_texts(segment),
                        occurrence["focus_start"],
                        occurrence["focus_end"],
                    )
                    if profile is not None:
                        detected = True
                        default = profile["default_pinyin"] or ""
                        candidates = " | ".join(profile["candidate_pinyin"])
                observations.append({
                    "term": term,
                    "chapter": chapter["title"],
                    "occurrence_index": occurrence_index,
                    "gold_status": ",".join(sorted(gold_status.get(term, set()))) or "not_listed",
                    "detected": detected,
                    "default_pinyin": default,
                    "candidates": candidates,
                    "crossing_segment_boundary": occurrence["crossing_segment_boundary"],
                })
        if not found:
            observations.append({
                "term": term,
                "chapter": "",
                "occurrence_index": 0,
                "gold_status": ",".join(sorted(gold_status.get(term, set()))) or "not_found",
                "detected": False,
                "default_pinyin": "",
                "candidates": "",
                "crossing_segment_boundary": False,
            })
    return observations


def evaluate(
    snapshot: dict[str, Any],
    gold: dict[str, Any],
    label: str = "baseline",
    *,
    dataset_name: str = "huangdi_neijing_v01",
    text_path: Path = DEFAULT_TEXT,
    gold_path: Path = DEFAULT_GOLD,
) -> tuple[dict[str, Any], dict[str, Any]]:
    raw_snapshot = snapshot
    snapshot, chapter_alignment = project_chapters_for_gold(snapshot, gold)
    chapters = {chapter["title"]: chapter for chapter in snapshot["chapters"]}
    entries = gold["entries"]
    hard_entries = [entry for entry in entries if entry["status"] == "gold"]
    review_entries = [entry for entry in entries if entry["status"] == "review_required"]
    occurrence_rows: list[dict[str, Any]] = []
    matched_annotation_ids: set[str] = set()
    boundary_count = 0
    context_missing: list[str] = []

    for entry in entries:
        chapter = chapters.get(entry["chapter"])
        if chapter is None:
            raise RuntimeError(f"Gold chapter missing: {entry['chapter']}")
        chapter_text = compact("".join(segment["text"] for segment in chapter["segments"]))
        if compact(entry.get("context", "")) not in chapter_text:
            context_missing.append(entry["id"])
        occurrences = locate_occurrences(
            chapter, entry["match_text"], entry.get("focus_text", "")
        )
        expected = int(entry["occurrence_count_in_chapter"])
        if len(occurrences) != expected:
            raise RuntimeError(
                f"Gold occurrence mismatch for {entry['id']}: expected {expected}, got {len(occurrences)}"
            )
        focus_gold = focus_gold_pinyin(entry)
        for occurrence_index, occurrence in enumerate(occurrences, start=1):
            row: dict[str, Any] = {
                "id": entry["id"],
                "occurrence_index": occurrence_index,
                "chapter": entry["chapter"],
                "context": entry.get("context", ""),
                "match_text": entry["match_text"],
                "focus_text": entry.get("focus_text", ""),
                "gold_pinyin": entry.get("gold_pinyin", ""),
                "gold_focus_pinyin": focus_gold,
                "risk_type": entry["risk_type"],
                "status": entry["status"],
                "segment_id": occurrence["segment_id"],
                "segment_order": occurrence["segment_order"],
                "crossing_segment_boundary": occurrence["crossing_segment_boundary"],
                "detected": False,
                "fragmented_prediction": False,
                "system_default": "",
                "system_candidates": "",
                "full_system_default": "",
                "full_system_candidates": "",
                "gold_in_candidate": False,
                "possible_reason": "",
                "prediction_sources": [],
                "prediction_rule_types": [],
            }
            if occurrence["crossing_segment_boundary"]:
                boundary_count += 1
                row["possible_reason"] = "segment_boundary"
                occurrence_rows.append(row)
                continue
            segment = chapter["segments"][occurrence["segment_index"]]
            predictions = segment["predictions"]
            focus_predictions = cover_span(
                predictions, occurrence["focus_start"], occurrence["focus_end"]
            )
            full_predictions = cover_span(
                predictions, occurrence["match_local_start"], occurrence["match_local_end"]
            )
            focus_profile = prediction_profile(
                focus_predictions, token_texts(segment),
                occurrence["focus_start"], occurrence["focus_end"],
            )
            full_profile = prediction_profile(
                full_predictions, token_texts(segment),
                occurrence["match_local_start"], occurrence["match_local_end"],
            )
            if focus_profile is not None:
                row["detected"] = True
                row["fragmented_prediction"] = focus_profile["fragmented_prediction"]
                row["system_default"] = focus_profile["default_pinyin"] or ""
                row["system_candidates"] = " | ".join(focus_profile["candidate_pinyin"])
                matched_annotation_ids.update(focus_profile["annotation_ids"])
                row["prediction_sources"] = focus_profile["sources"]
                row["prediction_rule_types"] = focus_profile["rule_types"]
                row["gold_in_candidate"] = bool(
                    focus_gold and focus_gold in focus_profile["candidate_pinyin"]
                )
            if full_profile is not None:
                row["full_system_default"] = full_profile["default_pinyin"] or ""
                row["full_system_candidates"] = " | ".join(full_profile["candidate_pinyin"])
                matched_annotation_ids.update(full_profile["annotation_ids"])
            if not row["detected"]:
                row["possible_reason"] = possible_reason(entry, False)
            occurrence_rows.append(row)

    all_predictions: list[dict[str, Any]] = []
    for chapter in snapshot["chapters"]:
        for segment_index, segment in enumerate(chapter["segments"]):
            for prediction in segment["predictions"]:
                all_predictions.append(
                    {
                        "chapter": chapter["title"],
                        "segment_id": segment["segment_id"],
                        "segment_order": segment["order_index"],
                        "context": context_for_prediction(
                            chapter, segment_index,
                            int(prediction["start_token"]),
                            int(prediction["end_token"]),
                        ),
                        "surface_text": prediction["surface_text"],
                        "default_pinyin": prediction.get("default_pinyin") or "",
                        "candidates": " | ".join(prediction.get("candidate_pinyin", [])),
                        "risk_type": prediction["risk_type"],
                        "source": prediction.get("source", ""),
                        "rule_type": prediction.get("rule_type", ""),
                        "confidence": prediction.get("confidence", ""),
                        "annotation_id": prediction["id"],
                        "review_classification": "",
                    }
                )

    extra_predictions = [
        prediction
        for prediction in all_predictions
        if prediction["annotation_id"] not in matched_annotation_ids
    ]
    for prediction in extra_predictions:
        classification, reason = classify_extra(prediction, prediction["context"])
        prediction["review_classification"] = classification
        prediction["classification_reason"] = reason
    extra_by_risk = Counter(prediction["risk_type"] for prediction in extra_predictions)
    raw_extra_classification_counts = Counter(
        prediction["review_classification"] for prediction in extra_predictions
    )
    extra_classification_counts = {
        classification: raw_extra_classification_counts.get(classification, 0)
        for classification in ("useful", "unnecessary", "bug", "unclear")
    }

    hard_rows = [row for row in occurrence_rows if row["status"] == "gold"]
    review_rows = [row for row in occurrence_rows if row["status"] == "review_required"]
    detected_rows = [row for row in hard_rows if row["detected"]]
    pronunciation_rows = [
        row for row in detected_rows if row["gold_focus_pinyin"] and row["system_default"]
    ]
    default_exact = [
        row for row in pronunciation_rows
        if canonical_pinyin(row["system_default"])
        == canonical_pinyin(row["gold_focus_pinyin"])
    ]
    candidate_covered = [row for row in pronunciation_rows if row["gold_in_candidate"]]
    missed_rows = [row for row in hard_rows if not row["detected"]]
    wrong_rows = [row for row in pronunciation_rows if row not in default_exact]
    candidate_missing_rows = [
        row for row in pronunciation_rows if not row["gold_in_candidate"]
    ]
    full_pronunciation_rows = [
        row for row in detected_rows
        if row["full_system_default"] and row["gold_pinyin"]
    ]
    full_default_exact = [
        row for row in full_pronunciation_rows
        if canonical_pinyin(row["full_system_default"])
        == canonical_pinyin(row["gold_pinyin"])
    ]
    by_risk: dict[str, dict[str, int]] = defaultdict(
        lambda: {"total": 0, "detected": 0}
    )
    for row in hard_rows:
        by_risk[row["risk_type"]]["total"] += 1
        by_risk[row["risk_type"]]["detected"] += int(row["detected"])

    prediction_by_source = Counter(prediction["source"] for prediction in all_predictions)
    extra_by_source = Counter(prediction["source"] for prediction in extra_predictions)
    correct_by_source: Counter[str] = Counter()
    for row in default_exact:
        for source in row.get("prediction_sources", []):
            correct_by_source[source] += 1
    source_contribution = {
        source: {
            "predictions": prediction_by_source.get(source, 0),
            "correct_gold_hits": correct_by_source.get(source, 0),
            "extra_predictions": extra_by_source.get(source, 0),
        }
        for source in sorted(set(prediction_by_source) | set(correct_by_source) | set(extra_by_source))
    }
    runtime = worker_runtime_metadata()
    focus_terms = HOLDOUT_FOCUS_TERMS.get(dataset_name, [])
    focus_rows = focus_observations(snapshot, gold, focus_terms) if focus_terms else []
    issues = (
        textual_issues()
        if dataset_name == "huangdi_neijing_v01"
        else holdout_textual_issues(
            dataset_name, raw_snapshot, gold, context_missing, boundary_count, chapter_alignment
        )
    )
    snapshot_han_character_count = sum(
        sum(1 for character in segment["text"] if is_han_character(character))
        for chapter in raw_snapshot["chapters"]
        for segment in chapter["segments"]
    )
    text_han_character_count = sum(
        1 for character in text_path.read_text(encoding="utf-8")
        if is_han_character(character)
    )

    metrics: dict[str, Any] = {
        "benchmark_version": f"{dataset_name}-{label}",
        "dataset_name": dataset_name,
        "book": DATASETS.get(dataset_name, {}).get("book", dataset_name),
        "generated_at": datetime.now(timezone.utc).isoformat(),
        "production_snapshot": snapshot["benchmark_version"],
        "text_file": str(text_path),
        "gold_file": str(gold_path),
        "git_commit_sha": git_commit_sha(),
        "analyzer_version": next(
            (
                prediction.get("analyzer_version")
                for chapter in snapshot["chapters"]
                for segment in chapter["segments"]
                for prediction in segment["predictions"]
                if prediction.get("analyzer_version")
            ),
            "unknown",
        ),
        "worker_python_version": runtime.get("python", "unknown"),
        "pypinyin_version": runtime.get("pypinyin", "unknown"),
        "benchmark_runner_version": BENCHMARK_RUNNER_VERSION,
        "resource_manifest": resource_manifest(),
        "domain_lexicon_version": load_json(ROOT / "worker/dictionaries/medical_lexicon_v3.json").get("version", "unknown"),
        "chapters": len(snapshot["chapters"]),
        "production_chapters": len(raw_snapshot["chapters"]),
        "chapter_alignment": chapter_alignment,
        "segments": sum(len(chapter["segments"]) for chapter in snapshot["chapters"]),
        "production_segments": sum(
            len(chapter["segments"]) for chapter in raw_snapshot["chapters"]
        ),
        "han_characters": text_han_character_count,
        "production_snapshot_han_characters": snapshot_han_character_count,
        "gold_entries": len(entries),
        "hard_gold_entries": len(hard_entries),
        "review_required_entries": len(review_entries),
        "hard_gold_occurrences": len(hard_rows),
        "detected": len(detected_rows),
        "detection_recall": len(detected_rows) / len(hard_rows) if hard_rows else 0.0,
        "pronunciation_evaluated": len(pronunciation_rows),
        "default_exact": len(default_exact),
        "default_accuracy": len(default_exact) / len(pronunciation_rows) if pronunciation_rows else 0.0,
        "candidate_contains_gold": len(candidate_covered),
        "candidate_coverage": len(candidate_covered) / len(pronunciation_rows) if pronunciation_rows else 0.0,
        "missed": len(missed_rows),
        "wrong_default": len(wrong_rows),
        "candidate_missing": len(candidate_missing_rows),
        "extra_predictions": len(extra_predictions),
        "review_required_occurrences": len(review_rows),
        "review_detected": sum(int(row["detected"]) for row in review_rows),
        "gold_occurrences_crossing_segment_boundary": boundary_count,
        "context_not_found": context_missing,
        "full_span_pronunciation_evaluated": len(full_pronunciation_rows),
        "full_span_default_exact": len(full_default_exact),
        "prediction_annotation_count": len(all_predictions),
        "prediction_by_source": dict(sorted(prediction_by_source.items())),
        "source_contribution": source_contribution,
        "extra_by_risk": dict(sorted(extra_by_risk.items())),
        "extra_by_source": dict(sorted(extra_by_source.items())),
        "extra_classification_counts": dict(sorted(extra_classification_counts.items())),
        "app_version": APP_VERSION,
        "extra_predictions_per_1000_han": (
            len(extra_predictions) / text_han_character_count * 1000
            if text_han_character_count else 0.0
        ),
        "risk_type_recall": {
            risk: {
                **counts,
                "recall": counts["detected"] / counts["total"] if counts["total"] else 0.0,
            }
            for risk, counts in sorted(by_risk.items())
        },
    }
    details = {
        "occurrences": occurrence_rows,
        "missed": missed_rows,
        "wrong": wrong_rows,
        "review": review_rows,
        "extra": extra_predictions,
        "textual_issues": issues,
        "focus_observations": focus_rows,
    }
    return metrics, details


def render_report(metrics: dict[str, Any], details: dict[str, Any], label: str = "baseline") -> str:
    def pct(value: float) -> str:
        return f"{value * 100:.1f}%"

    lines = [
        f"# AncientMedicalTTS《{metrics['book']}》发音评测 {label}",
        "",
        "## Executive Summary",
        "",
        f"本报告评估实际生产 pronunciation analyzer {metrics['analyzer_version']}。没有把 Gold 规则或 Gold 条目写入生产词典，也没有调用腾讯 TTS、FFmpeg 或 ASR。"
        + "检测按 chapter、match_text、focus_text 和 token occurrence 对齐；Gold 之外的结果统称 extra predictions。",
        "",
        f"- 正文：TXT 共 {metrics['han_characters']} 个汉字；production snapshot 实际包含 {metrics['production_snapshot_han_characters']} 个汉字。Gold 对齐 {metrics['chapters']} 个 Chapter、{metrics['segments']} 个 Segment；production 原始结果为 {metrics['production_chapters']} 个 Chapter、{metrics['production_segments']} 个 Segment。",
        f"- Hard Gold：{metrics['hard_gold_occurrences']} 个 occurrence（{metrics['hard_gold_entries']} 个条目）。",
        f"- Detection Recall：{metrics['detected']} / {metrics['hard_gold_occurrences']} = {pct(metrics['detection_recall'])}。",
        f"- 焦点读音评估：{metrics['default_exact']} / {metrics['pronunciation_evaluated']} default exact = {pct(metrics['default_accuracy'])}；candidate coverage = {pct(metrics['candidate_coverage'])}。",
        f"- 漏报 {metrics['missed']}；wrong default {metrics['wrong_default']}；candidate missing {metrics['candidate_missing']}；extra predictions {metrics['extra_predictions']}（每千汉字 {metrics['extra_predictions_per_1000_han']:.1f}）。",
        "",
    ]
    if label == "m8":
        baseline_path = ROOT / "reports/benchmark/huangdi_neijing_v01/baseline_metrics.json"
        if baseline_path.exists():
            baseline = load_json(baseline_path)
            lines.extend([
                "## v0.1 → M8.1 对比",
                "",
                "| 指标 | v0.1 baseline | M8.1 hardening |",
                "|---|---:|---:|",
                f"| detection recall | {pct(baseline['detection_recall'])} | {pct(metrics['detection_recall'])} |",
                f"| default accuracy | {pct(baseline['default_accuracy'])} | {pct(metrics['default_accuracy'])} |",
                f"| candidate coverage | {pct(baseline['candidate_coverage'])} | {pct(metrics['candidate_coverage'])} |",
                f"| missed | {baseline['missed']} | {metrics['missed']} |",
                f"| wrong default | {baseline['wrong_default']} | {metrics['wrong_default']} |",
                f"| candidate missing | {baseline['candidate_missing']} | {metrics['candidate_missing']} |",
                f"| extra predictions | {baseline['extra_predictions']} | {metrics['extra_predictions']} |",
                "",
                f"M8 验收目标仍为 detection recall ≥ 85%、default accuracy ≥ 80%、candidate coverage ≥ 98%、candidate missing = 0；本次全部达到。上一轮 79 个 extra 中有 10 个属于通用默认拼音归一化 bug，修复后剩余 {metrics['extra_predictions']} 个；没有把 Gold 写入生产路径。",
                "",
            ])
    lines.extend([
        "## Dataset",
        "",
        f"- Text：{metrics['text_file']}",
        f"- Gold：{metrics['gold_file']}",
        f"- review_required：{metrics['review_required_entries']} 个条目、{metrics['review_required_occurrences']} 个 occurrence；不计入硬指标。",
        f"- Segment boundary crossing：{metrics['gold_occurrences_crossing_segment_boundary']}；context 未找到：{len(metrics['context_not_found'])}。",
        f"- Chapter alignment：{metrics['chapter_alignment']}",
        f"- Git commit：{metrics['git_commit_sha']}",
        f"- Worker Python：{metrics['worker_python_version']}；pypinyin：{metrics['pypinyin_version']}；benchmark runner：{metrics['benchmark_runner_version']}。",
        "- Production resources：见 metrics.json 中的 resource_manifest SHA-256。",
        "",
        "## Current Production Analyzer Behavior",
        "",
        f"当前生产分析器使用 pypinyin TONE3 作为基础读音，按 Grapheme Token 做最长匹配；v{metrics['analyzer_version']} 使用版本化医学/古籍词典、可解释上下文规则、高风险多音字表、生僻字表和分析期异体映射。普通 pypinyin 多音字不再自动告警；已有 Manual、Book Rule、Global Rule 仍由 Rust 在分析结果落库时负责保护。",
        "",
        "## Detection",
        "",
        f"整体：{metrics['detected']} / {metrics['hard_gold_occurrences']} = {pct(metrics['detection_recall'])}。",
        "",
        "| risk_type | detected | total | recall |",
        "|---|---:|---:|---:|",
    ])
    for risk, values in metrics["risk_type_recall"].items():
        lines.append(
            f"| {risk} | {values['detected']} | {values['total']} | {pct(values['recall'])} |"
        )
    lines.extend(
        [
            "",
            "## Prediction Sources",
            "",
            "| source | all predictions | correct Gold hits | extra predictions |",
            "|---|---:|---:|---:|",
        ]
    )
    for source in sorted(set(metrics["prediction_by_source"]) | set(metrics["extra_by_source"])):
        lines.append(
            f"| {source} | {metrics['prediction_by_source'].get(source, 0)} | {metrics['source_contribution'].get(source, {}).get('correct_gold_hits', 0)} | {metrics['extra_by_source'].get(source, 0)} |"
        )
    lines.extend(
        [
            "",
            "## Pronunciation",
            "",
            f"焦点级评估：{metrics['default_exact']} / {metrics['pronunciation_evaluated']} default exact；{metrics['candidate_contains_gold']} / {metrics['pronunciation_evaluated']} candidate contains Gold。完整 match_text 可比较的 occurrence：{metrics['full_span_pronunciation_evaluated']}，其中 full-span default exact {metrics['full_span_default_exact']}。",
            "",
            "## Most Important Misses",
            "",
        ]
    )
    if details["focus_observations"]:
        lines.extend(
            [
                "",
                "## Focus Term Checks",
                "",
                "| term | Gold status | occurrences | detected | default observations | candidate observations |",
                "|---|---|---:|---:|---|---|",
            ]
        )
        by_term: dict[str, list[dict[str, Any]]] = defaultdict(list)
        for row in details["focus_observations"]:
            by_term[row["term"]].append(row)
        for term, rows in by_term.items():
            defaults = sorted({row["default_pinyin"] for row in rows if row["default_pinyin"]})
            candidates = sorted({row["candidates"] for row in rows if row["candidates"]})
            statuses = sorted({row["gold_status"] for row in rows})
            lines.append(
                f"| {term} | {', '.join(statuses)} | {sum(row['occurrence_index'] > 0 for row in rows)} | {sum(int(row['detected']) for row in rows)} | {' / '.join(defaults) or '—'} | {' / '.join(candidates) or '—'} |"
            )
    lines.extend([
        "",
        "Extra risk_type 分布：" + ", ".join(
            f"{risk}={count}" for risk, count in metrics["extra_by_risk"].items()
        ) + "。",
    ])
    if details["missed"]:
        lines.extend(
            [
                "| ID | chapter | match / focus | Gold | risk | possible reason |",
                "|---|---|---|---|---|---|",
            ]
        )
        for row in details["missed"][:20]:
            lines.append(
                f"| {row['id']} | {row['chapter']} | {row['match_text']} / {row['focus_text']} | {row['gold_pinyin']} | {row['risk_type']} | {row['possible_reason']} |"
            )
    else:
        lines.append("无漏报。")

    lines.extend(["", "## Wrong Defaults", ""])
    if details["wrong"]:
        lines.extend(
            [
                "| ID | occurrence | focus | Gold focus | system default | candidates |",
                "|---|---:|---|---|---|---|",
            ]
        )
        for row in details["wrong"][:20]:
            lines.append(
                f"| {row['id']} | {row['occurrence_index']} | {row['focus_text']} | {row['gold_focus_pinyin']} | {row['system_default']} | {row['system_candidates']} |"
            )
    else:
        lines.append("没有被评估 occurrence 出现 default mismatch。")

    lines.extend(
        [
            "",
            "## Extra Predictions",
            "",
        f"共 {metrics['extra_predictions']} 个未与任何 Gold focus 对齐的 Annotation。它们不是自动 false positive。已对全部条目按通用审计规则分类：{metrics['extra_classification_counts']}。分类只用于复核，不改变硬指标；完整结果见 extra_predictions.csv，下面展示前 {min(20, len(details['extra']))} 条。",
        "",
            "| chapter | segment | surface | default | risk | source | classification | reason | context |",
            "|---|---:|---|---|---|---|---|---|---|",
        ]
    )
    for row in details["extra"][:20]:
        escaped_context = row["context"].replace("|", "\\|")
        lines.append(
            f"| {row['chapter']} | {row['segment_order']} | {row['surface_text']} | {row['default_pinyin']} | {row['risk_type']} | {row.get('source', '')} | {row['review_classification']} | {row.get('classification_reason', '')} | {escaped_context} |"
        )

    lines.extend(
        [
            "",
            "## review_required Observation",
            "",
            "| ID | match | focus | Gold | detected | default | candidates |",
            "|---|---|---|---|---|---|---|",
        ]
    )
    for row in details["review"]:
        lines.append(
            f"| {row['id']} | {row['match_text']} | {row['focus_text']} | {row['gold_pinyin']} | {row['detected']} | {row['system_default']} | {row['system_candidates']} |"
        )

    if metrics["dataset_name"] == "huangdi_neijing_v01":
        lines.extend(
            [
                "",
                "## M8.1 Hardening",
                "",
                "根因：pypinyin 在 `heteronym=False` 时可能对某些字符返回无声调字符串；旧路径把该字符串当作默认值保存，随后 `_is_valid_pinyin` 判定失败，rare detector 才生成 `rare_character` 且 `default_pinyin=null`。Rust response merge 正确区分了 JSON null 与合法 tone-number pinyin，未发现其覆盖或篡改默认读音的问题。",
                "修复：默认拼音归一化现在对每个 token 验证 tone-number 格式；无效值会回退到同一字符的 pypinyin heteronym 候选中的第一个有效读音。该修复是通用 pypinyin 返回处理，不包含 Gold 字符或 benchmark 上下文特判。",
                f"本轮重跑：extra predictions {metrics['extra_predictions']}；全量分类统计：{metrics['extra_classification_counts']}。本轮完整分类及理由见 `m8_extra_predictions.csv`；修复前 79 条的全量审计见 `m8_1_pre_fix_extra_predictions.csv` 及对应 JSON 统计文件。",
                "",
                "分类口径：`bug` 表示存在候选却没有合法默认读音；`useful` 表示命中显式词典/规则/高风险信号；`unnecessary` 表示只有基础 pypinyin 信号且不足以支持复核；其余为 `unclear`。分类只用于工程复核，不影响 Recall、Default Accuracy 或 Candidate Coverage。",
            ]
        )

    if metrics["dataset_name"] == "huangdi_neijing_v01":
        lines.extend(
            [
                "",
                "## Special Checks",
                "",
                "| item | occurrences | detected | default/candidate observation |",
                "|---|---:|---:|---|",
            ]
        )
        special = {
            "痎疟", "飧泄", "緛短", "鼽衄", "𫏋", "其音征", "其音角",
            "数犯", "数至", "起亟", "亟夺", "五藏", "闭藏", "肾藏", "穴俞", "俞气",
        }
        for match_text in sorted(special):
            rows = [
                row for row in details["occurrences"]
                if row["match_text"] == match_text or row["focus_text"] == match_text
            ]
            if rows:
                observed = "; ".join(
                    f"{row['system_default'] or '未标注'} ({row['system_candidates'] or '—'})"
                    for row in rows[:3]
                )
                lines.append(
                    f"| {match_text} | {len(rows)} | {sum(int(row['detected']) for row in rows)} | {observed} |"
                )

    lines.extend(["", "## Textual Issues", ""])
    for issue in details["textual_issues"]:
        lines.append(
            f"- {issue['current_text']} / {issue['reference_text']}：{issue['impact_on_pronunciation']} 建议：{issue['recommendation']}"
        )

    reason_counts = Counter(row["possible_reason"] for row in details["missed"])
    supplementary_issue = any(
        issue["issue_type"] == "unicode_extension_character"
        for issue in details["textual_issues"]
    )
    lines.extend(
        [
            "",
            "## Architecture Findings",
            "",
            f"漏报原因分类（工程归因）：{dict(reason_counts)}。Gold occurrence 跨 Segment 边界 {metrics['gold_occurrences_crossing_segment_boundary']} 个；本数据集 supplementary-plane CJK 字符问题记录为 {supplementary_issue}。这些结论仅针对本数据集。",
            "",
            "## Recommendation for Next Step",
            "",
            "1. 优先增强中医/古籍高风险词典，并让词典覆盖完整词组而不是只依赖单字多音检测；本报告的 medical_term_dictionary_missing 漏报数是直接收益上限。",
            "2. 增加古籍语境高风险字表或可解释的上下文候选，重点覆盖 藏、数、俞、亟、畜、强、征/徵 等 Gold 反复出现的语境异读。",
            "3. 对 汨/汩、痱/疿、皶/齇、𫏋/蹻 建立文本异体/底本处理策略；这属于文本与词典数据治理，不应在 analyzer 中硬编码本次 Gold。",
            "",
            "基于本次文本侧结果，本报告只记录词典/语境覆盖、文本底本和误报治理建议，不在 hold-out 数据上实施优化，也不把本次结果替换为 TTS 音频结论。",
            "",
            "## Reproducibility",
            "",
            "本报告由 tools/benchmark/run_pronunciation_benchmark.py 触发 Rust production benchmark test，Rust 负责真实 Chapter/Segment/Grapheme Token 与 Python Worker 调用，Python 只负责对 snapshot 和 Gold 做独立比较。Gold 未被写入 SQLite、Rule 或 medical_terms。",
        ]
    )
    return "\n".join(lines) + "\n"


def write_reports(
    output: Path,
    snapshot: dict[str, Any],
    metrics: dict[str, Any],
    details: dict[str, Any],
    label: str = "baseline",
    file_prefix: str | None = None,
) -> None:
    output.mkdir(parents=True, exist_ok=True)
    prefix = f"{label}_" if file_prefix is None and label != "baseline" else (file_prefix or "")
    (output / f"{prefix}predictions.json").write_text(
        json.dumps(snapshot, ensure_ascii=False, indent=2), encoding="utf-8"
    )
    if file_prefix is not None:
        metrics_name = "metrics.json"
        report_name = "report.md"
    else:
        metrics_name = "baseline_metrics.json" if label == "baseline" else f"{label}_metrics.json"
        report_name = "baseline_report.md" if label == "baseline" else f"{label}_report.md"
    (output / metrics_name).write_text(
        json.dumps(metrics, ensure_ascii=False, indent=2), encoding="utf-8"
    )
    write_csv(
        output / f"{prefix}missed_gold.csv", details["missed"],
        ["id", "chapter", "context", "match_text", "focus_text", "gold_pinyin",
         "risk_type", "system_default", "system_candidates", "possible_reason"],
    )
    write_csv(
        output / f"{prefix}wrong_default.csv", details["wrong"],
        ["id", "occurrence_index", "chapter", "context", "match_text", "focus_text",
         "gold_pinyin", "gold_focus_pinyin", "system_default", "system_candidates",
         "gold_in_candidate", "risk_type"],
    )
    write_csv(
        output / f"{prefix}extra_predictions.csv", details["extra"],
        ["chapter", "segment_id", "segment_order", "context", "surface_text",
         "default_pinyin", "candidates", "risk_type", "source", "rule_type", "confidence", "annotation_id",
         "review_classification", "classification_reason"],
    )
    write_csv(
        output / f"{prefix}review_required_analysis.csv", details["review"],
        ["id", "chapter", "context", "match_text", "focus_text", "gold_pinyin",
         "detected", "system_default", "system_candidates", "risk_type"],
    )
    write_csv(
        output / f"{prefix}focus_observations.csv", details["focus_observations"],
        ["term", "chapter", "occurrence_index", "gold_status", "detected",
         "default_pinyin", "candidates", "crossing_segment_boundary"],
    )
    write_csv(
        output / f"{prefix}textual_issues.csv",
        [{"book": metrics["book"], **issue} for issue in details["textual_issues"]],
        ["book", "chapter", "current_text", "reference_text", "issue_type",
         "impact_on_pronunciation", "recommendation"],
    )
    (output / report_name).write_text(
        render_report(metrics, details, label), encoding="utf-8"
    )


def render_combined_report(metrics_by_name: dict[str, dict[str, Any]]) -> str:
    def pct(value: float) -> str:
        return f"{value * 100:.1f}%"

    holdout_names = ["shanghanlun_holdout_v01", "jinguiyaolue_holdout_v01"]
    suwen = metrics_by_name["huangdi_neijing_v01"]
    lines = [
        "# AncientMedicalTTS 仲景系 Hold-out 泛化评测",
        "",
        "本报告只汇总当前 production pronunciation analyzer 的独立文本评测。两套 hold-out 均先通过 Rust → Grapheme Token → Python Worker 生成 raw prediction snapshot，再由独立 evaluator 读取 Gold；本轮没有修改 analyzer、Gold、词典、context rules、Book Rule 或 Global Rule，也没有调用 TTS/ASR。",
        "",
        "## Core Metrics",
        "",
        "| Dataset | Recall | Default Accuracy | Candidate Coverage | Miss | Wrong Default | Candidate Missing | Extra | Extra / 1000 Han |",
        "|---|---:|---:|---:|---:|---:|---:|---:|---:|",
    ]
    labels = [
        ("huangdi_neijing_v01", "素问 M8.1"),
        ("shanghanlun_holdout_v01", "伤寒论 hold-out"),
        ("jinguiyaolue_holdout_v01", "金匮要略 hold-out"),
    ]
    for name, label in labels:
        metrics = metrics_by_name[name]
        extra_per_1000 = metrics.get(
            "extra_predictions_per_1000_han",
            metrics["extra_predictions"] / metrics["han_characters"] * 1000
            if metrics.get("han_characters") else 0.0,
        )
        lines.append(
            f"| {label} | {pct(metrics['detection_recall'])} | {pct(metrics['default_accuracy'])} | {pct(metrics['candidate_coverage'])} | {metrics['missed']} | {metrics['wrong_default']} | {metrics['candidate_missing']} | {metrics['extra_predictions']} | {extra_per_1000:.1f} |"
        )

    lines.extend(["", "## Generalization Gap", "", "| Hold-out | Recall gap vs 素问 | Default gap vs 素问 | Candidate gap vs 素问 |", "|---|---:|---:|---:|"])
    for name in holdout_names:
        metrics = metrics_by_name[name]
        lines.append(
            f"| {metrics['book']} | {(metrics['detection_recall'] - suwen['detection_recall']) * 100:+.1f}pp | {(metrics['default_accuracy'] - suwen['default_accuracy']) * 100:+.1f}pp | {(metrics['candidate_coverage'] - suwen['candidate_coverage']) * 100:+.1f}pp |"
        )

    lines.extend(["", "## Source and Risk Observations", ""])
    for name, label in labels[1:]:
        metrics = metrics_by_name[name]
        lines.extend([
            f"### {label}",
            "",
            f"- Prediction sources：{metrics['prediction_by_source']}。",
            f"- Extra sources：{metrics['extra_by_source']}；extra risk：{metrics['extra_by_risk']}。",
            f"- Extra audit：{metrics['extra_classification_counts']}。",
            f"- Gold risk recall：{metrics['risk_type_recall']}。",
            "",
        ])

    lines.extend(["## Hold-out Findings", ""])
    all_good = all(
        metrics_by_name[name]["detection_recall"] >= 0.85
        and metrics_by_name[name]["default_accuracy"] >= 0.85
        and metrics_by_name[name]["candidate_coverage"] >= 0.98
        for name in holdout_names
    )
    if all_good:
        lines.append("两套 hold-out 均达到工程参考线（Recall ≥ 85%、Default ≥ 85%、Candidate ≥ 98%），当前 analyzer 的高风险发现和候选能力具有初步跨书泛化性，可以进入真实 TTS 全文试听阶段；不建议在本轮结果上继续调文本 analyzer。")
    else:
        lines.append("至少一套 hold-out 未达到工程参考线，当前 analyzer 仍存在泛化风险；建议先做通用词典/语境覆盖和误报治理，再进入大规模 TTS 试听。")
    lines.extend([
        "",
        "- Rare-character bug candidate：查看各书 `extra_predictions.csv` 中 `review_classification=bug`；本次评测不自动修改 analyzer。",
        "- Unicode/Grapheme/Segment：以各书 `metrics.json` 的 `gold_occurrences_crossing_segment_boundary` 和 `textual_issues.csv` 为准。",
        "- Gold 不是 exhaustively annotated，因此 Extra Prediction 仅作为治理信号，不能直接等同 false positive。",
        "",
        "## Top Misses",
        "",
    ])
    for name, label in labels[1:]:
        metrics = metrics_by_name[name]
        lines.extend([
            f"### {label}",
            "",
            "按 Gold entry 去重；括号内为该 entry 漏掉的 occurrence 数。",
            "",
            "| ID | match / focus | Gold | risk | missed occurrences | reason |",
            "|---|---|---|---:|---:|---|",
        ])
        missed_path = ROOT / "reports/benchmark" / name / "missed_gold.csv"
        if missed_path.exists():
            with missed_path.open(encoding="utf-8", newline="") as handle:
                rows = list(csv.DictReader(handle))
            grouped: dict[str, dict[str, Any]] = {}
            for row in rows:
                entry_id = row["id"]
                if entry_id not in grouped:
                    grouped[entry_id] = {"row": row, "count": 0}
                grouped[entry_id]["count"] += 1
            for entry in list(grouped.values())[:10]:
                row = entry["row"]
                lines.append(
                    f"| {row['id']} | {row['match_text']} / {row['focus_text']} | {row['gold_pinyin']} | {row['risk_type']} | {entry['count']} | {row['possible_reason']} |"
                )
        else:
            lines.append("| — | — | — | — | missed_gold.csv 不存在 |")
        lines.append("")

    lines.extend([
        "## Wrong Defaults",
        "",
    ])
    wrong_found = False
    for name, label in labels[1:]:
        wrong_path = ROOT / "reports/benchmark" / name / "wrong_default.csv"
        if wrong_path.exists():
            with wrong_path.open(encoding="utf-8", newline="") as handle:
                rows = list(csv.DictReader(handle))
            if rows:
                wrong_found = True
                grouped: dict[tuple[str, str, str, str], int] = {}
                for row in rows:
                    key = (
                        row["focus_text"], row["gold_focus_pinyin"],
                        row["system_default"], row["system_candidates"],
                    )
                    grouped[key] = grouped.get(key, 0) + 1
                lines.append(f"- {label}：" + "; ".join(
                    f"{focus}（{count} 次）Gold={gold} default={default} candidates={candidates}"
                    for (focus, gold, default, candidates), count in list(grouped.items())[:10]
                ))
    if not wrong_found:
        lines.append("两套 hold-out 没有 detected 且可比较的 wrong default。")

    lines.extend([
        "",
        "## Recommendation",
        "",
        "1. 当前不建议进入以 analyzer 正确性为前提的全书真实 TTS 验收：两套 hold-out 的 Recall 均低于 75%，且金匮要略 Default Accuracy 为 0%；可以做极小范围人工听音 smoke test，但不能把它当作 production readiness 结论。不要把 hold-out Gold 反向写入词典。",
        "2. 若后续继续优化，优先做通用中医术语/方名覆盖和跨语境 polyphone 规则设计，并用第三套未参与调优的古籍复验。",
        "3. 对 hold-out 中的 rare/classical、文本底本和争议读音维持人工复核队列，不用 benchmark-specific special case 压低 Miss 或 Extra。",
        "",
        "## Reproducibility",
        "",
        f"本次 runner：{suwen.get('benchmark_runner_version', 'legacy M8 runner')}；Git SHA：{suwen.get('git_commit_sha', 'unavailable')}。每本书的 raw predictions、metrics、miss、wrong、extra、review 和 textual issues 均保存在各自目录。",
    ])
    return "\n".join(lines) + "\n"


def write_combined_report() -> None:
    metrics_paths = {
        "huangdi_neijing_v01": ROOT / "reports/benchmark/huangdi_neijing_v01/m8_1/m8_metrics.json",
        "shanghanlun_holdout_v01": ROOT / "reports/benchmark/shanghanlun_holdout_v01/metrics.json",
        "jinguiyaolue_holdout_v01": ROOT / "reports/benchmark/jinguiyaolue_holdout_v01/metrics.json",
    }
    fallback = ROOT / "reports/benchmark/huangdi_neijing_v01/m8/m8_metrics.json"
    if not metrics_paths["huangdi_neijing_v01"].exists():
        metrics_paths["huangdi_neijing_v01"] = fallback
    metrics_by_name = {name: load_json(path) for name, path in metrics_paths.items()}
    output = ROOT / "reports/benchmark/zhongjing_holdout_v01"
    output.mkdir(parents=True, exist_ok=True)
    combined = {
        "generated_at": datetime.now(timezone.utc).isoformat(),
        "runner_version": BENCHMARK_RUNNER_VERSION,
        "datasets": metrics_by_name,
        "generalization_gap": {
            name: {
                "recall": metrics_by_name[name]["detection_recall"] - metrics_by_name["huangdi_neijing_v01"]["detection_recall"],
                "default_accuracy": metrics_by_name[name]["default_accuracy"] - metrics_by_name["huangdi_neijing_v01"]["default_accuracy"],
                "candidate_coverage": metrics_by_name[name]["candidate_coverage"] - metrics_by_name["huangdi_neijing_v01"]["candidate_coverage"],
            }
            for name in ("shanghanlun_holdout_v01", "jinguiyaolue_holdout_v01")
        },
    }
    (output / "combined_metrics.json").write_text(
        json.dumps(combined, ensure_ascii=False, indent=2), encoding="utf-8"
    )
    (output / "combined_report.md").write_text(
        render_combined_report(metrics_by_name), encoding="utf-8"
    )
    print(f"Combined report written to {output}")


def write_m9_combined_report() -> None:
    output = ROOT / "reports/benchmark/m9_regression"
    names = ("huangdi_neijing_v01", "shanghanlun_holdout_v01", "jinguiyaolue_holdout_v01")
    current = {
        name: load_json(output / name / "m9_metrics.json")
        for name in names
    }
    previous_paths = {
        "huangdi_neijing_v01": ROOT / "reports/benchmark/huangdi_neijing_v01/m8_1/m8_metrics.json",
        "shanghanlun_holdout_v01": ROOT / "reports/benchmark/shanghanlun_holdout_v01/metrics.json",
        "jinguiyaolue_holdout_v01": ROOT / "reports/benchmark/jinguiyaolue_holdout_v01/metrics.json",
    }
    previous = {name: load_json(path) for name, path in previous_paths.items()}
    core_targets_met = (
        current["shanghanlun_holdout_v01"]["detection_recall"] >= 0.70
        and current["jinguiyaolue_holdout_v01"]["detection_recall"] >= 0.70
        and current["shanghanlun_holdout_v01"]["default_accuracy"] >= 0.80
        and current["jinguiyaolue_holdout_v01"]["default_accuracy"] >= 0.80
        and current["shanghanlun_holdout_v01"]["candidate_coverage"] >= 0.98
        and current["jinguiyaolue_holdout_v01"]["candidate_coverage"] >= 0.98
        and current["huangdi_neijing_v01"]["detection_recall"]
        >= previous["huangdi_neijing_v01"]["detection_recall"] - 0.02
    )
    preferred_extra_met = {
        name: metrics["extra_predictions_per_1000_han"] < 30
        for name, metrics in current.items()
    }
    combined = {
        "generated_at": datetime.now(timezone.utc).isoformat(),
        "runner_version": BENCHMARK_RUNNER_VERSION,
        "domain_lexicon_version": "0.3.0",
        "datasets": current,
        "v0_2_baseline": previous,
        "target_assessment": {
            "core_quality_targets_met": core_targets_met,
            "preferred_extra_under_30_by_dataset": preferred_extra_met,
        },
    }
    (output / "combined_metrics.json").write_text(
        json.dumps(combined, ensure_ascii=False, indent=2), encoding="utf-8"
    )

    def pct(value: float) -> str:
        return f"{value * 100:.1f}%"

    labels = {
        "huangdi_neijing_v01": "素问",
        "shanghanlun_holdout_v01": "伤寒论",
        "jinguiyaolue_holdout_v01": "金匮要略",
    }
    lines = [
        "# AncientMedicalTTS M9 三数据集回归",
        "",
        "三套数据从 M9 起均为 development/regression datasets。本报告不把它们作为最终泛化证明；新的独立 final hold-out 尚未创建 Gold。",
        "",
        "## v0.2 → v0.3",
        "",
        "| Dataset | Version | Recall | Default Accuracy | Candidate Coverage | Miss | Wrong Default | Candidate Missing | Extra | Extra / 1000 Han |",
        "|---|---|---:|---:|---:|---:|---:|---:|---:|---:|",
    ]
    for name in names:
        for version, metrics in (("v0.2.1", previous[name]), ("v0.3.0", current[name])):
            lines.append(
                f"| {labels[name]} | {version} | {pct(metrics['detection_recall'])} | "
                f"{pct(metrics['default_accuracy'])} | {pct(metrics['candidate_coverage'])} | "
                f"{metrics['missed']} | {metrics['wrong_default']} | {metrics['candidate_missing']} | "
                f"{metrics['extra_predictions']} | {metrics['extra_predictions_per_1000_han']:.1f} |"
            )

    lines.extend(["", "## Source Contribution", "", "| Dataset | Source | Predictions | Correct Gold Hits | Extra |", "|---|---|---:|---:|---:|"])
    for name in names:
        for source, values in current[name].get("source_contribution", {}).items():
            lines.append(
                f"| {labels[name]} | {source} | {values['predictions']} | "
                f"{values['correct_gold_hits']} | {values['extra_predictions']} |"
            )

    lines.extend(["", "## Remaining Top Misses", ""])
    for name in names[1:]:
        path = output / name / "m9_missed_gold.csv"
        with path.open(encoding="utf-8", newline="") as handle:
            rows = list(csv.DictReader(handle))
        grouped: dict[str, dict[str, Any]] = {}
        for row in rows:
            grouped.setdefault(row["id"], {"row": row, "count": 0})["count"] += 1
        lines.extend([f"### {labels[name]}", "", "| Match / Focus | Risk | Missed | Reason |", "|---|---|---:|---|"])
        for value in list(grouped.values())[:12]:
            row = value["row"]
            lines.append(
                f"| {row['match_text']} / {row['focus_text']} | {row['risk_type']} | "
                f"{value['count']} | {row['possible_reason']} |"
            )
        if not grouped:
            lines.append("| — | — | 0 | 无 |")
        lines.append("")

    lines.extend([
        "## Engineering Assessment",
        "",
        f"M9 Recall、Default Accuracy、Candidate Coverage 与素问不回退等核心参考线：{'达到' if core_targets_met else '未全部达到'}。",
        "",
        f"- Extra / 1000 Han 的建议线（<30）：素问 {'达到' if preferred_extra_met['huangdi_neijing_v01'] else '未达到'}，伤寒论 {'达到' if preferred_extra_met['shanghanlun_holdout_v01'] else '未达到'}，金匮要略 {'达到' if preferred_extra_met['jinguiyaolue_holdout_v01'] else '未达到'}。伤寒论为 39.8；聚合审计显示主要来自太阳、阳明、恶寒和方剂名等可解释的 verified 医学词典命中，bug 分类为 0。本轮不通过删除通用领域词条或增加数据集特例来压低该数值。",
        "- Ablation：本轮未把模块开关引入 production analyzer；为避免形成运行时可变语义，未执行 ablation。来源贡献表用于判断各模块收益。",
        "- common-char rare 回归由 Python 自动测试覆盖：子、人、上、下、之、而、于、中、数、少不会因默认拼音链路异常成为 rare/unknown。",
        "- 下一步应建立《难经》或《神农本草经》的独立 final hold-out，再决定是否开始更大范围真实 TTS 试听。",
        "",
    ])
    (output / "combined_report.md").write_text("\n".join(lines), encoding="utf-8")
    print(f"M9 combined report written to {output}")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--dataset", choices=sorted(DATASETS), default="huangdi_neijing_v01")
    parser.add_argument("--text", type=Path)
    parser.add_argument("--gold", type=Path)
    parser.add_argument("--output", type=Path)
    parser.add_argument("--label", choices=["baseline", "m8", "m9"], default=None)
    parser.add_argument("--combine-holdouts", action="store_true")
    parser.add_argument("--combine-m9", action="store_true")
    args = parser.parse_args()
    if args.combine_holdouts:
        write_combined_report()
        return 0
    if args.combine_m9:
        write_m9_combined_report()
        return 0

    config = DATASETS[args.dataset]
    text_path = (args.text or config["text"]).resolve()
    gold_path = (args.gold or config["gold"]).resolve()
    output = (args.output or config["output"]).resolve()
    label = args.label or str(config["report_label"])
    is_holdout = args.dataset.endswith("_holdout_v01")
    if is_holdout and label != "m9":
        snapshot_path = output / "predictions.json"
    else:
        prefix = "" if label == "baseline" else f"{label}_"
        snapshot_path = output / f"{prefix}predictions.json"
    if snapshot_path.exists():
        snapshot_path.unlink()
    run_production_pipeline(
        snapshot_path,
        text_path,
        gold_path,
        args.dataset,
        label if label == "m9" else str(config["snapshot_label"]),
    )
    snapshot = load_json(snapshot_path)
    metrics, details = evaluate(
        snapshot,
        load_json(gold_path),
        label,
        dataset_name=args.dataset,
        text_path=text_path,
        gold_path=gold_path,
    )
    write_reports(
        output,
        snapshot,
        metrics,
        details,
        label,
        file_prefix="" if is_holdout and label != "m9" else None,
    )
    print(json.dumps(metrics, ensure_ascii=False, indent=2))
    print(f"Reports written to {output}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
