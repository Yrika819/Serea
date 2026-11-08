# G10-B1-R5 identity repair record

- Repository: `Yrika819/Serea`
- Base: `main` at `676fd473e0d076c4fd4fdeaf974232a64fc303d3`
- PR #221 head before repair: `68ef6eee665748e7da0512f82c4cc9ff20d396ed`
- PR head after identity-only rebase: `23ee334a44a680118e1e5e1021a71011f4422cdd`
- Merge base: `676fd473e0d076c4fd4fdeaf974232a64fc303d3`
- Tree before and after identity-only rebase: `8f1ad06f379317ee2adbfebe4377294644e1dac2` (exact match)
- Working tree was clean before rebase. A local-only backup ref `backup/g10-b1-r5-pre-identity-68ef6ee` preserves the old head; no backup branch was pushed.

The original author names, commit messages, and content were retained. The two author emails were changed to the repository-approved GitHub noreply identity while preserving `ユウタ ゴトウ` as author name. Rebase-generated descendant committer metadata was normalized to the approved committer identity. The final introduced-commit set was checked with the unchanged `tools/check_commit_identity.py`; it passed for all 10 commits before subsequent Beta APK verification commits were added. The same checker passed for all 13 introduced commits at HEAD `bf1596ffe54743a3a2132d3fccd6a1d49afb4954`.

| Original commit | Rewritten commit | Message |
|---|---|---|
| `179e8af28b8fe6eea6f69e388208373e36f7c9e1` | `d5d89ddeb5eb07eaa5312c934d1e73971b870dbd` | Serialize native lease acquisition with close |
| `5a9831c4da171aecddddb0a78c282c85a6248131` | `2cc84462b4ab9bb885f38b947d1baa43162fc97c` | Check default APKs exclude native inference libraries |

Additional descendants were rewritten to preserve the linear history and approved committer metadata. The remote update used `--force-with-lease` pinned to the exact old remote PR head `68ef6eee665748e7da0512f82c4cc9ff20d396ed`; the PR remained OPEN / Draft.
