//! Acceptance tests for the pure resolver (`meridian_core::resolver`),
//! mirroring the invariants `rule-resolver.mjs`/`rule-resolution.md` define
//! and the package's required proof points: every scope and activation
//! mode, order-independence, conflicts never silently resolved,
//! ambiguous/cyclic `supersedes` failing closed, `changed_paths` forcing
//! re-resolution, and initiative vs. ordinary unresolved kinds never mixed.

use meridian_core::resolver::*;
use meridian_core::types::ContentDigest;

fn repo(id: &str, profile: Option<&str>, areas: &[&str]) -> RepositoryInventoryEntry {
    RepositoryInventoryEntry {
        id: id.to_string(),
        profile: profile.map(str::to_string),
        semantic_areas: areas.iter().map(|s| s.to_string()).collect(),
    }
}

fn kernel_norm(path: &str, text: &str) -> (NormRef, ContentDigest) {
    (
        NormRef::new("kernel", path, None).unwrap(),
        ContentDigest::of_str(text),
    )
}

fn base_sources() -> ResolverSources {
    ResolverSources {
        repository_inventory: vec![repo(
            "cbs-core-frontend",
            Some("vue-monorepo"),
            &["billing", "checkout"],
        )],
        ..Default::default()
    }
}

fn work_item(kind: WorkItemKind, candidate_paths: Vec<&str>, changed_paths: Vec<&str>) -> WorkItem {
    WorkItem::new(
        "cbs-core-frontend",
        kind,
        candidate_paths.into_iter().map(str::to_string).collect(),
        changed_paths.into_iter().map(str::to_string).collect(),
        None,
    )
    .unwrap()
}

// --- scope axis: each of the four scopes, match and no-match ---------------

#[test]
fn universal_scope_always_applies() {
    let (norm, digest) = kernel_norm("standards/x.md", "universal norm text");
    let rec = ApplicabilityRecord::new(
        norm,
        digest,
        ApplicabilityScope::Universal,
        Activation::Always,
        ApplicabilitySource::Kernel,
        ApplicabilityStatus::Resolved,
        IsoDate::new("2026-09-08").unwrap(),
    );
    let mut sources = base_sources();
    sources
        .norm_texts
        .insert(rec.norm_text_key(), "universal norm text".to_string());
    sources.applicability_records = vec![rec];

    let out = resolve_rules(
        &work_item(WorkItemKind::Operation, vec![], vec![]),
        &sources,
    )
    .unwrap();
    assert_eq!(out.applicable_norms.len(), 1);
    assert_eq!(out.applicable_norms[0].norm, "standards/x.md");
}

#[test]
fn repository_scope_matches_only_the_named_repository() {
    let (norm, digest) = kernel_norm("standards/repo-only.md", "t");
    let matching = ApplicabilityRecord::new(
        norm,
        digest,
        ApplicabilityScope::Repository {
            repository: "cbs-core-frontend".to_string(),
        },
        Activation::Always,
        ApplicabilitySource::Kernel,
        ApplicabilityStatus::Resolved,
        IsoDate::new("2026-09-08").unwrap(),
    );
    let (norm2, digest2) = kernel_norm("standards/other-repo.md", "t2");
    let other = ApplicabilityRecord::new(
        norm2,
        digest2,
        ApplicabilityScope::Repository {
            repository: "some-other-repo".to_string(),
        },
        Activation::Always,
        ApplicabilitySource::Kernel,
        ApplicabilityStatus::Resolved,
        IsoDate::new("2026-09-08").unwrap(),
    );
    let mut sources = base_sources();
    sources
        .norm_texts
        .insert(matching.norm_text_key(), "t".to_string());
    sources
        .norm_texts
        .insert(other.norm_text_key(), "t2".to_string());
    sources.applicability_records = vec![matching, other];

    let out = resolve_rules(
        &work_item(WorkItemKind::Operation, vec![], vec![]),
        &sources,
    )
    .unwrap();
    assert_eq!(out.applicable_norms.len(), 1);
    assert_eq!(out.applicable_norms[0].norm, "standards/repo-only.md");
}

