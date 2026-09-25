//! Orchestration of the instruction-intake checks over the ports: M-12
//! (references), M-15 (completeness against each repository's tree), and
//! the three further checks of decision D3 — topic existence, the packaging
//! of a `skill-package` and the parentage of an `adopt-edition`. Every
//! verdict is a pure `meridian_core::workspace_state` decision; this module
//! only observes the repositories through [`RepositoryAccess`] and reads a
//! `SKILL.md` into its typed [`SkillPackageText`].

use std::collections::{BTreeMap, BTreeSet};

use meridian_core::mechanical_integrity::instruction_topics::TopicPool;
use meridian_core::types::{ContentDigest, Diagnostic, WorkspaceRelativePath};
use meridian_core::workspace_state::intake::{
    self, ContainerRegions, DeclaredRegion, IntakeRecord, RepositoryTree, TreeFact,
};
use meridian_core::workspace_state::inventory::RepositoryIdentity;
use meridian_core::workspace_state::packaging::{self, RuleActivationKey, SkillPackageText};
use meridian_core::workspace_state::parentage::{
    self, ParentAtHead, ParentObservation, ParentReference, ParentRepository,
};

use super::ports::{RepositoryAccess, RevisionSelector};
use crate::source_format::regions::instruction_regions;

/// What the intake checks need beyond the records themselves.
pub(super) struct IntakeContext<'a> {
    /// The inventory, sorted by id.
    pub identities: &'a [&'a RepositoryIdentity],
    pub repositories: &'a dyn RepositoryAccess,
    /// The Kernel as [`RepositoryAccess`] addresses it: the parent
    /// repository `"kernel"`.
    pub kernel_root: &'a str,
    /// The Kernel's agreed topic pool; `None` when it did not load, which
    /// `instruction-topics` already reports.
    pub topic_pool: Option<&'a TopicPool>,
}

/// The intake verdicts: every diagnostic, and how many repository trees are
/// confirmed complete.
pub(super) struct IntakeOutcome {
    pub diagnostics: Vec<Diagnostic>,
    pub repositories: usize,
    pub complete: usize,
}

pub(super) fn check(records: &[IntakeRecord], context: &IntakeContext<'_>) -> IntakeOutcome {
    let identity_of = |id: &str| {
        context
            .identities
            .iter()
            .copied()
            .find(|identity| identity.id().as_str() == id)
    };
    let known: BTreeSet<String> = context
        .identities
        .iter()
        .map(|i| i.id().as_str().to_string())
        .collect();
    let mut diagnostics = intake::check_references(records, &known);

    let mut by_repository: BTreeMap<&str, Vec<&IntakeRecord>> = BTreeMap::new();
    for record in records {
        by_repository
            .entry(record.repository().as_str())
            .or_default()
            .push(record);
    }
    let mut complete = 0;
    for (repository, records) in &by_repository {
        if let Some(pool) = context.topic_pool {
            diagnostics.extend(intake::check_topics(repository, records, pool));
        }
        // An unknown repository is M-12's unresolvable reference, never an
        // unreachable one: nothing of its tree is judged.
        if let Some(identity) = identity_of(repository) {
            let tree = repository_tree(identity.path(), records, context.repositories);
            let check = intake::check_completeness(repository, records, tree.as_ref());
            complete += usize::from(check.complete);
            diagnostics.extend(check.diagnostics);
            for record in records.iter().filter(|r| packaging::is_packaged_skill(r)) {
                let text = skill_package(identity.path(), record.artifact(), context.repositories);
                diagnostics.extend(packaging::check_packaging(record.artifact(), text.as_ref()));
            }
        }
        for record in records {
            if let Some(parent) = record.parent() {
                let observed = observe_parent(parent, context, &identity_of);
                diagnostics.extend(parentage::judge(record.artifact(), parent, &observed));
            }
        }
    }
    IntakeOutcome {
        diagnostics,
        repositories: by_repository.len(),
        complete,
    }
}

// ------------------------------------------------------------------ tree

/// The tree of one reachable repository; `None` when its tracked files
/// cannot be listed at all. A later port failure is carried as
/// [`TreeFact::Unavailable`], never read as an empty answer.
fn repository_tree(
    root: &str,
    records: &[&IntakeRecord],
    repositories: &dyn RepositoryAccess,
) -> Option<RepositoryTree> {
    let tracked = repositories.tracked_files(root).ok()?;
    let untracked_norms = match repositories.untracked_files(root) {
        Ok(untracked) => TreeFact::Observed(
            untracked
                .into_iter()
                .filter(|p| intake::is_intake_candidate(p))
                .collect(),
        ),
        Err(unavailable) => TreeFact::Unavailable(unavailable.0),
    };
    let tracked_set: BTreeSet<&str> = tracked.iter().map(String::as_str).collect();
    let mut candidates: Vec<String> = records
        .iter()
        .map(|r| r.artifact().to_string())
        .filter(|a| !tracked_set.contains(a.as_str()))
        .collect();
    candidates.sort();
    candidates.dedup();
    let ignored_norms = if candidates.is_empty() {
        TreeFact::Observed(Vec::new())
    } else {
        match repositories.ignored_among(root, &candidates) {
            Ok(ignored) => TreeFact::Observed(ignored),
            Err(unavailable) => TreeFact::Unavailable(unavailable.0),
        }
    };
    let reader = repositories.reader(root);
    let containers = tracked
        .iter()
        .filter(|f| intake::is_intake_candidate(f) && intake::is_container(f))
        .map(|file| {
            let parsed = WorkspaceRelativePath::new(file.as_str())
                .ok()
                .and_then(|rel| reader.read_text(&rel).ok())
                .map(|body| {
                    let result = instruction_regions(&body);
                    ContainerRegions {
                        errors: result.errors,
                        regions: result
                            .regions
                            .into_iter()
                            .map(|r| DeclaredRegion {
                                id: r.id,
                                owner: r.owner,
                                generated: r.generated,
                            })
                            .collect(),
                        uncovered_lines: result.uncovered_lines,
                    }
                });
            (file.clone(), parsed)
        })
        .collect();
    Some(RepositoryTree {
        tracked,
        untracked_norms,
        ignored_norms,
        containers,
    })
}

