use super::{Policy, PolicyDecision, PolicyEvidence, Worker, WorkItem};

pub fn evaluate_policy(
    worker: &Worker,
    work: &WorkItem,
    policies: &[Policy],
) -> PolicyEvidence {
    let mut matching: Vec<&Policy> = policies
        .iter()
        .filter(|policy| policy.enabled && policy.organization_id == work.organization_id)
        .filter(|policy| {
            policy.worker_type.is_none_or(|kind| kind == worker.worker_type)
                && policy
                    .work_type_prefix
                    .as_ref()
                    .is_none_or(|prefix| work.work_type.starts_with(prefix))
                && policy.capability_prefix.as_ref().is_none_or(|prefix| {
                    work.required_capabilities
                        .iter()
                        .any(|requirement| requirement.capability.starts_with(prefix))
                })
                && policy
                    .minimum_risk
                    .is_none_or(|minimum| work.risk >= minimum)
        })
        .collect();

    matching.sort_by(|left, right| {
        right
            .priority
            .cmp(&left.priority)
            .then_with(|| right.decision.cmp(&left.decision))
            .then_with(|| left.id.cmp(&right.id))
    });

    if let Some(policy) = matching.first() {
        return PolicyEvidence {
            work_item_id: work.id,
            worker_id: worker.id,
            decision: policy.decision,
            policy_id: Some(policy.id),
            reason: policy.reason.clone(),
        };
    }

    PolicyEvidence {
        work_item_id: work.id,
        worker_id: worker.id,
        decision: PolicyDecision::Allow,
        policy_id: None,
        reason: "No restrictive policy matched; action is within delegated authority.".to_owned(),
    }
}
