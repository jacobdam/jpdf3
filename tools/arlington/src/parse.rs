//! TSV grammar (specs/tooling/arlington.md, "Grammar"). The data comes from a pinned commit,
//! but it is still outside input: every problem is a typed error with file and line, never a
//! panic, and sizes are capped ("Limits").

use crate::Error;
use crate::model::{
    Gate, Gated, Indirect, KeyDef, KeyName, Model, ObjectDef, Required, Since, TypeAlt, ValueType,
    Version,
};

/// The header every TSV file starts with, in this order.
pub const HEADER: [&str; 12] = [
    "Key",
    "Type",
    "SinceVersion",
    "DeprecatedIn",
    "Required",
    "IndirectReference",
    "Inheritable",
    "DefaultValue",
    "PossibleValues",
    "SpecialCase",
    "Link",
    "Note",
];

pub const MAX_FILES: usize = 4096;
pub const MAX_FILE_BYTES: usize = 1 << 20;
/// All files together (the pinned `tsv/latest` is about 2.4 MB).
pub const MAX_TOTAL_BYTES: usize = 64 << 20;
pub const MAX_ROWS: usize = 4096;
pub const MAX_FIELD_BYTES: usize = 16 << 10;
/// Maximum bracket/parenthesis nesting inside one field.
pub const MAX_DEPTH: usize = 16;
/// Maximum gates wrapped around one type or link.
pub const MAX_GATES: usize = 8;

/// Parses a whole `tsv/latest` directory given as `(file name, bytes)` pairs.
pub fn parse_files(files: &[(String, Vec<u8>)]) -> Result<Model, Error> {
    if files.len() > MAX_FILES {
        return Err(Error::Limit(format!(
            "{} files (max {MAX_FILES})",
            files.len()
        )));
    }
    let total = files
        .iter()
        .fold(0usize, |t, (_, b)| t.saturating_add(b.len()));
    if total > MAX_TOTAL_BYTES {
        return Err(Error::Limit(format!(
            "{total} bytes in all files (max {MAX_TOTAL_BYTES})"
        )));
    }
    let mut model = Model::default();
    for (file, bytes) in files {
        let Some(name) = file.strip_suffix(".tsv") else {
            return Err(Error::Parse {
                object: file.clone(),
                line: 0,
                message: "not a .tsv file".into(),
            });
        };
        let text = std::str::from_utf8(bytes).map_err(|e| Error::Parse {
            object: name.into(),
            line: 0,
            message: format!("not UTF-8: {e}"),
        })?;
        let object = parse_object(name, text)?;
        if model.objects.insert(name.to_string(), object).is_some() {
            return Err(Error::Parse {
                object: name.into(),
                line: 0,
                message: "duplicate object name".into(),
            });
        }
    }
    Ok(model)
}

/// Parses one TSV file whose stem is `name`.
pub fn parse_object(name: &str, text: &str) -> Result<ObjectDef, Error> {
    let err = |line: usize, message: String| Error::Parse {
        object: name.into(),
        line,
        message,
    };
    if !is_ident(name) {
        return Err(err(0, "object name must be [A-Za-z0-9_]+".into()));
    }
    if text.len() > MAX_FILE_BYTES {
        return Err(Error::Limit(format!(
            "{name}.tsv is {} bytes (max {MAX_FILE_BYTES})",
            text.len()
        )));
    }
    let mut lines = text.split('\n').map(|l| l.strip_suffix('\r').unwrap_or(l));
    let header: Vec<&str> = lines.next().unwrap_or_default().split('\t').collect();
    if header != HEADER {
        return Err(err(1, format!("header must be: {}", HEADER.join(" "))));
    }
    let mut keys: Vec<KeyDef> = Vec::new();
    for (i, line) in lines.enumerate() {
        let lineno = i.saturating_add(2);
        if line.is_empty() {
            continue;
        }
        if keys.len() >= MAX_ROWS {
            return Err(Error::Limit(format!(
                "{name}.tsv has more than {MAX_ROWS} rows"
            )));
        }
        keys.push(parse_row(line).map_err(|m| err(lineno, m))?);
    }
    if keys.is_empty() {
        return Err(err(0, "no rows".into()));
    }
    Ok(ObjectDef {
        name: name.into(),
        keys,
    })
}