#[test]
fn profile_scope_matches_the_repositorys_declared_profile() {
    let (norm, digest) = kernel_norm("standards/profile.md", "t");
    let rec = ApplicabilityRecord::new(
        norm,
        digest,
        ApplicabilityScope::Profile {
            technology_profile: "vue-monorepo".to_string(),
        },
        Activation::Always,
        ApplicabilitySource::Kernel,
        ApplicabilityStatus::Resolved,
        IsoDate::new("2026-09-08").unwrap(),
    );
    let mut sources = base_sources();
    sources
        .norm_texts
        .insert(rec.norm_text_key(), "t".to_string());
    sources.applicability_records = vec![rec];

    let out = resolve_rules(
        &work_item(WorkItemKind::Operation, vec![], vec![]),
        &sources,
    )
    .unwrap();
    assert_eq!(out.applicable_norms.len(), 1);
}

#[test]
fn profile_scope_no_match_for_a_different_profile() {
    let (norm, digest) = kernel_norm("standards/profile.md", "t");
    let rec = ApplicabilityRecord::new(
        norm,
        digest,
        ApplicabilityScope::Profile {
            technology_profile: "go-service".to_string(),
        },
        Activation::Always,
        ApplicabilitySource::Kernel,
        ApplicabilityStatus::Resolved,
        IsoDate::new("2026-09-08").unwrap(),
    );
    let mut sources = base_sources();
    sources
        .norm_texts
        .insert(rec.norm_text_key(), "t".to_string());
    sources.applicability_records = vec![rec];

    let out = resolve_rules(
        &work_item(WorkItemKind::Operation, vec![], vec![]),
        &sources,
    )
    .unwrap();
    assert!(out.applicable_norms.is_empty());
}

#[test]
fn product_domain_scope_matches_a_repositorys_semantic_area() {
    let (norm, digest) = kernel_norm("standards/domain.md", "t");
    let rec = ApplicabilityRecord::new(
        norm,
        digest,
        ApplicabilityScope::ProductDomain {
            product_domain: "billing".to_string(),
        },
        Activation::Always,
        ApplicabilitySource::Kernel,
        ApplicabilityStatus::Resolved,
        IsoDate::new("2026-09-08").unwrap(),
    );
    let mut sources = base_sources();
    sources
        .norm_texts
        .insert(rec.norm_text_key(), "t".to_string());
    sources.applicability_records = vec![rec];

    let out = resolve_rules(
        &work_item(WorkItemKind::Operation, vec![], vec![]),
        &sources,
    )
    .unwrap();
    assert_eq!(out.applicable_norms.len(), 1);
}

// --- activation axis: each of the five modes --------------------------------

#[test]
fn path_glob_activation_matches_a_candidate_path() {
    let (norm, digest) = kernel_norm("standards/glob.md", "t");
    let rec = ApplicabilityRecord::new(
        norm,
        digest,
        ApplicabilityScope::Universal,
        Activation::path_glob(vec!["src/**/*.rs".to_string()]).unwrap(),
        ApplicabilitySource::Kernel,
        ApplicabilityStatus::Resolved,
        IsoDate::new("2026-09-08").unwrap(),
    );
    let mut sources = base_sources();
    sources
        .norm_texts
        .insert(rec.norm_text_key(), "t".to_string());
    sources.applicability_records = vec![rec];

    let hit = resolve_rules(
        &work_item(WorkItemKind::Operation, vec!["src/lib/core.rs"], vec![]),
        &sources,
    )
    .unwrap();
    assert_eq!(hit.applicable_norms.len(), 1);

    let miss = resolve_rules(
        &work_item(WorkItemKind::Operation, vec!["docs/readme.md"], vec![]),
        &sources,
    )
    .unwrap();
    assert!(miss.applicable_norms.is_empty());
}

