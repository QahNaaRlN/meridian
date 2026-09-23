//! The built-in catalogue of the seven universal roles
//! (`registries/operating-model/role-registry.schema.json`;
//! `standards/workspace/role-registry.yaml` is the canonical instance).
//!
//! The schema fixes the catalogue's scope (`built-in-methodology`), origin
//! (`built-in`) and authority kind (`methodology-owner`) as constants and
//! each role id to the closed [`UniversalRole`] pool, so none of those is a
//! field here. Two entries with the same role id but different prose ARE
//! schema-valid (`uniqueItems` compares whole objects), which is why
//! duplicate and missing roles remain domain rules of
//! [`check_role_registry`] — the ONLY way to obtain a [`RoleCatalogue`].

use std::collections::HashSet;

use crate::task_contracts::non_portable_reason;
use crate::types::Diagnostic;

use super::envelope::{check_envelope, RecordEnvelope, RecordFamily};
use super::identity::RecordText;
use super::vocabulary::UniversalRole;
use super::{fail, has_cyrillic};

/// One catalogue entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoleDefinition {
    pub id: UniversalRole,
    pub title: RecordText,
    pub summary: RecordText,
    pub responsibilities: Vec<RecordText>,
}

/// One schema-clean role-registry record, not yet domain-checked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoleRegistryInput {
    pub envelope: RecordEnvelope,
    pub roles: Vec<RoleDefinition>,
}

/// A catalogue that defines every universal role exactly once.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoleCatalogue(RoleRegistryInput);

impl RoleCatalogue {
    pub fn envelope(&self) -> &RecordEnvelope {
        &self.0.envelope
    }

    /// The definition of `role` — always present in an accepted catalogue.
    pub fn definition(&self, role: UniversalRole) -> Option<&RoleDefinition> {
        self.0.roles.iter().find(|r| r.id == role)
    }

    pub fn roles(&self) -> &[RoleDefinition] {
        &self.0.roles
    }
}

/// The catalogue rules, in the Node reference's diagnostic order.
pub fn check_role_registry(input: RoleRegistryInput) -> (Option<RoleCatalogue>, Vec<Diagnostic>) {
    let mut problems = Vec::new();
    check_envelope(RecordFamily::RoleRegistry, &input.envelope, &mut problems);
    let id = input.envelope.id.as_str();

    if input.roles.is_empty() {
        problems.push(fail(format!(
            "role registry \"{id}\" states no roles; the catalogue body carries the closed pool of universal roles"
        )));
    } else {
        let mut seen = HashSet::new();
        for role in &input.roles {
            let at = role.id.as_str();
            if !seen.insert(role.id) {
                problems.push(fail(format!(
                    "role registry \"{id}\" role \"{at}\" is declared more than once"
                )));
            }
            if role.title.is_blank() {
                problems.push(fail(format!(
                    "role registry \"{id}\" role {at} has no Russian title"
                )));
            } else if !has_cyrillic(role.title.as_str()) {
                problems.push(fail(format!(
                    "role registry \"{id}\" role {at} title \"{}\" carries no Russian (Cyrillic) text",
                    role.title
                )));
            }
            if role.summary.is_blank() {
                problems.push(fail(format!(
                    "role registry \"{id}\" role {at} has no summary"
                )));
            }
            if role.responsibilities.is_empty() {
                problems.push(fail(format!(
                    "role registry \"{id}\" role {at} states no responsibilities; a universal role names its powers and duties"
                )));
            }
            for (j, duty) in role.responsibilities.iter().enumerate() {
                if duty.is_blank() {
                    problems.push(fail(format!(
                        "role registry \"{id}\" role {at} responsibility #{j} is empty or whitespace-only"
                    )));
                }
                if let Some(r) = non_portable_reason(Some(duty.as_str())) {
                    problems.push(fail(format!(
                        "role registry \"{id}\" role {at} responsibility #{j} contains {r}"
                    )));
                }
            }
            for text in [&role.title, &role.summary] {
                if let Some(r) = non_portable_reason(Some(text.as_str())) {
                    problems.push(fail(format!(
                        "role registry \"{id}\" role {at} contains {r}"
                    )));
                }
            }
        }
        let missing: Vec<&str> = UniversalRole::ALL
            .iter()
            .filter(|r| !seen.contains(*r))
            .map(|r| r.as_str())
            .collect();
        if !missing.is_empty() {
            problems.push(fail(format!(
                "role registry \"{id}\" is missing universal role(s) {}; the pool is closed and fully covered",
                missing.join(", ")
            )));
        }
    }

    let accepted = problems.is_empty().then_some(RoleCatalogue(input));
    (accepted, problems)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::run_contracts::envelope::tests::envelope;
    use crate::run_contracts::envelope::RecordOrigin;

    fn text(s: &str) -> RecordText {
        RecordText::new(s).unwrap()
    }

    fn catalogue() -> RoleRegistryInput {
        let mut env = envelope(RecordFamily::RoleRegistry);
        env.origin = RecordOrigin::BuiltIn;
        RoleRegistryInput {
            envelope: env,
            roles: UniversalRole::ALL
                .iter()
                .map(|r| RoleDefinition {
                    id: *r,
                    title: text("Роль"),
                    summary: text("Сводка."),
                    responsibilities: vec![text("Обязанность.")],
                })
                .collect(),
        }
    }

    #[test]
    fn the_seven_roles_form_an_accepted_catalogue() {
        let (accepted, problems) = check_role_registry(catalogue());
        assert!(problems.is_empty(), "{problems:?}");
        let accepted = accepted.unwrap();
        assert_eq!(accepted.roles().len(), 7);
        assert!(accepted.definition(UniversalRole::GitIntegrator).is_some());
    }

    #[test]
    fn a_duplicated_role_is_reported_with_the_role_it_displaces() {
        let mut input = catalogue();
        input.roles[6] = input.roles[0].clone();
        let (accepted, problems) = check_role_registry(input);
        assert!(accepted.is_none());
        let m: Vec<&str> = problems.iter().map(Diagnostic::message).collect();
        assert_eq!(
            m,
            [
                "role registry \"example-record\" role \"owner\" is declared more than once",
                "role registry \"example-record\" is missing universal role(s) deployer; the pool is closed and fully covered",
            ]
        );
    }

    #[test]
    fn prose_rules_are_reported_per_role() {
        let mut input = catalogue();
        input.roles[1].title = text("Operator");
        input.roles[1].responsibilities.push(text(" "));
        let (_, problems) = check_role_registry(input);
        let m: Vec<&str> = problems.iter().map(Diagnostic::message).collect();
        assert_eq!(
            m,
            [
                "role registry \"example-record\" role operator title \"Operator\" carries no Russian (Cyrillic) text",
                "role registry \"example-record\" role operator responsibility #1 is empty or whitespace-only",
            ]
        );
    }
}
