#!/usr/bin/env bash
# Reviewer checklist — mechanized semantic catches (RUSAA-367).
# Complements review-ready.sh (static checks). Targets the three classes of
# bugs that static analysis misses: version-skew, test-double parity,
# async safety, env-file parity, image smoke, and metric-value coverage.
#
# Usage:
#   make review-checklist
#   WAIVER_FILE=pr_body.txt bash scripts/review-checklist.sh
#   SKIP_DOCKER=1 bash scripts/review-checklist.sh   # skip check-6 (no docker)
#
# Waiver: to bypass a failing check add a line to the PR body:
#   reviewer-waiver: <check-id> — <reason>
# Set WAIVER_FILE=<path-to-pr-body> when running; waived checks count as SKIP.
#
# PR Reviewer must paste the full output into their verdict comment.
# A verdict is REJECTED if any check FAILs without a matching waiver.

set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
WAIVER_FILE="${WAIVER_FILE:-}"
SKIP_DOCKER="${SKIP_DOCKER:-}"

FAIL=0
RESULTS=()

# ── Waiver loading ─────────────────────────────────────────────────────────────
declare -A WAIVERS
if [[ -n "$WAIVER_FILE" && -f "$WAIVER_FILE" ]]; then
    while IFS= read -r line; do
        # Match both em-dash (—) and double-dash (--)
        if [[ "$line" =~ ^reviewer-waiver:[[:space:]]([a-z0-9_-]+)[[:space:]] ]]; then
            WAIVERS["${BASH_REMATCH[1]}"]="$line"
        fi
    done < "$WAIVER_FILE"
fi

# ── step helper ────────────────────────────────────────────────────────────────
step() {
    local id="$1"
    local label="$2"
    local cmd="$3"
    echo ""
    echo "==> [${id}] ${label}"
    if "$cmd"; then
        RESULTS+=("  PASS  [${id}] ${label}")
    elif [[ -v "WAIVERS[$id]" ]]; then
        echo "  WAIVED: ${WAIVERS[$id]}"
        RESULTS+=("  SKIP  [${id}] ${label}  (waived)")
    else
        FAIL=$((FAIL + 1))
        RESULTS+=("  FAIL  [${id}] ${label}")
    fi
}

# ── Check 1: Cargo version-skew ───────────────────────────────────────────────
# cargo tree -d lists every crate that appears at >1 version. We parse that
# output and FAIL if any GLOBAL-STATE crate (metrics, tracing, tokio, log,
# opentelemetry family) appears at more than one major version. These crates
# use global singletons — two major versions = two independent registries that
# silently ignore each other. Catch: RUSAA-361 (metrics 0.23 vs 0.24).
# (Transitive multi-major deps for non-global crates, e.g. thiserror v1+v2,
# are excluded because they don't cause silent data loss.)

_check_cargo_version_skew() {
    cd "$REPO_ROOT"
    # Source cargo env if not already in PATH (common on dev machines running via make)
    if ! command -v cargo &>/dev/null && [[ -f "$HOME/.cargo/env" ]]; then
        # shellcheck source=/dev/null
        source "$HOME/.cargo/env"
    fi
    if ! command -v cargo &>/dev/null; then
        echo "  SKIP: cargo not in PATH (install Rust toolchain to enable this check)"
        return 0
    fi
    local tmpdir; tmpdir=$(mktemp -d)
    # shellcheck disable=SC2064
    trap "rm -rf $tmpdir" RETURN
    cat > "$tmpdir/check.py" << 'PYEOF'
import sys, re, collections

# Only flag crates that use global singletons: two major versions means two
# independent registries/subscribers/runtimes, each silently ignoring the other.
GLOBAL_STATE_PREFIXES = ('metrics', 'tracing', 'tokio', 'log', 'opentelemetry')

def is_global_state(name):
    return name in GLOBAL_STATE_PREFIXES or any(
        name.startswith(p + '-') for p in GLOBAL_STATE_PREFIXES
    )

seen = collections.defaultdict(set)
for line in sys.stdin:
    m = re.search(r'\b([a-z][a-z0-9_-]*) v(\d+)\.', line)
    if m and is_global_state(m.group(1)):
        seen[m.group(1)].add(int(m.group(2)))
offenders = {k: sorted(v) for k, v in seen.items() if len(v) > 1}
if offenders:
    for name, majors in sorted(offenders.items()):
        print(f'  FAIL: crate "{name}" at multiple major versions: {majors}')
    sys.exit(1)
print('  OK: no global-state crate at multiple major versions')
PYEOF
    cargo tree -d --workspace > "$tmpdir/tree.txt" 2>/dev/null || true
    python3 "$tmpdir/check.py" < "$tmpdir/tree.txt"
}

