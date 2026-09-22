//! App-owned orchestration for `stack-profiles` (package 7, subpackage 7a) —
//! the pool-agreement half only. Reads the two pool files through the
//! [`WorkspaceReader`] port and hands the two ordered name lists to
//! `meridian_core::mechanical_integrity::stack_profiles`. Every fallible
//! read distinguishes the file's domain-meaningful absence
//! (`ReadOutcome::Absent`) from an access/encoding/walk failure, which
//! propagates as [`OperationError`] rather than becoming a false "missing"
//! diagnostic.

use fancy_regex::Regex;
use meridian_core::mechanical_integrity::dedupe_preserve_order;
use meridian_core::mechanical_integrity::stack_profiles::{self as checks, StackProfilePool};
use meridian_core::types::{Diagnostic, DiagnosticLevel, WorkspaceRelativePath};
use serde_json::Value;

use super::{read_text, regex_collect_captures, OperationError, ReadOutcome};
use crate::source_format::{parse_yaml, regions::marked_region};
use crate::workspace::WorkspaceReader;

#[derive(Debug)]
pub struct Outcome {
    pub diagnostics: Vec<Diagnostic>,
    /// The agreed, typed pool — present only when the pair parsed and its
    /// two halves agreed (mirrors the Node reference's `STACK_PROFILES`
    /// truthiness): the caller gates the "declarations were not checked"
    /// advisory on `.is_some()`, never on a pool that failed to load.
    pub pool: Option<StackProfilePool>,
}

fn fail(message: impl Into<String>) -> Diagnostic {
    Diagnostic::new(DiagnosticLevel::Fail, message).expect("message is non-empty")
}

const YAML_PATH: &str = "stack-profiles/stack-profiles.yaml";
const MD_PATH: &str = "stack-profiles/stack-profiles.md";

pub fn run(reader: &dyn WorkspaceReader) -> Result<Outcome, OperationError> {
    let yaml_path = WorkspaceRelativePath::new(YAML_PATH).expect("literal path is valid");
    let md_path = WorkspaceRelativePath::new(MD_PATH).expect("literal path is valid");

    let yaml_raw = match read_text(reader, &yaml_path)? {
        ReadOutcome::Present(text) => Some(text),
        ReadOutcome::Absent => None,
    };
    let md_raw = match read_text(reader, &md_path)? {
        ReadOutcome::Present(text) => Some(text),
        ReadOutcome::Absent => None,
    };

    if yaml_raw.is_none() || md_raw.is_none() {
        let mut missing = Vec::new();
        if yaml_raw.is_none() {
            missing.push(YAML_PATH);
        }
        if md_raw.is_none() {
            missing.push(MD_PATH);
        }
        return Ok(Outcome {
            diagnostics: vec![checks::missing_pool_files(&missing)],
            pool: None,
        });
    }
    let yaml_raw = yaml_raw.unwrap();
    let md_raw = md_raw.unwrap();

    let mut diagnostics = Vec::new();

    let (entries, parsed) = match parse_yaml(&yaml_raw) {
        Ok(doc) => match doc.get("profiles") {
            None | Some(Value::Null) | Some(Value::Bool(false)) => (Vec::new(), true),
            Some(Value::Number(n)) if n.as_f64() == Some(0.0) => (Vec::new(), true),
            Some(Value::String(s)) if s.is_empty() => (Vec::new(), true),
            Some(Value::Array(arr)) => (arr.clone(), true),
            Some(other) => {
                diagnostics.push(checks::profiles_not_a_list(type_name(other)));
                (Vec::new(), false)
            }
        },
        Err(error) => {
            diagnostics.push(fail(format!("stack-profiles: {error}")));
            (Vec::new(), false)
        }
    };

    let region = match marked_region(&md_raw, "stack-profile-pool") {
        Ok(region) => region,
        Err(error) => {
            diagnostics.push(checks::region_unreadable(&error));
            return Ok(Outcome {
                diagnostics,
                pool: None,
            });
        }
    };

    if !parsed {
        return Ok(Outcome {
            diagnostics,
            pool: None,
        });
    }

    let row_re = Regex::new(r"(?m)^\|\s*`([a-z0-9-]+)`\s*\|").expect("row pattern compiles");
    let documented_ordered: Vec<String> = dedupe_preserve_order(
        regex_collect_captures(&row_re, &region.text, MD_PATH)?
            .into_iter()
            .filter_map(|c| c.get(1).map(|g| g.as_str().to_string())),
    );
    let declared_ordered: Vec<String> = dedupe_preserve_order(entries.iter().map(|e| {
        e.get("name")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string()
    }));

    match checks::check_pool_agreement(&declared_ordered, &documented_ordered) {
        Ok(pool) => Ok(Outcome {
            diagnostics,
            pool: Some(pool),
        }),
        Err(diagnostic) => {
            diagnostics.push(diagnostic);
            Ok(Outcome {
                diagnostics,
                pool: None,
            })
        }
    }
}

