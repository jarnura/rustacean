# Contributing

## Before opening a PR

Run the local pre-PR check to catch issues before CI does:

```bash
make review-ready
```

This runs `cargo fmt --check`, `cargo clippy`, tests, `cargo deny`, and the codegen-drift checks.

## Regenerating generated artefacts

The repository tracks two generated files. Regenerate them whenever you change the API surface:

### OpenAPI spec (`openapi.json`)

```bash
cargo run -p control-api -- print-openapi > openapi.json
```

### Frontend TypeScript schema (`frontend/src/api/generated/schema.ts`)

```bash
cd frontend && npm run gen:api
```

Both commands must be re-run (in that order) whenever control-api handler signatures change. The CI `codegen-drift` job enforces this by running `git diff --exit-code` over both files on every PR.
