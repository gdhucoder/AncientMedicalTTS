from __future__ import annotations

import base64
import json
import os
import tempfile
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import patch

from main import handle_line
from tts.base import PronunciationOverride, SynthesisRequest, TTSError
from tts.providers.tencent import TencentTTSProvider, _map_error_code, _normalize_subtitles
from tts.ssml import build_ssml


class SsmlTests(unittest.TestCase):
    def test_plain_text_has_no_ssml_wrapper_without_override(self) -> None:
        ssml, used = build_ssml("学而", [{"index": 0, "text": "学"}, {"index": 1, "text": "而"}], [])
        self.assertEqual(ssml, "学而")
        self.assertFalse(used)

    def test_single_and_multi_token_phonemes_are_escaped(self) -> None:
        text = "不亦说乎。腧穴&A"
        tokens = [{"index": index, "text": character} for index, character in enumerate(text)]
        ssml, used = build_ssml(text, tokens, [
            PronunciationOverride(2, 3, "说", "yue4"),
            PronunciationOverride(5, 7, "腧穴", "shu4 xue2"),
        ])
        self.assertTrue(used)
        self.assertIn('<phoneme alphabet="py" ph="yue4">说</phoneme>', ssml)
        self.assertIn('<phoneme alphabet="py" ph="shu4 xue2">腧穴</phoneme>', ssml)
        self.assertIn("&amp;A", ssml)
        self.assertTrue(ssml.startswith("<speak>"))
        self.assertTrue(ssml.endswith("</speak>"))

    def test_overlap_surface_and_pinyin_count_are_rejected(self) -> None:
        tokens = [{"index": 0, "text": "腧"}, {"index": 1, "text": "穴"}]
        with self.assertRaises(TTSError) as mismatch:
            build_ssml("腧穴", tokens, [PronunciationOverride(0, 2, "腧穴", "shu4")])
        self.assertEqual(mismatch.exception.code, "PINYIN_TOKEN_COUNT_MISMATCH")
        with self.assertRaises(TTSError) as overlap:
            build_ssml("腧穴", tokens, [PronunciationOverride(0, 1, "腧", "shu4"), PronunciationOverride(0, 2, "腧穴", "shu4 xue2")])
        self.assertEqual(overlap.exception.code, "TTS_INVALID_SSML")

    def test_special_xml_characters_and_mismatch_are_checked(self) -> None:
        with self.assertRaises(TTSError):
            build_ssml("A&B", [{"index": 0, "text": "A"}], [])
        ssml, _ = build_ssml("腧&A", [{"index": 0, "text": "腧"}, {"index": 1, "text": "&"}, {"index": 2, "text": "A"}], [PronunciationOverride(0, 1, "腧", "shu4")])
        self.assertIn("&amp;A", ssml)

    def test_two_non_adjacent_overrides_and_cjk_extension(self) -> None:
        text = "学𠀀说乎"
        tokens = [{"index": index, "text": character} for index, character in enumerate(text)]
        ssml, used = build_ssml(
            text,
            tokens,
            [
                PronunciationOverride(0, 1, "学", "xue2"),
                PronunciationOverride(2, 3, "说", "yue4"),
            ],
        )
        self.assertTrue(used)
        self.assertIn("𠀀", ssml)
        self.assertIn('ph="xue2">学</phoneme>', ssml)
        self.assertIn('ph="yue4">说</phoneme>', ssml)

    def test_contextual_character_can_be_forced_to_e4(self) -> None:
        text = "遂恶之"
        tokens = [{"index": index, "text": character} for index, character in enumerate(text)]
        ssml, used = build_ssml(
            text,
            tokens,
            [PronunciationOverride(1, 2, "恶", "e4")],
        )
        self.assertTrue(used)
        self.assertIn('<phoneme alphabet="py" ph="e4">恶</phoneme>', ssml)

    def test_unconfirmed_reference_does_not_create_a_phoneme(self) -> None:
        ssml, used = build_ssml("恶寒", [{"index": 0, "text": "恶"}, {"index": 1, "text": "寒"}], [])
        self.assertEqual(ssml, "恶寒")
        self.assertFalse(used)


