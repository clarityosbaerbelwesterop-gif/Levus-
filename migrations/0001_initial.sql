BEGIN;

CREATE TABLE organizations (
    id UUID PRIMARY KEY,
    name TEXT NOT NULL CHECK (length(trim(name)) > 0),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (id, name)
);

CREATE TABLE workers (
    id UUID PRIMARY KEY,
    organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    name TEXT NOT NULL CHECK (length(trim(name)) > 0),
    worker_type TEXT NOT NULL CHECK (worker_type IN ('HUMAN','AI_AGENT','AUTOMATION','EXTERNAL_WORKER')),
    status TEXT NOT NULL CHECK (status IN ('AVAILABLE','LIMITED','BUSY','UNAVAILABLE','DISABLED')),
    total_capacity BIGINT NOT NULL CHECK (total_capacity >= 0),
    available_capacity BIGINT NOT NULL CHECK (available_capacity >= 0),
    reserved_capacity BIGINT NOT NULL DEFAULT 0 CHECK (reserved_capacity >= 0),
    maximum_concurrency INTEGER NOT NULL CHECK (maximum_concurrency >= 0),
    cost_metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    risk_metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    authority JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (organization_id, id),
    CHECK (available_capacity <= total_capacity),
    CHECK (reserved_capacity <= available_capacity)
);

CREATE TABLE capabilities (
    id UUID PRIMARY KEY,
    organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    capability_key TEXT NOT NULL CHECK (capability_key ~ '^[a-z0-9]+([._-][a-z0-9]+)*$'),
    description TEXT NOT NULL DEFAULT '',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (organization_id, id),
    UNIQUE (organization_id, capability_key)
);

CREATE TABLE worker_capabilities (
    organization_id UUID NOT NULL,
    worker_id UUID NOT NULL,
    capability_id UUID NOT NULL,
    level SMALLINT NOT NULL CHECK (level BETWEEN 0 AND 255),
    PRIMARY KEY (organization_id, worker_id, capability_id),
    FOREIGN KEY (organization_id, worker_id) REFERENCES workers(organization_id, id) ON DELETE CASCADE,
    FOREIGN KEY (organization_id, capability_id) REFERENCES capabilities(organization_id, id) ON DELETE CASCADE
);

CREATE TABLE work_items (
    id UUID PRIMARY KEY,
    organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    title TEXT NOT NULL CHECK (length(trim(title)) > 0),
    description TEXT NOT NULL DEFAULT '',
    work_type TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('QUEUED','READY','ASSIGNED','RUNNING','BLOCKED','COMPLETED','FAILED','CANCELLED','ESCALATED')),
    priority TEXT NOT NULL CHECK (priority IN ('LOW','NORMAL','HIGH','URGENT')),
    risk TEXT NOT NULL CHECK (risk IN ('LOW','MEDIUM','HIGH','CRITICAL')),
    estimated_effort BIGINT NOT NULL CHECK (estimated_effort > 0),
    deadline TIMESTAMPTZ,
    sla_due_at TIMESTAMPTZ,
    source TEXT NOT NULL,
    external_reference TEXT,
    assigned_worker_id UUID,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (organization_id, id),
    FOREIGN KEY (organization_id, assigned_worker_id) REFERENCES workers(organization_id, id)
);

CREATE TABLE work_requirements (
    organization_id UUID NOT NULL,
    work_item_id UUID NOT NULL,
    capability_id UUID NOT NULL,
    minimum_level SMALLINT NOT NULL CHECK (minimum_level BETWEEN 0 AND 255),
    PRIMARY KEY (organization_id, work_item_id, capability_id),
    FOREIGN KEY (organization_id, work_item_id) REFERENCES work_items(organization_id, id) ON DELETE CASCADE,
    FOREIGN KEY (organization_id, capability_id) REFERENCES capabilities(organization_id, id) ON DELETE CASCADE
);

CREATE TABLE capacity_snapshots (
    id UUID PRIMARY KEY,
    organization_id UUID NOT NULL,
    worker_id UUID NOT NULL,
    total_capacity BIGINT NOT NULL CHECK (total_capacity >= 0),
    used_capacity BIGINT NOT NULL CHECK (used_capacity >= 0),
    reserved_capacity BIGINT NOT NULL CHECK (reserved_capacity >= 0),
    available_capacity BIGINT NOT NULL CHECK (available_capacity >= 0),
    maximum_concurrency INTEGER NOT NULL CHECK (maximum_concurrency >= 0),
    observed_at TIMESTAMPTZ NOT NULL,
    UNIQUE (organization_id, id),
    FOREIGN KEY (organization_id, worker_id) REFERENCES workers(organization_id, id) ON DELETE CASCADE
);

