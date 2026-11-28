# Cloud C-07 Memory Retrieval

## Base and boundary

Branch `cloud/serea-v13-c07-memory-retrieval` starts from C-06R HEAD `af62369b6a63449033f485fc3cff2cdf5f68bdae`. Retrieval remains inside the disabled, test/development-only P7 experimental memory module. It does not add a normal database migration, persistent runtime surface, Android integration, or model-context injection.

## Implementation

`P7ExperimentalMemory::retrieve_context` reuses the P7 FTS5 index, owner handle, recovery-ready check, active flag, expiry, tombstones, classification, and provenance table. One bounded read-only query joins the matching memory and its provenance, filters by owner and class, and excludes expired, inactive, tombstoned, or unprovenanced rows.

Candidates receive a deterministic lexical score: an exact phrase substring adds 100 and each whitespace-delimited query term found in the body adds one. Ordering breaks ties by memory ID, body, source ID, and source-task ID. Duplicate memory IDs are collapsed. The score is lexical and does not represent semantic similarity.

The caller supplies candidate and result caps (at most the existing P7 1,000 candidate and 20 result bounds). The context budget is exact UTF-8 bytes for body plus source ID, with a one MiB absolute request ceiling. Returned entries retain the body and source attribution. No tokenizer is present, so the API reports bytes rather than estimated tokens.

## Synthetic Japanese evaluation

The existing `p7_japanese_fts_eval_tests` comparison was run on SQLite 3.54.1. It is a synthetic lexical comparison, not a production quality gate:

| Strategy | Exact queries | Complete recall | Relevant rows found | False positive rows | Mean query time |
| --- | ---: | ---: | ---: | ---: | ---: |
| Current `unicode61` baseline | 200/1,100 | 200/1,100 | 500/1,600 | 0 | 23 µs |
| `trigram` comparison | 500/1,100 | 500/1,100 | 700/1,600 | 0 | 27 µs |
| Synthetic `host-ja-v1` comparison | 1,000/1,100 | 1,000/1,100 | 1,500/1,600 | 400 | 291 µs |

The current tokenizer misses Japanese substrings inside continuous text. The alternative host-side bigram strategy improves synthetic recall but produces false positives and is slower; it was not adopted. New P7 retrieval tests cover exact and partial lexical matches, punctuation and reading variants, unrelated content, tombstones, different owners, classification, output limits, and deterministic ranking.

## Verification

- Retrieval-specific P7 tests: 4/4 PASS.
- P7 feature suite (`p7-experimental`, `p7-gate-c`): 630 unit tests, 2 foundation tests, 2 Gate C lock tests, and 35 doc tests PASS after the final implementation adjustment (2 resource bench tests are intentionally ignored).
- Default `serea-storage` suite: 511 unit tests, 2 foundation tests, 2 Gate C lock tests, and 35 doc tests PASS. Retrieval is compiled only in the P7 test/development feature path.
- Existing Japanese FTS evaluation: PASS with the measured synthetic metrics above.
- `cargo fmt --all -- --check` and `git diff --check`: PASS.

The repository's pre-existing deprecated `serde_json` `usize::max_value` warning appears in Rust test builds; it is outside this change.