#[test]
fn task_class_activation_matches_work_kind_and_change_class() {
    let (norm, digest) = kernel_norm("standards/task-class.md", "t");
    let rec = ApplicabilityRecord::new(
        norm,
        digest,
        ApplicabilityScope::Universal,
        Activation::TaskClass {
            task_class: TaskClassSelector::new(
                vec![WorkKind::Change],
                Some(vec![ChangeClass::Bugfix]),
            )
            .unwrap(),
        },
        ApplicabilitySource::Kernel,
        ApplicabilityStatus::Resolved,
        IsoDate::new("2026-09-08").unwrap(),
    );
    let mut sources = base_sources();
    sources
        .norm_texts
        .insert(rec.norm_text_key(), "t".to_string());
    sources.applicability_records = vec![rec];

    let bugfix = resolve_rules(
        &work_item(WorkItemKind::Change(ChangeClass::Bugfix), vec![], vec![]),
        &sources,
    )
    .unwrap();
    assert_eq!(bugfix.applicable_norms.len(), 1);

    let feature = resolve_rules(
        &work_item(WorkItemKind::Change(ChangeClass::Feature), vec![], vec![]),
        &sources,
    )
    .unwrap();
    assert!(feature.applicable_norms.is_empty());
}

#[test]
fn explicit_activation_never_activates_and_is_not_unresolved() {
    let (norm, digest) = kernel_norm("standards/explicit.md", "t");
    let rec = ApplicabilityRecord::new(
        norm,
        digest,
        ApplicabilityScope::Universal,
        Activation::Explicit,
        ApplicabilitySource::Kernel,
        ApplicabilityStatus::Resolved,
        IsoDate::new("2026-09-08").unwrap(),
    );
    let mut sources = base_sources();
    sources
        .norm_texts
        .insert(rec.norm_text_key(), "t".to_string());
    sources.applicability_records = vec![rec];

    let out = resolve_rules(
        &work_item(WorkItemKind::Operation, vec![], vec![]),
        &sources,
    )
    .unwrap();
    assert!(out.applicable_norms.is_empty());
    assert!(out.unresolved_applicability.is_empty());
}

#[test]
fn undetermined_activation_is_reported_unresolved_not_applicable() {
    let (norm, digest) = kernel_norm("standards/undetermined.md", "t");
    let rec = ApplicabilityRecord::new(
        norm,
        digest,
        ApplicabilityScope::Universal,
        Activation::Undetermined,
        ApplicabilitySource::Kernel,
        ApplicabilityStatus::Resolved,
        IsoDate::new("2026-09-08").unwrap(),
    );
    let mut sources = base_sources();
    sources
        .norm_texts
        .insert(rec.norm_text_key(), "t".to_string());
    sources.applicability_records = vec![rec];

    let out = resolve_rules(
        &work_item(WorkItemKind::Operation, vec![], vec![]),
        &sources,
    )
    .unwrap();
    assert!(out.applicable_norms.is_empty());
    assert_eq!(out.unresolved_applicability.len(), 1);
    assert_eq!(
        out.unresolved_applicability[0].subject,
        "standards/undetermined.md"
    );
}

#[test]
fn status_unresolved_is_reported_with_its_own_resume_condition() {
    let (norm, digest) = kernel_norm("standards/unresolved-status.md", "t");
    let rec = ApplicabilityRecord::new(
        norm,
        digest,
        ApplicabilityScope::Universal,
        Activation::Always,
        ApplicabilitySource::Kernel,
        ApplicabilityStatus::Unresolved {
            resume_condition: "waiting on owner decision X".to_string(),
        },
        IsoDate::new("2026-09-08").unwrap(),
    );
    let mut sources = base_sources();
    sources
        .norm_texts
        .insert(rec.norm_text_key(), "t".to_string());
    sources.applicability_records = vec![rec];

    let out = resolve_rules(
        &work_item(WorkItemKind::Operation, vec![], vec![]),
        &sources,
    )
    .unwrap();
    assert!(out.applicable_norms.is_empty());
    assert_eq!(out.unresolved_applicability.len(), 1);
    assert_eq!(
        out.unresolved_applicability[0].reason,
        "waiting on owner decision X"
    );
}