step "check-1" "cargo version-skew (multi-major duplicates)" "_check_cargo_version_skew"

# ── Check 2: Exporter ↔ recorder major match ──────────────────────────────────
# For every metrics-exporter-* in Cargo.lock, verify its metrics dep major
# matches the workspace metrics major. Specific named catch for RUSAA-361.

_check_exporter_recorder_match() {
    cd "$REPO_ROOT"
    python3 - << 'PYEOF'
import re, sys

with open('Cargo.lock') as f:
    lock_text = f.read()

def compat_key(version_str):
    """Semver compat bucket: (0, minor) for 0.x crates, (major,) for 1+."""
    parts = version_str.split('.')
    major = int(parts[0])
    if major == 0:
        return (0, int(parts[1]) if len(parts) > 1 else 0)
    return (major,)

# Collect all (name, version) pairs from [[package]] blocks
pkg_re = re.compile(
    r'\[\[package\]\]\s*\nname\s*=\s*"([^"]+)"\s*\nversion\s*=\s*"([^"]+)"',
    re.MULTILINE,
)
packages = {}  # name -> list of versions
for m in pkg_re.finditer(lock_text):
    packages.setdefault(m.group(1), []).append(m.group(2))

# Find workspace metrics compat keys
metrics_vers = packages.get('metrics', [])
if not metrics_vers:
    print('  WARN: no metrics crate found in Cargo.lock')
    sys.exit(0)

ws_compat_keys = {compat_key(v) for v in metrics_vers}

# Check each metrics-exporter-* block's metrics dep compat key
exporter_block_re = re.compile(
    r'\[\[package\]\]\s*\nname\s*=\s*"(metrics-exporter-[^"]+)".*?(?=\[\[package\]\]|\Z)',
    re.DOTALL,
)
issues = []
for m in exporter_block_re.finditer(lock_text):
    exporter_name = m.group(1)
    block = m.group(0)
    dep_ver_re = re.compile(r'"metrics (\d+\.\d+\.\d+)"')
    for dv in dep_ver_re.findall(block):
        dep_key = compat_key(dv)
        if dep_key not in ws_compat_keys:
            ws_readable = ['.'.join(str(x) for x in k) + '.x' for k in sorted(ws_compat_keys)]
            issues.append(
                f'  FAIL: {exporter_name} depends on metrics@{dv} '
                f'but workspace uses metrics@{ws_readable} '
                f'(upgrade exporter to match workspace metrics minor)'
            )

if issues:
    for i in issues:
        print(i)
    sys.exit(1)

ws_readable = ['.'.join(str(x) for x in k) + '.x' for k in sorted(ws_compat_keys)]
print(f'  OK: metrics-exporter-* compat key matches workspace ({ws_readable})')
PYEOF
}

step "check-2" "metrics exporter ↔ recorder major match" "_check_exporter_recorder_match"

# ── Check 3: Prod ↔ test-double trait parity ──────────────────────────────────
# For every pub struct Foo / pub struct TestFoo pair, collect each struct's
# public method names. FAIL if TestFoo exposes pub fns that Foo does not.
# Catch: RUSAA-300 K-HIGH-2 (TestProducer leaking test-only helpers).

