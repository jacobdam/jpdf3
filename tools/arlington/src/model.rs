//! Typed form of the Arlington PDF Model (specs/tooling/arlington.md, "Model").
//!
//! One [`ObjectDef`] per TSV file; one [`KeyDef`] per row. Columns the generator of later
//! test objects will interpret (DefaultValue, PossibleValues, SpecialCase, Note) are kept as
//! the original text.

use std::collections::BTreeMap;
use std::fmt;

/// A PDF version as Arlington writes it: `1.0`–`1.7` or `2.0` (ISO 32000-2 §7.5.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Version {
    pub major: u8,
    pub minor: u8,
}

impl Version {
    /// Parses exactly one of the versions PDF has had; anything else is `None`.
    pub fn parse(s: &str) -> Option<Self> {
        let (major, minor) = match s {
            "1.0" => (1, 0),
            "1.1" => (1, 1),
            "1.2" => (1, 2),
            "1.3" => (1, 3),
            "1.4" => (1, 4),
            "1.5" => (1, 5),
            "1.6" => (1, 6),
            "1.7" => (1, 7),
            "2.0" => (2, 0),
            _ => return None,
        };
        Some(Self { major, minor })
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}", self.major, self.minor)
    }
}

/// Arlington's value types (the `Type` column).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ValueType {
    Array,
    Bitmask,
    Boolean,
    Date,
    Dictionary,
    Integer,
    Matrix,
    Name,
    NameTree,
    Null,
    Number,
    NumberTree,
    Rectangle,
    Stream,
    String,
    StringAscii,
    StringByte,
    StringText,
}

impl ValueType {
    pub const ALL: [ValueType; 18] = [
        Self::Array,
        Self::Bitmask,
        Self::Boolean,
        Self::Date,
        Self::Dictionary,
        Self::Integer,
        Self::Matrix,
        Self::Name,
        Self::NameTree,
        Self::Null,
        Self::Number,
        Self::NumberTree,
        Self::Rectangle,
        Self::Stream,
        Self::String,
        Self::StringAscii,
        Self::StringByte,
        Self::StringText,
    ];

    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|t| t.arlington_name() == s)
    }

    /// The spelling used in the TSV files.
    pub fn arlington_name(self) -> &'static str {
        match self {
            Self::Array => "array",
            Self::Bitmask => "bitmask",
            Self::Boolean => "boolean",
            Self::Date => "date",
            Self::Dictionary => "dictionary",
            Self::Integer => "integer",
            Self::Matrix => "matrix",
            Self::Name => "name",
            Self::NameTree => "name-tree",
            Self::Null => "null",
            Self::Number => "number",
            Self::NumberTree => "number-tree",
            Self::Rectangle => "rectangle",
            Self::Stream => "stream",
            Self::String => "string",
            Self::StringAscii => "string-ascii",
            Self::StringByte => "string-byte",
            Self::StringText => "string-text",
        }
    }

    /// The Rust variant name used in generated tables.
    pub fn variant(self) -> &'static str {
        match self {
            Self::Array => "Array",
            Self::Bitmask => "Bitmask",
            Self::Boolean => "Boolean",
            Self::Date => "Date",
            Self::Dictionary => "Dictionary",
            Self::Integer => "Integer",
            Self::Matrix => "Matrix",
            Self::Name => "Name",
            Self::NameTree => "NameTree",
            Self::Null => "Null",
            Self::Number => "Number",
            Self::NumberTree => "NumberTree",
            Self::Rectangle => "Rectangle",
            Self::Stream => "Stream",
            Self::String => "String",
            Self::StringAscii => "StringAscii",
            Self::StringByte => "StringByte",
            Self::StringText => "StringText",
        }
    }

    /// Whether a value of this type must link to the object definition(s) describing it.
    pub fn needs_link(self) -> bool {
        matches!(self, Self::Array | Self::Dictionary | Self::Stream)
    }

    /// Whether a value of this type may link to object definitions at all.
    pub fn may_link(self) -> bool {
        self.needs_link() || matches!(self, Self::NameTree | Self::NumberTree)
    }
}

/// A condition wrapped around a type or link: `fn:SinceVersion(1.5,stream)` etc.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Gate {
    SinceVersion(Version),
    BeforeVersion(Version),
    IsPdfVersion(Version),
    Deprecated(Version),
    Extension(String),
}

/// A value with the gates wrapped around it, outermost first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Gated<T> {
    pub gates: Vec<Gate>,
    pub value: T,
}

/// Which key (or array index) a row describes (the `Key` column).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum KeyName {
    /// A dictionary key, without the leading `/`.
    Name(String),
    /// An array element at this index.
    Index(u32),
    /// `N*`: elements from index N on, repeating (Arlington's array repeats).
    IndexRepeat(u32),
    /// `*`: any key or any element.
    Wildcard,
}

impl fmt::Display for KeyName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Name(n) => f.write_str(n),
            Self::Index(i) => write!(f, "{i}"),
            Self::IndexRepeat(i) => write!(f, "{i}*"),
            Self::Wildcard => f.write_str("*"),
        }
    }
}

/// The `SinceVersion` column.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Since {
    Version(Version),
    /// `fn:Extension(NAME)` or `fn:Extension(NAME,VERSION)`.
    Extension {
        name: String,
        version: Option<Version>,
    },
    /// Any other predicate (`fn:Eval(...)`), kept as written.
    Expr(String),
}

/// The `Required` column.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Required {
    Always,
    Never,
    /// `fn:IsRequired(COND)`: the condition text.
    If(String),
}

/// The `IndirectReference` column (one value, or one per type).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Indirect {
    /// `TRUE`: must be an indirect reference.
    Must,
    /// `FALSE`: may be direct or indirect.
    May,
    /// `fn:MustBeDirect()`.
    MustBeDirect,
    /// Any other predicate, kept as written.
    Expr(String),
}

/// One type alternative of a key, with the objects it links to (empty for simple types).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeAlt {
    pub ty: Gated<ValueType>,
    pub links: Vec<Gated<String>>,
}

/// One row of a TSV file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyDef {
    pub key: KeyName,
    pub types: Vec<TypeAlt>,
    pub since: Since,
    pub deprecated: Option<Version>,
    pub required: Required,
    /// One entry for all types, or exactly one per type.
    pub indirect: Vec<Indirect>,
    pub inheritable: bool,
    pub default_value: String,
    pub possible_values: String,
    pub special_case: String,
    pub note: String,
}

/// One TSV file: an object definition such as `Catalog` or `ArrayOf_4Numbers`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectDef {
    pub name: String,
    pub keys: Vec<KeyDef>,
}

/// Every object definition, by name.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Model {
    pub objects: BTreeMap<String, ObjectDef>,
}

impl Model {
    /// Number of rows in all objects.
    pub fn key_count(&self) -> usize {
        self.objects.values().map(|o| o.keys.len()).sum()
    }
}
