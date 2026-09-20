use levus::domain::{DEMO_ORGANIZATION_ID, RuntimeStore};

#[test]
fn demo_moves_from_healthy_to_incident_and_coverage_plan() {
    let mut store = RuntimeStore::default();
    store.reset_demo();
    let before = store.organization(DEMO_ORGANIZATION_ID).unwrap().status();
    assert_eq!(before.continuity_status, "HEALTHY");
    assert_eq!(before.open_work, 0);

    {
        let runtime = store.organization_mut(DEMO_ORGANIZATION_ID).unwrap();
        runtime.disrupt_demo();
        assert!(runtime.reconcile_latest());
    }

    let after = store.organization(DEMO_ORGANIZATION_ID).unwrap();
    assert_eq!(after.incidents.len(), 1);
    assert_eq!(after.coverage_plans.len(), 1);
    assert_eq!(after.coverage_plans[0].assignments.len(), 57);
    assert_eq!(after.coverage_plans[0].escalations.len(), 3);
    assert_eq!(after.coverage_plans[0].uncovered_work.len(), 3);
    assert_eq!(after.coverage_plans[0].missing_capacity, 3);
}

#[test]
fn replaying_reconciliation_is_idempotent_for_incident_and_plan_identity() {
    let mut store = RuntimeStore::default();
    store.reset_demo();
    let runtime = store.organization_mut(DEMO_ORGANIZATION_ID).unwrap();
    runtime.disrupt_demo();
    assert!(runtime.reconcile_latest());
    let first_incident = runtime.incidents[0].id;
    let first_plan = runtime.coverage_plans[0].id;

    assert!(runtime.reconcile_latest());
    assert_eq!(runtime.incidents.len(), 1);
    assert_eq!(runtime.coverage_plans.len(), 1);
    assert_eq!(runtime.incidents[0].id, first_incident);
    assert_eq!(runtime.coverage_plans[0].id, first_plan);
}
