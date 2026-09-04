//! R-00070: monotonic allocator, abandon holes, double-finalize, stamp mapping.

use lumio_voxel_contracts::{SCHEMA_IDS, STABLE_ERROR_IDS};
use lumio_voxel_domain::revision::{REVISION_STAMP_SCHEMA, RevisionAllocator, to_generated_stamp};

#[test]
fn world_and_section_domains_are_independent_and_monotonic() {
    let mut a = RevisionAllocator::new();
    let mut w0 = a.reserve_world().unwrap();
    let mut w1 = a.reserve_world().unwrap();
    let mut c0 = a.reserve_section().unwrap();
    assert_eq!(w0.value().value(), 0);
    assert_eq!(w1.value().value(), 1);
    assert_eq!(c0.value().value(), 0);
    let fw0 = w0.finalize().unwrap();
    let fw1 = w1.finalize().unwrap();
    let fc0 = c0.finalize().unwrap();
    assert!(fw1 > fw0);
    assert_eq!(fc0.value(), 0);
    assert_eq!(
        fw0.value(),
        0,
        "finalize returns the reserved value unchanged"
    );
    // Independence is domain isolation, not different starting numbers: both domains
    // start at 0 by design, and the type system already forbids comparing a
    // WorldRevision with a SectionRevision. Prove instead that each counter advances
    // only on its own domain's reservations.
    let c1 = a.reserve_section().unwrap();
    assert_eq!(
        c1.value().value(),
        1,
        "two world reservations must not advance the section domain"
    );
    let w2 = a.reserve_world().unwrap();
    assert_eq!(
        w2.value().value(),
        2,
        "section reservations must not advance the world domain"
    );
}

#[test]
fn abandon_leaves_a_hole_and_double_finalize_is_stable_error() {
    let mut a = RevisionAllocator::new();
    let mut hole = a.reserve_world().unwrap();
    hole.abandon();
    assert_eq!(hole.finalize().unwrap_err().error_id(), "InvalidHandle");
    assert!(STABLE_ERROR_IDS.contains(&"InvalidHandle"));

    let mut once = a.reserve_world().unwrap();
    assert_eq!(once.value().value(), 1, "abandoned 0 is not reused");
    once.finalize().unwrap();
    let err = once.finalize().unwrap_err();
    assert_eq!(err.error_id(), "HandleDoubleRelease");
    assert!(STABLE_ERROR_IDS.contains(&err.error_id()));
}

#[test]
fn stamp_wraps_generated_schema_id_only() {
    let mut a = RevisionAllocator::new();
    let mut w = a.reserve_world().unwrap();
    let mut c = a.reserve_section().unwrap();
    let world = w.finalize().unwrap();
    let section = c.finalize().unwrap();
    let stamp = to_generated_stamp(
        "world-a",
        "ctx-1",
        7,
        world,
        &[("s:0:0:0".to_string(), section)],
    );
    assert_eq!(stamp.schema_id, REVISION_STAMP_SCHEMA);
    assert!(SCHEMA_IDS.contains(&stamp.schema_id));
    assert_eq!(stamp.world_revision, 0);
    assert_eq!(stamp.section_revision_set.get("s:0:0:0"), Some(&0));
}