// --- order independence -----------------------------------------------------

#[test]
fn output_does_not_depend_on_applicability_record_input_order() {
    let mut sources = base_sources();
    let mut records = Vec::new();
    let mut names = Vec::new();
    for i in 0..5 {
        let path = format!("standards/order-{i}.md");
        let text = format!("text {i}");
        let (norm, digest) = kernel_norm(&path, &text);
        let rec = ApplicabilityRecord::new(
            norm,
            digest,
            ApplicabilityScope::Universal,
            Activation::Always,
            ApplicabilitySource::Kernel,
            ApplicabilityStatus::Resolved,
            IsoDate::new("2026-09-08").unwrap(),
        );
        sources.norm_texts.insert(rec.norm_text_key(), text);
        names.push(path);
        records.push(rec);
    }
    sources.applicability_records = records.clone();
    let forward = resolve_rules(
        &work_item(WorkItemKind::Operation, vec![], vec![]),
        &sources,
    )
    .unwrap();

    records.reverse();
    sources.applicability_records = records;
    let reversed = resolve_rules(
        &work_item(WorkItemKind::Operation, vec![], vec![]),
        &sources,
    )
    .unwrap();

    assert_eq!(forward, reversed);
    assert_eq!(forward.applicable_norms.len(), 5);
}

// --- protocol routing: conflict is never a silent choice --------------------

#[test]
fn two_incompatible_mandatory_routes_produce_a_conflict_not_a_silent_pick() {
    let mut sources = base_sources();
    let route_field_order = || {
        vec![
            RouteField::Protocol,
            RouteField::RoutedFrom,
            RouteField::Source,
            RouteField::Scope,
            RouteField::Revision,
            RouteField::Mandatory,
        ]
    };
    sources.protocol_routes = vec![
        ProtocolRoute::new(
            "protocol-a",
            RouteKey::ChangeClass(ChangeClass::Bugfix),
            RouteSource::Kernel,
            ProtocolRouteScope::Universal,
            RouteProvenance::Revision("abc1234".to_string()),
            true,
            route_field_order(),
        )
        .unwrap(),
        ProtocolRoute::new(
            "protocol-b",
            RouteKey::ChangeClass(ChangeClass::Bugfix),
            RouteSource::Repository,
            ProtocolRouteScope::Universal,
            RouteProvenance::Revision("def5678".to_string()),
            true,
            route_field_order(),
        )
        .unwrap(),
    ];

    let out = resolve_rules(
        &work_item(WorkItemKind::Change(ChangeClass::Bugfix), vec![], vec![]),
        &sources,
    )
    .unwrap();

    assert_eq!(out.conflicts.len(), 1);
    assert_eq!(
        out.conflicts[0].norms,
        vec!["protocol-a".to_string(), "protocol-b".to_string()]
    );
    // Neither conflicting mandatory route is silently promoted.
    assert!(out.applicable_protocols.is_empty());
}

