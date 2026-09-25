from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path
from typing import Protocol


class TTSError(Exception):
    def __init__(self, code: str, message: str) -> None:
        self.code = code
        super().__init__(message)


@dataclass(frozen=True)
class PronunciationOverride:
    start_token: int
    end_token: int
    surface_text: str
    pinyin: str


@dataclass(frozen=True)
class SynthesisRequest:
    request_id: str
    text: str
    tokens: list[dict[str, object]]
    pronunciations: list[PronunciationOverride]
    voice_type: int
    sample_rate: int
    codec: str
    speed: float
    volume: float
    session_id: str
    output_path: Path


@dataclass(frozen=True)
class SynthesisResult:
    provider: str
    request_id: str
    session_id: str
    output_path: Path
    byte_length: int
    ssml_used: bool
    ssml: str | None
    duration_ms: int | None = None


class TTSProvider(Protocol):
    def test_connection(self, *, voice_type: int, sample_rate: int, request_id: str) -> dict[str, object]:
        ...

    def synthesize(self, request: SynthesisRequest) -> SynthesisResult:
        ...
