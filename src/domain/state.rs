use std::collections::{BTreeMap, BTreeSet};

use chrono::{Duration, TimeZone, Utc};
use serde::Serialize;
use uuid::Uuid;

use super::{
    reconcile, CapabilityLevel, ContinuityIncident, CoveragePlan, DesiredState, Event,
    MetricComparator, OperationalMetric, Policy, PolicyDecision, Priority, Risk, Worker,
    WorkerStatus, WorkerType, WorkItem, WorkStatus,
};

pub const DEMO_ORGANIZATION_ID: Uuid =
    Uuid::from_u128(0x00000000_0000_0000_0000_000000000001);
const DEMO_TIME_SECONDS: i64 = 1_800_000_000;

#[derive(Debug, Clone, Serialize)]
pub struct RuntimeStatus {
    pub organization_id: Uuid,
    pub continuity_status: String,
    pub open_work: usize,
    pub available_capacity: u32,
    pub capacity_deficit: u32,
    pub active_incidents: usize,
    pub active_coverage_plans: usize,
    pub human_capacity: u32,
    pub ai_capacity: u32,
    pub coverage_percentage: f64,
}

#[derive(Debug, Clone)]
pub struct OrganizationRuntime {
    pub organization_id: Uuid,
    pub workers: Vec<Worker>,
    pub work_items: Vec<WorkItem>,
    pub desired_states: Vec<DesiredState>,
    pub metrics: Vec<OperationalMetric>,
    pub policies: Vec<Policy>,
    pub incidents: Vec<ContinuityIncident>,
    pub coverage_plans: Vec<CoveragePlan>,
    pub events: Vec<Event>,
}

#[derive(Debug, Default)]
pub struct RuntimeStore {
    organizations: BTreeMap<Uuid, OrganizationRuntime>,
}

impl RuntimeStore {
    pub fn reset_demo(&mut self) -> Uuid {
        let runtime = demo_runtime();
        let id = runtime.organization_id;
        self.organizations.insert(id, runtime);
        id
    }

    pub fn ensure_demo(&mut self) {
        if !self.organizations.contains_key(&DEMO_ORGANIZATION_ID) {
            self.reset_demo();
        }
    }

    pub fn organization(&self, id: Uuid) -> Option<&OrganizationRuntime> {
        self.organizations.get(&id)
    }

    pub fn organization_mut(&mut self, id: Uuid) -> Option<&mut OrganizationRuntime> {
        self.organizations.get_mut(&id)
    }
}

impl OrganizationRuntime {
    pub fn status(&self) -> RuntimeStatus {
        let open_work: Vec<_> = self
            .work_items
            .iter()
            .filter(|item| item.status.is_schedulable())
            .collect();
        let demand: u32 = open_work.iter().map(|item| item.estimated_effort).sum();
        let available_capacity: u32 = self
            .workers
            .iter()
            .filter(|worker| worker.status.can_accept_work())
            .map(Worker::effective_available_capacity)
            .sum();
        let human_capacity: u32 = self
            .workers
            .iter()
            .filter(|worker| {
                worker.worker_type == WorkerType::Human && worker.status.can_accept_work()
            })
            .map(Worker::effective_available_capacity)
            .sum();
        let ai_capacity: u32 = self
            .workers
            .iter()
            .filter(|worker| {
                worker.worker_type == WorkerType::AiAgent && worker.status.can_accept_work()
            })
            .map(Worker::effective_available_capacity)
            .sum();
        let capacity_deficit = demand.saturating_sub(available_capacity);
        let coverage_percentage = if demand == 0 {
            100.0
        } else {
            f64::from(demand.saturating_sub(capacity_deficit)) / f64::from(demand) * 100.0
        };
        let continuity_status = if self
            .incidents
            .iter()
            .any(|incident| matches!(incident.severity, super::IncidentSeverity::Critical))
        {
            "CRITICAL"
        } else if self.incidents.is_empty() {
            "HEALTHY"
        } else {
            "DEGRADED"
        };

        RuntimeStatus {
            organization_id: self.organization_id,
            continuity_status: continuity_status.to_owned(),
            open_work: open_work.len(),
            available_capacity,
            capacity_deficit,
            active_incidents: self.incidents.len(),
            active_coverage_plans: self.coverage_plans.len(),
            human_capacity,
            ai_capacity,
            coverage_percentage,
        }
    }

