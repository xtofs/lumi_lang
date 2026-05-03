---
name: commit
description: clean up repo with a clean commit
---

Summarize staged/unstaged changes, stage everything, and commit. Use when you want to commit, stage and commit, or create a commit message.

1. Run `git status` and `git diff HEAD` (or `git diff` if nothing is staged) to see all changes.
2. Write a succinct commit message:
   - First line: imperative mood, ≤72 chars, no trailing period
   - Body (optional): bullet points for non-obvious context only — skip if the title says it all
3. Stage everything with `git add -A`.
4. Commit using a HEREDOC so multi-line messages are formatted correctly.