fn parse_row(line: &str) -> Result<KeyDef, String> {
    let fields: Vec<&str> = line.split('\t').collect();
    let [
        key,
        ty,
        since,
        deprecated,
        required,
        indirect,
        inheritable,
        default_value,
        possible_values,
        special_case,
        link,
        note,
    ] = fields.as_slice()
    else {
        return Err(format!(
            "{} fields (expected {})",
            fields.len(),
            HEADER.len()
        ));
    };
    for (f, col) in fields.iter().zip(HEADER) {
        if f.len() > MAX_FIELD_BYTES {
            return Err(format!("{col}: longer than {MAX_FIELD_BYTES} bytes"));
        }
    }
    let types = split_top(ty, ';')
        .and_then(|alts| {
            alts.into_iter()
                .map(|a| gated(a, ValueType::parse))
                .collect::<Result<Vec<_>, _>>()
        })
        .map_err(|m| format!("Type: {m}"))?;
    let links = parse_links(link, types.len()).map_err(|m| format!("Link: {m}"))?;
    let types = types
        .into_iter()
        .zip(links)
        .map(|(ty, links)| TypeAlt { ty, links })
        .collect();
    let indirect = parse_indirect(indirect).map_err(|m| format!("IndirectReference: {m}"))?;
    Ok(KeyDef {
        key: parse_key(key).map_err(|m| format!("Key: {m}"))?,
        types,
        since: parse_since(since).map_err(|m| format!("SinceVersion: {m}"))?,
        deprecated: match *deprecated {
            "" => None,
            v => Some(version(v).map_err(|m| format!("DeprecatedIn: {m}"))?),
        },
        required: parse_required(required).map_err(|m| format!("Required: {m}"))?,
        indirect,
        inheritable: parse_bool(inheritable).map_err(|m| format!("Inheritable: {m}"))?,
        default_value: (*default_value).into(),
        possible_values: (*possible_values).into(),
        special_case: (*special_case).into(),
        note: (*note).into(),
    })
}

fn is_ident(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

fn version(s: &str) -> Result<Version, String> {
    Version::parse(s).ok_or_else(|| format!("{s:?} is not a PDF version (1.0–1.7, 2.0)"))
}

fn parse_bool(s: &str) -> Result<bool, String> {
    match s {
        "TRUE" => Ok(true),
        "FALSE" => Ok(false),
        _ => Err(format!("{s:?} is not TRUE or FALSE")),
    }
}

fn parse_key(s: &str) -> Result<KeyName, String> {
    if s == "*" {
        return Ok(KeyName::Wildcard);
    }
    let (digits, repeat) = match s.strip_suffix('*') {
        Some(d) => (d, true),
        None => (s, false),
    };
    if !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()) {
        let n: u32 = digits
            .parse()
            .map_err(|_| format!("index {digits:?} is out of range"))?;
        return Ok(if repeat {
            KeyName::IndexRepeat(n)
        } else {
            KeyName::Index(n)
        });
    }
    // PDF names may hold almost any byte (§7.3.5); Arlington spells them plainly. Reject
    // what would make the grammar ambiguous.
    if s.is_empty()
        || s.chars()
            .any(|c| c.is_whitespace() || "[](),;*/".contains(c) || c.is_control())
    {
        return Err(format!("{s:?} is not a key name, index, N* or *"));
    }
    Ok(KeyName::Name(s.into()))
}

fn parse_since(s: &str) -> Result<Since, String> {
    if let Some(v) = Version::parse(s) {
        return Ok(Since::Version(v));
    }
    match call(s)? {
        Some(("Extension", args)) => match args.as_slice() {
            [name] if is_ident(name) => Ok(Since::Extension {
                name: (*name).into(),
                version: None,
            }),
            [name, v] if is_ident(name) => Ok(Since::Extension {
                name: (*name).into(),
                version: Some(version(v)?),
            }),
            _ => Err(format!("bad fn:Extension arguments in {s:?}")),
        },
        Some(("Eval", _)) => Ok(Since::Expr(s.into())),
        _ => Err(format!("{s:?} is not a version, fn:Extension or fn:Eval")),
    }
}

fn parse_required(s: &str) -> Result<Required, String> {
    match s {
        "TRUE" => return Ok(Required::Always),
        "FALSE" => return Ok(Required::Never),
        _ => {}
    }
    match call(s)? {
        Some(("IsRequired", args)) if !args.is_empty() => {
            // The condition is everything between the outer parentheses.
            let inner = s
                .strip_prefix("fn:IsRequired(")
                .and_then(|r| r.strip_suffix(')'))
                .unwrap_or_default();
            Ok(Required::If(inner.into()))
        }
        _ => Err(format!("{s:?} is not TRUE, FALSE or fn:IsRequired(...)")),
    }
}