    pub fn disrupt_demo(&mut self) {
        let unavailable = Uuid::from_u128(0x10000000_0000_0000_0000_000000000001);
        if let Some(worker) = self.workers.iter_mut().find(|worker| worker.id == unavailable) {
            worker.status = WorkerStatus::Unavailable;
            worker.available_capacity = 0;
        }

        let base = demo_time();
        let existing: BTreeSet<_> = self.work_items.iter().map(|item| item.id).collect();
        for index in 0..60_u128 {
            let id = Uuid::from_u128(0x30000000_0000_0000_0000_000000000001 + index);
            if existing.contains(&id) {
                continue;
            }
            let risk = if index % 20 == 0 {
                Risk::Critical
            } else if index % 5 == 0 {
                Risk::High
            } else {
                Risk::Medium
            };
            self.work_items.push(WorkItem {
                id,
                organization_id: self.organization_id,
                title: format!("Support ticket #{}", index + 1),
                description: "Deterministic support workload generated by the Levus demo.".into(),
                work_type: "support.ticket.respond".into(),
                status: WorkStatus::Ready,
                priority: if risk >= Risk::High {
                    Priority::High
                } else {
                    Priority::Normal
                },
                risk,
                required_capabilities: vec![CapabilityLevel {
                    capability: "support.ticket.respond".into(),
                    level: if risk >= Risk::High { 3 } else { 2 },
                }],
                estimated_effort: 1,
                deadline: Some(base + Duration::minutes(30)),
                sla_due_at: Some(base + Duration::minutes(5)),
                source: "demo.support".into(),
                external_reference: Some(format!("DEMO-{}", index + 1)),
                assigned_worker_id: None,
                created_at: base,
                updated_at: base,
            });
        }

        let disrupted_metric_id =
            Uuid::from_u128(0x50000000_0000_0000_0000_000000000002);
        if !self.metrics.iter().any(|metric| metric.id == disrupted_metric_id) {
            self.metrics.push(OperationalMetric {
                id: disrupted_metric_id,
                organization_id: self.organization_id,
                metric_key: "support.average_response_time".into(),
                value: 14.0,
                unit: "minutes".into(),
                observed_at: base + Duration::minutes(1),
            });
        }

        if !self.events.iter().any(|event| event.event_type == "worker.unavailable") {
            self.events.push(Event {
                id: Uuid::new_v4(),
                organization_id: self.organization_id,
                event_type: "worker.unavailable".into(),
                actor: "demo.scenario".into(),
                resource: unavailable.to_string(),
                timestamp: Utc::now(),
                correlation_id: Uuid::new_v4(),
                causation_id: None,
                schema_version: 1,
                payload: serde_json::json!({"reason": "private_reason_not_collected"}),
            });
        }
    }

    pub fn reconcile_latest(&mut self) -> bool {
        let Some(desired) = self.desired_states.first().cloned() else {
            return false;
        };
        let Some(metric) = self
            .metrics
            .iter()
            .filter(|metric| metric.metric_key == desired.metric_key)
            .max_by_key(|metric| metric.observed_at)
            .cloned()
        else {
            return false;
        };

        let result = reconcile(
            self.organization_id,
            &desired,
            &metric,
            &self.work_items,
            &self.workers,
            &self.policies,
        );

        if let Some(incident) = result.incident {
            if !self.incidents.iter().any(|existing| existing.id == incident.id) {
                self.events.push(Event {
                    id: Uuid::new_v4(),
                    organization_id: self.organization_id,
                    event_type: "continuity.incident.created".into(),
                    actor: "levus.reconciler".into(),
                    resource: incident.id.to_string(),
                    timestamp: Utc::now(),
                    correlation_id: incident.id,
                    causation_id: None,
                    schema_version: 1,
                    payload: serde_json::json!({
                        "metric_key": incident.metric_key,
                        "target": incident.target,
                        "actual": incident.actual,
                        "missing_capacity": incident.missing_capacity,
                    }),
                });
                self.incidents.push(incident);
            }
        }

        if let Some(plan) = result.coverage_plan {
            if !self.coverage_plans.iter().any(|existing| existing.id == plan.id) {
                self.events.push(Event {
                    id: Uuid::new_v4(),
                    organization_id: self.organization_id,
                    event_type: "coverage.plan.created".into(),
                    actor: "levus.scheduler".into(),
                    resource: plan.id.to_string(),
                    timestamp: Utc::now(),
                    correlation_id: plan.incident_id,
                    causation_id: Some(plan.incident_id),
                    schema_version: 1,
                    payload: serde_json::json!({
                        "assignments": plan.assignments.len(),
                        "uncovered_work": plan.uncovered_work.len(),
                        "escalations": plan.escalations.len(),
                    }),
                });
                self.coverage_plans.push(plan);
            }
        }

        true
    }
}

