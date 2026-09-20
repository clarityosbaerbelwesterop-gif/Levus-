use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Organization {
    pub id: Uuid,
    pub name: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum WorkerType {
    Human,
    AiAgent,
    Automation,
    ExternalWorker,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum WorkerStatus {
    Available,
    Limited,
    Busy,
    Unavailable,
    Disabled,
}

impl WorkerStatus {
    pub fn can_accept_work(self) -> bool {
        matches!(self, Self::Available | Self::Limited | Self::Busy)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CapabilityLevel {
    pub capability: String,
    pub level: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Worker {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub name: String,
    pub worker_type: WorkerType,
    pub status: WorkerStatus,
    pub capabilities: BTreeMap<String, u8>,
    pub total_capacity: u32,
    pub available_capacity: u32,
    pub reserved_capacity: u32,
    pub maximum_concurrency: u16,
    pub active_assignments: u16,
    pub cost_per_capacity_unit_micros: u64,
    pub authority: BTreeMap<String, u8>,
}

impl Worker {
    pub fn effective_available_capacity(&self) -> u32 {
        self.available_capacity.saturating_sub(self.reserved_capacity)
    }

    pub fn has_capabilities(&self, requirements: &[CapabilityLevel]) -> bool {
        requirements.iter().all(|requirement| {
            self.capabilities
                .get(&requirement.capability)
                .is_some_and(|level| *level >= requirement.level)
        })
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Priority {
    Low,
    Normal,
    High,
    Urgent,
}

impl Priority {
    pub fn rank(self) -> u8 {
        match self {
            Self::Low => 0,
            Self::Normal => 1,
            Self::High => 2,
            Self::Urgent => 3,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Risk {
    Low,
    Medium,
    High,
    Critical,
}

impl Risk {
    pub fn rank(self) -> u8 {
        match self {
            Self::Low => 0,
            Self::Medium => 1,
            Self::High => 2,
            Self::Critical => 3,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum WorkStatus {
    Queued,
    Ready,
    Assigned,
    Running,
    Blocked,
    Completed,
    Failed,
    Cancelled,
    Escalated,
}

impl WorkStatus {
    pub fn is_schedulable(self) -> bool {
        matches!(self, Self::Queued | Self::Ready | Self::Assigned)
    }

    pub fn can_transition_to(self, next: Self) -> bool {
        use WorkStatus::*;
        matches!(
            (self, next),
            (Queued, Ready | Cancelled | Escalated)
                | (Ready, Assigned | Blocked | Cancelled | Escalated)
                | (Assigned, Running | Ready | Cancelled | Escalated)
                | (Running, Completed | Failed | Blocked | Escalated)
                | (Blocked, Ready | Cancelled | Escalated)
                | (Failed, Ready | Cancelled | Escalated)
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorkItem {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub title: String,
    pub description: String,
    pub work_type: String,
    pub status: WorkStatus,
    pub priority: Priority,
    pub risk: Risk,
    pub required_capabilities: Vec<CapabilityLevel>,
    pub estimated_effort: u32,
    pub deadline: Option<DateTime<Utc>>,
    pub sla_due_at: Option<DateTime<Utc>>,
    pub source: String,
    pub external_reference: Option<String>,
    pub assigned_worker_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MetricComparator {
    LessThan,
    LessThanOrEqual,
    GreaterThan,
    GreaterThanOrEqual,
}

impl MetricComparator {
    pub fn satisfied(self, actual: f64, target: f64) -> bool {
        match self {
            Self::LessThan => actual < target,
            Self::LessThanOrEqual => actual <= target,
            Self::GreaterThan => actual > target,
            Self::GreaterThanOrEqual => actual >= target,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DesiredState {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub metric_key: String,
    pub comparator: MetricComparator,
    pub target: f64,
    pub unit: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OperationalMetric {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub metric_key: String,
    pub value: f64,
    pub unit: String,
    pub observed_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum IncidentSeverity {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ContinuityIncident {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub incident_type: String,
    pub desired_state_id: Uuid,
    pub metric_key: String,
    pub target: f64,
    pub actual: f64,
    pub projected: f64,
    pub missing_capacity: u32,
    pub affected_work: usize,
    pub severity: IncidentSeverity,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CoveragePlanStatus {
    Draft,
    Validated,
    Executing,
    Active,
    Completed,
    PartiallyCompleted,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Assignment {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub coverage_plan_id: Uuid,
    pub work_item_id: Uuid,
    pub worker_id: Uuid,
    pub capacity_units: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PolicyEvidence {
    pub work_item_id: Uuid,
    pub worker_id: Uuid,
    pub decision: PolicyDecision,
    pub policy_id: Option<Uuid>,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CoveragePlan {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub incident_id: Uuid,
    pub status: CoveragePlanStatus,
    pub assignments: Vec<Assignment>,
    pub policy_decisions: Vec<PolicyEvidence>,
    pub uncovered_work: Vec<Uuid>,
    pub escalations: Vec<Uuid>,
    pub estimated_cost_micros: u64,
    pub covered_capacity: u32,
    pub missing_capacity: u32,
    pub expected_metric: f64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PolicyDecision {
    Allow,
    Escalate,
    Deny,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Policy {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub name: String,
    pub priority: i32,
    pub worker_type: Option<WorkerType>,
    pub work_type_prefix: Option<String>,
    pub capability_prefix: Option<String>,
    pub minimum_risk: Option<Risk>,
    pub decision: PolicyDecision,
    pub reason: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Event {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub event_type: String,
    pub actor: String,
    pub resource: String,
    pub timestamp: DateTime<Utc>,
    pub correlation_id: Uuid,
    pub causation_id: Option<Uuid>,
    pub schema_version: u16,
    pub payload: serde_json::Value,
}
