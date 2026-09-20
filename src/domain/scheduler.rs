use std::collections::{BTreeMap, BTreeSet};

use uuid::Uuid;

use super::policy::evaluate_policy;
use super::{
    Assignment, CoveragePlan, CoveragePlanStatus, Policy, PolicyDecision, PolicyEvidence, WorkItem,
    Worker,
};

const ASSIGNMENT_NAMESPACE: Uuid = Uuid::from_u128(0x5f69d2ca_2251_4c31_996e_59ee246afe81);

#[derive(Debug, Clone)]
pub struct ScheduleInput<'a> {
    pub organization_id: Uuid,
    pub plan_id: Uuid,
    pub incident_id: Uuid,
    pub work_items: &'a [WorkItem],
    pub workers: &'a [Worker],
    pub policies: &'a [Policy],
    pub expected_metric_if_fully_covered: f64,
    pub expected_metric_if_uncovered: f64,
}

pub fn schedule(input: ScheduleInput<'_>) -> CoveragePlan {
    let mut work: Vec<&WorkItem> = input
        .work_items
        .iter()
        .filter(|item| {
            item.organization_id == input.organization_id && item.status.is_schedulable()
        })
        .collect();

    work.sort_by(|left, right| {
        right
            .priority
            .rank()
            .cmp(&left.priority.rank())
            .then_with(|| left.sla_due_at.cmp(&right.sla_due_at))
            .then_with(|| left.deadline.cmp(&right.deadline))
            .then_with(|| right.risk.rank().cmp(&left.risk.rank()))
            .then_with(|| left.id.cmp(&right.id))
    });

    let mut runtime_workers: BTreeMap<Uuid, Worker> = input
        .workers
        .iter()
        .filter(|worker| worker.organization_id == input.organization_id)
        .cloned()
        .map(|worker| (worker.id, worker))
        .collect();

    let mut assignments = Vec::new();
    let mut policy_decisions = Vec::new();
    let mut uncovered_work = Vec::new();
    let mut escalations = BTreeSet::new();
    let mut estimated_cost_micros = 0_u64;
    let mut covered_capacity = 0_u32;

    for item in work {
        let mut candidates: Vec<(Uuid, PolicyEvidence)> = runtime_workers
            .values()
            .filter(|worker| worker.status.can_accept_work())
            .filter(|worker| worker.has_capabilities(&item.required_capabilities))
            .filter(|worker| worker.active_assignments < worker.maximum_concurrency)
            .filter(|worker| worker.effective_available_capacity() >= item.estimated_effort)
            .map(|worker| (worker.id, evaluate_policy(worker, item, input.policies)))
            .collect();

        for (_, evidence) in &candidates {
            policy_decisions.push(evidence.clone());
            if evidence.decision == PolicyDecision::Escalate {
                escalations.insert(item.id);
            }
        }

        candidates.retain(|(_, evidence)| evidence.decision == PolicyDecision::Allow);
        candidates.sort_by(|(left_id, _), (right_id, _)| {
            let left = &runtime_workers[left_id];
            let right = &runtime_workers[right_id];

            let left_stability = u8::from(item.assigned_worker_id == Some(*left_id));
            let right_stability = u8::from(item.assigned_worker_id == Some(*right_id));

            right_stability
                .cmp(&left_stability)
                .then_with(|| {
                    right
                        .effective_available_capacity()
                        .cmp(&left.effective_available_capacity())
                })
                .then_with(|| {
                    left.cost_per_capacity_unit_micros
                        .cmp(&right.cost_per_capacity_unit_micros)
                })
                .then_with(|| left.id.cmp(&right.id))
        });

        if let Some((worker_id, _)) = candidates.first() {
            let worker = runtime_workers
                .get_mut(worker_id)
                .expect("candidate worker must exist in runtime map");
            worker.available_capacity = worker
                .available_capacity
                .saturating_sub(item.estimated_effort);
            worker.active_assignments = worker.active_assignments.saturating_add(1);

            let assignment_name = format!("{}:{}:{}", input.plan_id, item.id, worker.id);
            let assignment_id = Uuid::new_v5(&ASSIGNMENT_NAMESPACE, assignment_name.as_bytes());
            assignments.push(Assignment {
                id: assignment_id,
                organization_id: input.organization_id,
                coverage_plan_id: input.plan_id,
                work_item_id: item.id,
                worker_id: worker.id,
                capacity_units: item.estimated_effort,
            });
            estimated_cost_micros = estimated_cost_micros.saturating_add(
                worker
                    .cost_per_capacity_unit_micros
                    .saturating_mul(u64::from(item.estimated_effort)),
            );
            covered_capacity = covered_capacity.saturating_add(item.estimated_effort);
            escalations.remove(&item.id);
        } else {
            uncovered_work.push(item.id);
        }
    }

    let missing_capacity = input
        .work_items
        .iter()
        .filter(|item| uncovered_work.contains(&item.id))
        .map(|item| item.estimated_effort)
        .sum();

    let expected_metric = if missing_capacity == 0 {
        input.expected_metric_if_fully_covered
    } else {
        input.expected_metric_if_uncovered
    };

    CoveragePlan {
        id: input.plan_id,
        organization_id: input.organization_id,
        incident_id: input.incident_id,
        status: CoveragePlanStatus::Validated,
        assignments,
        policy_decisions,
        uncovered_work,
        escalations: escalations.into_iter().collect(),
        estimated_cost_micros,
        covered_capacity,
        missing_capacity,
        expected_metric,
    }
}
