# Git safety setup and recovery

This repository expects public commit history to use a GitHub noreply email.
The project guard also rejects empty, control-character, or option-like author
and committer names. Ordinary Unicode names, including Japanese names, remain
valid in existing history.

## Dedicated Codex Cloud clone

Keep a user's existing clone and global identity untouched. Create a separate
clone and configure only its local settings:

```sh
git clone https://github.com/Yrika819/Serea.git Serea-codex
cd Serea-codex
git config --local user.name 'Codex'
git config --local user.email 'noreply@github.com'
git config --local user.useConfigOnly true
git config --local push.default nothing
git config --local serea.identityProfile codex
git var GIT_AUTHOR_IDENT
git var GIT_COMMITTER_IDENT
python3 tools/check_commit_identity.py
```

Both `git var` commands must show `Codex <noreply@github.com>`. The history
guard must pass before editing. Do not “fix” a mismatch by changing global or
system config. Inspect effective settings and their source without dumping
credential-bearing remotes:

```sh
git config --show-origin --show-scope --get user.name
git config --show-origin --show-scope --get user.email
git config --show-origin --show-scope --get user.useConfigOnly
git config --show-origin --show-scope --get push.default
git status --short --branch
```

`git config --get ...` reads a setting; `git config ... VALUE` changes it.
Keep those forms distinct. In particular, never append an extra option token
to a write command. The earlier `--get` identity incident has no verified
command transcript, so do not infer a precise cause from the symptom. Check
the stored value and `git var` before any commit. The preflight prints setting
scope/source labels but redacts identity emails and remote URL values.

Before every commit, run `python3 tools/git_preflight.py --mode commit`.
After the commit, run `python3 tools/check_commit_identity.py`; it scans all
reachable history, including the new HEAD, and reports only offending SHAs.

Before pushing, fetch and review the base, then run an explicit-target check:

```sh
git fetch origin
python3 tools/git_preflight.py --mode push --remote origin \
  --target chore/git-safety-hardening --base-branch main
git push --set-upstream origin chore/git-safety-hardening
```

For later pushes, use the same explicit target preflight and `git push origin
branch-name`. The checker reads the live target with `git ls-remote`, compares
it to the fetched tracking ref, requires a fast-forward, rejects `main`, and
uses `push.default=nothing` to prevent implicit ref selection. Never use
`--force`, `--force-with-lease`, deletion pushes, or an unqualified `git push`.
The pre-push hook cannot inspect the literal push command-line flags; it
independently rejects non-fast-forward updates, which prevents the destructive
effect of a force push. Do not rely on this hook to bless a force option.

## Hooks

Hooks are versioned in `.githooks` but Git does not enable them automatically.
Enable and verify them in this clone:

```sh
python3 tools/install_git_hooks.py
git config --show-origin --show-scope --get core.hooksPath
```

The installer writes only repository-local `core.hooksPath`. The pre-commit
hook runs commit preflight; the pre-push hook first runs the reachable-history
identity guard and then checks the actual remote name, URL, pushed branch,
local HEAD, and live remote HEAD. It rejects deletion records whose local
object ID is all zeroes. Always run explicit commands too: hooks can be skipped
or absent in another clone.

## Windows and macOS/Linux

Personal Windows/macOS/Linux clones must opt in explicitly. Set `user.name` to
the exact approved local name, use that account's GitHub noreply address, and
select the personal profile in repository-local config. The checker requires
`serea.approvedName` and an exact match; it never infers a personal profile
from the current name or OS:

```sh
git config --local user.name 'Your GitHub display name'
git config --local user.email '12345678+yourname@users.noreply.github.com'
git config --local user.useConfigOnly true
git config --local push.default nothing
git config --local serea.identityProfile personal
git config --local serea.approvedName 'Your GitHub display name'
git var GIT_AUTHOR_IDENT
git var GIT_COMMITTER_IDENT
python3 tools/git_preflight.py --mode commit
python3 tools/check_commit_identity.py
```

Replace the example with the noreply address shown in that GitHub account's
email settings. Legacy `name@users.noreply.github.com` addresses are accepted;
private email addresses are rejected. Codex Cloud clones must keep the
`codex` profile and `Codex <noreply@github.com>` identity.

Use repository-local settings on each OS; do not apply one user's settings to
every repository. Let `.gitattributes`
specify repository text normalization. Check `core.autocrlf`, `core.eol`, and
`core.filemode` with `--show-origin --show-scope` before changing them.

On Windows, Git may not track executable-bit changes the same way as Unix;
`core.filemode=false` can be appropriate for a Windows clone if status reports
spurious mode changes. On macOS/Linux, the normal default is to honor tracked
executable bits. Do not change these settings merely to hide a real source
diff. If attributes do not define a file's line endings, inspect the specific
file and project convention before setting a repository-local conversion.

PowerShell uses the same commands (`python` may be the installed launcher):

```powershell
git config --local user.name 'Codex'
git config --local user.email 'noreply@github.com'
git config --local user.useConfigOnly true
git config --local push.default nothing
git config --local serea.identityProfile codex
git var GIT_AUTHOR_IDENT
git var GIT_COMMITTER_IDENT
python tools/check_commit_identity.py
python tools/git_preflight.py --mode commit
```

## Recovery after an identity mistake

If `git var` or a new commit shows an unexpected identity, stop before pushing.
Do not rewrite an already published branch as normal maintenance. Prefer a new
corrective commit/branch when repository policy permits. If a published history
rewrite is genuinely required, it is an exceptional operation requiring
explicit Owner approval for the exact branch and old/new SHA mapping. Before
approval, collect a read-only audit, preserve a backup ref, identify dependent
branches/PRs, and establish an agreed recovery window. Never rewrite `main`.

PR #16 documents a prior exceptional identity repair: three commit IDs were
mapped to repaired IDs while preserving trees and parent order, followed by a
branch-specific force push. PR #17 documents a one-commit identity amendment.
These are historical incident records, not permissions or templates for normal
work. This hardening workflow uses ordinary new-branch commits and pushes only.

If a remote branch changes between fetch and push, stop, fetch again, inspect
the new commits, and update by a normal merge/rebase workflow. Do not force past
the divergence. Never place tokens, credentials, private email addresses, or
remote URLs containing credentials in logs or issue reports.