fn demo_runtime() -> OrganizationRuntime {
    let organization_id = DEMO_ORGANIZATION_ID;
    let base = demo_time();
    let human_caps = BTreeMap::from([
        ("support.ticket.read".into(), 3),
        ("support.ticket.respond".into(), 3),
        ("support.ticket.close".into(), 3),
    ]);
    let ai_caps = BTreeMap::from([
        ("support.ticket.read".into(), 3),
        ("support.ticket.respond".into(), 2),
        ("support.ticket.close".into(), 2),
    ]);

    let workers = vec![
        worker(1, "Alice", WorkerType::Human, human_caps.clone(), 10, 90_000),
        worker(2, "Bob", WorkerType::Human, human_caps.clone(), 10, 90_000),
        worker(3, "Cara", WorkerType::Human, human_caps, 10, 95_000),
        worker(101, "AI Support A", WorkerType::AiAgent, ai_caps.clone(), 20, 8_000),
        worker(102, "AI Support B", WorkerType::AiAgent, ai_caps.clone(), 20, 8_000),
        worker(103, "AI Support C", WorkerType::AiAgent, ai_caps, 20, 10_000),
    ];

    OrganizationRuntime {
        organization_id,
        workers,
        work_items: Vec::new(),
        desired_states: vec![DesiredState {
            id: Uuid::from_u128(0x40000000_0000_0000_0000_000000000001),
            organization_id,
            metric_key: "support.average_response_time".into(),
            comparator: MetricComparator::LessThan,
            target: 5.0,
            unit: "minutes".into(),
            enabled: true,
        }],
        metrics: vec![OperationalMetric {
            id: Uuid::from_u128(0x50000000_0000_0000_0000_000000000001),
            organization_id,
            metric_key: "support.average_response_time".into(),
            value: 4.0,
            unit: "minutes".into(),
            observed_at: base,
        }],
        policies: vec![
            Policy {
                id: Uuid::from_u128(0x60000000_0000_0000_0000_000000000001),
                organization_id,
                name: "Critical support cases require escalation".into(),
                priority: 100,
                worker_type: None,
                work_type_prefix: Some("support.ticket".into()),
                capability_prefix: None,
                minimum_risk: Some(Risk::Critical),
                decision: PolicyDecision::Escalate,
                reason: "Critical support cases exceed delegated autonomous authority.".into(),
                enabled: true,
            },
            Policy {
                id: Uuid::from_u128(0x60000000_0000_0000_0000_000000000002),
                organization_id,
                name: "AI support risk boundary".into(),
                priority: 50,
                worker_type: Some(WorkerType::AiAgent),
                work_type_prefix: Some("support.ticket".into()),
                capability_prefix: None,
                minimum_risk: Some(Risk::High),
                decision: PolicyDecision::Deny,
                reason: "High-risk support work is reserved for qualified human authority.".into(),
                enabled: true,
            },
        ],
        incidents: Vec::new(),
        coverage_plans: Vec::new(),
        events: Vec::new(),
    }
}

fn worker(
    id: u128,
    name: &str,
    worker_type: WorkerType,
    capabilities: BTreeMap<String, u8>,
    capacity: u32,
    cost: u64,
) -> Worker {
    Worker {
        id: Uuid::from_u128(0x10000000_0000_0000_0000_000000000000 + id),
        organization_id: DEMO_ORGANIZATION_ID,
        name: name.into(),
        worker_type,
        status: WorkerStatus::Available,
        capabilities,
        total_capacity: capacity,
        available_capacity: capacity,
        reserved_capacity: 0,
        maximum_concurrency: capacity.min(u32::from(u16::MAX)) as u16,
        active_assignments: 0,
        cost_per_capacity_unit_micros: cost,
        authority: BTreeMap::new(),
    }
}

fn demo_time() -> chrono::DateTime<Utc> {
    Utc.timestamp_opt(DEMO_TIME_SECONDS, 0)
        .single()
        .expect("constant demo timestamp must be valid")
}