_check_test_double_parity() {
    cd "$REPO_ROOT"
    python3 - << 'PYEOF'
import os, re, sys, collections

REPO = os.environ.get('REPO_ROOT', '.')

# Walk all .rs files except target/
rs_files = []
for root, dirs, files in os.walk(REPO):
    dirs[:] = [d for d in dirs if d not in {'target', '.git', 'node_modules'}]
    for f in files:
        if f.endswith('.rs'):
            rs_files.append(os.path.join(root, f))

# For each struct name, collect its public methods from impl blocks.
# We do a simple structural parse: find `impl ... StructName ...  {`, then
# scan the balanced block for `pub (async )? fn name`.

def extract_impl_methods(text, struct_name):
    """Return set of pub method names for a struct across all impl blocks."""
    methods = set()
    # Match: impl<...> StructName<...> { ... }
    # We find the opening brace position, then balance braces.
    impl_re = re.compile(
        r'\bimpl(?:<[^>]*>)?\s+' + re.escape(struct_name) + r'(?:<[^>]*>)?\s*\{',
        re.DOTALL,
    )
    for m in impl_re.finditer(text):
        start = m.end()
        depth = 1
        pos = start
        while pos < len(text) and depth > 0:
            if text[pos] == '{':
                depth += 1
            elif text[pos] == '}':
                depth -= 1
            pos += 1
        impl_body = text[start : pos - 1]
        for mm in re.finditer(r'\bpub\s+(?:async\s+)?fn\s+(\w+)', impl_body):
            methods.add(mm.group(1))
    return methods

# Collect struct names and which files define them
struct_files = collections.defaultdict(list)  # name -> [file, ...]
for path in rs_files:
    try:
        with open(path) as f:
            text = f.read()
    except Exception:
        continue
    for m in re.finditer(r'\bpub\s+struct\s+(\w+)', text):
        struct_files[m.group(1)].append(path)

# Build method sets for prod types and their Test doubles
failures = []
checked = set()
for name in sorted(struct_files):
    if not name.startswith('Test') or len(name) <= 4:
        continue
    base = name[4:]
    if base not in struct_files:
        continue
    if base in checked:
        continue
    checked.add(base)

    # Collect methods for both types
    base_methods = set()
    test_methods = set()
    for path in rs_files:
        try:
            with open(path) as f:
                text = f.read()
        except Exception:
            continue
        base_methods |= extract_impl_methods(text, base)
        test_methods |= extract_impl_methods(text, name)

    extra = sorted(test_methods - base_methods)
    if extra:
        failures.append(
            f'  FAIL: Test{base} exposes pub fn(s) not on {base}: {extra}\n'
            f'        Test doubles must not add public API that prod types lack.'
        )

if failures:
    for ff in failures:
        print(ff)
    sys.exit(1)
print('  OK: no test-double public method leakage found')
PYEOF
}

step "check-3" "prod ↔ test-double public method parity" "_check_test_double_parity"

# ── Check 4: Async / Send safety ─────────────────────────────────────────────
# (a) cargo clippy with await_holding_lock + async_yields_async lints.
# (b) Custom grep: async fns where a lock/refcell/context guard is created
#     and a .await appears in the same function body without an explicit drop.
# Catch: RUSAA-324 (ContextGuard held across .await).

