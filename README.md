# Levus

**Autonomous Workforce Continuity Infrastructure**

Kubernetes orchestrates compute. Levus orchestrates work.

Levus is an enterprise control plane that continuously compares desired business state with actual operating state, detects continuity gaps, calculates capacity deficits, evaluates deterministic policy, and produces auditable coverage plans across human and machine workers.

## M1: deterministic continuity runtime

The first vertical slice contains:

- typed human / AI / automation / external-worker scheduling model;
- typed capability requirements and deterministic eligibility;
- explicit work states and transition rules;
- capacity and concurrency enforcement;
- deterministic policy evaluation (`ALLOW`, `DENY`, `ESCALATE`);
- deterministic coverage-plan generation with stable assignment IDs;
- desired-state reconciliation and continuity-incident creation;
- event evidence with correlation/causation fields;
- multi-tenant runtime scoping and negative invariant tests;
- normalized PostgreSQL schema and clean-migration verification;
- versioned Axum API and deterministic end-to-end demo scenario;
- machine-readable `quality/feature-map.json`.

M1 intentionally does **not** claim production readiness. Authentication/identity binding, database-backed repositories, PostgreSQL RLS, connector execution, and the web control plane are subsequent gates.

## Local run

Requirements: Rust 1.85+, and optionally PostgreSQL 17. Environment variables are read from the process environment; M1 does not auto-load `.env` files.

```bash
cp .env.example .env
export LEVUS_ENABLE_DEMO=true
cargo run
```

The default API binds to `127.0.0.1:8080`.

```bash
curl http://127.0.0.1:8080/health
curl -X POST http://127.0.0.1:8080/api/v1/demo/reset
curl -X POST http://127.0.0.1:8080/api/v1/demo/disrupt
curl -X POST \
  -H 'x-organization-id: 00000000-0000-0000-0000-000000000001' \
  http://127.0.0.1:8080/api/v1/reconcile
```

The demo is deterministic at the scheduling layer: one human worker becomes unavailable, 60 support work items arrive, critical cases cross an escalation boundary, high-risk work is denied to AI workers, and compatible capacity is assigned automatically.

## Database

```bash
docker compose up -d postgres
export DATABASE_URL=postgres://levus:levus@127.0.0.1:5432/levus
cargo run
```

When `DATABASE_URL` is present, SQLx applies the embedded migrations before the API starts. M1 domain state is still in-memory; the relational schema is validated in CI and becomes the persistence authority in M2.

## Quality gates

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo build --release --all-features
```

GitHub Actions additionally validates a clean PostgreSQL migration, a cross-tenant database constraint, and the real HTTP continuity demo.

## Privacy boundary

Levus schedules against availability and capacity, not private absence reasons. The demo deliberately emits `private_reason_not_collected`; the runtime does not infer medical conditions, religion, political views, sexual orientation, race, or hidden human productivity scores.
