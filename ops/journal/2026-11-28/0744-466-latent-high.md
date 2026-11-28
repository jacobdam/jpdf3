## 07:44 UTC #466 ADR 0009: latent sev:high issues wait, deferred, where their path appears
- Session: worker owner-comment https://github.com/jacobdam/jpdf3/issues/466#issuecomment-7235327110
- Picked: rule a because the owner endorsed tagged issue #466 with an untagged "Ok" and it had no "done:" reply
- Did: PR #487: new ADR 0009 section "Latent highs" (milestone where the path appears, never later than M4, `deferred` with `Undefer when:`; the filing comment names the missing path; when unsure, current milestone; rule d unchanged; one-time pass); ADR 0012 §2 term 3, TICK §2 rule d, §5 Discoveries term 3 and AUTOMATION.md §3 refer to it; TICK §9 step 1a gets the Planner's latent-high check (R1.1). One-time pass: only #146 is an open high; it already conforms under the owner's PR #465 instruction (milestone of #474, `Undefer when: #474 closed`), which overrides #466's proposed M2 move, so no move; #231 was closed as not planned. Commented on both. Removed the merged t/146-depends-on worktree (scheduler note).
- Result: `ai-review` success posted on this commit, auto-merge on. "done:" on the owner comment waits for the merge (the next session posts it, TICK §0).
- Cost: ~35% of context; review rounds 3/4; regraded 0, overturned 0; deferred: none
- Codex: skipped (no `crates/` or `tools/` path)
- Lesson: none
- Ignored: none. Seen, not this session's: the owner's newer comment on #478 (https://github.com/jacobdam/jpdf3/issues/478#issuecomment-7236076394) is pending for a later rule a session.
