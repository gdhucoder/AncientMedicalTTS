from __future__ import annotations

import re
import unicodedata
from typing import Any

from .base import PronunciationOverride, TTSError

_PINYIN_PATTERN = re.compile(r"^[a-z]+[1-5](?: [a-z]+[1-5])*$")
_XML_ESCAPES = {"&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&apos;"}


def build_ssml(text: str, raw_tokens: list[dict[str, object]], overrides: list[PronunciationOverride]) -> tuple[str, bool]:
    tokens = _validate_tokens(text, raw_tokens)
    if not overrides:
        return text, False
    ranges = sorted(overrides, key=lambda item: (item.start_token, item.end_token))
    by_start: dict[int, PronunciationOverride] = {}
    for override in ranges:
        _validate_override(override, tokens)
        if override.start_token in by_start:
            raise TTSError("TTS_INVALID_SSML", "Annotation 起点重复")
        if any(_overlap(override.start_token, override.end_token, previous.start_token, previous.end_token) for previous in by_start.values()):
            raise TTSError("TTS_INVALID_SSML", "pronunciation override 范围重叠")
        by_start[override.start_token] = override

    output: list[str] = ["<speak>"]
    position = 0
    while position < len(tokens):
        override = by_start.get(position)
        if override is not None:
            surface = "".join(tokens[index]["text"] for index in range(override.start_token, override.end_token))
            output.append(f'<phoneme alphabet="py" ph="{_escape_xml(override.pinyin)}">{_escape_xml(surface)}</phoneme>')
            position = override.end_token
        else:
            output.append(_escape_xml(str(tokens[position]["text"])))
            position += 1
    output.append("</speak>")
    return "".join(output), True


def _validate_tokens(text: str, raw_tokens: list[dict[str, object]]) -> list[dict[str, object]]:
    if not isinstance(text, str) or not isinstance(raw_tokens, list):
        raise TTSError("TTS_INVALID_SSML", "text 和 tokens 参数无效")
    tokens: list[dict[str, object]] = []
    for expected_index, raw in enumerate(raw_tokens):
        if not isinstance(raw, dict) or isinstance(raw.get("index"), bool) or raw.get("index") != expected_index or not isinstance(raw.get("text"), str) or not raw["text"]:
            raise TTSError("TTS_INVALID_SSML", "tokens index 必须连续，且 token text 不能为空")
        tokens.append(raw)
    if "".join(str(token["text"]) for token in tokens) != text:
        raise TTSError("TTS_INVALID_SSML", "tokens 与 text 不一致")
    return tokens


def _validate_override(override: PronunciationOverride, tokens: list[dict[str, object]]) -> None:
    if override.start_token < 0 or override.start_token >= override.end_token or override.end_token > len(tokens):
        raise TTSError("TTS_INVALID_SSML", "pronunciation override 范围无效")
    surface = "".join(str(tokens[index]["text"]) for index in range(override.start_token, override.end_token))
    if surface != override.surface_text:
        raise TTSError("TTS_INVALID_SSML", "surface_text 与 token 范围不一致")
    if not _PINYIN_PATTERN.fullmatch(override.pinyin):
        raise TTSError("TTS_INVALID_SSML", "拼音必须使用小写 ASCII 数字声调格式")
    han_count = sum(1 for character in override.surface_text if _is_han(character))
    if han_count != len(override.pinyin.split()):
        raise TTSError("PINYIN_TOKEN_COUNT_MISMATCH", "拼音音节数量与汉字数量不一致")


def _is_han(character: str) -> bool:
    name = unicodedata.name(character, "")
    return "CJK UNIFIED IDEOGRAPH" in name or "CJK COMPATIBILITY IDEOGRAPH" in name


def _overlap(a_start: int, a_end: int, b_start: int, b_end: int) -> bool:
    return a_start < b_end and b_start < a_end


def _escape_xml(value: str) -> str:
    return "".join(_XML_ESCAPES.get(character, character) for character in value)
