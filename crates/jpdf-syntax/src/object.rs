//! PDF objects. ISO 32000-2 §7.3.

/// An indirect reference `num gen R`. ISO 32000-2 §7.3.10.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ObjRef {
    pub num: u32,
    pub generation: u16,
}

/// Dictionary entries in file order. Keys are decoded name bytes (without the `/`).
/// ISO 32000-2 §7.3.7: a later duplicate key is not meaningful; `get` returns the first.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Dict(pub Vec<(Vec<u8>, Object)>);

impl Dict {
    pub fn get(&self, key: &[u8]) -> Option<&Object> {
        self.0
            .iter()
            .find(|(k, _)| k.as_slice() == key)
            .map(|(_, v)| v)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Object {
    Null,
    Bool(bool),
    Integer(i64),
    Real(f64),
    /// Decoded string bytes (literal or hexadecimal).
    String(Vec<u8>),
    /// Decoded name bytes, without the leading `/`.
    Name(Vec<u8>),
    Array(Vec<Object>),
    Dict(Dict),
    Ref(ObjRef),
}

impl Object {
    pub fn as_dict(&self) -> Option<&Dict> {
        match self {
            Self::Dict(d) => Some(d),
            _ => None,
        }
    }

    pub fn as_integer(&self) -> Option<i64> {
        match self {
            Self::Integer(i) => Some(*i),
            _ => None,
        }
    }

    pub fn as_name(&self) -> Option<&[u8]> {
        match self {
            Self::Name(n) => Some(n),
            _ => None,
        }
    }

    pub fn as_ref(&self) -> Option<ObjRef> {
        match self {
            Self::Ref(r) => Some(*r),
            _ => None,
        }
    }

    pub fn as_array(&self) -> Option<&[Object]> {
        match self {
            Self::Array(a) => Some(a),
            _ => None,
        }
    }
}
