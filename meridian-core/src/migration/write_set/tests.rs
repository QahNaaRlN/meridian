use super::*;
use crate::migration::plan::check_migration_plans;
use crate::migration::plan::tests::{plan, recompute, resolution_for, sid};
use crate::types::{AuthorityKind, NonEmptyString};

fn accepted(input: crate::migration::plan::PlanInput) -> MigrationPlan {
    let outcome = check_migration_plans(std::slice::from_ref(&input), &resolution_for(&input));
    assert!(outcome.diagnostics.is_empty(), "{:?}", outcome.diagnostics);
    outcome.accepted[0].clone().unwrap()
}

fn read_back(entry: &WriteSetEntry) -> ReadBackRecord {
    let t = entry.target();
    ReadBackRecord {
        scope: t.scope.clone(),
        id: t.id.clone(),
        schema: t.schema_ref.as_str().to_string(),
        schema_version: 1,
        title: t.title.as_str().to_string(),
        record_type: t.record_type.clone(),
        origin: Origin::migrated(t.origin_source_ref.as_str()).unwrap(),
        authority: Authority::new(
            t.authority.kind,
            t.authority.authority_ref.as_str(),
            Some(t.authority.decision_ref.as_str().to_string()),
        )
        .unwrap(),
        payload: entry.payload(),
    }
}

#[test]
fn migration_write_set_mints_every_target_once_and_keeps_retained_units() {
    let set = FrozenWriteSet::from_plan(&accepted(plan())).unwrap();
    assert_eq!(
        set.counts(),
        UnitCounts {
            record_units: 2,
            migrated: 1,
            retained: 1,
            merged: 0,
            minted_targets: 1
        }
    );
    assert_eq!(set.entries()[0].target().id, sid("target-a"));
    assert_eq!(set.retained()[0].unit_id, sid("unit-b"));
}

#[test]
fn migration_write_set_refuses_a_built_in_methodology_target_before_any_write() {
    let mut input = plan();
    if let DispositionInput::Migrated { target, .. } = &mut input.payload.mappings[0].disposition {
        target.scope = Scope::built_in_methodology();
        target.authority.kind = AuthorityKind::MethodologyOwner;
    }
    recompute(&mut input);
    let error = FrozenWriteSet::from_plan(&accepted(input)).unwrap_err();
    assert_eq!(
        error,
        WriteSetError::BuiltInMethodologyTarget {
            target_id: "target-a".to_string()
        }
    );
}

#[test]
fn migration_verification_of_an_exact_read_back_is_clean() {
    let set = FrozenWriteSet::from_plan(&accepted(plan())).unwrap();
    let stored: Vec<ReadBackRecord> = set.entries().iter().map(read_back).collect();
    let diff = verify_read_back(&set, &stored);
    assert!(diff.is_clean(), "{diff:?}");
    assert_eq!((diff.expected, diff.imported), (1, 1));
}

#[test]
fn migration_verification_names_missing_changed_extra_and_duplicate_records() {
    let set = FrozenWriteSet::from_plan(&accepted(plan())).unwrap();
    assert_eq!(verify_read_back(&set, &[]).missing, vec![sid("target-a")]);

    let mut changed = read_back(&set.entries()[0]);
    changed.title = "другой заголовок".to_string();
    changed.payload = CanonicalJson::string("tampered");
    let diff = verify_read_back(&set, &[changed]);
    assert_eq!(
        diff.changed[0].fields,
        vec![RecordField::Title, RecordField::Payload]
    );

    let original = read_back(&set.entries()[0]);
    let mut elsewhere = original.clone();
    elsewhere.scope = Scope::project_workspace(sid("other-project"), None);
    let mut retained_leak = original.clone();
    retained_leak.id = sid("unit-b-record");
    retained_leak.origin = Origin::migrated("record-unit:unit-b").unwrap();
    let diff = verify_read_back(&set, &[original, elsewhere, retained_leak]);
    assert_eq!(
        diff.extra,
        vec![
            "project-workspace:other-project/target-a".to_string(),
            "project-workspace:sample-project/unit-b-record".to_string()
        ]
    );
    assert_eq!(diff.duplicate, vec!["record-unit:unit-a".to_string()]);
    assert!(!diff.is_clean());
}

#[test]
fn migration_verification_ignores_records_of_other_provenance() {
    let set = FrozenWriteSet::from_plan(&accepted(plan())).unwrap();
    let mut unrelated = read_back(&set.entries()[0]);
    unrelated.id = sid("unrelated");
    unrelated.origin = Origin::Declared {
        source_ref: NonEmptyString::new("declared:elsewhere").unwrap(),
    };
    let stored = vec![read_back(&set.entries()[0]), unrelated];
    assert!(verify_read_back(&set, &stored).is_clean());
}

#[test]
fn migration_verification_compares_payloads_in_canonical_form() {
    let set = FrozenWriteSet::from_plan(&accepted(plan())).unwrap();
    let mut record = read_back(&set.entries()[0]);
    let t = set.entries()[0].target();
    // The same members in another order are the same payload.
    record.payload = CanonicalJson::object(vec![
        (
            "digest".to_string(),
            CanonicalJson::object(vec![
                (
                    "value".to_string(),
                    CanonicalJson::string(t.payload.digest.value()),
                ),
                ("algorithm".to_string(), CanonicalJson::string("sha-256")),
            ]),
        ),
        (
            "content".to_string(),
            CanonicalJson::string(t.payload.content.as_str()),
        ),
        ("encoding".to_string(), CanonicalJson::string("utf-8")),
        (
            "media_type".to_string(),
            CanonicalJson::string(t.payload.media_type.as_str()),
        ),
    ]);
    assert!(verify_read_back(&set, &[record]).is_clean());
}
