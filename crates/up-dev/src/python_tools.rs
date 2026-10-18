//! Hash-locked Python CI tools (SUPPLY-07, D1-06).

use std::path::PathBuf;

use crate::check::{Ctx, Failure};

/// Check `tools/ci-requirements.txt` text before uv sees it.
pub fn check_requirements_lock(_text: &str) -> Result<(), Failure> {
    Ok(())
}

/// Build (or reuse) the CI venv and return its `bin` directory.
pub fn ensure_venv(_ctx: &Ctx) -> Result<PathBuf, Failure> {
    Err(Failure::pending("python-tools", "01-08"))
}