fn parse_indirect(s: &str) -> Result<Vec<Indirect>, String> {
    if !s.starts_with('[') {
        return Ok(vec![indirect_one(s)?]);
    }
    split_top(s, ';')?
        .into_iter()
        .map(|part| {
            let inner = part
                .strip_prefix('[')
                .and_then(|p| p.strip_suffix(']'))
                .ok_or_else(|| format!("{part:?} is not [VALUE]"))?;
            indirect_one(inner)
        })
        .collect()
}

fn indirect_one(s: &str) -> Result<Indirect, String> {
    match s {
        "TRUE" => Ok(Indirect::Must),
        "FALSE" => Ok(Indirect::May),
        "fn:MustBeDirect()" => Ok(Indirect::MustBeDirect),
        _ => match call(s)? {
            Some(_) => Ok(Indirect::Expr(s.into())),
            None => Err(format!("{s:?} is not TRUE, FALSE or a predicate")),
        },
    }
}

fn parse_links(s: &str, types: usize) -> Result<Vec<Vec<Gated<String>>>, String> {
    if s.is_empty() {
        return Ok(vec![Vec::new(); types]);
    }
    let parts = split_top(s, ';')?;
    if parts.len() != types {
        return Err(format!(
            "{} link lists for {types} types (need one per type)",
            parts.len()
        ));
    }
    parts
        .into_iter()
        .map(|part| {
            let inner = part
                .strip_prefix('[')
                .and_then(|p| p.strip_suffix(']'))
                .ok_or_else(|| format!("{part:?} is not [NAME,...]"))?;
            if inner.is_empty() {
                return Ok(Vec::new());
            }
            split_top(inner, ',')?
                .into_iter()
                .map(|n| gated(n, |n| is_ident(n).then(|| n.to_string())))
                .collect()
        })
        .collect()
}

/// Unwraps `fn:SinceVersion(V,X)`, `fn:BeforeVersion(V,X)`, `fn:IsPDFVersion(V,X)`,
/// `fn:Deprecated(V,X)` and `fn:Extension(NAME,X)` around a leaf parsed by `leaf`.
fn gated<T>(s: &str, leaf: impl Fn(&str) -> Option<T>) -> Result<Gated<T>, String> {
    let mut gates = Vec::new();
    let mut rest = s;
    while let Some((name, args)) = call(rest)? {
        if gates.len() >= MAX_GATES {
            return Err(format!("more than {MAX_GATES} gates in {s:?}"));
        }
        let [arg, inner] = args.as_slice() else {
            return Err(format!("fn:{name} needs 2 arguments in {s:?}"));
        };
        gates.push(match name {
            "SinceVersion" => Gate::SinceVersion(version(arg)?),
            "BeforeVersion" => Gate::BeforeVersion(version(arg)?),
            "IsPDFVersion" => Gate::IsPdfVersion(version(arg)?),
            "Deprecated" => Gate::Deprecated(version(arg)?),
            "Extension" if is_ident(arg) => Gate::Extension((*arg).into()),
            _ => return Err(format!("fn:{name} is not allowed around a value in {s:?}")),
        });
        rest = inner;
    }
    let value = leaf(rest).ok_or_else(|| format!("{rest:?} is not a known value"))?;
    Ok(Gated { gates, value })
}

/// `fn:NAME(ARGS)` → `Some((NAME, top-level comma-separated ARGS))`; text that does not start
/// with `fn:` → `None`; a malformed call → an error.
fn call(s: &str) -> Result<Option<(&str, Vec<&str>)>, String> {
    let Some(rest) = s.strip_prefix("fn:") else {
        return Ok(None);
    };
    let (name, rest) = rest
        .split_once('(')
        .ok_or_else(|| format!("{s:?}: fn: without ("))?;
    if !is_ident(name) {
        return Err(format!("{s:?}: bad function name"));
    }
    let inner = rest
        .strip_suffix(')')
        .ok_or_else(|| format!("{s:?}: call does not end with )"))?;
    // A balanced inner part means the final `)` closes the opening `(`.
    let args = if inner.is_empty() {
        Vec::new()
    } else {
        split_top(inner, ',')?
    };
    Ok(Some((name, args)))
}

