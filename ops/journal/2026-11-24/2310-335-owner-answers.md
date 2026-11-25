## 23:10 UTC #335 (PR #336): the owner's answers to #311's open points
- Session: owner interactive session (no `cargo xtask next-session` pick)
- Picked: rule a because the owner answered in the session: "Simplfied, host themself, we host on github as demo. Publishing is phase 2"; PR #332 (#311) merged first on the owner's in-session approval of its merged head (e25d1ed), recorded on the PR
- Did: #335 filed for the owner (tagged, `human`); docs/PRODUCT.md, PLAN.md §1 assumptions, §4 Phase 2, §8 M10 and P2, AUTOMATION.md §15: Simplified Chinese, v1.0 released as source (users build the macOS app, build and host the web app), the GitHub Pages site CI already deploys is the demo, publishing in Phase 2. #334 told the variant.
- Result: reviewed in 3 rounds; waiting on the owner's approval (a PLAN.md change), then auto-merge.
- Cost: ~5% of context; review rounds 3/4; regraded 0, overturned 0 (R1.3 dispute accepted); deferred: none
- Codex: skipped (no `crates/` or `tools/` path)
- Lesson: "we host on github as demo" was already true (CI's `pages` job): grep the workflows before writing a new deliverable for something an owner names.
- Ignored: none