_check_async_send_safety() {
    cd "$REPO_ROOT"
    local fail=0

    # (a) clippy targeted lints — suppress all others to reduce noise
    echo "  running clippy async-safety lints..."
    if ! cargo clippy --workspace --all-targets --all-features 2>&1 \
        -- \
        -A clippy::all \
        -W clippy::await_holding_lock \
        -W clippy::async_yields_async \
        2>&1 | grep -E '^error' | head -20; then
        :  # grep exit code; actual failure detected below
    fi
    if cargo clippy --workspace --all-targets --all-features \
        -- \
        -A clippy::all \
        -W clippy::await_holding_lock \
        -W clippy::async_yields_async \
        2>&1 | grep -qE '^error\['; then
        echo "  FAIL: clippy found await_holding_lock or async_yields_async violations"
        fail=1
    else
        echo "  OK (clippy): no await_holding_lock / async_yields_async violations"
    fi

    # (b) Heuristic grep: RAII guard types held across .await
    # Looks for async fns that both create a guard and have a .await without
    # an intervening explicit drop().
    echo "  running guard-across-await grep..."
    local grep_fail=0
    python3 - << 'PYEOF'
import os, re, sys

REPO = os.environ.get('REPO_ROOT', '.')
# Guard patterns: types/calls that create RAII guards unsafe to hold across .await
GUARD_PAT = re.compile(
    r'(?:\.lock\(\)|\.borrow\(\)|\.borrow_mut\(\)|\.attach\(\)|'
    r'MutexGuard|RwLockReadGuard|RwLockWriteGuard|\bRefMut\b|\bRef\b)'
)

issues = []
for root, dirs, files in os.walk(REPO):
    dirs[:] = [d for d in dirs if d not in {'target', '.git', 'node_modules'}]
    for fn in files:
        if not fn.endswith('.rs'):
            continue
        path = os.path.join(root, fn)
        try:
            with open(path) as f:
                text = f.read()
        except Exception:
            continue

        # Find async fn bodies
        async_fn_re = re.compile(r'\basync\s+fn\s+\w+[^{]*\{', re.DOTALL)
        for m in async_fn_re.finditer(text):
            start = m.end()
            depth = 1
            pos = start
            while pos < len(text) and depth > 0:
                if text[pos] == '{':
                    depth += 1
                elif text[pos] == '}':
                    depth -= 1
                pos += 1
            body = text[start : pos - 1]

            # Skip functions suppressed with: // review-checklist-ok: async-guard
            # Use this for tokio::sync::Mutex guards which are designed for async.
            if 'review-checklist-ok: async-guard' in body:
                continue

            # Check for guard + .await + no explicit drop between them
            guard_m = GUARD_PAT.search(body)
            await_m = re.search(r'\.await\b', body)
            if not (guard_m and await_m):
                continue
            # Allow if the guard is scoped in a block that closes before .await
            # Heuristic: guard position comes before await position
            if guard_m.start() >= await_m.start():
                continue
            # Check for explicit drop() between guard and await
            between = body[guard_m.start() : await_m.start()]
            if re.search(r'\bdrop\s*\(', between):
                continue
            # Check if guard is scoped in an inner block { ... } ending before await
            # Count brace depth at guard position vs await position
            depth_at_guard = 0
            for ch in body[:guard_m.start()]:
                if ch == '{':
                    depth_at_guard += 1
                elif ch == '}':
                    depth_at_guard -= 1
            depth_at_await = depth_at_guard
            for ch in between:
                if ch == '{':
                    depth_at_await += 1
                elif ch == '}':
                    depth_at_await -= 1
            if depth_at_await < depth_at_guard:
                # Guard was in a deeper scope that closed before .await
                continue

            # Compute line number for reporting
            lineno = text[:m.start()].count('\n') + 1
            rel = os.path.relpath(path, REPO)
            issues.append(f'  WARN: {rel}:{lineno}: potential RAII guard held across .await')

if issues:
    for i in issues[:20]:
        print(i)
    print(f'  FAIL: {len(issues)} potential guard-across-.await site(s) found')
    print('        Scope guards into a block that closes before the .await point,')
    print('        or call drop() explicitly before awaiting.')
    sys.exit(1)
print('  OK (grep): no RAII guards detected across .await points')
PYEOF
    grep_fail=$?
    [[ $grep_fail -ne 0 ]] && fail=1

    return $fail
}

step "check-4" "async / Send safety (await_holding_lock + guard grep)" "_check_async_send_safety"

# ── Check 5: Env-file ↔ shell parity ─────────────────────────────────────────
# Find shell scripts that hardcode --env-file compose/*.env AND read vars from
# that file in the same script without sourcing it first.
# Catch: RUSAA-303 H1 (health-check curl reading CONTROL_API_HOST_PORT from
# a compose env-file the script never sourced).

_check_envfile_shell_parity() {
    cd "$REPO_ROOT"
    python3 - << 'PYEOF'
import os, re, sys

REPO = os.environ.get('REPO_ROOT', '.')
COMPOSE_DIR = os.path.join(REPO, 'compose')
SCRIPTS_DIR = os.path.join(REPO, 'scripts')

# Parse an env file: return set of var names
def parse_env_file(path):
    names = set()
    with open(path) as f:
        for line in f:
            line = line.strip()
            if not line or line.startswith('#'):
                continue
            m = re.match(r'^([A-Z_][A-Z0-9_]*)=', line)
            if m:
                names.add(m.group(1))
    return names

# Load all compose/*.env files
env_files = {}  # basename -> set of var names
for fname in os.listdir(COMPOSE_DIR):
    if fname.endswith('.env'):
        try:
            env_files[fname] = parse_env_file(os.path.join(COMPOSE_DIR, fname))
        except Exception:
            pass

if not env_files:
    print('  OK: no compose/*.env files found')
    sys.exit(0)

# Scan shell scripts for --env-file references and var reads
issues = []
for fname in os.listdir(SCRIPTS_DIR):
    if not fname.endswith('.sh'):
        continue
    path = os.path.join(SCRIPTS_DIR, fname)
    try:
        with open(path) as f:
            text = f.read()
    except Exception:
        continue

    # Strip comment lines for --env-file detection (avoid matching examples in comments)
    non_comment_text = '\n'.join(
        line for line in text.split('\n') if not re.match(r'\s*#', line)
    )

    for env_fname, vars_in_file in env_files.items():
        # Does this script reference the env file via --env-file (in non-comment code)?
        if not re.search(r'--env-file\s+[^\s]*' + re.escape(env_fname), non_comment_text):
            continue
        # Does the script source the env file? Accept both:
        #   source compose/tailscale.env  (explicit name)
        #   source "$COMPOSE_ENV_FILE"    (dynamic via COMPOSE_ENV_FILE env var)
        if re.search(r'\bsource\b[^\n]*' + re.escape(env_fname), text):
            continue
        if re.search(r'\bsource\b[^\n]*COMPOSE_ENV_FILE', text):
            continue
        # Check if any var from this env file is read outside a docker/compose call
        for var in sorted(vars_in_file):
            # grep for ${VAR:-*} or $VAR usage outside of docker compose lines
            # Find all lines that use this var, excluding lines that are part of a
            # docker compose / docker run command
            for lineno, line in enumerate(text.split('\n'), 1):
                # Skip docker compose / docker run invocation lines
                if re.search(r'docker\s+compose|docker\s+run', line):
                    continue
                # Skip commented lines
                if re.match(r'\s*#', line):
                    continue
                if re.search(r'\$\{?' + re.escape(var) + r'\b', line):
                    issues.append(
                        f'  FAIL: {fname} reads ${var} (from {env_fname}) '
                        f'on line {lineno} but does not source {env_fname}.\n'
                        f'        Add: source compose/{env_fname}  (or use COMPOSE_ENV_FILE)'
                    )
                    break

if issues:
    for i in issues:
        print(i)
    sys.exit(1)
print('  OK: no env-file vars read without sourcing detected')
PYEOF
}

