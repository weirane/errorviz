mod diagnostics;
mod file;

use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::PathBuf;
use std::process::Command;

use anyhow::{Context, Result};
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
    let source = &args[1];

    let output = Command::new("rustc")
        .args(["--error-format=json", source])
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

    let output_svg_path = &args[2];
    // --- rustviz related ---
    let rustviz_path = PathBuf::from(&args[3]);
    if !rustviz_path.exists() {
        anyhow::bail!("rustviz path does not exist");
    }
    let examples = rustviz_path.join("src/examples/ERRORVIZ");
    if examples.exists() {
        fs::remove_dir_all(&examples).context("cannot rm examples")?;
    }
    fs::create_dir_all(&examples.join("input")).context("failed to mkdir")?;
    let annotated = examples.join("main.rs");
    let escaped = examples.join("input/annotated_source.rs");
    modify_source(source, &annotated, &actions, &environ).context("failed to add annotations")?;
    escape_source(source, &escaped).context("failed to generate escaped source")?;
    fs::copy(&annotated, examples.join("source.rs")).context("cannot copy to source.rs")?;
    let o = Command::new("rustviz")
        .arg("ERRORVIZ")
        .current_dir(rustviz_path.join("src"))
        .output()?;
    println!("{}", String::from_utf8_lossy(&o.stdout));
    println!("{}", String::from_utf8_lossy(&o.stderr));
    let rustviz_svg = examples.join("vis_timeline.svg");
    // --- rustviz related ---
    fs::copy(rustviz_svg, output_svg_path).context("failed to copy to destination")?;
    Ok(())
}
