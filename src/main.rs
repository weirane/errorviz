mod diagnostics;
mod file;

use std::collections::{BTreeMap, HashMap};
use std::process::Command;

use anyhow::Result;
use cargo_metadata::diagnostic::*;

use crate::diagnostics::diagnostics;
use crate::file::{escape_source, modify_source};

/// Line number and rustviz spec
pub type Actions = BTreeMap<usize, Vec<String>>;

/// Variable declarations
pub type Environ = HashMap<String, &'static str>;

// next steps: (TODO)
// 1. static analysis, figure out var names and func names

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().collect();
    let path = &args[1];
    let annotated = &args[2];
    let escaped = &args[3];

    let output = Command::new("rustc")
        .args(["--error-format=json", path])
        .output()?;
    let stderr = String::from_utf8(output.stderr)?;

    let mut actions: BTreeMap<usize, Vec<String>> = BTreeMap::new();
    let mut environ = HashMap::new();
    environ.insert(String::from("f()"), "Function");
    for line in stderr.lines() {
        let diag: Diagnostic = serde_json::from_str(line)?;
        if diag.level == DiagnosticLevel::Error {
            if let Some(DiagnosticCode { ref code, .. }) = diag.code {
                let (acs, env) = diagnostics(&diag, code)?;
                for (k, mut v) in acs {
                    actions.entry(k).or_default().append(&mut v);
                }
                for (k, v) in env {
                    if environ.contains_key(&k) {
                        anyhow::bail!("duplicate key {k}");
                    } else {
                        environ.insert(k, v);
                    }
                }
            }
        }
    }
    modify_source(path, annotated, &actions, &environ)?;
    escape_source(path, escaped)?;
    Ok(())
}
