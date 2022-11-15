mod file;

use std::collections::{BTreeMap, HashMap};
use std::process::Command;

use anyhow::{Context, Result};
use cargo_metadata::diagnostic::*;

use syn::{Expr, Pat, Stmt};

use crate::file::{escape_source, modify_source};

/// Line number and rustviz spec
pub type Actions = BTreeMap<usize, Vec<String>>;

/// Variable declarations
pub type Environ = HashMap<String, &'static str>;

fn diagnostics(diag: &Diagnostic, code: &str) -> Result<(Actions, Environ)> {
    match code {
        "E0502" => diagnostics_502(diag),
        _ => unimplemented!(),
    }
}

fn diagnostics_502(diag: &Diagnostic) -> Result<(Actions, Environ)> {
    let imm_borrow = diag
        .spans
        .iter()
        .find(|s| s.label.as_deref() == Some("immutable borrow occurs here"))
        .context("cannot locate immutable borrow")?;
    let mut_borrow = diag
        .spans
        .iter()
        .find(|s| s.label.as_deref() == Some("mutable borrow occurs here"))
        .context("cannot locate mutable borrow")?;
    let imm_use = diag
        .spans
        .iter()
        .find(|s| s.label.as_deref() == Some("immutable borrow later used here"))
        .context("cannot locate immutable use")?;

    let mut act: BTreeMap<usize, Vec<String>> = BTreeMap::new();
    let mut env = HashMap::new();

    let imm_borrow_text = &imm_borrow.text[0];
    let ast: Stmt = syn::parse_str(&imm_borrow_text.text).context("can't parse first borrow")?;
    let user = match ast {
        Stmt::Local(loc) => {
            if let Pat::Ident(id) = loc.pat {
                id.ident.to_string()
            } else {
                todo!("pattern in let");
            }
        }
        _ => todo!(),
    };
    let init: Expr = syn::parse_str(
        &imm_borrow_text.text
            [imm_borrow_text.highlight_start - 1..imm_borrow_text.highlight_end - 1],
    )?;
    let borrower = if let Expr::Reference(r) = init {
        if let Expr::Path(p) = *r.expr {
            p.path
                .segments
                .first()
                .ok_or_else(|| anyhow::anyhow!("empty borrower"))?
                .ident
                .to_string()
        } else {
            todo!("not ref to ident");
        }
    } else {
        todo!("not a reference");
    };
    act.entry(imm_borrow.line_start)
        .or_default()
        .push(format!("StaticBorrow({}->{})", borrower, user));

    act.entry(mut_borrow.line_start).or_default().push(format!(
        "PassByMutableReference({}->{}|false)",
        borrower, "f()"
    ));

    act.entry(imm_use.line_start)
        .or_default()
        .push(format!("PassByStaticReference({}->{})", user, "f()"));

    env.insert(String::from("f()"), "Function");
    env.insert(user, "StaticRef");
    env.insert(borrower, "Owner");
    Ok((act, env))
}

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
