//! `cargo xtask check-specs`: feature-spec checklist, journey format and traceability checks.
//! Spec: specs/tooling/check-specs.md. Journey format: ops/judge/CONTRACT.md §4.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

/// AUTOMATION §5.2: always required, even if the template ever loses one.
pub const CHECKLIST: [&str; 8] = [
    "Persistence",
    "Undo",
    "Errors",
    "Limits",
    "Security",
    "Accessibility",
    "Platforms",
    "Performance",
];

const MAX_FILE_BYTES: u64 = 1 << 20;
const PLATFORMS: [&str; 2] = ["web", "macos"];
const FIELDS: [&str; 4] = ["milestone", "platform", "test", "superseded_by"];

/// One problem: a repo-relative path and a message.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Problem {
    pub path: String,
    pub msg: String,
}

impl std::fmt::Display for Problem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.path, self.msg)
    }
}

fn problem(path: &str, msg: impl Into<String>) -> Problem {
    Problem {
        path: path.to_owned(),
        msg: msg.into(),
    }
}

/// Checks `root/specs`. Returns problems sorted by path.
pub fn check(root: &Path) -> Vec<Problem> {
    let specs = root.join("specs");
    let mut out = Vec::new();
    if !specs.is_dir() {
        return out;
    }
    let mut files = Vec::new();
    walk(&specs, &mut files, &mut out, root);
    files.sort();

    // Required sections: the template's plus the §5.2 checklist.
    let mut required: Vec<String> = Vec::new();
    match read_text(&specs.join("_TEMPLATE.md")) {
        Ok(t) => {
            for s in sections(&t) {
                if !required.contains(&s.name) {
                    required.push(s.name);
                }
            }
        }
        Err(e) => out.push(problem("specs/_TEMPLATE.md", e)),
    }
    for c in CHECKLIST {
        if !required.iter().any(|r| r == c) {
            required.push(c.to_owned());
        }
    }

    let mut feature_specs = Vec::new();
    let mut journeys: BTreeMap<String, (String, Journey)> = BTreeMap::new();
    for rel in &files {
        let parts: Vec<&str> = rel.split('/').collect();
        let name = parts.last().copied().unwrap_or_default();
        let display = format!("specs/{rel}");
        // Same selection as the judge's journey parser: specs/journeys/*.md, not recursive.
        if parts.len() == 2 && parts.first() == Some(&"journeys") {
            match read_text(&specs.join(rel)) {
                Ok(t) => {
                    let (js, ps) = parse_journeys(&t);
                    out.extend(ps.into_iter().map(|m| problem(&display, m)));
                    for j in js {
                        if let Some((other, _)) = journeys.get(&j.id) {
                            out.push(problem(
                                &display,
                                format!("{}: duplicate journey ID (also in {other})", j.id),
                            ));
                        } else {
                            journeys.insert(j.id.clone(), (display.clone(), j));
                        }
                    }
                }
                Err(e) => out.push(problem(&display, e)),
            }
            continue;
        }
        // Same selection as the judge's policy check (ops/judge/policy.py check_specs).
        let skipped = parts.len() < 2
            || matches!(parts.first(), Some(&"journeys") | Some(&"ui"))
            || name.starts_with("README")
            || name.starts_with('_');
        if skipped {
            continue;
        }
        feature_specs.push(display.clone());
        match read_text(&specs.join(rel)) {
            Ok(t) => out.extend(
                check_feature_spec(&t, &required)
                    .into_iter()
                    .map(|m| problem(&display, m)),
            ),
            Err(e) => out.push(problem(&display, e)),
        }
    }

    for (id, (path, j)) in &journeys {
        if let Some(s) = &j.superseded_by {
            if s == id {
                out.push(problem(path, format!("{id}: superseded_by names itself")));
            } else if !journeys.contains_key(s) {
                out.push(problem(
                    path,
                    format!("{id}: superseded_by unknown journey {s}"),
                ));
            }
        }
    }

    if !journeys.is_empty() || !feature_specs.is_empty() {
        let ids: BTreeSet<&str> = journeys.keys().map(String::as_str).collect();
        match read_text(&specs.join("TRACE.md")) {
            Ok(t) => out.extend(
                check_trace(&t, &ids, &feature_specs, |p| root.join(p).is_file())
                    .into_iter()
                    .map(|m| problem("specs/TRACE.md", m)),
            ),
            Err(e) => out.push(problem("specs/TRACE.md", e)),
        }
    }

    out.sort();
    out
}

