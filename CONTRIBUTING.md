# Contributing

## Before pushing any branch

Run `make review-ready` from the repo root before opening a PR. It runs `cargo fmt --check`, `cargo clippy`, the full workspace test suite, `cargo deny check`, and an OpenAPI freshness check — and also runs `pnpm lint`, `pnpm typecheck`, and `pnpm test` automatically if your branch touches `frontend/`. All steps execute even if earlier ones fail so you see the complete picture in one pass. Fix everything flagged before pushing; PRs that fail these checks are returned without review.

## Getting started

See `docs/getting-started.md` for environment setup, local stack, and migration steps.