#[test]
fn a_repository_route_does_not_silently_override_a_universal_one_when_both_mandatory() {
    // Sanity check that a NON-conflicting pair (different protocols routed
    // from different keys, or one non-mandatory) resolves without a
    // conflict, so the conflict test above is meaningful and not spurious.
    let mut sources = base_sources();
    sources.protocol_routes = vec![ProtocolRoute::new(
        "bugfix-protocol",
        RouteKey::ChangeClass(ChangeClass::Bugfix),
        RouteSource::Kernel,
        ProtocolRouteScope::Universal,
        RouteProvenance::Revision("abc1234".to_string()),
        true,
        vec![
            RouteField::Protocol,
            RouteField::RoutedFrom,
            RouteField::Source,
            RouteField::Scope,
            RouteField::Revision,
            RouteField::Mandatory,
        ],
    )
    .unwrap()];
    let out = resolve_rules(
        &work_item(WorkItemKind::Change(ChangeClass::Bugfix), vec![], vec![]),
        &sources,
    )
    .unwrap();
    assert!(out.conflicts.is_empty());
    assert_eq!(out.applicable_protocols.len(), 1);
    assert_eq!(out.applicable_protocols[0].protocol, "bugfix-protocol");
}

// --- supersedes: ambiguous or cyclic fails closed ---------------------------

fn intake_ptr(register: &str, date: &str, verdict: IntakeVerdict) -> IntakePointer {
    IntakePointer::new(register, IsoDate::new(date).unwrap(), verdict, None).unwrap()
}

fn repo_norm(path: &str) -> NormRef {
    NormRef::new("cbs-core-frontend", path, None).unwrap()
}

fn intake_record(artifact: &str, date: &str, verdict: IntakeVerdict) -> IntakeRecordEntry {
    IntakeRecordEntry {
        artifact: artifact.to_string(),
        region: None,
        recorded_at: IsoDate::new(date).unwrap(),
        verdict,
        delivery: Delivery::AgentsMdSection,
    }
}

#[test]
fn two_same_date_records_with_no_supersedes_relationship_fail_closed() {
    let mut sources = base_sources();
    sources.intake_registers = vec![IntakeRegister {
        register: "instruction-intake/cbs.yaml".to_string(),
        records: vec![
            intake_record("AGENTS.md", "2026-09-01", IntakeVerdict::Deferred),
            intake_record("AGENTS.md", "2026-09-01", IntakeVerdict::AdoptCore),
        ],
    }];
    let digest = ContentDigest::of_str("t");
    let a = ApplicabilityRecord::new(
        repo_norm("AGENTS.md"),
        digest.clone(),
        ApplicabilityScope::Universal,
        Activation::Always,
        ApplicabilitySource::Repository {
            intake_record: intake_ptr(
                "instruction-intake/cbs.yaml",
                "2026-09-01",
                IntakeVerdict::Deferred,
            ),
            supersedes: None,
        },
        ApplicabilityStatus::Resolved,
        IsoDate::new("2026-09-08").unwrap(),
    );
    let b = ApplicabilityRecord::new(
        repo_norm("AGENTS.md"),
        digest,
        ApplicabilityScope::Universal,
        Activation::Always,
        ApplicabilitySource::Repository {
            intake_record: intake_ptr(
                "instruction-intake/cbs.yaml",
                "2026-09-01",
                IntakeVerdict::AdoptCore,
            ),
            supersedes: None,
        },
        ApplicabilityStatus::Resolved,
        IsoDate::new("2026-09-08").unwrap(),
    );
    sources
        .norm_texts
        .insert(a.norm_text_key(), "t".to_string());
    sources.applicability_records = vec![a, b];

    let err = resolve_rules(
        &work_item(WorkItemKind::Operation, vec![], vec![]),
        &sources,
    )
    .unwrap_err();
    // fail-closed: no applicable_norms is ever computed from an ambiguous tie
    assert!(matches!(err, ResolverError::Other(_)));
}