/// Collects `*.md` paths relative to `specs/`, never following symlinks.
fn walk(dir: &Path, files: &mut Vec<String>, out: &mut Vec<Problem>, root: &Path) {
    let specs = root.join("specs");
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) => {
            out.push(problem(
                &rel_display(root, dir),
                format!("cannot list: {e}"),
            ));
            return;
        }
    };
    for entry in entries {
        let Ok(entry) = entry else { continue };
        let path: PathBuf = entry.path();
        let Ok(ft) = entry.file_type() else { continue };
        if ft.is_dir() {
            walk(&path, files, out, root);
        } else if ft.is_file()
            && path.extension().is_some_and(|e| e == "md")
            && let Ok(rel) = path.strip_prefix(&specs)
        {
            let parts: Vec<String> = rel
                .components()
                .map(|c| c.as_os_str().to_string_lossy().into_owned())
                .collect();
            files.push(parts.join("/"));
        }
    }
}

fn rel_display(root: &Path, p: &Path) -> String {
    p.strip_prefix(root)
        .unwrap_or(p)
        .to_string_lossy()
        .into_owned()
}

fn read_text(path: &Path) -> Result<String, String> {
    let meta = fs::metadata(path).map_err(|e| format!("cannot read: {e}"))?;
    if meta.len() > MAX_FILE_BYTES {
        return Err(format!(
            "file is {} bytes, above the {MAX_FILE_BYTES}-byte limit",
            meta.len()
        ));
    }
    let bytes = fs::read(path).map_err(|e| format!("cannot read: {e}"))?;
    String::from_utf8(bytes).map_err(|_| "not valid UTF-8".to_owned())
}

/// Removes `<!-- ... -->` spans (an unclosed comment runs to the end).
fn strip_html_comments(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find("<!--") {
        out.push_str(rest.get(..start).unwrap_or_default());
        let after = rest.get(start + 4..).unwrap_or_default();
        match after.find("-->") {
            Some(end) => {
                // Keep line structure so line-based parsing still works.
                let inner = after.get(..end).unwrap_or_default();
                out.extend(inner.chars().filter(|&c| c == '\n'));
                rest = after.get(end + 3..).unwrap_or_default();
            }
            None => {
                out.extend(after.chars().filter(|&c| c == '\n'));
                rest = "";
            }
        }
    }
    out.push_str(rest);
    out
}

fn is_fence(line: &str) -> bool {
    let t = line.trim_start();
    t.starts_with("```") || t.starts_with("~~~")
}

/// A `## ` section: its first word (as the judge reads it) and whether it has content.
#[derive(Debug, PartialEq, Eq)]
pub struct Section {
    pub name: String,
    pub has_content: bool,
}

/// Level-2 sections outside code fences. A level-1 or level-2 heading ends a section;
/// deeper headings count as content.
pub fn sections(text: &str) -> Vec<Section> {
    let text = strip_html_comments(text);
    let mut out: Vec<Section> = Vec::new();
    let mut in_fence = false;
    let mut open = false;
    for line in text.lines() {
        if is_fence(line) {
            in_fence = !in_fence;
            if open && let Some(s) = out.last_mut() {
                s.has_content = true;
            }
            continue;
        }
        if !in_fence {
            if let Some(rest) = line.strip_prefix("##")
                && !rest.starts_with('#')
            {
                let name: String = rest
                    .trim_start()
                    .chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '_')
                    .collect();
                out.push(Section {
                    name,
                    has_content: false,
                });
                open = true;
                continue;
            }
            if line.starts_with("# ") || line == "#" {
                open = false;
                continue;
            }
        }
        if open
            && !line.trim().is_empty()
            && let Some(s) = out.last_mut()
        {
            s.has_content = true;
        }
    }
    out
}

