from __future__ import annotations

import sys
from pathlib import Path
import unittest


ROOT = Path(__file__).parents[2]
sys.path.insert(0, str(ROOT / "tools"))

from validate_pronunciation_lexicon import validate  # noqa: E402


class PronunciationLexiconValidationTests(unittest.TestCase):
    def test_v3_resources_are_valid(self) -> None:
        result = validate()
        self.assertEqual(result.errors, ())
        self.assertGreaterEqual(result.lexicon_entries, 100)
        self.assertGreaterEqual(result.semantic_rules, 10)


if __name__ == "__main__":
    unittest.main()