/// Splits on `sep` outside `()` and `[]`, rejecting unbalanced or too deeply nested text.
pub(crate) fn split_top(s: &str, sep: char) -> Result<Vec<&str>, String> {
    let mut stack: Vec<char> = Vec::new();
    let mut parts = Vec::new();
    let mut start = 0;
    for (i, c) in s.char_indices() {
        match c {
            '(' | '[' => {
                if stack.len() >= MAX_DEPTH {
                    return Err(format!("nested deeper than {MAX_DEPTH} in {s:?}"));
                }
                stack.push(c);
            }
            ')' | ']' => {
                let open = if c == ')' { '(' } else { '[' };
                if stack.pop() != Some(open) {
                    return Err(format!("unbalanced {c:?} in {s:?}"));
                }
            }
            _ if c == sep && stack.is_empty() => {
                parts.push(s.get(start..i).unwrap_or_default());
                start = i.saturating_add(c.len_utf8());
            }
            _ => {}
        }
    }
    if !stack.is_empty() {
        return Err(format!("unclosed bracket in {s:?}"));
    }
    parts.push(s.get(start..).unwrap_or_default());
    if parts.iter().any(|p| p.is_empty()) {
        return Err(format!("empty item in {s:?}"));
    }
    Ok(parts)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_top_respects_nesting() {
        assert_eq!(
            split_top("a,fn:X(b,c),[d,e]", ','),
            Ok(vec!["a", "fn:X(b,c)", "[d,e]"])
        );
        assert!(split_top("a,(b", ',').is_err());
        assert!(split_top("a)", ',').is_err());
        assert!(split_top("(]", ',').is_err());
        assert!(split_top("a,,b", ',').is_err());
        assert!(split_top("", ',').is_err());
        let deep = format!("{}{}", "(".repeat(MAX_DEPTH + 1), ")".repeat(MAX_DEPTH + 1));
        assert!(split_top(&deep, ',').is_err());
        let ok = format!("{}{}", "(".repeat(MAX_DEPTH), ")".repeat(MAX_DEPTH));
        assert!(split_top(&ok, ',').is_ok());
    }

    #[test]
    fn keys() {
        assert_eq!(parse_key("Type"), Ok(KeyName::Name("Type".into())));
        assert_eq!(parse_key("3"), Ok(KeyName::Index(3)));
        assert_eq!(parse_key("1*"), Ok(KeyName::IndexRepeat(1)));
        assert_eq!(parse_key("*"), Ok(KeyName::Wildcard));
        assert!(parse_key("99999999999").is_err());
        for bad in ["", "a b", "a,b", "/Type", "**", "a*"] {
            assert!(parse_key(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn gated_types() {
        let g = gated(
            "fn:IsPDFVersion(1.7,fn:Extension(ADBE_Extn3,stream))",
            ValueType::parse,
        );
        assert_eq!(
            g,
            Ok(Gated {
                gates: vec![
                    Gate::IsPdfVersion(Version { major: 1, minor: 7 }),
                    Gate::Extension("ADBE_Extn3".into())
                ],
                value: ValueType::Stream
            })
        );
        assert!(gated("fn:SinceVersion(1.8,stream)", ValueType::parse).is_err());
        assert!(gated("fn:SinceVersion(1.5)", ValueType::parse).is_err());
        assert!(gated("fn:IsPresent(1.5,stream)", ValueType::parse).is_err());
        assert!(gated("fn:SinceVersion(1.5,streams)", ValueType::parse).is_err());
        assert!(gated("fn:SinceVersion(1.5,stream", ValueType::parse).is_err());
        let many = format!(
            "{}stream{}",
            "fn:SinceVersion(1.0,".repeat(MAX_GATES + 1),
            ")".repeat(MAX_GATES + 1)
        );
        assert!(gated(&many, ValueType::parse).is_err());
        let enough = format!(
            "{}stream{}",
            "fn:SinceVersion(1.0,".repeat(MAX_GATES),
            ")".repeat(MAX_GATES)
        );
        assert!(gated(&enough, ValueType::parse).is_ok());
    }

    #[test]
    fn since_required_indirect() {
        let v17 = Version { major: 1, minor: 7 };
        assert_eq!(parse_since("1.7"), Ok(Since::Version(v17)));
        assert_eq!(
            parse_since("fn:Extension(X_1,1.7)"),
            Ok(Since::Extension {
                name: "X_1".into(),
                version: Some(v17)
            })
        );
        assert_eq!(
            parse_since("fn:Extension(X)"),
            Ok(Since::Extension {
                name: "X".into(),
                version: None
            })
        );
        let eval = "fn:Eval(fn:Extension(X,1.7) || 2.0)";
        assert_eq!(parse_since(eval), Ok(Since::Expr(eval.into())));
        for bad in [
            "",
            "1.8",
            "fn:Extension()",
            "fn:Extension(X,Y,Z)",
            "fn:Foo(1)",
        ] {
            assert!(parse_since(bad).is_err(), "{bad}");
        }
        assert_eq!(
            parse_required("fn:IsRequired(fn:IsPresent(A) || fn:IsPresent(B))"),
            Ok(Required::If("fn:IsPresent(A) || fn:IsPresent(B)".into()))
        );
        assert!(parse_required("fn:IsRequired()").is_err());
        assert!(parse_required("true").is_err());
        assert_eq!(
            parse_indirect("[TRUE];[fn:MustBeDirect()]"),
            Ok(vec![Indirect::Must, Indirect::MustBeDirect])
        );
        assert_eq!(
            parse_indirect("fn:MustBeIndirect(fn:BeforeVersion(2.0))"),
            Ok(vec![Indirect::Expr(
                "fn:MustBeIndirect(fn:BeforeVersion(2.0))".into()
            )])
        );
        assert!(parse_indirect("[TRUE];FALSE").is_err());
        assert!(parse_indirect("yes").is_err());
    }

    #[test]
    fn links() {
        assert_eq!(parse_links("", 2), Ok(vec![vec![], vec![]]));
        let l = parse_links("[];[A,fn:SinceVersion(2.0,B)]", 2);
        assert_eq!(
            l,
            Ok(vec![
                vec![],
                vec![
                    Gated {
                        gates: vec![],
                        value: "A".into()
                    },
                    Gated {
                        gates: vec![Gate::SinceVersion(Version { major: 2, minor: 0 })],
                        value: "B".into()
                    }
                ]
            ])
        );
        assert!(parse_links("[A]", 2).is_err());
        assert!(parse_links("A;[]", 2).is_err());
        assert!(parse_links("[A B];[]", 2).is_err());
    }

    #[test]
    fn rows_and_files() {
        let h = HEADER.join("\t");
        let row = "Type\tname\t1.0\t\tTRUE\tFALSE\tFALSE\t\t[Foo]\t\t\tTable 1";
        let obj = parse_object("Foo", &format!("{h}\n{row}\n")).map_err(|e| e.to_string());
        assert_eq!(obj.as_ref().map(|o| o.keys.len()), Ok(1));
        // Wrong field count, bad header, empty file, bad name, bad UTF-8.
        let short = parse_object("Foo", &format!("{h}\nType\tname\n"));
        assert!(
            matches!(short, Err(Error::Parse { line: 2, .. })),
            "{short:?}"
        );
        assert!(parse_object("Foo", "Key\tType\n").is_err());
        assert!(parse_object("Foo", &h).is_err());
        assert!(parse_object("Foo.bar", &format!("{h}\n{row}")).is_err());
        let files = vec![("Foo.tsv".to_string(), vec![0xff, 0xfe])];
        assert!(parse_files(&files).is_err());
        let files = vec![("Foo.txt".to_string(), format!("{h}\n{row}").into_bytes())];
        assert!(parse_files(&files).is_err());
        // Row and size limits, at the boundary and one past it.
        let rows = |n: usize| format!("{h}\n{}", format!("{row}\n").repeat(n));
        assert!(parse_object("Foo", &rows(MAX_ROWS)).is_ok());
        assert!(matches!(
            parse_object("Foo", &rows(MAX_ROWS + 1)),
            Err(Error::Limit(_))
        ));
        let big = format!("{h}\n{row}\n{}", " ".repeat(MAX_FILE_BYTES));
        assert!(matches!(parse_object("Foo", &big), Err(Error::Limit(_))));
        let long_note = format!("{h}\n{row}{}", "x".repeat(MAX_FIELD_BYTES));
        assert!(parse_object("Foo", &long_note).is_err());
    }
}
