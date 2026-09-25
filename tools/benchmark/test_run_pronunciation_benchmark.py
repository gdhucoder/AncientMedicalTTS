from __future__ import annotations

from tools.benchmark.run_pronunciation_benchmark import classify_extra, evaluate


def token(index: int, text: str) -> dict[str, object]:
    return {"index": index, "text": text}


def prediction(
    identifier: str,
    start: int,
    end: int,
    surface: str,
    default: str,
    candidates: list[str],
    risk: str = "polyphone",
) -> dict[str, object]:
    return {
        "id": identifier,
        "segment_id": "segment-1",
        "start_token": start,
        "end_token": end,
        "surface_text": surface,
        "default_pinyin": default,
        "target_pinyin": None,
        "candidate_pinyin": candidates,
        "risk_type": risk,
    }


def segment(
    text: str, tokens: list[str], predictions: list[dict[str, object]]
) -> dict[str, object]:
    return {
        "segment_id": "segment-1",
        "order_index": 0,
        "text": text,
        "tokens": [token(index, value) for index, value in enumerate(tokens)],
        "predictions": predictions,
    }


def gold(identifier: str, match: str, focus: str, pinyin: str, status: str = "gold") -> dict[str, object]:
    return {
        "id": identifier,
        "chapter": "第一篇",
        "match_text": match,
        "focus_text": focus,
        "gold_pinyin": pinyin,
        "recommended_pinyin": "",
        "candidate_pinyin": "",
        "status": status,
        "risk_type": "polyphone",
        "occurrence_count_in_chapter": 1,
        "context": match,
        "note": "",
    }


def test_evaluator_handles_focus_fragmentation_occurrences_and_candidates() -> None:
    predictions = [
        prediction("p1", 0, 1, "腧", "shu4", ["shu4"]),
        prediction("p2", 1, 2, "穴", "xue2", ["xue2"]),
    ]
    snapshot = {
        "benchmark_version": "test",
        "chapters": [
            {
                "title": "第一篇",
                "segments": [segment("腧穴", ["腧", "穴"], predictions)],
            }
        ],
    }
    metrics, details = evaluate(
        snapshot,
        {
            "entries": [
                gold("G1", "腧穴", "腧穴", "shu4 xue2"),
                gold("G2", "腧穴", "穴", "shu4 xue2"),
            ]
        },
    )
    assert metrics["hard_gold_occurrences"] == 2
    assert metrics["detected"] == 2
    assert details["occurrences"][0]["fragmented_prediction"] is True
    assert details["occurrences"][1]["fragmented_prediction"] is False
    assert metrics["candidate_contains_gold"] == 2


def test_evaluator_uses_occurrence_and_reports_cross_segment_span() -> None:
    snapshot = {
        "benchmark_version": "test",
        "chapters": [
            {
                "title": "第一篇",
                "segments": [
                    segment("藏", ["藏"], [prediction("p1", 0, 1, "藏", "cang2", ["cang2", "zang4"])]),
                    {
                        "segment_id": "segment-2",
                        "order_index": 1,
                        "text": "府",
                        "tokens": [token(0, "府")],
                        "predictions": [],
                    },
                ],
            }
        ],
    }
    metrics, details = evaluate(
        snapshot,
        {
            "entries": [
                gold("G1", "藏", "藏", "zang4"),
                {
                    **gold("G2", "藏府", "藏", "zang4 fu3"),
                    "occurrence_count_in_chapter": 1,
                    "context": "藏府",
                },
            ]
        },
    )
    assert metrics["detected"] == 1
    assert metrics["wrong_default"] == 1
    assert metrics["candidate_missing"] == 0
    assert metrics["gold_occurrences_crossing_segment_boundary"] == 1
    assert any(row["possible_reason"] == "segment_boundary" for row in details["occurrences"])


def test_evaluator_handles_supplementary_plane_and_review_required() -> None:
    snapshot = {
        "benchmark_version": "test",
        "chapters": [
            {
                "title": "第一篇",
                "segments": [
                    segment(
                        "按𫏋",
                        ["按", "𫏋"],
                        [prediction("p1", 1, 2, "𫏋", "qiao1", ["qiao1"])],
                    )
                ],
            }
        ],
    }
    metrics, details = evaluate(
        snapshot,
        {
            "entries": [
                gold("G1", "按𫏋", "𫏋", "an4 qiao1"),
                gold("R1", "按𫏋", "𫏋", "", "review_required"),
            ]
        },
    )
    assert metrics["detected"] == 1
    assert metrics["review_required_occurrences"] == 1
    assert metrics["review_detected"] == 1
    assert metrics["gold_occurrences_crossing_segment_boundary"] == 0
    assert details["review"][0]["detected"] is True


def test_extra_review_uses_generic_four_way_classification() -> None:
    bug, _ = classify_extra(
        {
            "risk_type": "rare_character",
            "source": "pypinyin",
            "default_pinyin": "",
            "candidates": "zi3",
        },
        "子",
    )
    assert bug == "bug"

    useful, _ = classify_extra(
        {
            "risk_type": "polyphone",
            "source": "high_risk_polyphone",
            "default_pinyin": "cang2",
            "candidates": "cang2 | zang4",
        },
        "五藏",
    )
    assert useful == "useful"

    unnecessary, _ = classify_extra(
        {
            "risk_type": "polyphone",
            "source": "pypinyin",
            "default_pinyin": "xing2",
            "candidates": "xing2",
        },
        "行",
    )
    assert unnecessary == "unnecessary"


if __name__ == "__main__":
    tests = [
        test_evaluator_handles_focus_fragmentation_occurrences_and_candidates,
        test_evaluator_uses_occurrence_and_reports_cross_segment_span,
        test_evaluator_handles_supplementary_plane_and_review_required,
        test_extra_review_uses_generic_four_way_classification,
    ]
    for test in tests:
        test()
        print(f"{test.__name__}: ok")
