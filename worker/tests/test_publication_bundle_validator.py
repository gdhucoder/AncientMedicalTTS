from __future__ import annotations

import json
import tempfile
import unittest
from pathlib import Path

from tools.validate_publication_bundle import BundleValidationError, validate_bundle


class PublicationBundleValidatorTests(unittest.TestCase):
    def make_bundle(self, root: Path) -> None:
        (root / "chapters").mkdir()
        (root / "audio").mkdir()
        (root / "audio" / "chapter-001.mp3").write_bytes(b"test-audio")
        chapter = {
            "id": "chapter-1",
            "order": 1,
            "title": "上古天真论篇第一",
            "audio": {"src": "audio/chapter-001.mp3", "duration_ms": 3250, "format": "mp3"},
            "segments": [
                {"id": "s-1", "order": 1, "text": "甲", "speak_enabled": True, "start_ms": 0, "end_ms": 1000, "duration_ms": 1000, "pronunciation": None},
                {"id": "s-2", "order": 2, "text": "标题", "speak_enabled": False, "start_ms": None, "end_ms": None, "duration_ms": None, "pronunciation": None},
                {"id": "s-3", "order": 3, "text": "乙", "speak_enabled": True, "start_ms": 1000, "end_ms": 3250, "duration_ms": 2250, "pronunciation": None},
            ],
        }
        (root / "chapters" / "chapter-001.json").write_text(json.dumps(chapter, ensure_ascii=False), encoding="utf-8")
        book = {
            "id": "book-1",
            "title": "测试古籍",
            "author": None,
            "dynasty": None,
            "edition": None,
            "language": "zh-CN",
            "reading_mode": "modern_standard_mandarin",
            "audio": {"format": "mp3"},
            "chapters": [{"id": "chapter-1", "order": 1, "title": "上古天真论篇第一", "content": "chapters/chapter-001.json", "audio": "audio/chapter-001.mp3"}],
        }
        (root / "book.json").write_text(json.dumps(book, ensure_ascii=False), encoding="utf-8")
        manifest = {
            "format": "ancient-medical-publication-bundle",
            "format_version": "1.0",
            "generator": {"name": "AncientMedicalTTS", "version": "0.1.0"},
            "generated_at": "2026-09-28T00:00:00Z",
            "book": "book.json",
        }
        (root / "manifest.json").write_text(json.dumps(manifest), encoding="utf-8")

    def test_validates_reading_text_result_and_non_speaking_segment(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.make_bundle(root)
            summary = validate_bundle(root)
            self.assertEqual(summary["chapters"], 1)
            self.assertEqual(summary["segments"], 3)
            self.assertEqual(summary["speakable_segments"], 2)

    def test_rejects_path_traversal(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.make_bundle(root)
            book_path = root / "book.json"
            book = json.loads(book_path.read_text(encoding="utf-8"))
            book["chapters"][0]["audio"] = "../secret.mp3"
            book_path.write_text(json.dumps(book), encoding="utf-8")
            with self.assertRaises(BundleValidationError):
                validate_bundle(root)


if __name__ == "__main__":
    unittest.main()
