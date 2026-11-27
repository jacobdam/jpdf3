//! `cargo xtask status` (specs/tooling/status.md; #386 part 1, #390): the three sections of
//! ops/STATUS.md that copy state kept elsewhere, generated from the scheduler's own inputs and
//! parse rules, so the two can't disagree. Read-only, like the scheduler.

use std::fmt::Write as _;

use super::{
    Inputs, Issue, Pr, choose, current_milestone, has_label, held_prs, iso, linked_issue,
    parse_issues, parse_prs, unix,
};

/// The labels of the Blockers / owner-todo section, in the order a line lists them.
const BLOCKER_LABELS: [&str; 2] = ["blocked", "owner-todo"];
const IN_PROGRESS: &str = "in-progress";
const NONE: &str = "- none\n";

/// The text on one line: every control character (a newline in data, say) and the Unicode
/// line and paragraph separators (U+2028, U+2029) become a space, so no value can start a line
/// or a section of its own.
fn one_line(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_control() || matches!(c, '\u{2028}' | '\u{2029}') {
                ' '
            } else {
                c
            }
        })
        .collect()
}

/// `#N <title>` for an open issue, `#N (not an open issue)` otherwise.
fn issue_ref(issues: &[Issue], n: u64) -> String {
    match issues.iter().find(|i| i.number == n) {
        Some(i) => format!("#{n} {}", one_line(&i.title)),
        None => format!("#{n} (not an open issue)"),
    }
}

/// The state words of an open PR, each from one field the scheduler reads.
fn pr_state(p: &Pr) -> String {
    let mut w: Vec<&str> = vec![
        if p.draft { "draft" } else { "ready" },
        if p.auto_merge {
            "auto-merge on"
        } else {
            "auto-merge off"
        },
    ];
    if p.conflict {
        w.push("conflicting");
    }
    w.push(if p.rollup_bad {
        "not green (a failing check or no ai-review success)"
    } else {
        "green"
    });
    if p.held {
        w.push("on the owner's /hold");
    }
    if p.owner_branch() {
        w.push("the owner's branch");
    }
    for l in ["needs-design", "design", "owner-todo", "blocked"] {
        if has_label(&p.labels, l) {
            w.push(l);
        }
    }
    w.join(", ")
}

/// The labels of `BLOCKER_LABELS` it has, joined, or `None`.
fn blocker_labels(labels: &[String]) -> Option<String> {
    let l: Vec<&str> = BLOCKER_LABELS
        .into_iter()
        .filter(|b| has_label(labels, b))
        .collect();
    (!l.is_empty()).then(|| l.join(", "))
}

pub(super) fn render(inp: &Inputs) -> Result<String, String> {
    // The scheduler runs first, so every input passes its parse rules or nothing prints.
    let (pick, _) = choose(inp)?;
    let mut prs = parse_prs(&inp.prs)?;
    let held = held_prs(&inp.holds)?;
    for p in &mut prs {
        p.held = p.open && held.contains(&p.number);
    }
    let open: Vec<&Pr> = prs.iter().filter(|p| p.open).collect();
    let issues = parse_issues(&inp.issues)?;
    let now = iso(unix(&inp.now, "now.txt")?);
    let milestone = current_milestone(&inp.milestones)?;

    let mut out = format!(
        "# Status (cargo xtask status, {now}; specs/tooling/status.md)\n\nMilestone: {}\n",
        milestone.as_deref().map_or("none".to_owned(), one_line)
    );

    out.push_str("\n## In flight\n");
    let mut any = false;
    for p in &open {
        any = true;
        let target = match linked_issue(&p.body) {
            Some(n) => format!("closes {}", issue_ref(&issues, n)),
            None => "no Closes line".to_owned(),
        };
        let _ = writeln!(out, "- PR #{}: {target}: {}", p.number, pr_state(p));
    }
    for i in issues.iter().filter(|i| has_label(&i.labels, IN_PROGRESS)) {
        any = true;
        let linking: Vec<String> = open
            .iter()
            .filter(|p| linked_issue(&p.body) == Some(i.number))
            .map(|p| format!("PR #{}", p.number))
            .collect();
        let by = if linking.is_empty() {
            "no open agent PR closes it".to_owned()
        } else {
            linking.join(", ")
        };
        let _ = writeln!(
            out,
            "- #{} {IN_PROGRESS}: {} ({by})",
            i.number,
            one_line(&i.title)
        );
    }
    if !any {
        out.push_str(NONE);
    }

    // Exactly the scheduler's first line and Rule: line (`decide` prints the same two).
    let _ = write!(out, "\n## Next step\n{}\nRule: {}\n", pick.first, pick.rule);

    out.push_str("\n## Blockers / owner-todo\n");
    let mut any = false;
    for i in &issues {
        if let Some(l) = blocker_labels(&i.labels) {
            any = true;
            let _ = writeln!(out, "- #{} {l}: {}", i.number, one_line(&i.title));
        }
    }
    for p in &open {
        if let Some(l) = blocker_labels(&p.labels) {
            any = true;
            let _ = writeln!(out, "- PR #{} {l}", p.number);
        }
    }
    if !any {
        out.push_str(NONE);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_line_drops_control_characters() {
        assert_eq!(one_line("a\nb\r\tc"), "a b  c");
        assert_eq!(one_line("a\u{2028}b\u{2029}c\u{85}d"), "a b c d");
        assert_eq!(one_line("plain — ünïcode"), "plain — ünïcode");
    }

    #[test]
    fn blocker_labels_in_order() {
        let l = |v: &[&str]| v.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
        assert_eq!(
            blocker_labels(&l(&["owner-todo", "x", "blocked"])).as_deref(),
            Some("blocked, owner-todo")
        );
        assert_eq!(
            blocker_labels(&l(&["owner-todo"])).as_deref(),
            Some("owner-todo")
        );
        assert_eq!(blocker_labels(&l(&["blocked-ish", "owner"])), None);
        assert_eq!(blocker_labels(&[]), None);
    }
}
