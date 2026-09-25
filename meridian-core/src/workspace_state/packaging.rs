//! Packaging discipline of a `skill-package` intake record (D3 of the
//! `rust-workspace-state-validation` handoff).
//!
//! A skill is found by its own name: the name `SKILL.md` declares must be
//! the name of the directory that holds it. A package activates through the
//! package vocabulary; the rule vocabulary (`alwaysApply`, `globs`) in its
//! Front Matter declares activation a second time and leaves the choice to
//! the tool. The text itself is read by `meridian-app` into
//! [`SkillPackageText`]; this module only judges it.

use crate::types::{Diagnostic, DiagnosticLevel};

use super::diagnostic;
use super::intake::{Delivery, IntakeRecord};

/// A Front Matter key that belongs to the rule vocabulary, not the package
/// one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RuleActivationKey {
    AlwaysApply,
    Globs,
}

impl RuleActivationKey {
    pub const ALL: [RuleActivationKey; 2] = [Self::AlwaysApply, Self::Globs];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::AlwaysApply => "alwaysApply",
            Self::Globs => "globs",
        }
    }
}

/// What a `SKILL.md` says about its own packaging.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SkillPackageText {
    /// The name the file declares, when it declares one.
    pub declared_name: Option<String>,
    /// The rule-vocabulary keys its Front Matter carries, in
    /// [`RuleActivationKey::ALL`] order.
    pub rule_activation: Vec<RuleActivationKey>,
}

/// The records whose packaging is judged: a skill package whose artifact is
/// a `SKILL.md` inside a directory.
pub fn is_packaged_skill(record: &IntakeRecord) -> bool {
    record.delivery() == Delivery::SkillPackage && record.artifact().ends_with("/SKILL.md")
}

/// The name of the directory holding `artifact`.
fn directory_name(artifact: &str) -> &str {
    let directory = artifact.rsplit_once('/').map_or("", |(dir, _)| dir);
    directory.rsplit('/').next().unwrap_or(directory)
}

/// The verdict on one packaged skill; `text` is `None` when the file could
/// not be read from here.
pub fn check_packaging(artifact: &str, text: Option<&SkillPackageText>) -> Vec<Diagnostic> {
    let Some(text) = text else {
        return vec![diagnostic(
            DiagnosticLevel::Info,
            format!("instruction-intake: {artifact} not reachable; its packaging is UNVERIFIED, not confirmed"),
        )];
    };
    let mut out = Vec::new();
    let directory = directory_name(artifact);
    if let Some(name) = &text.declared_name {
        if name != directory {
            out.push(diagnostic(
                DiagnosticLevel::Fail,
                format!("instruction-intake: {artifact} is a skill named \"{name}\" in a directory named \"{directory}\"; a norm that cannot be found by its own name makes every reference to it dangling"),
            ));
        }
    }
    if !text.rule_activation.is_empty() {
        let keys: Vec<&str> = text.rule_activation.iter().map(|k| k.as_str()).collect();
        out.push(diagnostic(
            DiagnosticLevel::Fail,
            format!(
                "instruction-intake: {artifact} declares activation twice — {} belong to the rule vocabulary, not the package one; which one applies is then decided by the tool, not by the author",
                keys.join(" and ")
            ),
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fails(text: &SkillPackageText) -> Vec<String> {
        check_packaging("skills/review/SKILL.md", Some(text))
            .iter()
            .map(|d| {
                assert_eq!(d.level(), DiagnosticLevel::Fail);
                d.message().to_string()
            })
            .collect()
    }

    #[test]
    fn workspace_state_packaging_a_package_named_by_its_directory_is_clean() {
        assert!(fails(&SkillPackageText {
            declared_name: Some("review".into()),
            rule_activation: vec![],
        })
        .is_empty());
        assert!(fails(&SkillPackageText::default()).is_empty());
        assert_eq!(directory_name("a/b/c/SKILL.md"), "c");
    }

    #[test]
    fn workspace_state_packaging_a_misnamed_or_doubly_activated_package_fails() {
        assert_eq!(
            fails(&SkillPackageText {
                declared_name: Some("reviewer".into()),
                rule_activation: vec![RuleActivationKey::AlwaysApply, RuleActivationKey::Globs],
            }),
            [
                "instruction-intake: skills/review/SKILL.md is a skill named \"reviewer\" in a directory named \"review\"; a norm that cannot be found by its own name makes every reference to it dangling",
                "instruction-intake: skills/review/SKILL.md declares activation twice — alwaysApply and globs belong to the rule vocabulary, not the package one; which one applies is then decided by the tool, not by the author",
            ]
        );
    }

    #[test]
    fn workspace_state_packaging_an_unreadable_package_is_unverified() {
        let out = check_packaging("skills/review/SKILL.md", None);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].level(), DiagnosticLevel::Info);
        assert_eq!(
            out[0].message(),
            "instruction-intake: skills/review/SKILL.md not reachable; its packaging is UNVERIFIED, not confirmed"
        );
    }
}
