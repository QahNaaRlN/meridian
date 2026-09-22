//! App-owned orchestration for `instruction-topics` (package 7, subpackage
//! 7a): reads the two pool files through the [`WorkspaceReader`] port,
//! parses YAML and the marked-region table with this crate's own strict
//! adapters, and hands the two ordered id lists to
//! `meridian_core::mechanical_integrity::instruction_topics` for the
//! agreement check. Every fallible read distinguishes the file's
//! domain-meaningful absence from an access/encoding/walk failure, which
//! propagates as [`OperationError`].

use fancy_regex::Regex;
use meridian_core::mechanical_integrity::dedupe_preserve_order;
use meridian_core::mechanical_integrity::instruction_topics as checks;
pub use meridian_core::mechanical_integrity::instruction_topics::TopicPool;
use meridian_core::types::{Diagnostic, DiagnosticLevel, WorkspaceRelativePath};
use serde_json::Value;

use super::{read_text, regex_collect_captures, OperationError, ReadOutcome};
use crate::source_format::{parse_yaml, regions::marked_region};
use crate::workspace::WorkspaceReader;

#[derive(Debug)]
pub struct Outcome {
    pub diagnostics: Vec<Diagnostic>,
    pub topic_pool: Option<TopicPool>,
}

fn fail(message: impl Into<String>) -> Diagnostic {
    Diagnostic::new(DiagnosticLevel::Fail, message).expect("message is non-empty")
}

const YAML_PATH: &str = "standards/workspace/instruction-topics.yaml";
const MD_PATH: &str = "standards/workspace/instruction-topics.md";

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
            topic_pool: None,
        });
    }
    let yaml_raw = yaml_raw.unwrap();
    let md_raw = md_raw.unwrap();

    let mut diagnostics = Vec::new();

    let names: Vec<String> = match parse_yaml(&yaml_raw) {
        Ok(doc) => match doc.get("topics") {
            // Absent, or a JS-falsy scalar in the Node reference's own
            // `(yamlParse(...)?.topics || []).map(String)` — both fall back
            // to an empty, legal pool there.
            None | Some(Value::Null) | Some(Value::Bool(false)) => Vec::new(),
            Some(Value::Number(n)) if n.as_f64() == Some(0.0) => Vec::new(),
            Some(Value::String(s)) if s.is_empty() => Vec::new(),
            Some(Value::Array(arr)) => arr
                .iter()
                .map(|v| match v {
                    Value::String(s) => s.clone(),
                    other => other.to_string(),
                })
                .collect(),
            Some(other) => {
                diagnostics.push(checks::topics_not_a_list(type_name(other)));
                Vec::new()
            }
        },
        Err(error) => {
            diagnostics.push(fail(format!("instruction-topics: {error}")));
            Vec::new()
        }
    };

    let region = match marked_region(&md_raw, "topic-pool") {
        Ok(region) => region,
        Err(error) => {
            diagnostics.push(checks::region_unreadable(&error));
            return Ok(Outcome {
                diagnostics,
                topic_pool: None,
            });
        }
    };

    let row_re = Regex::new(r"(?m)^\|\s*`([a-z0-9-]+)`\s*\|").expect("row pattern compiles");
    let documented_ordered: Vec<String> = dedupe_preserve_order(
        regex_collect_captures(&row_re, &region.text, MD_PATH)?
            .into_iter()
            .filter_map(|c| c.get(1).map(|g| g.as_str().to_string())),
    );
    let declared_ordered: Vec<String> = dedupe_preserve_order(names);

    match checks::check_pool_agreement(&declared_ordered, &documented_ordered) {
        Ok(pool) => Ok(Outcome {
            diagnostics,
            topic_pool: Some(pool),
        }),
        Err(diagnostic) => {
            diagnostics.push(diagnostic);
            Ok(Outcome {
                diagnostics,
                topic_pool: None,
            })
        }
    }
}

/// The JS "typeof"-adjacent label the Node reference's uncaught `TypeError`
/// implicitly names for a non-array `topics` value.
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
    fn agreeing_halves_produce_the_declared_pool_and_no_diagnostics() {
        let reader = FakeReader::new()
            .with_file(YAML_PATH, "topics:\n  - agent-conduct\n  - test-planning\n")
            .with_file(
                MD_PATH,
                "before\n<!-- meridian:begin topic-pool -->\n| `agent-conduct` | x |\n| `test-planning` | y |\n<!-- meridian:end topic-pool -->\nafter\n",
            );
        let outcome = run(&reader).unwrap();
        assert!(outcome.diagnostics.is_empty(), "{:?}", outcome.diagnostics);
        assert_eq!(outcome.topic_pool.unwrap().len(), 2);
    }

    #[test]
    fn a_missing_pool_file_fails_closed() {
        let outcome = run(&FakeReader::new()).unwrap();
        assert_eq!(outcome.diagnostics.len(), 1);
        assert!(outcome.diagnostics[0]
            .message()
            .contains("is missing from the Kernel"));
        assert!(outcome.topic_pool.is_none());
    }

    #[test]
    fn a_wrong_type_topics_key_is_a_failure() {
        let reader = FakeReader::new()
            .with_file(YAML_PATH, "topics: not-a-list\n")
            .with_file(
                MD_PATH,
                "<!-- meridian:begin topic-pool -->\n<!-- meridian:end topic-pool -->\n",
            );
        let outcome = run(&reader).unwrap();
        assert!(outcome
            .diagnostics
            .iter()
            .any(|d| d.message().contains("must be a list")));
    }

    #[test]
    fn an_io_failure_reading_the_yaml_pool_propagates_as_an_operation_error() {
        let reader = FakeReader::new()
            .with_error(YAML_PATH, ReadError::Io("permission denied".to_string()))
            .with_file(
                MD_PATH,
                "<!-- meridian:begin topic-pool -->\n<!-- meridian:end topic-pool -->\n",
            );
        let error = run(&reader).unwrap_err();
        assert_eq!(error.path, YAML_PATH);
        assert!(error.message.contains("permission denied"));
    }

    #[test]
    fn an_io_failure_reading_the_md_pool_propagates_as_an_operation_error() {
        let reader = FakeReader::new()
            .with_file(YAML_PATH, "topics: []\n")
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
        assert!(outcome.topic_pool.is_some());
    }
}