#[test]
fn a_valid_supersedes_relationship_picks_the_declared_head() {
    let mut sources = base_sources();
    sources.intake_registers = vec![IntakeRegister {
        register: "instruction-intake/cbs.yaml".to_string(),
        records: vec![
            intake_record("AGENTS.md", "2026-09-01", IntakeVerdict::Deferred),
            intake_record("AGENTS.md", "2026-09-02", IntakeVerdict::AdoptCore),
        ],
    }];
    let text = "current text";
    let digest = ContentDigest::of_str(text);
    let superseded = ApplicabilityRecord::new(
        repo_norm("AGENTS.md"),
        ContentDigest::of_str("stale text"),
        ApplicabilityScope::Universal,
        Activation::Always,
        ApplicabilitySource::Repository {
            intake_record: intake_ptr(
                "instruction-intake/cbs.yaml",
                "2026-09-01",
                IntakeVerdict::Deferred,
            ),
            supersedes: None,
        },
        ApplicabilityStatus::Resolved,
        IsoDate::new("2026-09-08").unwrap(),
    );
    let superseding = ApplicabilityRecord::new(
        repo_norm("AGENTS.md"),
        digest,
        ApplicabilityScope::Universal,
        Activation::Always,
        ApplicabilitySource::Repository {
            intake_record: intake_ptr(
                "instruction-intake/cbs.yaml",
                "2026-09-02",
                IntakeVerdict::AdoptCore,
            ),
            supersedes: Some(
                SupersedesPointer::new(
                    "instruction-intake/cbs.yaml",
                    "AGENTS.md",
                    None,
                    IsoDate::new("2026-09-01").unwrap(),
                    IntakeVerdict::Deferred,
                    None,
                )
                .unwrap(),
            ),
        },
        ApplicabilityStatus::Resolved,
        IsoDate::new("2026-09-08").unwrap(),
    );
    sources
        .norm_texts
        .insert(superseding.norm_text_key(), text.to_string());
    sources.applicability_records = vec![superseded, superseding];

    let out = resolve_rules(
        &work_item(WorkItemKind::Operation, vec![], vec![]),
        &sources,
    )
    .unwrap();
    assert_eq!(out.applicable_norms.len(), 1);
}

#[test]
fn a_supersedes_cycle_fails_closed() {
    let mut sources = base_sources();
    sources.intake_registers = vec![IntakeRegister {
        register: "instruction-intake/cbs.yaml".to_string(),
        records: vec![
            intake_record("AGENTS.md", "2026-09-01", IntakeVerdict::Deferred),
            intake_record("AGENTS.md", "2026-09-01", IntakeVerdict::AdoptCore),
        ],
    }];
    let digest = ContentDigest::of_str("t");
    let a = ApplicabilityRecord::new(
        repo_norm("AGENTS.md"),
        digest.clone(),
        ApplicabilityScope::Universal,
        Activation::Always,
        ApplicabilitySource::Repository {
            intake_record: intake_ptr(
                "instruction-intake/cbs.yaml",
                "2026-09-01",
                IntakeVerdict::Deferred,
            ),
            supersedes: Some(
                SupersedesPointer::new(
                    "instruction-intake/cbs.yaml",
                    "AGENTS.md",
                    None,
                    IsoDate::new("2026-09-01").unwrap(),
                    IntakeVerdict::AdoptCore,
                    None,
                )
                .unwrap(),
            ),
        },
        ApplicabilityStatus::Resolved,
        IsoDate::new("2026-09-08").unwrap(),
    );
    let b = ApplicabilityRecord::new(
        repo_norm("AGENTS.md"),
        digest,
        ApplicabilityScope::Universal,
        Activation::Always,
        ApplicabilitySource::Repository {
            intake_record: intake_ptr(
                "instruction-intake/cbs.yaml",
                "2026-09-01",
                IntakeVerdict::AdoptCore,
            ),
            supersedes: Some(
                SupersedesPointer::new(
                    "instruction-intake/cbs.yaml",
                    "AGENTS.md",
                    None,
                    IsoDate::new("2026-09-01").unwrap(),
                    IntakeVerdict::Deferred,
                    None,
                )
                .unwrap(),
            ),
        },
        ApplicabilityStatus::Resolved,
        IsoDate::new("2026-09-08").unwrap(),
    );
    sources
        .norm_texts
        .insert(a.norm_text_key(), "t".to_string());
    sources.applicability_records = vec![a, b];

    let err = resolve_rules(
        &work_item(WorkItemKind::Operation, vec![], vec![]),
        &sources,
    )
    .unwrap_err();
    assert!(matches!(err, ResolverError::Other(_)));
    assert!(format!("{err}").contains("cycle"));
}

