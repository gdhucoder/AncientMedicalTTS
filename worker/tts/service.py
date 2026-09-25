from __future__ import annotations

import os
from pathlib import Path
from typing import Any

from .base import PronunciationOverride, SynthesisRequest, TTSError
from .providers.tencent import TencentTTSProvider


def test_connection(params: Any, request_id: str) -> dict[str, object]:
    provider_name = _provider_name(params)
    voice_type = _integer(params, "voice_type")
    sample_rate = _integer(params, "sample_rate")
    if sample_rate != 16000:
        raise TTSError("TTS_INVALID_FORMAT", "当前只支持 16000Hz")
    return _provider(provider_name).test_connection(voice_type=voice_type, sample_rate=sample_rate, request_id=request_id)


def synthesize(params: Any, request_id: str) -> dict[str, object]:
    if not isinstance(params, dict):
        raise TTSError("TTS_PROVIDER_ERROR", "params 必须是对象")
    provider_name = _provider_name(params)
    raw_tokens = params.get("tokens")
    raw_overrides = params.get("pronunciations")
    if not isinstance(params.get("text"), str) or not isinstance(raw_tokens, list) or not isinstance(raw_overrides, list):
        raise TTSError("TTS_PROVIDER_ERROR", "text、tokens、pronunciations 参数不完整")
    overrides = [_override(value) for value in raw_overrides]
    output_path = params.get("output_path")
    if not isinstance(output_path, str) or not output_path:
        raise TTSError("TTS_PROVIDER_ERROR", "output_path 必须由 Rust 提供")
    request = SynthesisRequest(
        request_id=request_id,
        text=params["text"],
        tokens=raw_tokens,
        pronunciations=overrides,
        voice_type=_integer(params, "voice_type"),
        sample_rate=_integer(params, "sample_rate"),
        codec=_string(params, "codec"),
        speed=_number(params, "speed"),
        volume=_number(params, "volume"),
        session_id=_string(params, "session_id"),
        output_path=Path(output_path),
    )
    if request.sample_rate != 16000 or request.codec != "wav":
        raise TTSError("TTS_INVALID_FORMAT", "当前只支持 16000Hz WAV")
    if not -2 <= request.speed <= 6:
        raise TTSError("TTS_INVALID_SPEED", "Speed 必须在 -2 到 6 之间")
    if not -10 <= request.volume <= 10:
        raise TTSError("TTS_INVALID_VOLUME", "Volume 必须在 -10 到 10 之间")
    result = _provider(provider_name).synthesize(request)
    return {"provider": result.provider, "request_id": result.request_id, "session_id": result.session_id, "output_path": str(result.output_path), "byte_length": result.byte_length, "ssml_used": result.ssml_used, "ssml": result.ssml, "duration_ms": result.duration_ms}


def _provider(name: str) -> TencentTTSProvider:
    if name != "tencent":
        raise TTSError("TTS_PROVIDER_UNSUPPORTED", "当前仅支持腾讯云 TTS")
    secret_id = os.environ.get("ANCIENT_TTS_TENCENT_SECRET_ID", "").strip()
    secret_key = os.environ.get("ANCIENT_TTS_TENCENT_SECRET_KEY", "").strip()
    if not secret_id or not secret_key:
        raise TTSError("TTS_CREDENTIALS_MISSING", "腾讯云 SecretId / SecretKey 尚未配置")
    return TencentTTSProvider(secret_id, secret_key)


def _provider_name(params: Any) -> str:
    if not isinstance(params, dict) or params.get("provider") != "tencent":
        raise TTSError("TTS_PROVIDER_UNSUPPORTED", "当前仅支持 provider=tencent")
    return "tencent"


def _override(value: Any) -> PronunciationOverride:
    if not isinstance(value, dict) or any(key not in value for key in ("start_token", "end_token", "surface_text", "pinyin")):
        raise TTSError("TTS_INVALID_SSML", "pronunciation override 参数不完整")
    if any(isinstance(value.get(key), bool) or not isinstance(value.get(key), int) for key in ("start_token", "end_token")) or not isinstance(value["surface_text"], str) or not isinstance(value["pinyin"], str):
        raise TTSError("TTS_INVALID_SSML", "pronunciation override 参数类型无效")
    return PronunciationOverride(value["start_token"], value["end_token"], value["surface_text"], value["pinyin"])


def _integer(params: dict[str, Any], key: str) -> int:
    value = params.get(key)
    if isinstance(value, bool) or not isinstance(value, int):
        raise TTSError("TTS_PROVIDER_ERROR", f"{key} 必须是整数")
    return value


def _number(params: dict[str, Any], key: str) -> float:
    value = params.get(key)
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise TTSError("TTS_PROVIDER_ERROR", f"{key} 必须是数字")
    return float(value)


def _string(params: dict[str, Any], key: str) -> str:
    value = params.get(key)
    if not isinstance(value, str) or not value:
        raise TTSError("TTS_PROVIDER_ERROR", f"{key} 必须是非空字符串")
    return value