/// Problems with one feature spec, given the required section names.
pub fn check_feature_spec(text: &str, required: &[String]) -> Vec<String> {
    let secs = sections(text);
    let mut out = Vec::new();
    let mut seen = BTreeSet::new();
    for s in &secs {
        if !s.name.is_empty() && !seen.insert(s.name.as_str()) {
            out.push(format!("section `## {}` appears more than once", s.name));
        }
    }
    for r in required {
        match secs.iter().find(|s| &s.name == r) {
            None => out.push(format!(
                "missing checklist section `## {r}` (see specs/_TEMPLATE.md)"
            )),
            Some(s) if !s.has_content => out.push(format!("section `## {r}` is empty")),
            Some(_) => {}
        }
    }
    out
}

/// A parsed journey (CONTRACT §4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Journey {
    pub id: String,
    pub milestone: Option<String>,
    pub platforms: Vec<String>,
    pub tests: Vec<String>,
    pub superseded_by: Option<String>,
}

/// If `s` starts with `J-[A-Z0-9]+-[0-9]+` followed by a non-word character or the end
/// (the judge's `J-[A-Z0-9]+-\d+\b`), returns the ID and the rest.
fn split_journey_id(s: &str) -> Option<(&str, &str)> {
    let body = s.strip_prefix("J-")?;
    // `[A-Z0-9]+` cannot contain `-`, so the area always ends at the first other character.
    let area = body
        .bytes()
        .take_while(|b| b.is_ascii_uppercase() || b.is_ascii_digit())
        .count();
    if area == 0 {
        return None;
    }
    let num = body.get(area..)?.strip_prefix('-')?;
    // `\d+\b`: backtracking inside a digit run never reaches a boundary, so take them all.
    let digits = num.bytes().take_while(u8::is_ascii_digit).count();
    if digits == 0 {
        return None;
    }
    let rest = num.get(digits..)?;
    let boundary = rest
        .chars()
        .next()
        .is_none_or(|c| !(c.is_alphanumeric() || c == '_'));
    let id_len = 2 + area + 1 + digits;
    boundary.then(|| s.get(..id_len).map(|id| (id, rest)))?
}

fn is_journey_heading(line: &str) -> Option<&str> {
    let rest = line.strip_prefix("##")?;
    let trimmed = rest.trim_start();
    (trimmed.len() < rest.len()).then_some(trimmed)
}

/// Parses journeys the way the judge does and reports format problems.
pub fn parse_journeys(text: &str) -> (Vec<Journey>, Vec<String>) {
    let mut out: Vec<Journey> = Vec::new();
    let mut problems = Vec::new();
    // (id, field lines) per journey; the judge's body runs to the next journey heading.
    let mut bodies: Vec<(String, Vec<String>)> = Vec::new();
    let mut in_fence = false;
    for (n, line) in text.lines().enumerate() {
        let lineno = n + 1;
        if is_fence(line) {
            in_fence = !in_fence;
        }
        if let Some(head) = is_journey_heading(line) {
            if let Some((id, rest)) = split_journey_id(head) {
                match rest.strip_prefix(':') {
                    Some(title) if !title.trim().is_empty() => {}
                    _ => problems.push(format!("line {lineno}: expected `## {id}: <title>`")),
                }
                bodies.push((id.to_owned(), Vec::new()));
                continue;
            }
            if !in_fence && head.starts_with("J-") {
                problems.push(format!(
                    "line {lineno}: malformed journey heading (expected `## J-<AREA>-<NN>: <title>`)"
                ));
                continue;
            }
        }
        if let Some((_, lines)) = bodies.last_mut() {
            lines.push(line.to_owned());
        }
    }

    for (id, lines) in bodies {
        let mut fields: BTreeMap<&str, String> = BTreeMap::new();
        for line in &lines {
            let Some(rest) = line.strip_prefix('-') else {
                continue;
            };
            let rest = rest.trim_start();
            let Some((key, value)) = rest.split_once(':') else {
                continue;
            };
            let Some(&key) = FIELDS.iter().find(|f| **f == key) else {
                continue;
            };
            if fields.contains_key(key) {
                problems.push(format!(
                    "{id}: field `{key}` given more than once (the judge reads only the first)"
                ));
                continue;
            }
            fields.insert(key, value.trim().to_owned());
        }
        let j = check_journey_fields(&id, &fields, &mut problems);
        out.push(j);
    }
    (out, problems)
}

