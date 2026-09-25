from __future__ import annotations

import json
import os
import tempfile
import unittest
from pathlib import Path
import sys

sys.path.insert(0, str(Path(__file__).parents[1]))
from main import handle_line


RUN_INTEGRATION = os.environ.get("RUN_TENCENT_TTS_INTEGRATION") == "1"
HAS_CREDENTIALS = bool(
    os.environ.get("ANCIENT_TTS_TENCENT_SECRET_ID")
    and os.environ.get("ANCIENT_TTS_TENCENT_SECRET_KEY")
)


@unittest.skipUnless(
    RUN_INTEGRATION and HAS_CREDENTIALS,
    "set RUN_TENCENT_TTS_INTEGRATION=1 and Tencent credentials to run live API checks",
)
class TencentIntegrationTests(unittest.TestCase):
    def test_connection_and_pronunciation_variants(self) -> None:
        connection = json.loads(
            handle_line(
                json.dumps(
                    {
                        "id": "live-connection",
                        "method": "tts.test_connection",
                        "params": {"provider": "tencent", "voice_type": 501000, "sample_rate": 16000},
                    }
                )
            )
            or ""
        )
        self.assertTrue(connection["ok"], connection)
        cases = (
            ("不亦说乎。", []),
            ("不亦说乎。", [{"start_token": 2, "end_token": 3, "surface_text": "说", "pinyin": "yue4"}]),
            ("不亦说乎。", [{"start_token": 2, "end_token": 3, "surface_text": "说", "pinyin": "shuo1"}]),
            ("行医。", [{"start_token": 0, "end_token": 1, "surface_text": "行", "pinyin": "xing2"}]),
            ("行医。", [{"start_token": 0, "end_token": 1, "surface_text": "行", "pinyin": "hang2"}]),
            ("腧穴。", [{"start_token": 0, "end_token": 2, "surface_text": "腧穴", "pinyin": "shu4 xue2"}]),
        )
        with tempfile.TemporaryDirectory() as directory:
            for index, (text, pronunciations) in enumerate(cases, start=1):
                output = Path(directory) / f"segment-{index}.wav"
                result = json.loads(
                    handle_line(
                        json.dumps(
                            {
                                "id": f"live-{index}",
                                "method": "tts.synthesize",
                                "params": {
                                    "provider": "tencent",
                                    "text": text,
                                    "tokens": [{"index": token_index, "text": character} for token_index, character in enumerate(text)],
                                    "pronunciations": pronunciations,
                                    "voice_type": 501000,
                                    "sample_rate": 16000,
                                    "codec": "wav",
                                    "speed": 0,
                                    "volume": 0,
                                    "session_id": f"live-session-{index}",
                                    "output_path": str(output),
                                },
                            }
                        )
                    )
                    or ""
                )
                self.assertTrue(result["ok"], result)
                self.assertTrue(output.read_bytes().startswith(b"RIFF"))


if __name__ == "__main__":
    unittest.main()
