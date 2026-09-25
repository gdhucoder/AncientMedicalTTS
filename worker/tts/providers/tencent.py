from __future__ import annotations

import base64
import binascii
import sys
from pathlib import Path

from tencentcloud.common import credential
from tencentcloud.common.exception.tencent_cloud_sdk_exception import TencentCloudSDKException
from tencentcloud.common.profile.client_profile import ClientProfile
from tencentcloud.common.profile.http_profile import HttpProfile
from tencentcloud.tts.v20190823 import models, tts_client

from ..base import SynthesisRequest, SynthesisResult, TTSError
from ..ssml import build_ssml

ENDPOINT = "tts.tencentcloudapi.com"


class TencentTTSProvider:
    def __init__(self, secret_id: str, secret_key: str, region: str = "ap-guangzhou") -> None:
        self._secret_id = secret_id
        self._secret_key = secret_key
        self._region = region

    def _client(self) -> tts_client.TtsClient:
        http_profile = HttpProfile()
        http_profile.endpoint = ENDPOINT
        client_profile = ClientProfile()
        client_profile.httpProfile = http_profile
        return tts_client.TtsClient(credential.Credential(self._secret_id, self._secret_key), self._region, client_profile)

    def _call(
        self,
        *,
        text: str,
        voice_type: int,
        sample_rate: int,
        codec: str,
        speed: float,
        volume: float,
        session_id: str,
        enable_subtitle: bool,
    ) -> tuple[str, str, str, list[dict[str, object]] | None]:
        request = models.TextToVoiceRequest()
        request.Text = text
        request.SessionId = session_id
        request.Volume = volume
        request.Speed = speed
        request.ProjectId = 0
        request.ModelType = 1
        request.VoiceType = voice_type
        request.PrimaryLanguage = 1
        request.SampleRate = sample_rate
        request.Codec = codec
        request.EnableSubtitle = enable_subtitle
        try:
            response = self._client().TextToVoice(request)
        except TencentCloudSDKException as error:
            code = _map_error_code(error)
            provider_code = str(getattr(error, "code", "UNKNOWN"))
            print(f"Tencent TTS request failed: code={provider_code}, mapped={code}", file=sys.stderr)
            raise TTSError(code, _friendly_error_message(code)) from error
        if not isinstance(response.Audio, str) or not response.Audio:
            raise TTSError("TTS_PROVIDER_ERROR", "腾讯云未返回音频数据")
        realized_pronunciation = _normalize_subtitles(getattr(response, "Subtitles", None)) if enable_subtitle else None
        return response.Audio, response.RequestId or "", response.SessionId or session_id, realized_pronunciation

    def test_connection(self, *, voice_type: int, sample_rate: int, request_id: str) -> dict[str, object]:
        self._call(text="测试", voice_type=voice_type, sample_rate=sample_rate, codec="wav", speed=0.0, volume=0.0, session_id=request_id, enable_subtitle=False)
        return {"provider": "tencent", "request_id": request_id, "voice_type": voice_type}

    def synthesize(self, request: SynthesisRequest) -> SynthesisResult:
        if len(request.text) > 150:
            raise TTSError("TTS_TEXT_TOO_LONG", "当前 Segment 超过腾讯基础 TTS 的 150 字限制")
        text, ssml_used = build_ssml(request.text, request.tokens, request.pronunciations)
        audio, provider_request_id, session_id, realized_pronunciation = self._call(
            text=text,
            voice_type=request.voice_type,
            sample_rate=request.sample_rate,
            codec=request.codec,
            speed=request.speed,
            volume=request.volume,
            session_id=request.session_id,
            enable_subtitle=True,
        )
        try:
            audio_bytes = base64.b64decode(audio, validate=True)
        except (binascii.Error, ValueError) as error:
            raise TTSError("TTS_PROVIDER_ERROR", "腾讯云返回的 Audio 不是有效 Base64") from error
        if not audio_bytes:
            raise TTSError("TTS_PROVIDER_ERROR", "腾讯云返回空音频")
        output_path = Path(request.output_path)
        try:
            output_path.write_bytes(audio_bytes)
        except OSError as error:
            raise TTSError("FILE_IO_ERROR", f"写入 WAV 失败: {error}") from error
        return SynthesisResult(provider="tencent", request_id=provider_request_id, session_id=session_id, output_path=output_path, byte_length=len(audio_bytes), ssml_used=ssml_used, ssml=text if ssml_used else None, realized_pronunciation=realized_pronunciation)


def _normalize_subtitles(value: object) -> list[dict[str, object]] | None:
    if not isinstance(value, list):
        return None
    normalized: list[dict[str, object]] = []
    for item in value:
        text = _subtitle_text(item, "Text")
        phoneme = _subtitle_text(item, "Phoneme")
        begin_ms = _subtitle_integer(item, "BeginTime")
        end_ms = _subtitle_integer(item, "EndTime")
        if text is None and phoneme is None and begin_ms is None and end_ms is None:
            continue
        normalized.append({
            "text": text or "",
            "phoneme": phoneme,
            "begin_ms": begin_ms,
            "end_ms": end_ms,
        })
    return normalized or None


def _subtitle_value(item: object, name: str) -> object:
    if isinstance(item, dict):
        return item.get(name)
    return getattr(item, name, None)


def _subtitle_text(item: object, name: str) -> str | None:
    value = _subtitle_value(item, name)
    return value.strip() if isinstance(value, str) and value.strip() else None


def _subtitle_integer(item: object, name: str) -> int | None:
    value = _subtitle_value(item, name)
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        return None
    return int(value)


def _map_error_code(error: TencentCloudSDKException) -> str:
    code = str(getattr(error, "code", ""))
    lowered = code.lower()
    if "auth" in lowered or "credential" in lowered or "secret" in lowered:
        return "TTS_AUTH_ERROR"
    if "notopen" in lowered or "notenabled" in lowered or "unsupportedoperation" in lowered:
        return "TTS_SERVICE_NOT_ENABLED"
    if "limit" in lowered or "frequency" in lowered or "ratelimit" in lowered:
        return "TTS_RATE_LIMITED"
    if "quota" in lowered or "balance" in lowered:
        return "TTS_QUOTA_EXHAUSTED"
    if "text" in lowered and "long" in lowered:
        return "TTS_TEXT_TOO_LONG"
    if "ssml" in lowered:
        return "TTS_INVALID_SSML"
    if "voice" in lowered:
        return "TTS_INVALID_VOICE"
    return "TTS_PROVIDER_ERROR"


def _friendly_error_message(code: str) -> str:
    return {
        "TTS_AUTH_ERROR": "腾讯云凭据无效或没有调用权限",
        "TTS_SERVICE_NOT_ENABLED": "腾讯云语音服务尚未开通",
        "TTS_RATE_LIMITED": "腾讯云请求过于频繁，请稍后重试",
        "TTS_QUOTA_EXHAUSTED": "腾讯云 TTS 配额或余额不足",
        "TTS_TEXT_TOO_LONG": "文本超过腾讯云当前接口限制",
        "TTS_INVALID_SSML": "腾讯云不接受当前 SSML",
        "TTS_INVALID_VOICE": "当前腾讯云音色无效",
    }.get(code, "腾讯云 TTS 请求失败")
