# Rustacean Frontend

React 18 + Vite + TypeScript + Tailwind CSS + shadcn/ui control-plane UI for
Rustacean. Routing uses TanStack Router; data fetching uses TanStack Query.

## REQ-FE-01 — Application shell

- **Routing**: [TanStack Router](https://tanstack.com/router) with a code-based
  route tree in `src/router.tsx` and a single `__root` route that renders the
  global `<AppShell>` layout via `<Outlet>`.
- **Layout**: `src/components/AppShell.tsx` provides the header (brand,
  theme toggle), main content area, and footer.
- **Theming**: light / dark / system. `ThemeProvider`
  (`src/components/theme/ThemeProvider.tsx`) toggles the `dark` class on
  `<html>` and persists the choice in `localStorage`. The Tailwind config
  uses `darkMode: ["class"]`.
- **Styling**: Tailwind CSS with shadcn/ui design tokens
  (`src/index.css`, `tailwind.config.ts`). `cn()` helper in `src/lib/utils.ts`.
  shadcn/ui components live under `src/components/ui/`.
- **Resilience**: a top-level `ErrorBoundary` (from `react-error-boundary`)
  with `AppErrorFallback` wraps the whole tree. A `<Suspense>` boundary
  inside the root route renders `GlobalSuspenseFallback` while routes load.

## REQ-FE-10 — OpenAPI client (this package's scope)

- **Types** are generated from the repo-root `openapi.json` (produced by
  `cargo run -p control-api -- print-openapi`) into
  `src/api/generated/schema.ts` via `openapi-typescript`.
- **Typed client** `apiClient` is exported from `src/api/client.ts` using
  `openapi-fetch`. It is the only site allowed to call `fetch` directly;
  ESLint forbids raw `fetch()` elsewhere in the app.
- **Hooks** under `src/api/hooks/` wrap each endpoint with TanStack Query.
  Components import from `@/api` only.

## Scripts

```bash
npm install
npm run gen:api          # regenerate src/api/generated/schema.ts
npm run gen:api:check    # CI guard — fails if regen produces a diff
npm run typecheck        # tsc -b --noEmit
npm run lint             # eslint
npm run dev              # vite dev server (proxies /v1, /health, /ready)
npm run build            # tsc -b && vite build
```

## End-to-end type sync

```
control-api handlers  ──cargo build──▶  print-openapi  ──▶  openapi.json
        ▲                                                       │
        │                                                       ▼
   openapi-sync CI  ◀───── diff ───────  scripts/check-openapi-sync.sh
                                                                │
                                                                ▼
                                                  openapi-typescript
                                                                │
                                                                ▼
                                            src/api/generated/schema.ts
                                                                │
                                                                ▼
                                                  frontend-typecheck CI
                                              (gen:api:check + tsc -b)
```

Two CI jobs guarantee no drift:

1. `openapi-sync` — `openapi.json` matches the Rust handlers.
2. `frontend-typecheck` — `schema.ts` matches `openapi.json`, and the
   TypeScript code compiles cleanly against those types.

## Adding a new endpoint hook

1. Add the route + schema in `services/control-api`.
2. Run `cargo run -p control-api -- print-openapi > openapi.json` from the
   repo root and commit the result.
3. From `frontend/`, run `npm run gen:api`.
4. Add a hook under `src/api/hooks/` and re-export from `hooks/index.ts`.

## Dev proxy

`vite.config.ts` proxies `/v1`, `/health`, `/ready` to the control-api
running on `http://localhost:8080` by default. Override with
`VITE_API_BASE_URL` in `.env.local`.
