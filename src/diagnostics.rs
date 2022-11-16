use std::collections::{BTreeMap, HashMap};

use anyhow::{Context, Result};
use cargo_metadata::diagnostic::*;
use syn::{Expr, Pat, Stmt};

use crate::{Actions, Environ};

pub fn diagnostics(diag: &Diagnostic, code: &str) -> Result<(Actions, Environ)> {
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
