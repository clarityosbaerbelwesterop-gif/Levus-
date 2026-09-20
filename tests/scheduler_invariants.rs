use std::collections::BTreeMap;

use chrono::{TimeZone, Utc};
use levus::domain::{
    CapabilityLevel, Policy, PolicyDecision, Priority, Risk, ScheduleInput, WorkItem, WorkStatus,
    Worker, WorkerStatus, WorkerType, schedule,
};
use pretty_assertions::assert_eq;
use uuid::Uuid;

fn worker(
    id: u128,
    org: Uuid,
    worker_type: WorkerType,
    level: u8,
    capacity: u32,
    cost: u64,
) -> Worker {
    Worker {
        id: Uuid::from_u128(id),
        organization_id: org,
        name: format!("worker-{id}"),
        worker_type,
        status: WorkerStatus::Available,
        capabilities: BTreeMap::from([("support.ticket.respond".to_owned(), level)]),
        total_capacity: capacity,
        available_capacity: capacity,
        reserved_capacity: 0,
        maximum_concurrency: capacity as u16,
        active_assignments: 0,
        cost_per_capacity_unit_micros: cost,
        authority: BTreeMap::new(),
    }
}

fn work(id: u128, org: Uuid, level: u8, risk: Risk) -> WorkItem {
    let at = Utc.timestamp_opt(1_800_000_000, 0).single().unwrap();
    WorkItem {
        id: Uuid::from_u128(id),
        organization_id: org,
        title: format!("work-{id}"),
        description: String::new(),
        work_type: "support.ticket.respond".to_owned(),
        status: WorkStatus::Ready,
        priority: Priority::Normal,
        risk,
        required_capabilities: vec![CapabilityLevel {
            capability: "support.ticket.respond".to_owned(),
            level,
        }],
        estimated_effort: 1,
        deadline: Some(at),
        sla_due_at: Some(at),
        source: "test".to_owned(),
        external_reference: None,
        assigned_worker_id: None,
        created_at: at,
        updated_at: at,
    }
}

#[test]
fn identical_inputs_produce_identical_plan() {
    let org = Uuid::from_u128(1);
    let plan_id = Uuid::from_u128(2);
    let incident_id = Uuid::from_u128(3);
    let work_items = vec![
        work(10, org, 2, Risk::Medium),
        work(11, org, 2, Risk::Medium),
    ];
    let workers = vec![
        worker(20, org, WorkerType::AiAgent, 2, 2, 10),
        worker(21, org, WorkerType::Human, 3, 2, 100),
    ];

    let input = || ScheduleInput {
        organization_id: org,
        plan_id,
        incident_id,
        work_items: &work_items,
        workers: &workers,
        policies: &[],
        expected_metric_if_fully_covered: 4.0,
        expected_metric_if_uncovered: 14.0,
    };

    assert_eq!(schedule(input()), schedule(input()));
}

#[test]
fn insufficient_capability_is_never_assigned() {
    let org = Uuid::from_u128(1);
    let work_items = vec![work(10, org, 3, Risk::High)];
    let workers = vec![worker(20, org, WorkerType::AiAgent, 2, 10, 1)];
    let plan = schedule(ScheduleInput {
        organization_id: org,
        plan_id: Uuid::from_u128(2),
        incident_id: Uuid::from_u128(3),
        work_items: &work_items,
        workers: &workers,
        policies: &[],
        expected_metric_if_fully_covered: 4.0,
        expected_metric_if_uncovered: 14.0,
    });

    assert!(plan.assignments.is_empty());
    assert_eq!(plan.uncovered_work, vec![work_items[0].id]);
}

#[test]
fn policy_denial_is_a_hard_scheduling_boundary() {
    let org = Uuid::from_u128(1);
    let work_items = vec![work(10, org, 2, Risk::High)];
    let workers = vec![worker(20, org, WorkerType::AiAgent, 3, 10, 1)];
    let policies = vec![Policy {
        id: Uuid::from_u128(30),
        organization_id: org,
        name: "deny AI high risk".to_owned(),
        priority: 100,
        worker_type: Some(WorkerType::AiAgent),
        work_type_prefix: Some("support.ticket".to_owned()),
        capability_prefix: None,
        minimum_risk: Some(Risk::High),
        decision: PolicyDecision::Deny,
        reason: "test denial".to_owned(),
        enabled: true,
    }];

    let plan = schedule(ScheduleInput {
        organization_id: org,
        plan_id: Uuid::from_u128(2),
        incident_id: Uuid::from_u128(3),
        work_items: &work_items,
        workers: &workers,
        policies: &policies,
        expected_metric_if_fully_covered: 4.0,
        expected_metric_if_uncovered: 14.0,
    });

    assert!(plan.assignments.is_empty());
    assert_eq!(plan.policy_decisions[0].decision, PolicyDecision::Deny);
}

#[test]
fn scheduler_never_crosses_tenant_boundary() {
    let org_a = Uuid::from_u128(1);
    let org_b = Uuid::from_u128(2);
    let work_items = vec![work(10, org_a, 2, Risk::Medium)];
    let workers = vec![worker(20, org_b, WorkerType::Human, 3, 10, 1)];
    let plan = schedule(ScheduleInput {
        organization_id: org_a,
        plan_id: Uuid::from_u128(3),
        incident_id: Uuid::from_u128(4),
        work_items: &work_items,
        workers: &workers,
        policies: &[],
        expected_metric_if_fully_covered: 4.0,
        expected_metric_if_uncovered: 14.0,
    });

    assert!(plan.assignments.is_empty());
    assert_eq!(plan.uncovered_work, vec![work_items[0].id]);
}

#[test]
fn capacity_and_concurrency_are_enforced() {
    let org = Uuid::from_u128(1);
    let work_items = vec![
        work(10, org, 2, Risk::Medium),
        work(11, org, 2, Risk::Medium),
    ];
    let mut limited = worker(20, org, WorkerType::Human, 3, 2, 1);
    limited.maximum_concurrency = 1;
    let workers = vec![limited];

    let plan = schedule(ScheduleInput {
        organization_id: org,
        plan_id: Uuid::from_u128(3),
        incident_id: Uuid::from_u128(4),
        work_items: &work_items,
        workers: &workers,
        policies: &[],
        expected_metric_if_fully_covered: 4.0,
        expected_metric_if_uncovered: 14.0,
    });

    assert_eq!(plan.assignments.len(), 1);
    assert_eq!(plan.uncovered_work.len(), 1);
}
