from __future__ import annotations

import json
import sys
from pathlib import Path
import unittest

sys.path.insert(0, str(Path(__file__).parents[1]))
from main import handle_line  # noqa: E402
from pronunciation.analyzer import (  # noqa: E402
    KnowledgeMatch,
    Token,
    _candidates_for_token,
    _default_pinyin_by_token,
    _select_knowledge_matches,
)


class ProtocolTests(unittest.TestCase):
    def analyze_text(self, text: str) -> dict[str, object]:
        tokens = [{"index": index, "text": character} for index, character in enumerate(text)]
        payload = json.loads(handle_line(json.dumps({
            "id": f"analysis-{text}",
            "method": "pronunciation.analyze",
            "params": {"segment_id": "seg-test", "text": text, "tokens": tokens},
        }, ensure_ascii=False)) or "")
        self.assertTrue(payload["ok"])
        return payload["result"]

    def test_system_ping(self) -> None:
        payload = json.loads(handle_line(json.dumps({"id": "abc", "method": "system.ping", "params": {}})) or "")
        self.assertEqual(payload["id"], "abc")
        self.assertTrue(payload["ok"])
        self.assertEqual(payload["result"]["version"], "0.2.0")

    def test_unknown_method(self) -> None:
        payload = json.loads(handle_line(json.dumps({"id": "abc", "method": "unknown", "params": {}})) or "")
        self.assertFalse(payload["ok"])
        self.assertEqual(payload["error"]["code"], "METHOD_NOT_FOUND")

    def test_display_pinyin_returns_ascii_tone_numbers_for_han_tokens(self) -> None:
        text = "腧穴。"
        tokens = [{"index": index, "text": value} for index, value in enumerate(text)]
        payload = json.loads(handle_line(json.dumps({
            "id": "display",
            "method": "pronunciation.display_pinyin",
            "params": {"text": text, "tokens": tokens},
        }, ensure_ascii=False)) or "")
        self.assertTrue(payload["ok"])
        self.assertEqual(payload["result"]["token_pinyin"], ["shu4", "xue2", None])

    def test_tone_less_pypinyin_result_uses_valid_candidate_for_zi(self) -> None:
        token = Token(0, "子")
        self.assertEqual(_candidates_for_token("子"), ["zi3"])
        self.assertEqual(_default_pinyin_by_token("子", [token]), ["zi3"])

        payload = json.loads(handle_line(json.dumps({
            "id": "zi",
            "method": "pronunciation.analyze",
            "params": {
                "segment_id": "seg-zi",
                "text": "子",
                "tokens": [{"index": 0, "text": "子"}],
            },
        }, ensure_ascii=False)) or "")
        self.assertTrue(payload["ok"])
        self.assertEqual(payload["result"]["items"], [])

    def test_invalid_json_is_recoverable(self) -> None:
        payload = json.loads(handle_line("not-json") or "")
        self.assertFalse(payload["ok"])
        self.assertEqual(payload["error"]["code"], "INVALID_JSON")

    def test_pronunciation_analysis_uses_supplied_token_indices(self) -> None:
        text = "学而时习之，数至。"
        tokens = [{"index": index, "text": character} for index, character in enumerate(text)]
        payload = json.loads(handle_line(json.dumps({"id": "analysis", "method": "pronunciation.analyze", "params": {"segment_id": "seg-1", "text": text, "tokens": tokens}}, ensure_ascii=False)) or "")
        self.assertTrue(payload["ok"])
        self.assertEqual(payload["result"]["analyzer_version"], "0.3.0")
        self.assertEqual(payload["result"]["domain_lexicon_version"], "0.3.0")
        speaking_items = [item for item in payload["result"]["items"] if item["surface_text"] == "数至"]
        self.assertTrue(speaking_items)
        self.assertEqual(speaking_items[0]["start_token"], 6)
        self.assertEqual(speaking_items[0]["end_token"], 8)
        self.assertEqual(speaking_items[0]["risk_type"], "context_pronunciation")

    def test_medical_term_is_one_multi_token_annotation(self) -> None:
        text = "腧穴"
        tokens = [{"index": index, "text": character} for index, character in enumerate(text)]
        payload = json.loads(handle_line(json.dumps({"id": "term", "method": "pronunciation.analyze", "params": {"segment_id": "seg-1", "text": text, "tokens": tokens}}, ensure_ascii=False)) or "")
        terms = [item for item in payload["result"]["items"] if item["risk_type"] == "medical_term"]
        self.assertEqual(len(terms), 1)
        self.assertEqual(terms[0]["start_token"], 0)
        self.assertEqual(terms[0]["end_token"], 2)
        self.assertEqual(terms[0]["default_pinyin"], "shu4 xue2")

    def test_only_high_risk_polyphones_produce_annotations(self) -> None:
        text = "行数"
        tokens = [{"index": index, "text": character} for index, character in enumerate(text)]
        payload = json.loads(handle_line(json.dumps({"id": "polyphone", "method": "pronunciation.analyze", "params": {"segment_id": "seg-1", "text": text, "tokens": tokens}}, ensure_ascii=False)) or "")
        items = {item["surface_text"]: item for item in payload["result"]["items"]}
        self.assertIn("数", items)
        self.assertTrue(len(items["数"]["candidate_pinyin"]) > 1)
        self.assertEqual(items["数"]["source"], "high_risk_polyphone")

    def test_context_rule_keeps_candidates_and_overrides_default(self) -> None:
        text = "五藏"
        tokens = [{"index": index, "text": character} for index, character in enumerate(text)]
        payload = json.loads(handle_line(json.dumps({"id": "context", "method": "pronunciation.analyze", "params": {"segment_id": "seg-1", "text": text, "tokens": tokens}}, ensure_ascii=False)) or "")
        item = payload["result"]["items"][0]
        self.assertEqual(item["risk_type"], "context_pronunciation")
        self.assertEqual(item["source"], "context_exact")
        self.assertEqual(item["default_pinyin"], "wu3 zang4")
        self.assertIn("wu3 cang2", item["candidate_pinyin"])

    def test_variant_keeps_surface_and_token_range(self) -> None:
        text = "按𫏋"
        tokens = [{"index": index, "text": character} for index, character in enumerate(text)]
        payload = json.loads(handle_line(json.dumps({"id": "variant", "method": "pronunciation.analyze", "params": {"segment_id": "seg-1", "text": text, "tokens": tokens}}, ensure_ascii=False)) or "")
        item = payload["result"]["items"][0]
        self.assertEqual(item["surface_text"], text)
        self.assertEqual((item["start_token"], item["end_token"]), (0, 2))
        self.assertEqual(item["default_pinyin"], "an4 qiao1")

    def test_known_rare_character_is_reported_even_with_pypinyin_reading(self) -> None:
        text = "皶"
        payload = json.loads(handle_line(json.dumps({"id": "rare", "method": "pronunciation.analyze", "params": {"segment_id": "seg-1", "text": text, "tokens": [{"index": 0, "text": text}]}}, ensure_ascii=False)) or "")
        self.assertEqual(payload["result"]["items"][0]["risk_type"], "classical_term")
        self.assertEqual(payload["result"]["items"][0]["source"], "rare_classical_lexicon")

    def test_known_rare_character_remains_a_rare_character_signal(self) -> None:
        text = "坼"
        payload = json.loads(handle_line(json.dumps({"id": "rare-known", "method": "pronunciation.analyze", "params": {"segment_id": "seg-1", "text": text, "tokens": [{"index": 0, "text": text}]}}, ensure_ascii=False)) or "")
        item = payload["result"]["items"][0]
        self.assertEqual(item["risk_type"], "rare_character")
        self.assertEqual(item["source"], "rare_character")
        self.assertEqual(item["default_pinyin"], "che4")

    def test_knowledge_conflict_keeps_candidates_and_emits_stable_warning(self) -> None:
        tokens = [Token(0, "藏")]
        medical = [KnowledgeMatch(0, 1, "zang4", ("zang4",), "medical_term", "medical_lexicon_v3", "medical", 300, "verified", "organ")]
        context = [KnowledgeMatch(0, 1, "cang2", ("cang2",), "context_pronunciation", "context_semantic", "context", 200, "high", "organ_context")]
        selected, warnings = _select_knowledge_matches([], medical, context, [], tokens)
        self.assertEqual(selected[0].default_pinyin, "")
        self.assertTrue(selected[0].conflict)
        self.assertEqual(set(selected[0].candidates), {"zang4", "cang2"})
        self.assertEqual(warnings[0]["code"], "ANALYZER_KNOWLEDGE_CONFLICT")

    def test_supplementary_plane_character_does_not_crash_worker(self) -> None:
        text = "𠮷"
        payload = json.loads(handle_line(json.dumps({"id": "rare", "method": "pronunciation.analyze", "params": {"segment_id": "seg-1", "text": text, "tokens": [{"index": 0, "text": text}]}}, ensure_ascii=False)) or "")
        self.assertTrue(payload["ok"])
        self.assertEqual(payload["result"]["items"][0]["risk_type"], "unknown_character")

    def test_pronunciation_fixture_reports_real_sample_counts(self) -> None:
        fixture = Path(__file__).parents[2] / "tests" / "fixtures" / "pronunciation_classic.txt"
        segments = [line.strip() for line in fixture.read_text(encoding="utf-8").splitlines() if line.strip()]
        counts: dict[str, int] = {}
        annotation_count = 0
        for index, text in enumerate(segments):
            tokens = [{"index": token_index, "text": character} for token_index, character in enumerate(text)]
            payload = json.loads(handle_line(json.dumps({"id": f"fixture-{index}", "method": "pronunciation.analyze", "params": {"segment_id": f"seg-{index}", "text": text, "tokens": tokens}}, ensure_ascii=False)) or "")
            self.assertTrue(payload["ok"])
            for item in payload["result"]["items"]:
                annotation_count += 1
                counts[item["risk_type"]] = counts.get(item["risk_type"], 0) + 1
        print(f"pronunciation_classic.txt: segments={len(segments)}, annotations={annotation_count}, counts={counts}")
        self.assertGreaterEqual(len(segments), 20)
        self.assertGreater(annotation_count, 0)

    def test_medical_lexicon_longest_match_and_formula_name(self) -> None:
        result = self.analyze_text("服栝蒌桂枝汤。")
        items = result["items"]
        formula = next(item for item in items if item["surface_text"] == "栝蒌桂枝汤")
        self.assertEqual(formula["default_pinyin"], "gua1 lou2 gui4 zhi1 tang1")
        self.assertEqual(formula["rule_type"], "formula")
        self.assertFalse(any(item["surface_text"] == "栝蒌" for item in items))

    def test_semantic_context_reranks_number_character(self) -> None:
        pulse = self.analyze_text("脉来数疾。")["items"]
        numeric = self.analyze_text("阳数七。")["items"]
        self.assertEqual(next(item for item in pulse if item["surface_text"] == "数")["default_pinyin"], "shuo4")
        self.assertEqual(next(item for item in numeric if item["surface_text"] == "数")["default_pinyin"], "shu4")

    def test_semantic_context_reranks_zhong(self) -> None:
        afflicted = self.analyze_text("邪气中人。")["items"]
        disease = self.analyze_text("中风之后。")["items"]
        location = self.analyze_text("病在中焦。")["items"]
        self.assertEqual(next(item for item in afflicted if item["surface_text"] == "中")["default_pinyin"], "zhong4")
        self.assertEqual(next(item for item in disease if item["surface_text"] == "中风")["default_pinyin"], "zhong4 feng1")
        self.assertEqual(next(item for item in location if item["surface_text"] == "中")["default_pinyin"], "zhong1")

    def test_semantic_context_reranks_shao_and_cang(self) -> None:
        meridian = self.analyze_text("少阳主半表半里。")["items"]
        quantity = self.analyze_text("少食则气衰。")["items"]
        ambiguous_location = self.analyze_text("少腹拘急。")["items"]
        verb = self.analyze_text("气血收藏于内。")["items"]
        store_essence = self.analyze_text("肾者主藏精。")["items"]
        organ = self.analyze_text("脏腑经络相连。")["items"]
        self.assertEqual(next(item for item in meridian if item["surface_text"] == "少阳")["default_pinyin"], "shao4 yang2")
        self.assertEqual(next(item for item in quantity if item["surface_text"] == "少")["default_pinyin"], "shao3")
        self.assertEqual(next(item for item in ambiguous_location if item["surface_text"] == "少")["source"], "high_risk_polyphone")
        self.assertEqual(next(item for item in verb if item["surface_text"] == "藏")["default_pinyin"], "cang2")
        self.assertEqual(next(item for item in store_essence if item["surface_text"] == "藏精")["default_pinyin"], "cang2 jing1")
        self.assertEqual(next(item for item in organ if item["surface_text"] == "脏腑")["default_pinyin"], "zang4 fu3")

    def test_acupoint_context_and_normal_yu_are_distinct(self) -> None:
        acupoint = self.analyze_text("此为穴俞。")["items"]
        normal = self.analyze_text("普通俞字。")["items"]
        self.assertEqual(next(item for item in acupoint if item["surface_text"] == "穴俞")["default_pinyin"], "xue2 shu4")
        normal_item = next(item for item in normal if item["surface_text"] == "俞")
        self.assertEqual(normal_item["default_pinyin"], "yu2")
        self.assertEqual(normal_item["confidence"], "low")

    def test_classical_lexicon_lookup(self) -> None:
        item = self.analyze_text("其状翕翕然。")["items"][0]
        self.assertEqual(item["surface_text"], "翕翕")
        self.assertEqual(item["source"], "rare_classical_lexicon")
        self.assertEqual(item["default_pinyin"], "xi1 xi1")

    def test_common_characters_are_never_rare_or_unknown(self) -> None:
        result = self.analyze_text("子人上下之而于中数少")
        flagged = {
            item["surface_text"]: item["risk_type"]
            for item in result["items"]
            if item["risk_type"] in {"rare_character", "unknown_character"}
        }
        self.assertEqual(flagged, {})


if __name__ == "__main__":
    unittest.main()
