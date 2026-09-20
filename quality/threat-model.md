# Levus M1 threat model

## Assets
Tenant work metadata, worker and capability data, policies, coverage decisions, execution and audit evidence, connector configuration, and future credentials.

## Trust boundaries
- HTTP client to Levus API.
- Levus API to deterministic runtime/domain kernel.
- Levus process to PostgreSQL when DATABASE_URL is configured.
- Future connector adapters to external systems.

## M1 controls
- Tenant-scoped API reads and mutations require an explicit organization identifier and organization-scoped runtime lookup.
- The scheduler filters work, workers and policies by organization before evaluation.
- PostgreSQL child relations use organization-coherent composite foreign keys.
- Demo endpoints are disabled unless explicitly enabled.
- Deterministic policy code, not a language model, is the scheduling security boundary.
- Events store concise evidence and do not collect private absence reasons or hidden model reasoning.
- Structured tracing excludes request bodies and credentials.

## Known M1 security limitations
- Organization headers provide tenancy scoping, not authentication. Identity-to-tenant binding is not implemented yet.
- Database row-level security and a non-owner application role are not wired yet.
- Connector credential storage and outbound-request controls are not implemented because connector execution is not in M1.
- CORS is permissive for development and must be restricted before any internet-facing deployment.

These limitations block production release and are explicit security gates for the next milestone.
