# External Boundary: OWASP LLM06 — Replication Notes

This directory contains everything an independent replicator needs to
reproduce the six-boundary numbers referenced in the paper's
§5 "External-framework boundary (OWASP LLM06)" subsection.

## Files

- `boundary-006.yaml` — the LLM06 boundary definition, not tuned
  against the evasion corpus.
- `generate_prompt_diff.md` — the diff against
  `benchmarks/generate_evasion_corpus.py` required to produce
  LLM06-specific rephrasings.
- `setfit_training_diff.md` — the diff against
  `benchmarks/v2/setfit/train_setfit.py` required to add 40 positive
  and 24 negative LLM06 examples (5:3 ratio, consistent with the
  original five boundaries).

## Status

**Numbers not yet reported in the main paper tables.** The main
tables (Tables 3, 6, 7, 8, 9, 10) reflect BOUNDARY-001 through
BOUNDARY-005 only. Regeneration requires:

1. `python benchmarks/generate_evasion_corpus.py --include-llm06`
2. `python benchmarks/eval_auto_corpus.py` against both PreFlight
   (recompile with BOUNDARY-006 loaded) and NeMo (prompt template
   extended with the LLM06 rule).
3. `python benchmarks/v2/setfit/train_setfit.py --include-llm06`
   with the five canonical seeds (42, 123, 456, 789, 1024).
4. `python benchmarks/v2/safe_corpus/eval_dolly_fpr.py` unchanged,
   to observe FPR shift from the LLM06 trigger set on open-domain
   traffic.

## Expected risks

The `pii`, `personal`, and `model` trigger words are higher-risk for
false positives on Dolly-15k than the original trigger set
(BOUNDARY-005's `home` and `location` already caused all 13 FPs at
2.6%). Adding LLM06 is expected to raise the PreFlight FPR above
the current 2.6% floor. The quantitative shift is a pending
measurement, not a claim.

## Why this boundary

OWASP LLM06 was chosen because it is (a) keyword-matchable in
principle (the protected concepts are nouns, not predicates), (b)
authored by an independent body with no connection to this work,
and (c) widely cited, giving the external-validity argument a
concrete anchor point.

It was NOT chosen because keyword matching "works well" on it.
Whether it does or does not is the measurement the subsection
invites.
