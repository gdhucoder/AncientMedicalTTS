# Pronunciation Knowledge Layer v3

Analyzer v0.3.0 keeps pypinyin as the offline candidate provider and adds a deterministic, explainable pronunciation knowledge layer. It does not call an LLM or any network service and does not alter source text.

## Pipeline

```text
Rust Grapheme Tokens
  → analysis-only variant view
  → exact context rules
  → domain medical lexicon (longest match)
  → pypinyin candidates
  → semantic context rules
  → classical / rare lexicon
  → high-risk and rare filtering
  → deterministic annotation merge
  → source + rule_type + confidence + reason
```

All ranges are `[start_token, end_token)` Grapheme Token ranges supplied by Rust. Python does not derive JavaScript or byte offsets. Longest terms win at the same start position; ties are resolved by explicit priority and stable rule identifiers rather than dictionary iteration order.

## Resources

- `medical_lexicon_v3.json`: 117 entries across acupoint, classical term, organ, meridian, pulse, symptom, disease, formula, herb, treatment, and other categories.
- `classical_lexicon_v3.json`: 55 classical or rare terms kept separate from the medical domain lexicon.
- `context_rules_v3.json`: 19 semantic rules for contexts including pulse, organ, meridian, disease, numerical, and classical usage.
- `context_pronunciation_rules.json`: 25 retained exact phrase rules.
- `variant_characters.json`: 5 analysis-only mappings; surface text is never rewritten.
- `high_risk_polyphones.json`: 14 low-confidence characters that still merit review without stronger context.

Each v3 lexicon entry records a category, source, and discrete confidence. Sources are named public terminology standards, pharmacopoeia or teaching references, authoritative dictionaries, and published classical-text collation traditions. `source=benchmark` is invalid project policy.

## Priority and conflicts

The application-wide effective priority remains:

```text
Manual Annotation
  > Book Rule
  > Global Rule
  > Exact Context
  > Medical Lexicon
  > Semantic Context
  > Classical Lexicon
  > High-risk Polyphone
  > pypinyin
```

Manual, Book, and Global data are applied and protected by Rust. Worker output never overwrites confirmed or ignored annotations. A complete medical lexicon match normally prevents semantic context from replacing its pronunciation. If both knowledge sources apply to the same range and disagree, the Worker emits `knowledge_conflict`, combines the candidates, leaves the default unset, and explains the disagreement for human review.

## Semantic context

Semantic rules are data, not benchmark-specific Python branches. A rule can define `focus`, `left_terms`, `right_terms`, `nearby_terms`, `window`, `target`, `priority`, `category`, `confidence`, and a reason template. Examples include pulse-context `数 → shuo4`, numeric `数 → shu4`, affliction `中 → zhong4`, location `中 → zhong1`, meridian `少 → shao4`, quantity `少 → shao3`, and organ/verb distinctions for `藏`.

If no rule has adequate contextual evidence, candidates remain available and the analyzer does not force a high-confidence reading. Exact and semantic sources are distinguishable as `context_exact` and `context_semantic`.

## Rare and unknown characters

Known classical or rare entries are detected explicitly. If pypinyin can parse a normal character, an internal missing default must not turn it into `rare_character`. An actually unparsed Han character is reported as `unknown_character`. Regression tests cover `子、人、上、下、之、而、于、中、数、少` and require that none be mislabeled rare or unknown.

## Confidence and review behavior

Confidence is one of `verified`, `high`, `medium`, or `low`; it is not a probability. It is persisted with `source`, `rule_type`, and `reason` for the Inspector. M9 does not auto-confirm even verified entries: every Worker annotation keeps the existing `needs_review` workflow. User-created Rule annotations remain confirmed through the existing Rust rule service.

## Data maintenance

Before accepting a resource change, run:

```bash
uv run --project worker python tools/validate_pronunciation_lexicon.py
uv run --project worker python -m unittest discover -s worker/tests -p 'test_*.py'
```

The validator checks nonempty text, tone-number syntax, syllable count, duplicate/conflicting terms, known categories, source, confidence, semantic-rule structure, and duplicate variants. Add coherent terminology groups from attributable references; do not copy individual Gold rows into the production resources or add sentence-specific conditions.

## Benchmark policy

`素问`, `伤寒论`, and `金匮要略` are development/regression datasets from M9 onward. Reports are written under `reports/benchmark/m9_regression/`, preserve historical v0.2 results, include source contribution, and do not import Gold into the application. They measure regression but no longer prove generalization. A future independent final hold-out should be prepared from a text such as `难经` or `神农本草经`; M9 intentionally does not create that Gold set.