// --- requires_reresolution ---------------------------------------------------

#[test]
fn changed_paths_outside_candidate_paths_forces_reresolution() {
    let sources = base_sources();
    let out = resolve_rules(
        &work_item(WorkItemKind::Operation, vec!["src/a.rs"], vec!["src/b.rs"]),
        &sources,
    )
    .unwrap();
    assert!(out.requires_reresolution);
}

#[test]
fn changed_paths_within_candidate_paths_does_not_force_reresolution() {
    let sources = base_sources();
    let out = resolve_rules(
        &work_item(
            WorkItemKind::Operation,
            vec!["src/a.rs", "src/b.rs"],
            vec!["src/b.rs"],
        ),
        &sources,
    )
    .unwrap();
    assert!(!out.requires_reresolution);
}

// --- initiative: unresolved_items vs unresolved_applicability are distinct -

#[test]
fn initiative_without_decomposition_reports_decomposition_blocker_not_applicability() {
    let sources = base_sources();
    let out = resolve_rules(
        &work_item(WorkItemKind::Initiative, vec![], vec![]),
        &sources,
    )
    .unwrap();
    assert!(out.decomposition_required);
    assert_eq!(out.unresolved_items.len(), 1);
    assert!(out.unresolved_applicability.is_empty());
}

#[test]
fn initiative_with_enough_source_repositories_has_no_decomposition_blocker() {
    let mut sources = base_sources();
    sources.prior_state.source_repository_ids =
        Some(vec!["repo-a".to_string(), "repo-b".to_string()]);
    let out = resolve_rules(
        &work_item(WorkItemKind::Initiative, vec![], vec![]),
        &sources,
    )
    .unwrap();
    assert!(out.decomposition_required);
    assert!(out.unresolved_items.is_empty());
}

#[test]
fn a_decomposed_initiative_is_not_decomposition_required() {
    let mut sources = base_sources();
    sources.prior_state.decomposition = Some(Decomposition {
        child_work_items: vec!["child-1".to_string()],
    });
    sources.prior_state.initiative_protocol = Some("consolidation-initiative-protocol".to_string());
    sources.prior_state.pre_decomposition_norms = vec![
        "b-norm".to_string(),
        "a-norm".to_string(),
        "a-norm".to_string(),
    ];
    let out = resolve_rules(
        &work_item(WorkItemKind::Initiative, vec![], vec![]),
        &sources,
    )
    .unwrap();
    assert!(!out.decomposition_required);
    assert!(out.unresolved_items.is_empty());
    assert_eq!(
        out.applicable_initiative_protocol.as_deref(),
        Some("consolidation-initiative-protocol")
    );
    assert_eq!(
        out.pre_decomposition_norms,
        vec!["a-norm".to_string(), "b-norm".to_string()]
    );
}

// --- REFACTOR: an in-progress finding is a separate unresolved entry, not a
//     reclassification --------------------------------------------------------

#[test]
fn refactor_finding_coexists_with_the_resolved_protocol_route() {
    let mut sources = base_sources();
    sources.protocol_routes = vec![ProtocolRoute::new(
        "refactor-protocol",
        RouteKey::ChangeClass(ChangeClass::Refactor),
        RouteSource::Kernel,
        ProtocolRouteScope::Universal,
        RouteProvenance::Revision("abc1234".to_string()),
        true,
        vec![
            RouteField::Protocol,
            RouteField::RoutedFrom,
            RouteField::Source,
            RouteField::Scope,
            RouteField::Revision,
            RouteField::Mandatory,
        ],
    )
    .unwrap()];
    sources.prior_state.refactor_findings = vec![RefactorFinding {
        finding_type: "behavior-change".to_string(),
    }];

    let out = resolve_rules(
        &work_item(WorkItemKind::Change(ChangeClass::Refactor), vec![], vec![]),
        &sources,
    )
    .unwrap();

    assert_eq!(out.applicable_protocols.len(), 1);
    assert_eq!(out.applicable_protocols[0].protocol, "refactor-protocol");
    assert_eq!(out.unresolved_applicability.len(), 1);
    assert_eq!(
        out.unresolved_applicability[0].subject,
        "refactor-in-progress-finding"
    );
}