// ------------------------------------------------------------- packaging

/// The packaging facts of a `SKILL.md`, or `None` when it cannot be read.
fn skill_package(
    root: &str,
    artifact: &str,
    repositories: &dyn RepositoryAccess,
) -> Option<SkillPackageText> {
    let path = WorkspaceRelativePath::new(artifact).ok()?;
    let body = repositories.reader(root).read_text(&path).ok()?;
    Some(skill_package_text(&body))
}

/// The line starts of `text`: offset 0 and every offset after a `\n`.
fn line_starts(text: &str) -> impl Iterator<Item = &str> {
    std::iter::once(text).chain(text.match_indices('\n').map(move |(i, _)| &text[i + 1..]))
}

/// The name a `SKILL.md` declares: the first line that starts with
/// `name:` and continues, after any whitespace, with one token followed by
/// nothing but whitespace up to the end of its line.
fn declared_name(body: &str) -> Option<String> {
    line_starts(body).find_map(|rest| {
        let value = rest.strip_prefix("name:")?.trim_start();
        let end = value.find(char::is_whitespace).unwrap_or(value.len());
        let (token, after) = value.split_at(end);
        let line_rest = after.split('\n').next().unwrap_or("");
        (!token.is_empty() && line_rest.trim().is_empty()).then(|| token.to_string())
    })
}

/// The Front Matter block: the text between an opening `---` line at the
/// very start of the file and the next line that starts with `---`.
fn front_matter(body: &str) -> &str {
    let Some(rest) = body.strip_prefix("---\n") else {
        return "";
    };
    rest.find("\n---").map_or("", |end| &rest[..end])
}

fn skill_package_text(body: &str) -> SkillPackageText {
    let front = front_matter(body);
    let rule_activation = RuleActivationKey::ALL
        .into_iter()
        .filter(|key| {
            line_starts(front).any(|line| {
                line.strip_prefix(key.as_str())
                    .is_some_and(|after| after.trim_start().starts_with(':'))
            })
        })
        .collect();
    SkillPackageText {
        declared_name: declared_name(body),
        rule_activation,
    }
}

// -------------------------------------------------------------- parentage

fn observe_parent<'a>(
    parent: &ParentReference,
    context: &IntakeContext<'a>,
    identity_of: &dyn Fn(&str) -> Option<&'a RepositoryIdentity>,
) -> ParentObservation {
    let root = match &parent.repository {
        ParentRepository::Kernel => context.kernel_root,
        ParentRepository::Inventory(id) => match identity_of(id.as_str()) {
            Some(identity) => identity.path(),
            None => return ParentObservation::UnknownRepository,
        },
    };
    let repositories = context.repositories;
    if repositories.vcs_state(root).is_err() {
        return ParentObservation::Unreachable;
    }
    match repositories.has_revision(root, &parent.revision) {
        Ok(true) => {}
        Ok(false) => return ParentObservation::RevisionAbsent,
        Err(unavailable) => return ParentObservation::Unreadable(unavailable.0),
    }
    let taken = match repositories.file_at(
        root,
        RevisionSelector::Commit(&parent.revision),
        &parent.path,
    ) {
        Ok(Some(text)) => ContentDigest::of_str(&text),
        Ok(None) => return ParentObservation::AbsentAtRevision,
        Err(unavailable) => return ParentObservation::Unreadable(unavailable.0),
    };
    let at_head = match repositories.file_at(root, RevisionSelector::Head, &parent.path) {
        Ok(Some(text)) => ParentAtHead::Present(ContentDigest::of_str(&text)),
        Ok(None) => ParentAtHead::Absent,
        Err(unavailable) => ParentAtHead::Unreadable(unavailable.0),
    };
    ParentObservation::Present {
        at_revision: taken,
        at_head,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workspace_state_skill_package_text_reads_name_and_rule_vocabulary() {
        let text = skill_package_text(
            "---\nname: review\ndescription: x\nalwaysApply : true\nglobs: '*.ts'\n---\nbody\n",
        );
        assert_eq!(text.declared_name.as_deref(), Some("review"));
        assert_eq!(
            text.rule_activation,
            [RuleActivationKey::AlwaysApply, RuleActivationKey::Globs]
        );
        // The rule vocabulary counts only inside the Front Matter.
        let text = skill_package_text("---\nname: review\n---\nglobs: in the body\n");
        assert!(text.rule_activation.is_empty());
        // No Front Matter at the very start: nothing is Front Matter.
        assert!(skill_package_text("\n---\nalwaysApply: true\n---\n")
            .rule_activation
            .is_empty());
        // A name with more than one token on its line declares nothing.
        assert_eq!(skill_package_text("name: two words\n").declared_name, None);
        assert_eq!(
            skill_package_text("# x\nname: late  \r\n")
                .declared_name
                .as_deref(),
            Some("late")
        );
    }
}