CREATE TABLE desired_states (
    id UUID PRIMARY KEY,
    organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    metric_key TEXT NOT NULL,
    comparator TEXT NOT NULL CHECK (comparator IN ('LESS_THAN','LESS_THAN_OR_EQUAL','GREATER_THAN','GREATER_THAN_OR_EQUAL')),
    target DOUBLE PRECISION NOT NULL,
    unit TEXT NOT NULL,
    enabled BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (organization_id, id)
);

CREATE TABLE operational_metrics (
    id UUID PRIMARY KEY,
    organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    metric_key TEXT NOT NULL,
    value DOUBLE PRECISION NOT NULL,
    unit TEXT NOT NULL,
    observed_at TIMESTAMPTZ NOT NULL,
    UNIQUE (organization_id, id)
);

CREATE TABLE policies (
    id UUID PRIMARY KEY,
    organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    priority INTEGER NOT NULL DEFAULT 0,
    worker_type TEXT CHECK (worker_type IS NULL OR worker_type IN ('HUMAN','AI_AGENT','AUTOMATION','EXTERNAL_WORKER')),
    work_type_prefix TEXT,
    capability_prefix TEXT,
    minimum_risk TEXT CHECK (minimum_risk IS NULL OR minimum_risk IN ('LOW','MEDIUM','HIGH','CRITICAL')),
    decision TEXT NOT NULL CHECK (decision IN ('ALLOW','DENY','ESCALATE')),
    reason TEXT NOT NULL,
    enabled BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (organization_id, id)
);

CREATE TABLE continuity_incidents (
    id UUID PRIMARY KEY,
    organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    incident_type TEXT NOT NULL,
    desired_state_id UUID NOT NULL,
    metric_key TEXT NOT NULL,
    target DOUBLE PRECISION NOT NULL,
    actual DOUBLE PRECISION NOT NULL,
    projected DOUBLE PRECISION NOT NULL,
    missing_capacity BIGINT NOT NULL CHECK (missing_capacity >= 0),
    affected_work INTEGER NOT NULL CHECK (affected_work >= 0),
    severity TEXT NOT NULL CHECK (severity IN ('LOW','MEDIUM','HIGH','CRITICAL')),
    status TEXT NOT NULL DEFAULT 'OPEN' CHECK (status IN ('OPEN','MITIGATING','RESOLVED','FAILED')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    resolved_at TIMESTAMPTZ,
    UNIQUE (organization_id, id),
    FOREIGN KEY (organization_id, desired_state_id) REFERENCES desired_states(organization_id, id)
);

CREATE TABLE coverage_plans (
    id UUID PRIMARY KEY,
    organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    incident_id UUID NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('DRAFT','VALIDATED','EXECUTING','ACTIVE','COMPLETED','PARTIALLY_COMPLETED','FAILED')),
    estimated_cost_micros BIGINT NOT NULL DEFAULT 0 CHECK (estimated_cost_micros >= 0),
    covered_capacity BIGINT NOT NULL DEFAULT 0 CHECK (covered_capacity >= 0),
    missing_capacity BIGINT NOT NULL DEFAULT 0 CHECK (missing_capacity >= 0),
    expected_metric DOUBLE PRECISION NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (organization_id, id),
    FOREIGN KEY (organization_id, incident_id) REFERENCES continuity_incidents(organization_id, id)
);

CREATE TABLE coverage_actions (
    id UUID PRIMARY KEY,
    organization_id UUID NOT NULL,
    coverage_plan_id UUID NOT NULL,
    work_item_id UUID NOT NULL,
    worker_id UUID,
    action_type TEXT NOT NULL,
    policy_decision TEXT NOT NULL CHECK (policy_decision IN ('ALLOW','DENY','ESCALATE')),
    reason TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (organization_id, id),
    FOREIGN KEY (organization_id, coverage_plan_id) REFERENCES coverage_plans(organization_id, id) ON DELETE CASCADE,
    FOREIGN KEY (organization_id, work_item_id) REFERENCES work_items(organization_id, id),
    FOREIGN KEY (organization_id, worker_id) REFERENCES workers(organization_id, id)
);

