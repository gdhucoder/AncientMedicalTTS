#!/usr/bin/env python3
"""Validate an AncientMedicalTTS Publication Bundle v1 without app dependencies."""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path, PurePosixPath
from typing import Any

FORMAT = "ancient-medical-publication-bundle"
FORMAT_VERSION = "1.0"
TIMELINE_TOLERANCE_MS = 100


class BundleValidationError(ValueError):
    """Raised when a publication bundle is not safe or internally consistent."""


def _read_json(path: Path) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as exc:
        raise BundleValidationError(f"无法读取 JSON：{path}: {exc}") from exc
    if not isinstance(value, dict):
        raise BundleValidationError(f"JSON 顶层必须是对象：{path}")
    return value


def _resource_path(value: Any) -> Path:
    if not isinstance(value, str) or not value:
        raise BundleValidationError("资源路径不能为空")
    if "\\" in value or ":" in value:
        raise BundleValidationError(f"资源路径不是安全的相对路径：{value}")
    posix = PurePosixPath(value)
    if posix.is_absolute() or any(part in {"", ".", ".."} for part in posix.parts):
        raise BundleValidationError(f"资源路径不是安全的相对路径：{value}")
    return Path(*posix.parts)


def _required(mapping: dict[str, Any], key: str) -> Any:
    if key not in mapping:
        raise BundleValidationError(f"缺少字段：{key}")
    return mapping[key]


def validate_bundle(root: Path) -> dict[str, int | str]:
    root = root.resolve()
    manifest = _read_json(root / "manifest.json")
    if manifest.get("format") != FORMAT or manifest.get("format_version") != FORMAT_VERSION:
        raise BundleValidationError("format 或 format_version 无效")
    if manifest.get("book") != "book.json":
        raise BundleValidationError("manifest.book 必须指向 book.json")
    book = _read_json(root / "book.json")
    chapters = _required(book, "chapters")
    if not isinstance(chapters, list) or not chapters:
        raise BundleValidationError("book.chapters 不能为空")

    chapter_ids: set[str] = set()
    segment_ids: set[str] = set()
    previous_chapter_order = 0
    segment_count = 0
    speakable_count = 0
    for chapter_ref in chapters:
        if not isinstance(chapter_ref, dict):
            raise BundleValidationError("章节引用必须是对象")
        chapter_id = _required(chapter_ref, "id")
        order = _required(chapter_ref, "order")
        if not isinstance(chapter_id, str) or chapter_id in chapter_ids or not isinstance(order, int) or order <= previous_chapter_order:
            raise BundleValidationError("章节 ID 或顺序无效")
        chapter_ids.add(chapter_id)
        previous_chapter_order = order
        content = _resource_path(_required(chapter_ref, "content"))
        audio = _resource_path(_required(chapter_ref, "audio"))
        if not (root / content).is_file() or not (root / audio).is_file():
            raise BundleValidationError("章节 JSON 或音频文件不存在")
        chapter = _read_json(root / content)
        if chapter.get("id") != chapter_id or chapter.get("audio", {}).get("src") != chapter_ref["audio"]:
            raise BundleValidationError("book.json 与章节 JSON 不一致")
        chapter_audio = chapter.get("audio")
        if not isinstance(chapter_audio, dict) or not isinstance(chapter_audio.get("duration_ms"), int):
            raise BundleValidationError("章节音频时长无效")
        segments = chapter.get("segments")
        if not isinstance(segments, list) or not segments:
            raise BundleValidationError("章节 segments 不能为空")
        previous_segment_order = 0
        previous_end = 0
        last_end = 0
        for segment in segments:
            if not isinstance(segment, dict):
                raise BundleValidationError("Segment 必须是对象")
            segment_id = _required(segment, "id")
            segment_order = _required(segment, "order")
            if not isinstance(segment_id, str) or segment_id in segment_ids or not isinstance(segment_order, int) or segment_order <= previous_segment_order:
                raise BundleValidationError("Segment ID 或顺序无效")
            segment_ids.add(segment_id)
            previous_segment_order = segment_order
            segment_count += 1
            if segment.get("speak_enabled"):
                speakable_count += 1
                start = segment.get("start_ms")
                end = segment.get("end_ms")
                duration = segment.get("duration_ms")
                if not all(isinstance(value, int) for value in (start, end, duration)):
                    raise BundleValidationError("可朗读 Segment 缺少完整时间轴")
                if start < previous_end or end < start or end - start != duration:
                    raise BundleValidationError("Segment 时间轴重叠或不单调")
                previous_end = end
                last_end = end
            elif any(segment.get(key) is not None for key in ("start_ms", "end_ms", "duration_ms")):
                raise BundleValidationError("不朗读 Segment 不应有时间轴")
        audio_duration = chapter_audio["duration_ms"]
        if last_end > audio_duration or audio_duration - last_end > TIMELINE_TOLERANCE_MS:
            raise BundleValidationError("章节末尾时间轴与音频时长差异过大")

    for json_path in root.rglob("*.json"):
        serialized = json_path.read_text(encoding="utf-8")
        if any(secret in serialized for secret in ("SecretId", "SecretKey", "secret_id", "secret_key")):
            raise BundleValidationError("发布包不能包含云凭证")
    return {
        "format_version": FORMAT_VERSION,
        "chapters": len(chapters),
        "segments": segment_count,
        "speakable_segments": speakable_count,
    }


def main() -> int:
    parser = argparse.ArgumentParser(description="验证 AncientMedicalTTS Publication Bundle v1")
    parser.add_argument("bundle", type=Path)
    args = parser.parse_args()
    try:
        summary = validate_bundle(args.bundle)
    except BundleValidationError as exc:
        print(f"无效发布包：{exc}", file=sys.stderr)
        return 1
    print(json.dumps(summary, ensure_ascii=False, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
