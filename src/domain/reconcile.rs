use chrono::Utc;
use uuid::Uuid;

use super::{
    ContinuityIncident, CoveragePlan, DesiredState, IncidentSeverity, OperationalMetric, Policy,
    ScheduleInput, WorkItem, Worker, schedule,
};

const INCIDENT_NAMESPACE: Uuid = Uuid::from_u128(0x7c4458d4_b4d7_43de_b682_1e28fd357188);
const PLAN_NAMESPACE: Uuid = Uuid::from_u128(0x25eb20ab_27a2_423a_8e0d_fa9ebf383301);

#[derive(Debug, Clone)]
pub struct ReconcileResult {
    pub incident: Option<ContinuityIncident>,
    pub coverage_plan: Option<CoveragePlan>,
}

pub fn reconcile(
    organization_id: Uuid,
    desired: &DesiredState,
    actual: &OperationalMetric,
    work_items: &[WorkItem],
    workers: &[Worker],
    policies: &[Policy],
) -> ReconcileResult {
    if desired.organization_id != organization_id
        || actual.organization_id != organization_id
        || desired.metric_key != actual.metric_key
        || !desired.enabled
        || desired.comparator.satisfied(actual.value, desired.target)
    {
        return ReconcileResult {
            incident: None,
            coverage_plan: None,
        };
    }

    let incident_fingerprint = format!(
        "{}:{}:{}:{:.6}:{:.6}:{}",
        organization_id,
        desired.id,
        actual.id,
        desired.target,
        actual.value,
        actual.observed_at.timestamp_millis()
    );
    let incident_id = Uuid::new_v5(&INCIDENT_NAMESPACE, incident_fingerprint.as_bytes());
    let plan_id = Uuid::new_v5(&PLAN_NAMESPACE, incident_id.as_bytes());

    let affected_work = work_items
        .iter()
        .filter(|item| item.organization_id == organization_id && item.status.is_schedulable())
        .count();

    let plan = schedule(ScheduleInput {
        organization_id,
        plan_id,
        incident_id,
        work_items,
        workers,
        policies,
        expected_metric_if_fully_covered: desired.target * 0.9,
        expected_metric_if_uncovered: actual.value,
    });

    let ratio = if desired.target.abs() < f64::EPSILON {
        f64::INFINITY
    } else {
        actual.value / desired.target
    };
    let severity = if ratio >= 3.0 {
        IncidentSeverity::Critical
    } else if ratio >= 2.0 {
        IncidentSeverity::High
    } else if ratio >= 1.25 {
        IncidentSeverity::Medium
    } else {
        IncidentSeverity::Low
    };

    let incident = ContinuityIncident {
        id: incident_id,
        organization_id,
        incident_type: format!(
            "{}_GAP",
            desired.metric_key.to_ascii_uppercase().replace('.', "_")
        ),
        desired_state_id: desired.id,
        metric_key: desired.metric_key.clone(),
        target: desired.target,
        actual: actual.value,
        projected: plan.expected_metric,
        missing_capacity: plan.missing_capacity,
        affected_work,
        severity,
        created_at: Utc::now(),
    };

    ReconcileResult {
        incident: Some(incident),
        coverage_plan: Some(plan),
    }
}
