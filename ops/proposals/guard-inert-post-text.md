# Proposal: the guard's Rust-keyword rule skips inert gh post text

Status: proposed by the session for owner issue #124, 2026-11-21. Touches `.claude/hooks/guard.py`
(protected, CONSTITUTION 12).

## Problem
Owner issue #124 asks that every AI review be posted on its PR. TICK §4 now has sessions post each
reviewer round verbatim with `gh pr comment N --body '🤖 jpdf-agent ...'`. Reviews of Rust code
routinely contain all three of:
- the Rust keyword for unchecked code (for example "no un-safe blocks added", written here with a
  hyphen because this file goes through the same guard when a session edits it with Bash);
- a `.rs` path (`crates/x/src/lib.rs:10`);
- a `>` (`n > 2^30`, or a markdown quote).

`check_bash` line 187 blocks any Bash command containing all three, to stop a shell redirect that
writes the keyword into a Rust file. It runs on the whole command, so it also blocks a gh comment
whose single-quoted body is inert text. Probe (2026-11-21, guard at main 4ce8115): a
`gh pr comment 127 --body '<tag> ... lib.rs ... n > 2^30 ... <keyword> ...'` call is blocked with
"writing ... into Rust sources is forbidden". The cmux rule already solves the same problem with
`strip_inert_text`.

Until this is applied, TICK §4 tells sessions to write the keyword as `un-safe` in the posted copy,
so the posted review is not quite verbatim.

## Proposed diff (`.claude/hooks/guard.py`)
```diff
@@ def check_bash(cmd, cwd):
     post_text = check_gh_posts(cmd)
+    # Text with every inert gh title/body removed (see strip_inert_text); rules about what a
+    # command *does* run on this, so quoting a word in a comment doesn't trip them.
+    inert_c = " ".join(strip_inert_text(cmd, post_text).split())
@@
-    if re.search(r"\bunsafe\b", c) and re.search(r"\.rs\b", c) and re.search(r"(>|\btee\b|sed\s+-i|perl\s+-[a-z]*i)", c):
+    if re.search(r"\bunsafe\b", inert_c) and re.search(r"\.rs\b", inert_c) \
+            and re.search(r"(>|\btee\b|sed\s+-i|perl\s+-[a-z]*i)", inert_c):
         raise Block("writing `unsafe` into Rust sources is forbidden (CONSTITUTION 3)")
@@ def selftest():
+        ("Bash", {"command": "gh pr comment 5 --body '%s a.rs: n > 1, no unsafe added'" % TAG}, False),
+        ("Bash", {"command": "gh pr comment 5 --body '%s x'; echo 'unsafe {}' > a.rs" % TAG}, True),
+        ("Bash", {"command": "gh pr comment 5 --body \"%s $(echo unsafe > a.rs)\"" % TAG}, True),
```
(The hyphen-free keyword appears in the diff because the owner applies it with an editor, not
through the guard.)

Why it stays safe: `strip_inert_text` removes a body only in a form the shell can't expand
(single quotes, or double quotes with no `$`, backtick, `\` or `"`), and only one occurrence per
parsed body, so a redirect outside the quotes or inside `$(...)` is still seen. The two added
`True` cases pin that.

Checked: a script applied exactly these edits to a scratch copy of the guard (main 4ce8115). The
copy's self-test passes 89/89 (86 existing + 3 new), and the review-comment probe above is allowed.

## Risk if not applied
Posted reviews replace one word with `un-safe`. A session that forgets gets a guard block and
retries; nothing unsafe happens either way. Low priority.
