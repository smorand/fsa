//! Environment variables renamed to the `FSA_` prefix (SPEC-0099 DR-006).
//!
//! Each renamed variable is read by its new name first and by its old name as a
//! fallback, so an existing deployment keeps working until release 1.0.0 removes the
//! fallback. One module owns the precedence, the blank value rule and the warning,
//! so every reader behaves the same (DEC-013).

use std::collections::HashSet;
use std::sync::{Mutex, OnceLock};

/// Names already warned about in this process: the warning is printed once per old name.
fn warned() -> &'static Mutex<HashSet<String>> {
    static WARNED: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    WARNED.get_or_init(|| Mutex::new(HashSet::new()))
}

/// Every warning line printed so far, so a unit test can count them.
#[cfg(test)]
static PRINTED: Mutex<Vec<String>> = Mutex::new(Vec::new());

/// The warning lines printed so far by this test process (a test of another module reads them).
#[cfg(test)]
pub(crate) fn printed_lines() -> Vec<String> {
    PRINTED.lock().unwrap_or_else(std::sync::PoisonError::into_inner).clone()
}

/// The value of `new`, else the value of `old`, from `env`. A value that is empty or only
/// whitespace counts as unset, as the existing readers treat it. The old name prints
/// `warning: environment variable <OLD> is deprecated, use <NEW>` on standard error,
/// once per process, whenever it carries a value.
pub fn read_with(new: &str, old: &str, env: &dyn Fn(&str) -> Option<String>) -> Option<String> {
    let non_blank = |v: String| {
        let t = v.trim();
        (!t.is_empty()).then(|| t.to_string())
    };
    let legacy = env(old).and_then(non_blank);
    if legacy.is_some() {
        warn_once(old, new);
    }
    env(new).and_then(non_blank).or(legacy)
}

/// The name of the variable to read: `new` when it carries a value, else `old` when it does,
/// else `new` (so a message names the variable the operator should set). Like [`read_with`],
/// an old name that carries a value prints its deprecation line.
pub fn name_with(new: &str, old: &str, env: &dyn Fn(&str) -> Option<String>) -> String {
    let has = |n: &str| env(n).is_some_and(|v| !v.trim().is_empty());
    let legacy = has(old);
    if legacy {
        warn_once(old, new);
    }
    if !has(new) && legacy { old.to_string() } else { new.to_string() }
}

/// The user configuration directory before the rename. When it is the one loaded, print
/// `warning: configuration directory <OLD> is deprecated, move it to <NEW>` once per process.
pub fn warn_directory(old: &std::path::Path, new: &std::path::Path) {
    let line = format!(
        "warning: configuration directory {} is deprecated, move it to {}",
        old.display(),
        new.display()
    );
    emit_once("directory", line);
}

/// Print the deprecation line for `old` the first time this process meets it.
fn warn_once(old: &str, new: &str) {
    emit_once(old, format!("warning: environment variable {old} is deprecated, use {new}"));
}

/// Print `line` on standard error unless `key` was already printed in this process.
fn emit_once(key: &str, line: String) {
    let first =
        warned().lock().unwrap_or_else(std::sync::PoisonError::into_inner).insert(key.to_string());
    if !first {
        return;
    }
    // The contract is one line on standard error, not a log record: it must reach the
    // operator before any logger is configured.
    eprintln!("{line}");
    #[cfg(test)]
    PRINTED.lock().unwrap_or_else(std::sync::PoisonError::into_inner).push(line);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env_of<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
        move |k| pairs.iter().find(|(n, _)| *n == k).map(|(_, v)| (*v).to_string())
    }

    fn printed_for(old: &str) -> usize {
        PRINTED.lock().unwrap().iter().filter(|l| l.contains(&format!("variable {old} "))).count()
    }

    #[test]
    fn only_the_old_name_is_used_and_warned_once() {
        let env = env_of(&[("OLD_A", "from-old")]);
        assert_eq!(read_with("NEW_A", "OLD_A", &env).as_deref(), Some("from-old"));
        assert_eq!(read_with("NEW_A", "OLD_A", &env).as_deref(), Some("from-old"));
        assert_eq!(printed_for("OLD_A"), 1);
    }

    #[test]
    fn the_new_name_wins_and_the_old_one_is_still_warned() {
        let env = env_of(&[("NEW_B", "from-new"), ("OLD_B", "from-old")]);
        assert_eq!(read_with("NEW_B", "OLD_B", &env).as_deref(), Some("from-new"));
        assert_eq!(printed_for("OLD_B"), 1);
    }

    #[test]
    fn only_the_new_name_prints_nothing() {
        let env = env_of(&[("NEW_C", "from-new")]);
        assert_eq!(read_with("NEW_C", "OLD_C", &env).as_deref(), Some("from-new"));
        assert_eq!(printed_for("OLD_C"), 0);
    }

    #[test]
    fn a_blank_new_value_falls_back_and_a_blank_old_value_is_silent() {
        let env = env_of(&[("NEW_D", "  "), ("OLD_D", "from-old")]);
        assert_eq!(read_with("NEW_D", "OLD_D", &env).as_deref(), Some("from-old"));
        assert_eq!(printed_for("OLD_D"), 1);
        let env = env_of(&[("OLD_E", " ")]);
        assert_eq!(read_with("NEW_E", "OLD_E", &env), None);
        assert_eq!(printed_for("OLD_E"), 0);
    }

    #[test]
    fn the_warning_line_is_exact() {
        let env = env_of(&[("OLD_F", "x")]);
        let _ = read_with("NEW_F", "OLD_F", &env);
        let line = PRINTED.lock().unwrap().iter().find(|l| l.contains("OLD_F")).cloned();
        assert_eq!(
            line.as_deref(),
            Some("warning: environment variable OLD_F is deprecated, use NEW_F")
        );
    }
}
