//! Pinned corpus sources (specs/tooling/corpus.md, item 3).
//!
//! Bumping a pin: change `commit`, run `jpdf-corpus build regress`, review the manifest diff.

/// One pinned git repository whose `*.pdf` blobs feed a tier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Source {
    /// Short name: the cache directory and the manifest header use it.
    pub name: &'static str,
    /// Clone URL.
    pub repo: &'static str,
    /// Pinned commit (40 hex digits).
    pub commit: &'static str,
    /// Raw-file URL prefix; the row URL is `raw_prefix + commit + "/" + path`.
    pub raw_prefix: &'static str,
    /// License note for the header line (the repository's license; files may differ).
    pub license: &'static str,
}

impl Source {
    /// The manifest URL of `path` in this source (unencoded, as in the judge's manifests).
    pub fn url(&self, path: &str) -> String {
        format!("{}{}/{}", self.raw_prefix, self.commit, path)
    }
}

/// Hosts a manifest URL may name (item 4).
pub const ALLOWED_HOSTS: [&str; 2] = ["raw.githubusercontent.com", "gitlab.freedesktop.org"];

/// Largest file `build` takes into the regress tier.
pub const MAX_BUILD_FILE: u64 = 16 << 20;

/// Regress-tier sources in priority order (dedupe keeps the first occurrence).
pub const REGRESS: [Source; 11] = [
    Source {
        name: "pdfjs",
        repo: "https://github.com/mozilla/pdf.js.git",
        commit: "62466e5b8f75e14f288abdc10fc2deda94c86c26",
        raw_prefix: "https://raw.githubusercontent.com/mozilla/pdf.js/",
        license: "Apache-2.0 repository; test files carry their own terms",
    },
    Source {
        name: "verapdf",
        repo: "https://github.com/veraPDF/veraPDF-corpus.git",
        commit: "bb75f4f0073d9350dfd058c0162a367e6fadf25e",
        raw_prefix: "https://raw.githubusercontent.com/veraPDF/veraPDF-corpus/",
        license: "CC-BY-4.0 (README)",
    },
    Source {
        name: "qpdf",
        repo: "https://github.com/qpdf/qpdf.git",
        commit: "4eba95899886e851cc41d76886483b347612f2a8",
        raw_prefix: "https://raw.githubusercontent.com/qpdf/qpdf/",
        license: "Apache-2.0 repository",
    },
    Source {
        name: "pdfbox",
        repo: "https://github.com/apache/pdfbox.git",
        commit: "fdfeaa44e27340fbe57e0667c2ba7e01e069638c",
        raw_prefix: "https://raw.githubusercontent.com/apache/pdfbox/",
        license: "Apache-2.0 repository; test files carry their own terms",
    },
    Source {
        name: "poppler",
        repo: "https://gitlab.freedesktop.org/poppler/test.git",
        commit: "0ef006dca68d2206387eca1d36ae06733812e387",
        raw_prefix: "https://gitlab.freedesktop.org/poppler/test/-/raw/",
        license: "GPL-2.0 COPYING in repository; test files carry their own terms",
    },
    Source {
        name: "tika",
        repo: "https://github.com/apache/tika.git",
        commit: "f1f785f533877ce7426f45b1460ed52e154bcaca",
        raw_prefix: "https://raw.githubusercontent.com/apache/tika/",
        license: "Apache-2.0 repository; test files carry their own terms",
    },
    Source {
        name: "pdf20examples",
        repo: "https://github.com/pdf-association/pdf20examples.git",
        commit: "c20f2c17bfcc4baab7cfe62e70fae64caf14d5fa",
        raw_prefix: "https://raw.githubusercontent.com/pdf-association/pdf20examples/",
        license: "CC-BY-SA-4.0 (LICENSE.md)",
    },
    Source {
        name: "samplefiles",
        repo: "https://github.com/py-pdf/sample-files.git",
        commit: "80d67b91f3e28909756ca7807bbe8f43b599d12f",
        raw_prefix: "https://raw.githubusercontent.com/py-pdf/sample-files/",
        license: "CC-BY-SA-4.0 repository",
    },
    Source {
        name: "pypdf",
        repo: "https://github.com/py-pdf/pypdf.git",
        commit: "d0620116c87df3dcb97365ae3d631af2f8c8dfa9",
        raw_prefix: "https://raw.githubusercontent.com/py-pdf/pypdf/",
        license: "BSD-3-Clause repository; test files carry their own terms",
    },
    Source {
        name: "pdfminer",
        repo: "https://github.com/pdfminer/pdfminer.six.git",
        commit: "a18de2a9c479b4c847538500017b449ddaec177e",
        raw_prefix: "https://raw.githubusercontent.com/pdfminer/pdfminer.six/",
        license: "MIT repository; test files carry their own terms",
    },
    Source {
        name: "pdfplumber",
        repo: "https://github.com/jsvine/pdfplumber.git",
        commit: "4c64b92d5caccd71c645e98e0fabb0c4dba7ff45",
        raw_prefix: "https://raw.githubusercontent.com/jsvine/pdfplumber/",
        license: "MIT repository; test files carry their own terms",
    },
];
