"""AncientMedicalTTS pronunciation worker.

The process speaks JSON Lines on stdout. Diagnostics must stay on stderr.
"""

from __future__ import annotations

import json
import sys
from typing import Any

VERSION = "0.2.0"


def response(request_id: str, *, result: dict[str, Any] | None = None,
             code: str | None = None, message: str | None = None) -> str:
    if code is None:
        payload: dict[str, Any] = {"id": request_id, "ok": True, "result": result or {}}
    else:
        payload = {"id": request_id, "ok": False, "error": {"code": code, "message": message or code}}
    return json.dumps(payload, ensure_ascii=False, separators=(",", ":"))


def handle_line(line: str) -> str | None:
    try:
        payload = json.loads(line)
    except json.JSONDecodeError:
        return response("", code="INVALID_JSON", message="request must be valid JSON")

    if not isinstance(payload, dict):
        return response("", code="INVALID_REQUEST", message="request must be a JSON object")

    request_id = payload.get("id")
    method = payload.get("method")
    if not isinstance(request_id, str) or not isinstance(method, str):
        return response(request_id if isinstance(request_id, str) else "", code="INVALID_REQUEST", message="id and method are required")

    if method == "system.ping":
        return response(request_id, result={"version": VERSION})
    if method == "pronunciation.analyze":
        try:
            from pronunciation.analyzer import AnalysisError, analyze
            result = analyze(payload.get("params"))
            for warning in result.get("warnings", []):
                if isinstance(warning, dict):
                    print(f"{warning.get('code', 'ANALYZER_WARNING')}: {warning.get('message', '')}", file=sys.stderr)
            return response(request_id, result=result)
        except Exception as error:  # Keep a malformed request from killing the Worker.
            if error.__class__.__name__ == "AnalysisError" and isinstance(getattr(error, "code", None), str):
                return response(request_id, code=error.code, message=str(error))
            print(f"pronunciation.analyze failed: {error}", file=sys.stderr)
            return response(request_id, code="PRONUNCIATION_ANALYSIS_FAILED", message="发音分析失败")
    if method == "pronunciation.display_pinyin":
        try:
            from pronunciation.analyzer import AnalysisError, display_pinyin
            return response(request_id, result=display_pinyin(payload.get("params")))
        except Exception as error:
            if error.__class__.__name__ == "AnalysisError" and isinstance(getattr(error, "code", None), str):
                return response(request_id, code=error.code, message=str(error))
            print(f"pronunciation.display_pinyin failed: {error}", file=sys.stderr)
            return response(request_id, code="PRONUNCIATION_DISPLAY_FAILED", message="全文拼音生成失败")
    if method in {"tts.test_connection", "tts.synthesize"}:
        try:
            from tts.base import TTSError
            from tts.service import synthesize, test_connection
            result = test_connection(payload.get("params"), request_id) if method == "tts.test_connection" else synthesize(payload.get("params"), request_id)
            return response(request_id, result=result)
        except Exception as error:
            if error.__class__.__name__ == "TTSError" and isinstance(getattr(error, "code", None), str):
                return response(request_id, code=error.code, message=str(error))
            print(f"{method} failed: {error}", file=sys.stderr)
            return response(request_id, code="TTS_PROVIDER_ERROR", message="TTS 请求失败")
    return response(request_id, code="METHOD_NOT_FOUND", message=f"unknown method: {method}")


def main() -> None:
    for raw_line in sys.stdin:
        line = raw_line.strip()
        if not line:
            continue
        output = handle_line(line)
        if output is not None:
            sys.stdout.write(output + "\n")
            sys.stdout.flush()


if __name__ == "__main__":
    main()