step "check-5" "env-file ↔ shell parity (compose vars sourced before use)" "_check_envfile_shell_parity"

# ── Check 6: Runtime image smoke (lite) ───────────────────────────────────────
# Build the control-api Docker image and run print-openapi (quick exit, no DB
# needed). Proves the binary loads all dynamic libs (libz, libsasl2, libcurl).
# Catch: RUSAA-356 (distroless image missing libz.so.1).

_check_runtime_image_smoke() {
    cd "$REPO_ROOT"
    if [[ -n "$SKIP_DOCKER" ]]; then
        echo "  SKIP: SKIP_DOCKER=1 set (set to empty to enable)"
        return 0
    fi
    if ! command -v docker &>/dev/null; then
        echo "  SKIP: docker not available in this environment"
        return 0
    fi

    local compose_file="$REPO_ROOT/compose/dev.yml"
    local image
    image=$(grep -A1 'control-api:' "$compose_file" | grep 'image:' | awk '{print $2}' | head -1)
    if [[ -z "$image" ]]; then
        # Fallback: parse image from service block
        image=$(python3 -c "
import sys, re
with open('$compose_file') as f: text = f.read()
m = re.search(r'control-api:.*?image:\s*(\S+)', text, re.DOTALL)
print(m.group(1) if m else 'rustbrain/control-api:dev')
")
    fi

    echo "  building ${image}..."
    if ! docker compose -f "$compose_file" build control-api 2>&1; then
        echo "  FAIL: docker compose build control-api failed"
        return 1
    fi

    echo "  running ${image} print-openapi (dynamic-lib smoke)..."
    local output exit_code
    output=$(docker run --rm "$image" print-openapi 2>&1)
    exit_code=$?
    if [[ $exit_code -ne 0 ]]; then
        echo "  FAIL: binary exited $exit_code — possible missing dynamic library"
        echo "$output" | head -10
        return 1
    fi
    # Sanity: output should contain OpenAPI boilerplate
    if ! echo "$output" | grep -q '"openapi"'; then
        echo "  FAIL: print-openapi output did not contain OpenAPI JSON"
        echo "$output" | head -5
        return 1
    fi
    echo "  OK: image builds and binary loads all dynamic libs"
}

step "check-6" "runtime image smoke (binary loads dynamic libs)" "_check_runtime_image_smoke"

# ── Check 7: Metric emission ↔ value semantic ─────────────────────────────────
# For every metrics::counter!/histogram!/gauge! call in non-test source, check
# that at least one test file asserts on that metric's value (not just runs the
# function). Uses DebuggingRecorder / metrics-util as the value-assertion marker.
# Catch: RUSAA-300 (consume_lag_seconds / e2e_latency_seconds emitted but value
# reconstruction broken; tests passed because they only checked emission).

_check_metric_value_coverage() {
    cd "$REPO_ROOT"
    python3 - << 'PYEOF'
import os, re, sys, collections

REPO = os.environ.get('REPO_ROOT', '.')

SRC_DIRS = [os.path.join(REPO, d) for d in ('crates', 'services')]
METRIC_MACRO = re.compile(
    r'\b(counter|histogram|gauge|increment|record)\s*!\s*\(\s*"([^"]+)"'
)
# Value-asserting test markers: use of metrics-util's DebuggingRecorder,
# or explicit snapshot/value assertions
VALUE_ASSERT_PAT = re.compile(
    r'DebuggingRecorder|metrics_util|\.get_counter\b|\.get_histogram\b|'
    r'\.get_gauge\b|debug_recorder|install_recording_recorder|'
    r'assert.*metrics|snapshot\(\)'
)

# Collect metric names emitted in non-test source
metric_sources = collections.defaultdict(list)  # name -> [file:lineno, ...]
for src_dir in SRC_DIRS:
    if not os.path.isdir(src_dir):
        continue
    for root, dirs, files in os.walk(src_dir):
        dirs[:] = [d for d in dirs if d not in {'target', '.git'}]
        for fname in files:
            if not fname.endswith('.rs'):
                continue
            path = os.path.join(root, fname)
            # Skip test files and test-util modules
            if 'testing' in fname or 'test_util' in fname:
                continue
            try:
                with open(path) as f:
                    text = f.read()
            except Exception:
                continue
            # Skip files that are only test modules
            if re.search(r'#\[cfg\(test\)\]', text) and not re.search(
                r'pub\s+(async\s+)?fn\s+(?!test_)', text
            ):
                continue
            for m in METRIC_MACRO.finditer(text):
                lineno = text[:m.start()].count('\n') + 1
                rel = os.path.relpath(path, REPO)
                metric_sources[m.group(2)].append(f'{rel}:{lineno}')

if not metric_sources:
    print('  OK: no metrics::*! macro calls found in source')
    sys.exit(0)

# Collect all test files
test_texts = []
for src_dir in SRC_DIRS:
    if not os.path.isdir(src_dir):
        continue
    for root, dirs, files in os.walk(src_dir):
        dirs[:] = [d for d in dirs if d not in {'target', '.git'}]
        for fname in files:
            if not fname.endswith('.rs'):
                continue
            path = os.path.join(root, fname)
            if 'test' not in fname and 'testing' not in fname:
                # Also include files with #[cfg(test)] blocks
                try:
                    with open(path) as f:
                        text = f.read()
                    if '#[cfg(test)]' in text or '#[test]' in text:
                        test_texts.append((path, text))
                except Exception:
                    pass
            else:
                try:
                    with open(path) as f:
                        text = f.read()
                    test_texts.append((path, text))
                except Exception:
                    pass

# Build a set of metric names that have value-asserting tests
value_tested = set()
for _, text in test_texts:
    if not VALUE_ASSERT_PAT.search(text):
        continue
    # This test file uses a debugging recorder; find which metrics it references
    for m in re.finditer(r'"([^"]+)"', text):
        name = m.group(1)
        if name in metric_sources:
            value_tested.add(name)

missing = sorted(set(metric_sources.keys()) - value_tested)
if missing:
    print(f'  FAIL: {len(missing)} metric(s) have no value-asserting test:')
    for name in missing[:10]:
        sites = metric_sources[name][:3]
        print(f'    {name}  (emitted at: {", ".join(sites)})')
    if len(missing) > 10:
        print(f'    ... and {len(missing) - 10} more')
    print()
    print('    Each metric must have at least one test that uses DebuggingRecorder')
    print('    (metrics-util) to assert the emitted value, not just run the code path.')
    sys.exit(1)

print(f'  OK: all {len(metric_sources)} metric(s) have value-asserting tests')
PYEOF
}

step "check-7" "metric emission ↔ value semantic (DebuggingRecorder coverage)" "_check_metric_value_coverage"

# ── Summary ────────────────────────────────────────────────────────────────────

echo ""
echo "==========================================="
echo "review-checklist summary:"
for r in "${RESULTS[@]}"; do
    echo "$r"
done
echo "==========================================="

if [[ "$FAIL" -eq 0 ]]; then
    echo "ALL CHECKS PASSED"
else
    echo "${FAIL} check(s) FAILED"
    echo ""
    echo "To waive a failing check, add to your PR body:"
    echo "  reviewer-waiver: <check-id> — <reason>"
    echo "Then re-run: WAIVER_FILE=pr_body.txt make review-checklist"
fi

exit "$FAIL"
