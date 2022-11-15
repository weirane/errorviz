use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;

use anyhow::Result;

use crate::{Actions, Environ};

pub fn modify_source(
    path: impl AsRef<Path>,
    annotated: impl AsRef<Path>,
    actions: &Actions,
    environ: &Environ,
) -> Result<()> {
    let f = BufReader::new(File::open(path)?);
    let mut fannotated = File::create(annotated)?;
    writeln!(fannotated, "/* --- BEGIN Variable Definitions ---")?;
    for (k, v) in environ {
        writeln!(fannotated, "{v} {k};")?;
    }
    writeln!(fannotated, "--- END Variable Definitions --- */")?;

    let mut it = actions.iter().peekable();
    for (i, line) in (1..).zip(f.lines()) {
        let line = line?;
        match it.peek() {
            Some((&j, spec)) if i == j => {
                writeln!(fannotated, "{} /* !{{ {} }} */", line, spec.join(", "))?;
                it.next();
            }
            _ => {
                writeln!(fannotated, "{}", line)?;
            }
        }
    }
    Ok(())
}

pub fn escape_source(path: impl AsRef<Path>, annotated: impl AsRef<Path>) -> Result<()> {
    let f = BufReader::new(File::open(path)?);
    let mut fannotated = File::create(annotated)?;
    for line in f.lines() {
        let line = line?;
        let escaped = line
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;");
        writeln!(fannotated, "{}", escaped)?;
    }
    Ok(())
}
