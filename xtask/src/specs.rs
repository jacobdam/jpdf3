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
        if parts.len() > 2 && parts.first() == Some(&"journeys") {
            out.push(problem(
                &display,
                "journey files must be directly in specs/journeys/ (the judge ignores subdirectories)",
            ));
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
            } else if supersede_cycle(id, &journeys) {
                out.push(problem(
                    path,
                    format!(
                        "{id}: superseded_by chain is a cycle (it must end in a journey with tests)"
                    ),
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
    // Identical lines are merged, so a file read by name that the walk also flags (a FIFO
    // `_TEMPLATE.md`) is reported once; a repeated identical problem adds nothing either.
    out.dedup();
    out
}

/// True if following `superseded_by` from `start` never reaches a journey without one.
fn supersede_cycle(start: &str, journeys: &BTreeMap<String, (String, Journey)>) -> bool {
    let mut cur = start;
    for _ in 0..=journeys.len() {
        match journeys
            .get(cur)
            .and_then(|(_, j)| j.superseded_by.as_deref())
        {
            Some(next) if journeys.contains_key(next) => cur = next,
            _ => return false,
        }
    }
    true
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
        let entry = match entry {
            Ok(e) => e,
            Err(e) => {
                out.push(problem(
                    &rel_display(root, dir),
                    format!("cannot list: {e}"),
                ));
                continue;
            }
        };
        let path: PathBuf = entry.path();
        let ft = match entry.file_type() {
            Ok(ft) => ft,
            Err(e) => {
                out.push(problem(
                    &rel_display(root, &path),
                    format!("cannot stat: {e}"),
                ));
                continue;
            }
        };
        if ft.is_symlink() {
            out.push(problem(
                &rel_display(root, &path),
                "symlink not followed; specs must be regular files",
            ));
        } else if ft.is_dir() {
            // The judge's `glob("*.md")` matches directories too, then fails to read them.
            if entry.file_name().to_string_lossy().ends_with(".md") {
                out.push(problem(
                    &rel_display(root, &path),
                    "directory named like a Markdown file (the judge would try to read it)",
                ));
            }
            walk(&path, files, out, root);
        } else if ft.is_file()
            // Not `extension()`: a file named exactly `.md` has none, but the judge's glob matches it.
            && entry.file_name().to_string_lossy().ends_with(".md")
            && let Ok(rel) = path.strip_prefix(&specs)
        {
            let parts: Vec<String> = rel
                .components()
                .map(|c| c.as_os_str().to_string_lossy().into_owned())
                .collect();
            files.push(parts.join("/"));
        } else if !ft.is_file() && entry.file_name().to_string_lossy().ends_with(".md") {
            // A FIFO, socket or device: the judge's glob matches it and would block or fail.
            out.push(problem(
                &rel_display(root, &path),
                "cannot read: not a regular file",
            ));
        }
    }
}

/// Every test ID named on a `- test:` line in `root/specs/journeys/*.md` (the judge's selection:
/// not recursive; CONTRACT §2 grades only these). `ci linux` and `ci macos` fail on a reported
/// ID outside this set and the harness IDs (docs/adr/0002 item 2). Any file that can't be read
/// is an error, so a damaged tree never shrinks the set silently; format problems are
/// check-specs' job and don't stop the collection.
pub fn journey_test_ids(root: &Path) -> Result<BTreeSet<String>, String> {
    let dir = root.join("specs").join("journeys");
    let entries = fs::read_dir(&dir).map_err(|e| format!("{}: cannot list: {e}", dir.display()))?;
    let mut ids = BTreeSet::new();
    for entry in entries {
        let entry = entry.map_err(|e| format!("{}: cannot list: {e}", dir.display()))?;
        // Same match as the walk: a name ending in `.md` (the judge's glob), any file type.
        if !entry.file_name().to_string_lossy().ends_with(".md") {
            continue;
        }
        let path = entry.path();
        let text = read_text(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        for j in parse_journeys(&text).0 {
            ids.extend(j.tests);
        }
    }
    Ok(ids)
}

fn rel_display(root: &Path, p: &Path) -> String {
    p.strip_prefix(root)
        .unwrap_or(p)
        .to_string_lossy()
        .into_owned()
}

fn read_text(path: &Path) -> Result<String, String> {
    // Only a regular file is opened: a symlink isn't followed (like the walk), and a FIFO or
    // device would block the open or never end. The open file is checked again (fstat), and
    // the read is capped, so a file grown after the check can't flood us. Not covered: a
    // swap to a FIFO between the lstat and the open (the open would block); these are
    // repository files. Closing it needs `O_NONBLOCK` through `custom_flags`, with
    // per-target flag values; not done. Likewise a swap to a symlink in that window is
    // followed (the fstat then sees whatever it points to).
    let link = fs::symlink_metadata(path).map_err(|e| format!("cannot read: {e}"))?;
    if !link.is_file() {
        return Err(not_regular(&link));
    }
    let file = fs::File::open(path).map_err(|e| format!("cannot read: {e}"))?;
    let meta = file.metadata().map_err(|e| format!("cannot read: {e}"))?;
    if !meta.is_file() {
        return Err(not_regular(&meta));
    }
    if meta.len() > MAX_FILE_BYTES {
        return Err(format!(
            "file is {} bytes, above the {MAX_FILE_BYTES}-byte limit",
            meta.len()
        ));
    }
    let bytes = read_bounded(file, MAX_FILE_BYTES)?;
    String::from_utf8(bytes).map_err(|_| "not valid UTF-8".to_owned())
}

fn not_regular(meta: &fs::Metadata) -> String {
    let kind = if meta.file_type().is_symlink() {
        "a symlink (not followed)"
    } else {
        "not a regular file"
    };
    format!("cannot read: {kind}")
}

/// Reads at most `limit` bytes; more is an error (the file grew after its size was checked).
fn read_bounded(r: impl std::io::Read, limit: u64) -> Result<Vec<u8>, String> {
    use std::io::Read as _;
    let mut bytes = Vec::new();
    r.take(limit.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|e| format!("cannot read: {e}"))?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > limit {
        return Err(format!(
            "file grew above the {limit}-byte limit while being read"
        ));
    }
    Ok(bytes)
}

/// Splits a line outside code fences into visible text, tracking `<!-- -->` comments across
/// lines. Comment markers inside inline code spans are text, not comments.
fn visible_text(line: &str, in_comment: &mut bool) -> String {
    let mut out = String::new();
    let mut rest = line;
    let mut in_code = false;
    while !rest.is_empty() {
        if *in_comment {
            match rest.find("-->") {
                Some(i) => {
                    *in_comment = false;
                    rest = rest.get(i + 3..).unwrap_or_default();
                }
                None => return out,
            }
            continue;
        }
        let mut chars = rest.char_indices();
        let Some((_, c)) = chars.next() else { break };
        if c == '`' {
            in_code = !in_code;
        } else if !in_code && rest.starts_with("<!--") {
            *in_comment = true;
            rest = rest.get(4..).unwrap_or_default();
            continue;
        }
        out.push(c);
        rest = rest.get(c.len_utf8()..).unwrap_or_default();
    }
    out
}

/// Whitespace as Python's `\s` (the judge's regexes) sees it: Rust's `White_Space` plus the
/// information separators U+001C–U+001F.
fn py_space(c: char) -> bool {
    c.is_whitespace() || ('\u{1c}'..='\u{1f}').contains(&c)
}

/// Fenced code blocks per CommonMark §4.5: an opening fence is up to 3 spaces of indent and a
/// run of at least 3 backticks or tildes; it closes only on the same character, at least as
/// long a run, and nothing but whitespace after it.
#[derive(Debug, Default)]
struct Fence {
    open: Option<(char, usize)>,
}

impl Fence {
    /// Feeds one line; returns true if it is a fence line or inside a fenced block.
    fn line(&mut self, line: &str) -> bool {
        let indent = line.bytes().take_while(|b| *b == b' ').count();
        let t = line.get(indent..).unwrap_or_default();
        let marker = t
            .chars()
            .next()
            .filter(|c| indent <= 3 && (*c == '`' || *c == '~'));
        let run = marker.map_or(0, |m| t.chars().take_while(|c| *c == m).count());
        // The marker is ASCII, so `run` chars are `run` bytes.
        let after = t.get(run..).unwrap_or_default();
        match (self.open, marker) {
            (None, Some(m)) if run >= 3 && (m == '~' || !after.contains('`')) => {
                self.open = Some((m, run));
                true
            }
            (None, _) => false,
            (Some((m, n)), Some(c)) if c == m && run >= n && after.trim().is_empty() => {
                self.open = None;
                true
            }
            (Some(_), _) => true,
        }
    }
}

/// A `## ` section: its first word (as the judge reads it) and whether it has content.
#[derive(Debug, PartialEq, Eq)]
pub struct Section {
    pub name: String,
    pub has_content: bool,
}

/// Level-2 sections (`##` + whitespace + name, at column 0, as the judge requires) outside
/// code fences and HTML comments. A level-1 or level-2 heading ends a section; deeper
/// headings count as content.
pub fn sections(text: &str) -> Vec<Section> {
    let mut out: Vec<Section> = Vec::new();
    let mut fence = Fence::default();
    let mut in_comment = false;
    let mut open = false;
    for line in text.lines() {
        if !in_comment && fence.line(line) {
            if open && let Some(s) = out.last_mut() {
                s.has_content = true;
            }
            continue;
        }
        let starts_in_comment = in_comment;
        let visible = visible_text(line, &mut in_comment);
        if !starts_in_comment {
            if let Some(rest) = line.strip_prefix("##")
                && rest.starts_with(py_space)
            {
                let name: String = rest
                    .trim_start_matches(py_space)
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
            && !visible.trim().is_empty()
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
    let trimmed = rest.trim_start_matches(py_space);
    (trimmed.len() < rest.len()).then_some(trimmed)
}

/// Headings the judge would silently skip: `### J-…`, ` ## J-…`, `##J-…`, `# J-…`.
fn is_almost_journey_heading(line: &str) -> bool {
    // A leading BOM or indent also hides the heading from the judge's `^##`.
    let t = line.trim_start_matches(|c| py_space(c) || c == '\u{feff}');
    t.starts_with('#')
        && t.trim_start_matches('#')
            .trim_start_matches(py_space)
            .starts_with("J-")
        && is_journey_heading(line).is_none_or(|h| !h.starts_with("J-"))
}

/// Parses journeys the way the judge does and reports format problems.
pub fn parse_journeys(text: &str) -> (Vec<Journey>, Vec<String>) {
    let mut out: Vec<Journey> = Vec::new();
    let mut problems = Vec::new();
    // (id, field lines) per journey; the judge's body runs to the next journey heading.
    let mut bodies: Vec<(String, Vec<String>)> = Vec::new();
    let mut fence = Fence::default();
    let mut fence_start = 0;
    for (n, line) in text.lines().enumerate() {
        let lineno = n + 1;
        let was_open = fence.open.is_some();
        let in_fence = fence.line(line);
        if !was_open && fence.open.is_some() {
            fence_start = lineno;
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
        // The judge's `^##\s+` crosses newlines: a bare `##` line takes the next lines' text
        // as its heading.
        if line
            .strip_prefix("##")
            .is_some_and(|r| r.chars().all(py_space))
        {
            problems.push(format!(
                "line {lineno}: bare `##` line (the judge reads the following text as a heading)"
            ));
        } else if !in_fence && is_almost_journey_heading(line) {
            problems.push(format!(
                "line {lineno}: malformed journey heading (expected `## J-<AREA>-<NN>: <title>`)"
            ));
        }
        if let Some((_, lines)) = bodies.last_mut() {
            lines.push(line.to_owned());
        }
    }

    // The judge ignores fences, so an unclosed one would hide malformed headings to the end.
    if fence.open.is_some() {
        problems.push(format!("line {fence_start}: code fence never closed"));
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
            if value.trim().is_empty() {
                problems.push(format!(
                    "{id}: field `{key}` has no value (the judge would read the next line as its value)"
                ));
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
        None => problems.push(format!("{id}: missing `- milestone: M<N>`")),
        Some("") => {} // reported as "has no value"
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
        None => problems.push(format!("{id}: missing `- platform: web, macos`")),
        Some("") => {} // reported as "has no value"
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
        Some("") => {} // reported as "has no value"
        None => {
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

/// Cleans a path token: drops Markdown emphasis around it, a `#anchor` and trailing
/// punctuation. Returns `None` for globs (`*` inside) and non-`.md` tokens.
fn clean_md_path(token: &str) -> Option<&str> {
    let t = token.trim_matches(['*', '_']);
    let t = t.split('#').next().unwrap_or_default();
    let t = t.trim_end_matches(['.', ':']).trim_matches(['*', '_']);
    (t.ends_with(".md") && !t.contains('*')).then_some(t)
}

/// Lexically resolves `.` and `..` in a repo-relative path; `None` if it leaves the repo.
fn normalize(path: &str) -> Option<String> {
    let mut parts: Vec<&str> = Vec::new();
    for c in path.split('/') {
        match c {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            c => parts.push(c),
        }
    }
    Some(parts.join("/"))
}

/// `specs/...md` paths mentioned anywhere in `text`, not as part of a longer path.
fn mentioned_spec_paths(text: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let mut rest = text;
    while let Some(i) = rest.find("specs/") {
        let tail = rest.get(i..).unwrap_or_default();
        let len = tail
            .find(|c: char| c.is_whitespace() || "()[]|`'\"<>,;".contains(c))
            .unwrap_or(tail.len());
        let token = tail.get(..len).unwrap_or_default();
        let preceded_ok = rest
            .get(..i)
            .and_then(|b| b.chars().next_back())
            .is_none_or(|p| !(p.is_alphanumeric() || "_-./".contains(p)));
        if preceded_ok && let Some(t) = clean_md_path(token) {
            // Lexical only; a path leaving the repo is kept as-is and reported by `check_trace`.
            out.insert(normalize(t).unwrap_or_else(|| t.to_owned()));
        }
        rest = tail.get(len.max(1)..).unwrap_or_default();
    }
    out
}

/// Relative Markdown link targets `](target.md)` in `text`, resolved lexically against
/// `specs/`, and targets that would leave the repository (never resolved).
fn link_targets(text: &str) -> (BTreeSet<String>, BTreeSet<String>) {
    let mut out = BTreeSet::new();
    let mut escaping = BTreeSet::new();
    let mut rest = text;
    while let Some(i) = rest.find("](") {
        let tail = rest.get(i + 2..).unwrap_or_default();
        let len = tail.find([')', ' ']).unwrap_or(tail.len());
        let target = tail.get(..len).unwrap_or_default();
        let external = target.contains("://") || target.starts_with('/') || target.starts_with('#');
        if !external && let Some(t) = clean_md_path(target) {
            match normalize(&format!("specs/{t}")) {
                Some(n) => out.insert(n),
                None => escaping.insert(t.to_owned()),
            };
        }
        rest = tail;
    }
    (out, escaping)
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
        if normalize(p).is_none() {
            out.push(format!("path {p} leaves the repository (not checked)"));
        } else if !exists(p) {
            out.push(format!("mentions missing file {p}"));
        }
    }
    let (links, escaping) = link_targets(text);
    for p in links {
        if !paths.contains(&p) && !exists(&p) {
            out.push(format!("links to missing file {p}"));
        }
    }
    for p in escaping {
        out.push(format!(
            "link target {p} leaves the repository (not checked)"
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_are_capped_at_the_limit() {
        // An endless reader stands in for a file that grows after its size was checked.
        assert_eq!(read_bounded(&[b'x'; 8][..], 8).unwrap().len(), 8);
        assert_eq!(
            read_bounded(&[b'x'; 9][..], 8).unwrap_err(),
            "file grew above the 8-byte limit while being read"
        );
        assert!(read_bounded(std::io::repeat(b'x'), 1 << 20).is_err());
        assert_eq!(read_bounded(&b""[..], 0).unwrap().len(), 0);
    }

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
        let cases: &[(&str, &str, &[&str])] = &[
            (
                "- milestone: M5\n",
                "",
                &["J-ANN-01: missing `- milestone: M<N>`"],
            ),
            (
                "- milestone: M5",
                "- milestone: m5",
                &["J-ANN-01: milestone `m5` is not M<N>"],
            ),
            (
                "- milestone: M5",
                "- milestone: M",
                &["J-ANN-01: milestone `M` is not M<N>"],
            ),
            (
                "- platform: web, macos\n",
                "",
                &[
                    "J-ANN-01: missing `- platform: web, macos`",
                    "J-ANN-01: test `e2e.web.ann_highlight` is for platform `web`, which is not in `platform:`",
                    "J-ANN-01: test `e2e.macos.ann_highlight` is for platform `macos`, which is not in `platform:`",
                ],
            ),
            (
                "web, macos",
                "web, ios",
                &[
                    "J-ANN-01: unknown platform `ios` (web, macos)",
                    "J-ANN-01: test `e2e.macos.ann_highlight` is for platform `macos`, which is not in `platform:`",
                ],
            ),
            (
                "web, macos",
                "web, , macos",
                &["J-ANN-01: empty item in platform list"],
            ),
            (
                "web, macos",
                "web, web, macos",
                &["J-ANN-01: platform `web` listed twice"],
            ),
            (
                "- test: e2e.web.ann_highlight, e2e.macos.ann_highlight\n",
                "",
                &["J-ANN-01: missing `- test: e2e.<platform>.<id>`"],
            ),
            (
                "e2e.macos.ann_highlight",
                "e2e.macos.Ann",
                &["J-ANN-01: test `e2e.macos.Ann` is not `e2e.<web|macos>.<id>` (id: a-z, 0-9, _)"],
            ),
            (
                "e2e.macos.ann_highlight",
                "e2e.macos.a.b",
                &["J-ANN-01: test `e2e.macos.a.b` is not `e2e.<web|macos>.<id>` (id: a-z, 0-9, _)"],
            ),
            (
                "e2e.macos.ann_highlight",
                "e2e.web.ann_highlight",
                &[
                    "J-ANN-01: test `e2e.web.ann_highlight` listed twice",
                    "J-ANN-01: platform `macos` has no test",
                ],
            ),
            (
                "- platform: web, macos",
                "- platform: web",
                &[
                    "J-ANN-01: test `e2e.macos.ann_highlight` is for platform `macos`, which is not in `platform:`",
                ],
            ),
            (
                ", e2e.macos.ann_highlight",
                "",
                &["J-ANN-01: platform `macos` has no test"],
            ),
            (
                "Steps:",
                "- test: e2e.web.x\nSteps:",
                &["J-ANN-01: field `test` given more than once (the judge reads only the first)"],
            ),
            (
                "## J-ANN-01: highlight",
                "## J-ANN-01:",
                &["line 1: expected `## J-ANN-01: <title>`"],
            ),
            (
                "## J-ANN-01: highlight",
                "## J-ANN-01 highlight",
                &["line 1: expected `## J-ANN-01: <title>`"],
            ),
        ];
        for (from, to, want) in cases {
            let text = GOOD.replacen(from, to, 1);
            assert_ne!(text, GOOD, "case {want:?} did not change the input");
            let got = problems_for(&text);
            assert_eq!(
                got, *want,
                "input:
{text}"
            );
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
        // `../specs/b.md` is part of a longer path, `specs/j/*.md` is a glob: neither counts.
        assert_eq!(
            mentioned_spec_paths("myspecs/a.md ../specs/b.md specs/j/*.md (specs/e.md)"),
            BTreeSet::from(["specs/e.md".to_owned()])
        );
    }

    #[test]
    fn comment_markers_in_inline_code_are_text() {
        let mut c = false;
        assert_eq!(
            visible_text("Strip `<!--` comments", &mut c),
            "Strip `<!--` comments"
        );
        assert!(!c);
        assert_eq!(visible_text("a<!-- x --> b <!-- y", &mut c), "a b ");
        assert!(c);
        assert_eq!(visible_text("still --> z", &mut c), " z");
        assert!(!c);
        assert_eq!(visible_text("é<!--é-->é", &mut c), "éé");
        let text = full_spec().replace("## Undo\ntext\n", "## Undo\nStrip `<!--` from input.\n");
        assert_eq!(check_feature_spec(&text, &req()), Vec::<String>::new());
    }

    #[test]
    fn heading_needs_whitespace_after_hashes_like_the_judge() {
        let text = full_spec().replace("## Undo", "##Undo");
        assert_eq!(
            check_feature_spec(&text, &req()),
            vec!["missing checklist section `## Undo` (see specs/_TEMPLATE.md)".to_owned()]
        );
    }

    #[test]
    fn headings_the_judge_would_skip_are_reported() {
        for (line, want) in [
            (
                "### J-A-1: x",
                "line 1: malformed journey heading (expected `## J-<AREA>-<NN>: <title>`)",
            ),
            (
                " ## J-A-1: x",
                "line 1: malformed journey heading (expected `## J-<AREA>-<NN>: <title>`)",
            ),
            (
                "##J-A-1: x",
                "line 1: malformed journey heading (expected `## J-<AREA>-<NN>: <title>`)",
            ),
            (
                "##\nJ-A-1: x",
                "line 1: bare `##` line (the judge reads the following text as a heading)",
            ),
        ] {
            assert_eq!(problems_for(line), vec![want.to_owned()], "input {line:?}");
        }
    }

    #[test]
    fn judge_whitespace_includes_information_separators() {
        assert_eq!(
            problems_for("##\u{1c}J-A-1: x\n- milestone: M1\n- platform: web\n- test: e2e.web.a\n"),
            Vec::<String>::new()
        );
        assert_eq!(parse_journeys("##\u{1c}J-A-1: x\n").0.len(), 1);
        assert_eq!(
            problems_for("##\u{1f}\nJ-A-1"),
            vec![
                "line 1: bare `##` line (the judge reads the following text as a heading)"
                    .to_owned()
            ]
        );
        let text = full_spec().replace("## Undo", "##\u{1c}Undo");
        assert_eq!(check_feature_spec(&text, &req()), Vec::<String>::new());
    }

    #[test]
    fn bom_before_a_journey_heading_is_reported() {
        assert_eq!(
            problems_for("\u{feff}## J-A-1: x\n"),
            vec![
                "line 1: malformed journey heading (expected `## J-<AREA>-<NN>: <title>`)"
                    .to_owned()
            ]
        );
    }

    #[test]
    fn fences_follow_commonmark() {
        // A 3-backtick line inside a 4-backtick block does not close it, and `~~~` does not
        // close a backtick block: the bad headings after the real close are still reported.
        let text =
            "````md\n```text\n~~~\n## J-bad-1: in code\n````\n## J-ann-02: b\n### J-ANN-03: c\n";
        assert_eq!(
            problems_for(text),
            vec![
                "line 6: malformed journey heading (expected `## J-<AREA>-<NN>: <title>`)"
                    .to_owned(),
                "line 7: malformed journey heading (expected `## J-<AREA>-<NN>: <title>`)"
                    .to_owned(),
            ]
        );
        // Four spaces of indent is not a fence; a closing fence may not have an info string.
        let mut f = Fence::default();
        assert!(!f.line("    ```"));
        assert!(f.line("```rust"));
        assert!(f.line("``` rust"));
        assert!(f.line("```  "));
        assert!(!f.line("text"));
        assert!(!f.line("``` a`b"));
    }

    #[test]
    fn link_leaving_specs_is_reported_not_resolved() {
        let text = "[x](../../etc/x.md) specs/../../h.md [plan](../PLAN.md) specs/./a/../b.md";
        let p = check_trace(text, &BTreeSet::new(), &[], |p| {
            assert!(p == "PLAN.md" || p == "specs/b.md", "resolved {p}");
            true
        });
        assert_eq!(
            p,
            vec![
                "path specs/../../h.md leaves the repository (not checked)".to_owned(),
                "link target ../../etc/x.md leaves the repository (not checked)".to_owned(),
            ]
        );
    }

    #[test]
    fn unclosed_fence_in_journey_file_is_reported() {
        let text = "## J-A-1: a\n- milestone: M1\n- platform: web\n- test: e2e.web.a\n```\n## J-ann-02: b\n";
        assert_eq!(
            problems_for(text),
            vec!["line 5: code fence never closed".to_owned()]
        );
    }

    #[test]
    fn empty_field_value_is_reported() {
        let text =
            "## J-A-1: old\n- milestone: M1\n- platform: web\n- test:\n- superseded_by: J-A-2\n";
        assert_eq!(
            problems_for(text),
            vec!["J-A-1: field `test` has no value (the judge would read the next line as its value)".to_owned()]
        );
    }

    #[test]
    fn trace_paths_ignore_longer_paths_emphasis_and_anchors() {
        let got = mentioned_spec_paths("x/specs/a.md ops.specs/b.md **specs/c.md** specs/d.md#top");
        let want: BTreeSet<String> = ["specs/c.md", "specs/d.md"]
            .iter()
            .map(|s| (*s).to_owned())
            .collect();
        assert_eq!(got, want);
    }

    #[test]
    fn broken_link_target_is_reported() {
        let specs = vec!["specs/x/y.md".to_owned()];
        let text = "[specs/x/y.md](x/typo.md) [web](https://example.org/a.md) [s](#top)";
        let p = check_trace(text, &BTreeSet::new(), &specs, |p| p == "specs/x/y.md");
        assert_eq!(p, vec!["links to missing file specs/x/typo.md".to_owned()]);
    }

    #[test]
    fn journey_test_ids_come_from_every_journey_file_only() {
        let base = std::env::temp_dir().join(format!("xtask-ids-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let j = base.join("specs").join("journeys");
        assert!(fs::create_dir_all(j.join("sub")).is_ok());
        let one = "## J-A-1: a\n- milestone: M1\n- platform: web, macos\n- test: e2e.web.a, e2e.macos.a\n";
        let two = "## J-B-1: b\n- milestone: M1\n- platform: web\n- test: e2e.web.b\n\
                   ## J-B-2: old\n- milestone: M1\n- platform: web\n- superseded_by: J-B-1\n";
        assert!(fs::write(j.join("a.md"), one).is_ok());
        assert!(fs::write(j.join("b.md"), two).is_ok());
        // Not journey files: another extension, a subdirectory (the judge doesn't recurse).
        assert!(fs::write(j.join("c.txt"), "- test: e2e.web.c\n").is_ok());
        assert!(
            fs::write(
                j.join("sub").join("d.md"),
                "## J-D-1: d\n- test: e2e.web.d\n"
            )
            .is_ok()
        );
        let want: BTreeSet<String> = ["e2e.macos.a", "e2e.web.a", "e2e.web.b"]
            .iter()
            .map(|s| (*s).to_owned())
            .collect();
        assert_eq!(journey_test_ids(&base), Ok(want));
        // A file that can't be read is an error, never a smaller set.
        assert!(fs::write(j.join("e.md"), [0xff_u8]).is_ok());
        assert!(journey_test_ids(&base).is_err(), "non-UTF-8 journey file");
        assert!(fs::remove_file(j.join("e.md")).is_ok());
        assert!(fs::create_dir(j.join("f.md")).is_ok());
        assert!(
            journey_test_ids(&base).is_err(),
            "directory named like a journey file"
        );
        let _ = fs::remove_dir_all(&base);
        assert!(journey_test_ids(&base).is_err(), "missing specs/journeys");
    }

    #[test]
    fn the_repository_journeys_name_test_ids() {
        // Tests run in xtask/; the repository root is its parent.
        let ids = journey_test_ids(Path::new("..")).unwrap_or_default();
        assert!(
            ids.contains("e2e.web.ann_highlight_persists"),
            "{} ids",
            ids.len()
        );
        assert!(ids.contains("e2e.macos.ann_highlight_persists"));
        for harness in crate::web::HARNESS_IDS {
            assert!(
                !ids.contains(harness),
                "{harness}: a harness ID, no journey lists it"
            );
        }
    }
}
