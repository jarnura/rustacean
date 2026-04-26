# Business Context

## What problem does rust-brain solve?

Software teams spend a surprising fraction of their time navigating codebases they don't fully understand: finding where a function is defined, tracing why a value changes, or understanding how a service boundary evolved over the past six months.

Existing code-hosting platforms (GitHub, GitLab, Gitea) store source code well but provide shallow search: keyword grep or, at best, file-path fuzzy matching. AI coding assistants add generation capability but typically lack deep, structured knowledge of a specific codebase's history and architecture.

**rust-brain** fills the gap between storage and intelligence. It hosts Git repositories and simultaneously builds a structured, queryable knowledge graph of each codebase — definitions, call sites, type relationships, dependency edges, and historical changes. That graph powers semantic code search, impact analysis, onboarding guides, and AI features that can answer questions like:

- "Which services call `UserService.createAccount`?"
- "What changed in the payment module in the last sprint?"
- "Is this function safe to remove?"

---

## Target users

**Phase 1 (current)**: The primary user is a small, technically sophisticated team (2–10 engineers) that self-hosts and wants fine-grained control over their code-intelligence tooling. They are comfortable configuring Docker Compose and prefer open, inspectable systems over black-box SaaS.

**Phase 2+**: Multi-team organisations running internal developer platforms. Each team gets an isolated tenant workspace; a platform team manages user provisioning and billing.

---

## Product vision

rust-brain is a self-hostable, multi-tenant developer platform with three layers:

```
┌──────────────────────────────────────────┐
│         Developer Experience             │
│  Code search · Impact analysis · Docs   │
│  AI chat · Onboarding guides             │
└──────────────────────────────────────────┘
┌──────────────────────────────────────────┐
│          Knowledge Graph                 │
│  Definitions · Call graph · Types        │
│  Dependency edges · Blame history        │
└──────────────────────────────────────────┘
┌──────────────────────────────────────────┐
│          Repository Hosting              │
│  Git push/pull · Access control          │
│  Webhooks · CI triggers                  │
└──────────────────────────────────────────┘
```

Every layer serves the one above it. The knowledge graph is useless without the repositories it indexes. The developer-experience features are only as good as the graph beneath them.

---

## Phase roadmap

### Phase 0 — Foundation (complete)

Core infrastructure that every subsequent feature builds on.

| What | Status |
|------|--------|
| Rust monorepo workspace setup | Done |
| PostgreSQL (multi-tenant, schema-per-tenant) | Done |
| Kafka (KRaft, no ZooKeeper) | Done |
| OpenTelemetry + Prometheus + Grafana | Done |
| Docker Compose dev stack | Done |
| CI pipeline (GitHub Actions) | Done |
| Tailscale remote deployment on mars | Done |

### Phase 1 — Auth & control plane (complete)

Everything needed to safely identify users, manage tenants, and control API access.

| What | Status |
|------|--------|
| Signup with email verification | Done |
| Login with argon2id, rate limiting | Done |
| Sliding-window sessions (HttpOnly cookie) | Done |
| Password reset (token, 15-min expiry) | Done |
| GET /v1/me with session refresh | Done |
| Multi-tenant switching | Done |
| API keys (scopes: read / write / admin) | Done |
| Tenant member management (invite, role, remove, transfer) | Done |
| React frontend shell (Tailwind + shadcn/ui + TanStack Router) | Done |
| Auth pages (signup, login, verify, forgot, reset) | Done |
| OpenAPI-typed frontend client (openapi-typescript) | Done |

### Phase 2 — Repository hosting (planned)

Git hosting on top of the authenticated multi-tenant base.

- Gitea or bare-git repository management per tenant
- SSH key management
- Repository CRUD and listing API
- Webhook support for push events

### Phase 3 — Ingest pipeline (planned)

Streaming pipeline to process pushed code and build the knowledge graph.

- Kafka topics: clone → expand → parse → typecheck → graph → embed
- Language-specific parsers (tree-sitter)
- PostgreSQL + Neo4j persistence for AST / call graph
- Qdrant for embedding storage (semantic search)
- Ollama for local embedding generation

### Phase 4 — Developer experience (planned)

User-facing features powered by the knowledge graph.

- Semantic code search
- Call-graph traversal and impact analysis
- AI chat grounded in the codebase graph
- Onboarding guides auto-generated from commit history

---

## Design decisions

### Why Rust?

Performance, correctness guarantees, and a rich async ecosystem (tokio, sqlx, axum). The indexing pipeline will process large codebases; allocation pressure and GC pauses matter. Rust also makes the codebase an honest advertisement for what the platform can analyse.

### Why schema-per-tenant instead of row-level multi-tenancy?

Schema-per-tenant (each tenant gets `tenant_<uuid>` schema) provides strong isolation without the operational overhead of separate databases. PostgreSQL's schema system allows per-tenant objects (tables, views, indexes) without cross-tenant data leakage risks that plague row-level filtering. Migration complexity is higher, but isolation correctness is easier to reason about.

### Why OpenAPI-first?

A single `openapi.json` generated from Rust handler annotations is the contract between backend and frontend. The frontend generates TypeScript types from it; CI verifies both sides of the sync. This eliminates an entire class of runtime errors caused by API drift.

### Why Kafka instead of direct database writes for the ingest pipeline?

The ingest pipeline involves CPU-intensive work (parsing, type-checking, embedding) that must not block the API. Kafka provides durable buffering, independent scaling of each pipeline stage, and replay capability when a stage fails. The `rb.projector.events` topic (90-day retention) also serves as an audit log and event-sourcing base for future read models.

### Why self-hosted instead of cloud-only?

Many engineering teams, particularly those working on proprietary or regulated codebases, cannot send source code to a third-party API. Self-hosting means the knowledge graph stays inside the organisation's network. Cloud-hosted SaaS is a future option for teams that prefer managed infrastructure.