fn list(value: &str) -> (Vec<String>, bool) {
    let items: Vec<String> = value.split(',').map(|s| s.trim().to_owned()).collect();
    let has_empty = items.iter().any(String::is_empty);
    (
        items.into_iter().filter(|s| !s.is_empty()).collect(),
        has_empty,
    )
}

fn check_journey_fields(
    id: &str,
    fields: &BTreeMap<&str, String>,
    problems: &mut Vec<String>,
) -> Journey {
    let milestone = fields.get("milestone").cloned();
    match milestone.as_deref() {
        None | Some("") => problems.push(format!("{id}: missing `- milestone: M<N>`")),
        Some(m) => {
            let ok = m
                .strip_prefix('M')
                .is_some_and(|d| !d.is_empty() && d.bytes().all(|b| b.is_ascii_digit()));
            if !ok {
                problems.push(format!("{id}: milestone `{m}` is not M<N>"));
            }
        }
    }

    let mut platforms = Vec::new();
    match fields.get("platform").map(String::as_str) {
        None | Some("") => problems.push(format!("{id}: missing `- platform: web, macos`")),
        Some(v) => {
            let (items, has_empty) = list(v);
            if has_empty {
                problems.push(format!("{id}: empty item in platform list"));
            }
            for p in items {
                if !PLATFORMS.contains(&p.as_str()) {
                    problems.push(format!("{id}: unknown platform `{p}` (web, macos)"));
                } else if platforms.contains(&p) {
                    problems.push(format!("{id}: platform `{p}` listed twice"));
                } else {
                    platforms.push(p);
                }
            }
        }
    }

    let superseded_by = fields
        .get("superseded_by")
        .filter(|s| !s.is_empty())
        .cloned();
    if let Some(s) = &superseded_by
        && split_journey_id(s).is_none_or(|(_, rest)| !rest.is_empty())
    {
        problems.push(format!("{id}: superseded_by `{s}` is not a journey ID"));
    }

    let mut tests = Vec::new();
    match fields.get("test").map(String::as_str) {
        None | Some("") => {
            if superseded_by.is_none() {
                problems.push(format!("{id}: missing `- test: e2e.<platform>.<id>`"));
            }
        }
        Some(v) => {
            let (items, has_empty) = list(v);
            if has_empty {
                problems.push(format!("{id}: empty item in test list"));
            }
            for t in items {
                if tests.contains(&t) {
                    problems.push(format!("{id}: test `{t}` listed twice"));
                    continue;
                }
                let parts: Vec<&str> = t.split('.').collect();
                let well_formed = parts.len() == 3
                    && parts.first() == Some(&"e2e")
                    && parts.get(2).is_some_and(|s| {
                        !s.is_empty()
                            && s.bytes()
                                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
                    });
                match parts.get(1) {
                    Some(p) if well_formed && PLATFORMS.contains(p) => {
                        if !platforms.iter().any(|d| d == p) {
                            problems.push(format!(
                                "{id}: test `{t}` is for platform `{p}`, which is not in `platform:`"
                            ));
                        }
                    }
                    _ => problems.push(format!(
                        "{id}: test `{t}` is not `e2e.<web|macos>.<id>` (id: a-z, 0-9, _)"
                    )),
                }
                tests.push(t);
            }
            for p in &platforms {
                let prefix = format!("e2e.{p}.");
                if !tests.iter().any(|t| t.starts_with(&prefix)) {
                    problems.push(format!("{id}: platform `{p}` has no test"));
                }
            }
        }
    }

    Journey {
        id: id.to_owned(),
        milestone,
        platforms,
        tests,
        superseded_by,
    }
}

