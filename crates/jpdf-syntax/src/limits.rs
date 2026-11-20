/// Resource limits enforced on every operation (PLAN.md §2.2). Never raise a check for speed
/// (CONSTITUTION 4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Limits {
    /// Largest accepted file, in bytes.
    pub max_file_size: usize,
    /// Deepest nesting of arrays/dictionaries inside one object.
    pub max_nesting_depth: usize,
    /// Objects (scalars and containers) produced while parsing one top-level object;
    /// bounds memory for huge arrays/dictionaries.
    pub max_objects_per_parse: usize,
    /// Decoded string and name bytes produced while parsing one top-level object (memory bound).
    pub max_string_bytes_per_parse: usize,
    /// Total cross-reference entries across all sections.
    pub max_xref_entries: usize,
    /// Cross-reference sections followed through `/Prev`.
    pub max_xref_sections: usize,
    /// Pages in one document.
    pub max_pages: usize,
    /// Bytes the lexer may scan per document, as a multiple of the file size (plus
    /// `lex_budget_slack`). Legitimate files lex each byte about once.
    pub lex_budget_factor: usize,
    /// Fixed allowance added to the lexing budget (covers tiny files).
    pub lex_budget_slack: usize,
}

impl Limits {
    /// Total bytes the lexer may scan for a file of `data_len` bytes.
    pub fn lex_budget(&self, data_len: usize) -> usize {
        data_len
            .saturating_mul(self.lex_budget_factor)
            .saturating_add(self.lex_budget_slack)
    }
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_file_size: 512 * 1024 * 1024,
            max_nesting_depth: 256,
            max_objects_per_parse: 4_000_000,
            max_string_bytes_per_parse: 64 * 1024 * 1024,
            max_xref_entries: 10_000_000,
            max_xref_sections: 1024,
            max_pages: 1_000_000,
            lex_budget_factor: 4,
            lex_budget_slack: 1024 * 1024,
        }
    }
}