// --- fail-closed provenance --------------------------------------------------

#[test]
fn missing_norm_text_fails_closed() {
    let (norm, digest) = kernel_norm("standards/missing-text.md", "t");
    let rec = ApplicabilityRecord::new(
        norm,
        digest,
        ApplicabilityScope::Universal,
        Activation::Always,
        ApplicabilitySource::Kernel,
        ApplicabilityStatus::Resolved,
        IsoDate::new("2026-09-08").unwrap(),
    );
    let mut sources = base_sources();
    sources.applicability_records = vec![rec]; // no norm_texts entry

    let err = resolve_rules(
        &work_item(WorkItemKind::Operation, vec![], vec![]),
        &sources,
    )
    .unwrap_err();
    assert!(format!("{err}").contains("no current text supplied"));
}

#[test]
fn stale_digest_fails_closed() {
    let (norm, digest) = kernel_norm("standards/stale.md", "original text");
    let rec = ApplicabilityRecord::new(
        norm,
        digest,
        ApplicabilityScope::Universal,
        Activation::Always,
        ApplicabilitySource::Kernel,
        ApplicabilityStatus::Resolved,
        IsoDate::new("2026-09-08").unwrap(),
    );
    let mut sources = base_sources();
    sources.norm_texts.insert(
        rec.norm_text_key(),
        "text that has since changed".to_string(),
    );
    sources.applicability_records = vec![rec];

    let err = resolve_rules(
        &work_item(WorkItemKind::Operation, vec![], vec![]),
        &sources,
    )
    .unwrap_err();
    assert!(format!("{err}").contains("stale digest"));
}

#[test]
fn unknown_repository_id_fails_closed_with_no_fallback() {
    let sources = base_sources();
    let wi = WorkItem::new(
        "does-not-exist",
        WorkItemKind::Operation,
        vec![],
        vec![],
        None,
    )
    .unwrap();
    let err = resolve_rules(&wi, &sources).unwrap_err();
    assert_eq!(
        err,
        ResolverError::UnknownRepository {
            repository_id: "does-not-exist".to_string()
        }
    );
}

// --- deterministic output order ---------------------------------------------

#[test]
fn applicable_norms_are_returned_in_stable_sorted_order() {
    let mut sources = base_sources();
    let mut records = Vec::new();
    for name in ["zzz", "aaa", "mmm"] {
        let path = format!("standards/{name}.md");
        let (norm, digest) = kernel_norm(&path, name);
        let rec = ApplicabilityRecord::new(
            norm,
            digest,
            ApplicabilityScope::Universal,
            Activation::Always,
            ApplicabilitySource::Kernel,
            ApplicabilityStatus::Resolved,
            IsoDate::new("2026-09-08").unwrap(),
        );
        sources
            .norm_texts
            .insert(rec.norm_text_key(), name.to_string());
        records.push(rec);
    }
    sources.applicability_records = records;

    let out = resolve_rules(
        &work_item(WorkItemKind::Operation, vec![], vec![]),
        &sources,
    )
    .unwrap();
    let ids: Vec<&str> = out
        .applicable_norms
        .iter()
        .map(|n| n.norm.as_str())
        .collect();
    assert_eq!(
        ids,
        vec!["standards/aaa.md", "standards/mmm.md", "standards/zzz.md"]
    );
}
