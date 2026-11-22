//! jpdf-bench binary. Spec: specs/tooling/bench.md.
#![forbid(unsafe_code)]

use std::ffi::OsString;
use std::process::ExitCode;

use jpdf_bench::args::{self, USAGE};
use jpdf_bench::bench::{self, BenchError};

fn main() -> ExitCode {
    let argv: Vec<OsString> = std::env::args_os().skip(1).collect();
    let opts = match args::parse(&argv) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("jpdf-bench: {e}\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    let result = bench::run(&opts).and_then(|r| bench::write(&opts.out, &r).map(|()| r));
    match result {
        Ok(r) => {
            eprintln!(
                "jpdf-bench: {} file(s), report {}",
                r.files.len(),
                opts.out.display()
            );
            ExitCode::SUCCESS
        }
        Err(BenchError::Usage(e)) => {
            eprintln!("jpdf-bench: {e}\n{USAGE}");
            ExitCode::from(2)
        }
        Err(BenchError::Io(e)) => {
            eprintln!("jpdf-bench: {e}");
            ExitCode::from(1)
        }
    }
}