/// Journey IDs mentioned anywhere in `text`.
fn mentioned_journeys(text: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let mut prev: Option<char> = None;
    for (i, c) in text.char_indices() {
        let at_word_start = prev.is_none_or(|p| !(p.is_alphanumeric() || p == '_' || p == '-'));
        if c == 'J'
            && at_word_start
            && let Some(rest) = text.get(i..)
            && let Some((id, _)) = split_journey_id(rest)
        {
            out.insert(id.to_owned());
        }
        prev = Some(c);
    }
    out
}

/// `specs/...md` paths mentioned anywhere in `text`.
fn mentioned_spec_paths(text: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let mut rest = text;
    while let Some(i) = rest.find("specs/") {
        let tail = rest.get(i..).unwrap_or_default();
        let len = tail
            .find(|c: char| c.is_whitespace() || "()[]|`'\"<>,;".contains(c))
            .unwrap_or(tail.len());
        let token = tail.get(..len).unwrap_or_default();
        let token = token.trim_end_matches(['.', ':']);
        let preceded_ok = rest
            .get(..i)
            .and_then(|b| b.chars().next_back())
            .is_none_or(|p| !(p.is_alphanumeric() || p == '_' || p == '-'));
        // Glob patterns such as `specs/journeys/*.md` are not file mentions.
        if preceded_ok && token.ends_with(".md") && !token.contains('*') {
            out.insert(token.to_owned());
        }
        rest = tail.get(len.max(1)..).unwrap_or_default();
    }
    out
}