class TencentProviderTests(unittest.TestCase):
    def test_request_uses_frozen_text_to_voice_parameters(self) -> None:
        class FakeClient:
            def TextToVoice(self, request: object) -> SimpleNamespace:
                self.request = request
                return SimpleNamespace(Audio=base64.b64encode(b"wav").decode(), RequestId="rid", SessionId="sid", Subtitles=None)

        client = FakeClient()
        request = SynthesisRequest("request", "测试", [{"index": 0, "text": "测"}, {"index": 1, "text": "试"}], [], 501000, 16000, "wav", 0.0, 0.0, "session", Path(tempfile.gettempdir()) / "ancient-test-tts-params.wav")
        try:
            with patch.object(TencentTTSProvider, "_client", return_value=client):
                TencentTTSProvider("id", "key").synthesize(request)
            sent = client.request
            self.assertEqual(sent.Text, "测试")
            self.assertEqual(sent.SessionId, "session")
            self.assertEqual(sent.Speed, 0.0)
            self.assertEqual(sent.Volume, 0.0)
            self.assertEqual(sent.VoiceType, 501000)
            self.assertEqual(sent.ProjectId, 0)
            self.assertEqual(sent.ModelType, 1)
            self.assertEqual(sent.PrimaryLanguage, 1)
            self.assertEqual(sent.SampleRate, 16000)
            self.assertEqual(sent.Codec, "wav")
            self.assertTrue(sent.EnableSubtitle)
        finally:
            request.output_path.unlink(missing_ok=True)

    def test_provider_decodes_audio_and_writes_output(self) -> None:
        wav_bytes = b"RIFF" + b"\x00" * 4 + b"WAVE" + b"\x00" * 36
        request = SynthesisRequest("request", "说", [{"index": 0, "text": "说"}], [PronunciationOverride(0, 1, "说", "yue4")], 501000, 16000, "wav", 0.0, 0.0, "session", Path(tempfile.gettempdir()) / "ancient-test-tts.wav")
        try:
            with patch.object(TencentTTSProvider, "_call", return_value=(base64.b64encode(wav_bytes).decode(), "provider-request", "session", None)):
                result = TencentTTSProvider("id", "key").synthesize(request)
            self.assertEqual(result.request_id, "provider-request")
            self.assertEqual(request.output_path.read_bytes(), wav_bytes)
        finally:
            request.output_path.unlink(missing_ok=True)

    def test_provider_preserves_actual_subtitle_phonemes_in_result(self) -> None:
        wav_bytes = b"RIFF" + b"\x00" * 4 + b"WAVE" + b"\x00" * 36

        class FakeProvider(TencentTTSProvider):
            def _call(self, **_: object) -> tuple[str, str, str, list[dict[str, object]]]:
                return (
                    base64.b64encode(wav_bytes).decode(),
                    "provider-request",
                    "session",
                    [{"text": "恶", "phoneme": "wu4", "begin_ms": 210, "end_ms": 430}],
                )

        request = SynthesisRequest("request", "恶", [{"index": 0, "text": "恶"}], [], 501000, 16000, "wav", 0.0, 0.0, "session", Path(tempfile.gettempdir()) / "ancient-test-tts-realized.wav")
        try:
            result = FakeProvider("id", "key").synthesize(request)
            self.assertEqual(result.realized_pronunciation, [{"text": "恶", "phoneme": "wu4", "begin_ms": 210, "end_ms": 430}])
        finally:
            request.output_path.unlink(missing_ok=True)

    def test_subtitles_are_normalized_to_provider_neutral_metadata(self) -> None:
        subtitles = [
            SimpleNamespace(Text="恶", Phoneme="wu4", BeginTime=210, EndTime=430),
            SimpleNamespace(Text="寒", Phoneme="han2", BeginTime=430, EndTime=650),
        ]
        self.assertEqual(
            _normalize_subtitles(subtitles),
            [
                {"text": "恶", "phoneme": "wu4", "begin_ms": 210, "end_ms": 430},
                {"text": "寒", "phoneme": "han2", "begin_ms": 430, "end_ms": 650},
            ],
        )
        self.assertIsNone(_normalize_subtitles(None))

    def test_invalid_base64_and_empty_audio_are_provider_errors(self) -> None:
        request = SynthesisRequest("request", "测", [{"index": 0, "text": "测"}], [], 501000, 16000, "wav", 0.0, 0.0, "session", Path(tempfile.gettempdir()) / "ancient-test-tts-invalid.wav")
        try:
            with patch.object(TencentTTSProvider, "_call", return_value=("not-base64", "", "session", None)):
                with self.assertRaisesRegex(TTSError, "不是有效 Base64"):
                    TencentTTSProvider("id", "key").synthesize(request)
            with patch.object(TencentTTSProvider, "_call", return_value=("", "", "session", None)):
                with self.assertRaisesRegex(TTSError, "返回空音频"):
                    TencentTTSProvider("id", "key").synthesize(request)
        finally:
            request.output_path.unlink(missing_ok=True)

    def test_tencent_errors_are_mapped_to_stable_codes(self) -> None:
        self.assertEqual(_map_error_code(SimpleNamespace(code="AuthFailure.SecretIdNotMatch")), "TTS_AUTH_ERROR")
        self.assertEqual(_map_error_code(SimpleNamespace(code="UnsupportedOperation.ServerNotOpen")), "TTS_SERVICE_NOT_ENABLED")
        self.assertEqual(_map_error_code(SimpleNamespace(code="RequestLimitExceeded")), "TTS_RATE_LIMITED")
        self.assertEqual(_map_error_code(SimpleNamespace(code="InvalidParameterValue.TextTooLong")), "TTS_TEXT_TOO_LONG")

    def test_worker_reports_missing_credentials_without_crashing(self) -> None:
        with patch.dict(os.environ, {"ANCIENT_TTS_TENCENT_SECRET_ID": "", "ANCIENT_TTS_TENCENT_SECRET_KEY": ""}):
            payload = json.loads(handle_line(json.dumps({"id": "tts", "method": "tts.test_connection", "params": {"provider": "tencent", "voice_type": 501000, "sample_rate": 16000}})) or "")
        self.assertFalse(payload["ok"])
        self.assertEqual(payload["error"]["code"], "TTS_CREDENTIALS_MISSING")


if __name__ == "__main__":
    unittest.main()
