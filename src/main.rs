mod diagnostics;
mod file;

use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result};
use cargo_metadata::diagnostic::*;
use serde::Serialize;

use crate::diagnostics::diagnostics;
use crate::file::{escape_source, modify_source};

/// Line number and rustviz spec
pub type Actions = BTreeMap<usize, Vec<String>>;

/// Variable declarations
pub type Environ = HashMap<String, &'static str>;

// next steps: (TODO)
// 1. static analysis, figure out var names and func names

#[derive(Debug, Serialize)]
struct Output {
    svgs: Vec<Item>,
}

#[derive(Debug, Serialize)]
struct Item {
    lineno: usize,
    svg: String,
}

impl Output {
    fn empty() -> Self {
        Self { svgs: vec![] }
    }
}

impl Item {
    fn new(lineno: usize, svg: String) -> Self {
        Self { lineno, svg }
    }
}

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().collect();
    let source = &args[1];

    let output = Command::new("rustc")
        .args(["--error-format=json", source])
        .output()?;
    let stderr = String::from_utf8(output.stderr)?;

    let mut results = Output::empty();
    for line in stderr.lines() {
        let mut actions: BTreeMap<usize, Vec<String>> = BTreeMap::new();
        let mut environ = HashMap::new();
        environ.insert(String::from("f()"), "Function");
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
                if let Some(&lineno) = actions.keys().min() {
                    let svg = run_rustviz(source, &actions, &environ, &args[2])?;
                    results.svgs.push(Item::new(lineno, svg));
                }
            }
        }
    }
    let s = serde_json::to_string(&results).context("failed to serialize result")?;
    println!("{}", s);

    Ok(())
}

fn run_rustviz(
    source: impl AsRef<Path>,
    actions: &Actions,
    environ: &Environ,
    rustviz_path: impl AsRef<Path>,
) -> Result<String> {
    // TODO: after rustviz is improved, we can remove the rustviz_path bit
    let source = source.as_ref();
    let rustviz_path = rustviz_path.as_ref();
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
    modify_source(source, &annotated, actions, environ).context("failed to add annotations")?;
    escape_source(source, &escaped).context("failed to generate escaped source")?;
    fs::copy(&annotated, examples.join("source.rs")).context("cannot copy to source.rs")?;
    let o = Command::new("rustviz")
        .arg("ERRORVIZ")
        .current_dir(rustviz_path.join("src"))
        .output()?;
    eprintln!("{}", String::from_utf8_lossy(&o.stdout));
    eprintln!("{}", String::from_utf8_lossy(&o.stderr));
    let rustviz_svg = examples.join("vis_timeline.svg");
    let output = fs::read_to_string(rustviz_svg).context("cannot read svg file")?;
    Ok(output)
}