/// TRACE.md cross-checks.
pub fn check_trace(
    text: &str,
    journeys: &BTreeSet<&str>,
    feature_specs: &[String],
    exists: impl Fn(&str) -> bool,
) -> Vec<String> {
    let mut out = Vec::new();
    let mentioned = mentioned_journeys(text);
    for j in journeys {
        if !mentioned.contains(*j) {
            out.push(format!("journey {j} is not traced"));
        }
    }
    for m in &mentioned {
        if !journeys.contains(m.as_str()) {
            out.push(format!("mentions unknown journey {m}"));
        }
    }
    let paths = mentioned_spec_paths(text);
    for s in feature_specs {
        if !paths.contains(s) {
            out.push(format!("feature spec {s} is not traced"));
        }
    }
    for p in &paths {
        if !exists(p) {
            out.push(format!("mentions missing file {p}"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn req() -> Vec<String> {
        CHECKLIST.iter().map(|s| (*s).to_owned()).collect()
    }

    fn full_spec() -> String {
        CHECKLIST
            .iter()
            .map(|s| format!("## {s}\ntext\n\n"))
            .collect()
    }

    #[test]
    fn complete_spec_passes() {
        assert_eq!(
            check_feature_spec(&full_spec(), &req()),
            Vec::<String>::new()
        );
    }

    #[test]
    fn missing_and_empty_sections_are_reported() {
        let text = full_spec()
            .replace("## Undo\ntext\n", "")
            .replace("## Limits\ntext\n", "## Limits\n   \n<!-- todo -->\n");
        let p = check_feature_spec(&text, &req());
        assert_eq!(
            p,
            vec![
                "missing checklist section `## Undo` (see specs/_TEMPLATE.md)".to_owned(),
                "section `## Limits` is empty".to_owned(),
            ]
        );
    }

    #[test]
    fn duplicate_section_is_reported() {
        let text = format!("{}## Undo\nmore\n", full_spec());
        assert_eq!(
            check_feature_spec(&text, &req()),
            vec!["section `## Undo` appears more than once".to_owned()]
        );
    }

    #[test]
    fn headings_in_fences_and_comments_do_not_count() {
        let text = full_spec().replace(
            "## Security\ntext\n",
            "```\n## Security\n```\n<!--\n## Security\n-->\n",
        );
        let p = check_feature_spec(&text, &req());
        assert_eq!(
            p,
            vec!["missing checklist section `## Security` (see specs/_TEMPLATE.md)".to_owned()]
        );
    }

    #[test]
    fn level1_heading_ends_a_section_and_level3_is_content() {
        let s = sections("## A\n# Top\nx\n## B\n### sub\n");
        assert_eq!(
            s,
            vec![
                Section {
                    name: "A".into(),
                    has_content: false
                },
                Section {
                    name: "B".into(),
                    has_content: true
                },
            ]
        );
    }

    #[test]
    fn unclosed_comment_hides_the_rest() {
        let s = sections("## A\n<!-- x\n## B\nbody\n");
        assert_eq!(
            s,
            vec![Section {
                name: "A".into(),
                has_content: false
            }]
        );
    }

    const GOOD: &str = "## J-ANN-01: highlight\n- milestone: M5\n- platform: web, macos\n\
        - test: e2e.web.ann_highlight, e2e.macos.ann_highlight\n\nSteps: open → highlight\n";

    #[test]
    fn good_journey_parses() {
        let (js, ps) = parse_journeys(GOOD);
        assert_eq!(ps, Vec::<String>::new());
        assert_eq!(
            js,
            vec![Journey {
                id: "J-ANN-01".into(),
                milestone: Some("M5".into()),
                platforms: vec!["web".into(), "macos".into()],
                tests: vec![
                    "e2e.web.ann_highlight".into(),
                    "e2e.macos.ann_highlight".into()
                ],
                superseded_by: None,
            }]
        );
    }

    fn problems_for(text: &str) -> Vec<String> {
        parse_journeys(text).1
    }

    #[test]
    fn journey_field_rules() {
        let cases: &[(&str, &str, &str)] = &[
            (
                "- milestone: M5\n",
                "",
                "J-ANN-01: missing `- milestone: M<N>`",
            ),
            (
                "- milestone: M5",
                "- milestone: m5",
                "J-ANN-01: milestone `m5` is not M<N>",
            ),
            (
                "- milestone: M5",
                "- milestone: M",
                "J-ANN-01: milestone `M` is not M<N>",
            ),
            (
                "- platform: web, macos\n",
                "",
                "J-ANN-01: missing `- platform: web, macos`",
            ),
            (
                "web, macos",
                "web, ios",
                "J-ANN-01: unknown platform `ios` (web, macos)",
            ),
            (
                "web, macos",
                "web, , macos",
                "J-ANN-01: empty item in platform list",
            ),
            (
                "web, macos",
                "web, web, macos",
                "J-ANN-01: platform `web` listed twice",
            ),
            (
                "- test: e2e.web.ann_highlight, e2e.macos.ann_highlight\n",
                "",
                "J-ANN-01: missing `- test: e2e.<platform>.<id>`",
            ),
            (
                "e2e.macos.ann_highlight",
                "e2e.macos.Ann",
                "J-ANN-01: test `e2e.macos.Ann` is not `e2e.<web|macos>.<id>` (id: a-z, 0-9, _)",
            ),
            (
                "e2e.macos.ann_highlight",
                "e2e.macos.a.b",
                "J-ANN-01: test `e2e.macos.a.b` is not `e2e.<web|macos>.<id>` (id: a-z, 0-9, _)",
            ),
            (
                "e2e.macos.ann_highlight",
                "e2e.web.ann_highlight",
                "J-ANN-01: test `e2e.web.ann_highlight` listed twice",
            ),
            (
                "- platform: web, macos",
                "- platform: web",
                "J-ANN-01: test `e2e.macos.ann_highlight` is for platform `macos`, which is not in `platform:`",
            ),
            (
                ", e2e.macos.ann_highlight",
                "",
                "J-ANN-01: platform `macos` has no test",
            ),
            (
                "Steps:",
                "- test: e2e.web.x\nSteps:",
                "J-ANN-01: field `test` given more than once (the judge reads only the first)",
            ),
            (
                "## J-ANN-01: highlight",
                "## J-ANN-01:",
                "line 1: expected `## J-ANN-01: <title>`",
            ),
            (
                "## J-ANN-01: highlight",
                "## J-ANN-01 highlight",
                "line 1: expected `## J-ANN-01: <title>`",
            ),
        ];
        for (from, to, want) in cases {
            let text = GOOD.replacen(from, to, 1);
            assert_ne!(text, GOOD, "case {want} did not change the input");
            let got = problems_for(&text);
            assert!(got.iter().any(|p| p == want), "want {want:?} in {got:?}");
        }
    }

    #[test]
    fn malformed_heading_outside_fence_is_reported_inside_is_not() {
        let text = "## J-ann-01: x\n```\n## J-AREA-NN: title\n```\n";
        assert_eq!(
            problems_for(text),
            vec![
                "line 1: malformed journey heading (expected `## J-<AREA>-<NN>: <title>`)"
                    .to_owned()
            ]
        );
    }

    #[test]
    fn superseded_journey_needs_no_test_but_a_valid_id() {
        let text = "## J-A-1: old\n- milestone: M1\n- platform: web\n- superseded_by: J-A-2\n";
        assert_eq!(problems_for(text), Vec::<String>::new());
        let bad = text.replace("J-A-2", "nope");
        assert_eq!(
            problems_for(&bad),
            vec!["J-A-1: superseded_by `nope` is not a journey ID".to_owned()]
        );
    }

    #[test]
    fn body_runs_to_the_next_journey_heading_like_the_judge() {
        let text = "## J-A-1: x\n## Notes\n- milestone: M1\n- platform: web\n- test: e2e.web.a\n";
        let (js, ps) = parse_journeys(text);
        assert_eq!(ps, Vec::<String>::new());
        assert_eq!(js.first().map(|j| j.tests.len()), Some(1));
    }

    #[test]
    fn journey_id_split_matches_judge_regex() {
        assert_eq!(split_journey_id("J-ANN-01: x"), Some(("J-ANN-01", ": x")));
        assert_eq!(split_journey_id("J-ANN-01"), Some(("J-ANN-01", "")));
        assert_eq!(split_journey_id("J-A1-2-3"), Some(("J-A1-2", "-3")));
        assert_eq!(split_journey_id("J-ANN-01x"), None);
        assert_eq!(split_journey_id("J-ANN-"), None);
        assert_eq!(split_journey_id("J--1"), None);
        assert_eq!(split_journey_id("J-AREA-NN"), None);
    }

    #[test]
    fn trace_cross_checks() {
        let ids: BTreeSet<&str> = ["J-A-1", "J-A-2"].into_iter().collect();
        let specs = vec!["specs/x/y.md".to_owned(), "specs/x/z.md".to_owned()];
        let text = "| R1 | J-A-1, J-B-9 | [specs/x/y.md](x/y.md) | specs/x/gone.md. |\nJ-AREA-NN";
        let p = check_trace(text, &ids, &specs, |p| p != "specs/x/gone.md");
        assert_eq!(
            p,
            vec![
                "journey J-A-2 is not traced".to_owned(),
                "mentions unknown journey J-B-9".to_owned(),
                "feature spec specs/x/z.md is not traced".to_owned(),
                "mentions missing file specs/x/gone.md".to_owned(),
            ]
        );
    }

    #[test]
    fn mention_scanners_respect_word_boundaries() {
        assert!(mentioned_journeys("XJ-A-1 and -J-A-2").is_empty());
        assert_eq!(
            mentioned_spec_paths("myspecs/a.md ../specs/b.md specs/j/*.md"),
            {
                let mut s = BTreeSet::new();
                s.insert("specs/b.md".to_owned());
                s
            }
        );
    }

    #[test]
    fn comments_keep_line_numbers() {
        assert_eq!(strip_html_comments("a<!--\n\n-->b\n<!-- x"), "a\n\nb\n");
    }
}