CREATE TABLE assignments (
    id UUID PRIMARY KEY,
    organization_id UUID NOT NULL,
    coverage_plan_id UUID NOT NULL,
    work_item_id UUID NOT NULL,
    worker_id UUID NOT NULL,
    capacity_units BIGINT NOT NULL CHECK (capacity_units > 0),
    status TEXT NOT NULL DEFAULT 'ASSIGNED' CHECK (status IN ('ASSIGNED','RUNNING','COMPLETED','FAILED','CANCELLED')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (organization_id, id),
    UNIQUE (organization_id, coverage_plan_id, work_item_id),
    FOREIGN KEY (organization_id, coverage_plan_id) REFERENCES coverage_plans(organization_id, id) ON DELETE CASCADE,
    FOREIGN KEY (organization_id, work_item_id) REFERENCES work_items(organization_id, id),
    FOREIGN KEY (organization_id, worker_id) REFERENCES workers(organization_id, id)
);

CREATE TABLE executions (
    id UUID PRIMARY KEY,
    organization_id UUID NOT NULL,
    assignment_id UUID NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('PENDING','RUNNING','SUCCEEDED','FAILED','CANCELLED')),
    attempt INTEGER NOT NULL DEFAULT 1 CHECK (attempt > 0),
    idempotency_key TEXT NOT NULL,
    started_at TIMESTAMPTZ,
    finished_at TIMESTAMPTZ,
    error_code TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (organization_id, id),
    UNIQUE (organization_id, idempotency_key),
    FOREIGN KEY (organization_id, assignment_id) REFERENCES assignments(organization_id, id) ON DELETE CASCADE
);

CREATE TABLE outcomes (
    id UUID PRIMARY KEY,
    organization_id UUID NOT NULL,
    execution_id UUID NOT NULL,
    outcome_type TEXT NOT NULL,
    summary TEXT NOT NULL,
    evidence JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (organization_id, id),
    FOREIGN KEY (organization_id, execution_id) REFERENCES executions(organization_id, id) ON DELETE CASCADE
);

CREATE TABLE events (
    id UUID PRIMARY KEY,
    organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    event_type TEXT NOT NULL,
    actor TEXT NOT NULL,
    resource TEXT NOT NULL,
    occurred_at TIMESTAMPTZ NOT NULL,
    correlation_id UUID NOT NULL,
    causation_id UUID,
    schema_version INTEGER NOT NULL CHECK (schema_version > 0),
    payload JSONB NOT NULL,
    idempotency_key TEXT,
    UNIQUE (organization_id, id),
    UNIQUE (organization_id, idempotency_key)
);

CREATE TABLE audit_events (
    id UUID PRIMARY KEY,
    organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    actor TEXT NOT NULL,
    action TEXT NOT NULL,
    resource TEXT NOT NULL,
    decision_reason TEXT NOT NULL,
    input_evidence JSONB NOT NULL DEFAULT '{}'::jsonb,
    policy_result JSONB NOT NULL DEFAULT '{}'::jsonb,
    scheduler_result JSONB NOT NULL DEFAULT '{}'::jsonb,
    execution_evidence JSONB NOT NULL DEFAULT '{}'::jsonb,
    outcome_evidence JSONB NOT NULL DEFAULT '{}'::jsonb,
    occurred_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    correlation_id UUID NOT NULL,
    UNIQUE (organization_id, id)
);

CREATE TABLE connectors (
    id UUID PRIMARY KEY,
    organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    connector_type TEXT NOT NULL,
    display_name TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('CONNECTED','DEGRADED','DISCONNECTED','DISABLED')),
    configuration JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (organization_id, id)
);

CREATE INDEX idx_workers_org_status ON workers (organization_id, status);
CREATE INDEX idx_work_items_org_status_priority ON work_items (organization_id, status, priority);
CREATE INDEX idx_work_items_org_deadline ON work_items (organization_id, deadline) WHERE deadline IS NOT NULL;
CREATE INDEX idx_metrics_org_key_observed ON operational_metrics (organization_id, metric_key, observed_at DESC);
CREATE INDEX idx_incidents_org_status ON continuity_incidents (organization_id, status, created_at DESC);
CREATE INDEX idx_plans_org_incident ON coverage_plans (organization_id, incident_id, created_at DESC);
CREATE INDEX idx_assignments_org_worker ON assignments (organization_id, worker_id, status);
CREATE INDEX idx_events_org_type_time ON events (organization_id, event_type, occurred_at DESC);
CREATE INDEX idx_audit_org_time ON audit_events (organization_id, occurred_at DESC);

COMMIT;