fn type_name(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "a boolean",
        Value::Number(_) => "a number",
        Value::String(_) => "a string",
        Value::Array(_) => "an array",
        Value::Object(_) => "an object",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::validation::mechanical_integrity::tests::FakeReader;
    use crate::workspace::ReadError;

    #[test]
    fn agreeing_halves_produce_no_diagnostics() {
        let reader = FakeReader::new()
            .with_file(YAML_PATH, "profiles:\n  - name: vue-spa\n")
            .with_file(
                MD_PATH,
                "<!-- meridian:begin stack-profile-pool -->\n| `vue-spa` | x | y | z |\n<!-- meridian:end stack-profile-pool -->\n",
            );
        let outcome = run(&reader).unwrap();
        assert!(outcome.diagnostics.is_empty(), "{:?}", outcome.diagnostics);
        assert!(outcome.pool.is_some());
    }

    #[test]
    fn universal_in_the_pool_is_rejected() {
        let reader = FakeReader::new()
            .with_file(YAML_PATH, "profiles:\n  - name: universal\n")
            .with_file(
                MD_PATH,
                "<!-- meridian:begin stack-profile-pool -->\n| `universal` | x | y | z |\n<!-- meridian:end stack-profile-pool -->\n",
            );
        let outcome = run(&reader).unwrap();
        assert_eq!(outcome.diagnostics.len(), 1);
        assert!(outcome.diagnostics[0]
            .message()
            .contains("is not a stack profile"));
        assert!(outcome.pool.is_none());
    }

    #[test]
    fn a_missing_pool_file_fails_closed() {
        let outcome = run(&FakeReader::new()).unwrap();
        assert_eq!(outcome.diagnostics.len(), 1);
        assert!(outcome.pool.is_none());
    }

    #[test]
    fn a_wrong_type_profiles_key_reports_and_does_not_load() {
        let reader = FakeReader::new()
            .with_file(YAML_PATH, "profiles: not-a-list\n")
            .with_file(
                MD_PATH,
                "<!-- meridian:begin stack-profile-pool -->\n<!-- meridian:end stack-profile-pool -->\n",
            );
        let outcome = run(&reader).unwrap();
        assert!(outcome
            .diagnostics
            .iter()
            .any(|d| d.message().contains("must be a list")));
        assert!(outcome.pool.is_none());
    }

    #[test]
    fn an_io_failure_reading_the_yaml_pool_propagates_as_an_operation_error_not_a_missing_pool_diagnostic(
    ) {
        let reader = FakeReader::new()
            .with_error(YAML_PATH, ReadError::Io("permission denied".to_string()))
            .with_file(
                MD_PATH,
                "<!-- meridian:begin stack-profile-pool -->\n<!-- meridian:end stack-profile-pool -->\n",
            );
        let error = run(&reader).unwrap_err();
        assert_eq!(error.path, YAML_PATH);
        assert!(error.message.contains("permission denied"));
    }

    #[test]
    fn an_io_failure_reading_the_md_pool_propagates_as_an_operation_error() {
        let reader = FakeReader::new()
            .with_file(YAML_PATH, "profiles: []\n")
            .with_error(MD_PATH, ReadError::Io("not a regular file".to_string()));
        let error = run(&reader).unwrap_err();
        assert_eq!(error.path, MD_PATH);
    }

    #[test]
    fn the_real_kernel_pool_agrees_with_itself() {
        let kernel_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .to_path_buf();
        let reader = crate::validation::mechanical_integrity::tests::real_fs_reader(kernel_root);
        let outcome = run(&reader).unwrap();
        assert!(outcome.diagnostics.is_empty(), "{:?}", outcome.diagnostics);
    }
}
