# Pronunciation baseline benchmark

This benchmark evaluates the checked-in Huangdi Neijing v0.1 fixture without importing Gold data into the app.

Run from the repository root:

    python3 tools/benchmark/run_pronunciation_benchmark.py

The runner invokes the Rust production benchmark test. That test imports the text through the production Chapter/Segment/Grapheme Token path and calls the production Python Worker. The runner then compares the resulting read-only JSON snapshot with the Gold JSON.

Reports are written to:

    reports/benchmark/huangdi_neijing_v01/

The runner does not call Tencent TTS, FFmpeg, ASR, LLM, or pronunciation rules. It does not modify the fixture files, medical_terms.json, or the analyzer.

The evaluator self-tests can be run without pytest:

    python3 -m tools.benchmark.test_run_pronunciation_benchmark

## Independent hold-out evaluation

The v0.2.x production analyzer can also be evaluated on the original independent
Zhongjing hold-out fixtures. Read the fixture READMEs and manifest before
running; the runner first writes a raw production snapshot and only then
loads Gold in the evaluator. Gold is never imported into the application.

    python3 tools/benchmark/run_pronunciation_benchmark.py \
      --dataset shanghanlun_holdout_v01
    python3 tools/benchmark/run_pronunciation_benchmark.py \
      --dataset jinguiyaolue_holdout_v01
    python3 tools/benchmark/run_pronunciation_benchmark.py \
      --combine-holdouts

Hold-out reports are written to the two dataset directories and the combined
report is written to `reports/benchmark/zhongjing_holdout_v01/`. The raw
production snapshots remain separate from evaluator-only Gold alignment.
The current production Segmenter does not recognize every unnumbered Chinese
chapter heading in these fixtures, so the evaluator may project Gold chapter
boundaries in memory for per-chapter metrics. This does not alter the raw
snapshot and is reported as a textual/chapter-alignment issue.

The hold-out evaluator reports Recall, Default Accuracy, Candidate Coverage,
missed Gold entries, wrong defaults, extra prediction source/risk statistics,
focus-term observations, and textual/Unicode/segment-boundary issues. It does
not modify the pronunciation analyzer, dictionaries, context rules, Gold, or
production data.

## M9 regression

From M9 onward, all three checked-in datasets are development/regression data;
they are not final evidence of generalization. Run analyzer v0.3 without
overwriting the historical reports:

    PYTHONPATH=. uv run --project worker python tools/benchmark/run_pronunciation_benchmark.py --dataset huangdi_neijing_v01 --label m9 --output reports/benchmark/m9_regression/huangdi_neijing_v01
    PYTHONPATH=. uv run --project worker python tools/benchmark/run_pronunciation_benchmark.py --dataset shanghanlun_holdout_v01 --label m9 --output reports/benchmark/m9_regression/shanghanlun_holdout_v01
    PYTHONPATH=. uv run --project worker python tools/benchmark/run_pronunciation_benchmark.py --dataset jinguiyaolue_holdout_v01 --label m9 --output reports/benchmark/m9_regression/jinguiyaolue_holdout_v01
    PYTHONPATH=. uv run --project worker python tools/benchmark/run_pronunciation_benchmark.py --combine-m9

The combined report includes v0.2→v0.3 metrics and source contribution. A new
Gold set from a separate text such as Nanjing or Shennong Bencao Jing is still
required for a future final hold-out.
