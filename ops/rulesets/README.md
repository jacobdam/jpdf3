# Branch ruleset (owner applies once)

`main.json` protects `main`:
- no deletion and no force-push; linear history
- every change goes through a PR
- PRs touching CODEOWNERS paths need the owner's review; other PRs need no review
- required checks: `judge-verdict` (protected CI judge) and `ai-review` (independent AI reviewer)
- no bypass actors

## Import (GitHub web UI)
1. Repo → **Settings → Rules → Rulesets**.
2. **New ruleset → Import a ruleset**, then choose `ops/rulesets/main.json`.
3. Check that enforcement shows **Active**, then **Create**.
4. Repo → **Settings → General → Pull Requests**: enable **Allow auto-merge** and
   **Allow squash merging**. Optionally disable merge commits and rebase merging.

## Or with gh (needs a token with Administration: write; the agent's token deliberately lacks it)
```sh
gh api -X POST repos/jacobdam/jpdf3/rulesets --input ops/rulesets/main.json
gh api -X PATCH repos/jacobdam/jpdf3 -F allow_auto_merge=true -F allow_squash_merge=true
```
