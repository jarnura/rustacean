#!/usr/bin/env node
// REQ-FE-10: ensure src/api/generated/schema.ts is in sync with ../openapi.json.
//
// Regenerates the schema into a temp file, diffs it against the committed
// schema, and fails CI if they differ. The actual codegen tool is
// openapi-typescript; this wrapper exists so the message is actionable.
import { spawnSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const root = resolve(new URL(".", import.meta.url).pathname, "..");
const committed = resolve(root, "src/api/generated/schema.ts");
const spec = resolve(root, "..", "openapi.json");

const tmp = mkdtempSync(join(tmpdir(), "rb-api-types-"));
const generated = join(tmp, "schema.ts");

try {
  const result = spawnSync(
    process.execPath,
    [
      resolve(root, "node_modules/openapi-typescript/bin/cli.js"),
      spec,
      "-o",
      generated,
      "--immutable",
    ],
    { cwd: root, stdio: "inherit" },
  );
  if (result.status !== 0) {
    process.exit(result.status ?? 1);
  }

  const a = readFileSync(committed, "utf8");
  const b = readFileSync(generated, "utf8");
  if (a !== b) {
    console.error(
      "\nERROR: src/api/generated/schema.ts is out of sync with ../openapi.json.",
    );
    console.error(
      "       Run `npm run gen:api` from frontend/ and commit the result.",
    );
    process.exit(1);
  }
  console.log("api-types: schema is in sync ✓");
} finally {
  rmSync(tmp, { recursive: true, force: true });
}
