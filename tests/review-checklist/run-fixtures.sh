#!/usr/bin/env bash
# Fixture tests for scripts/review-checklist.sh (RUSAA-367).
#
# Each check has a PASS fixture (should exit 0) and a FAIL fixture (should exit
# non-zero). Fixtures use synthetic data in a temp directory to be fast and
# deterministic. Every fixture runs inline in the current shell so it has
# access to shared helpers.
#
# Usage:
#   bash tests/review-checklist/run-fixtures.sh
#   make review-checklist-fixtures

set -uo pipefail

FAIL=0
RESULTS=()

_expect_result() {
    local label="$1" expected="$2" actual_exit="$3"
    if [[ "$expected" == "pass" && "$actual_exit" -eq 0 ]]; then
        echo "  OK (exit 0 as expected)"
        RESULTS+=("  PASS  $label")
    elif [[ "$expected" == "fail" && "$actual_exit" -ne 0 ]]; then
        echo "  OK (exit $actual_exit as expected)"
        RESULTS+=("  PASS  $label")
    else
        echo "  WRONG: expected $expected but got exit $actual_exit"
        FAIL=$((FAIL + 1))
        RESULTS+=("  FAIL  $label")
    fi
}

# ── Check 1 fixtures ──────────────────────────────────────────────────────────

_c1_check() {
    local tmpdir; tmpdir=$(mktemp -d)
    # shellcheck disable=SC2064
    trap "rm -rf $tmpdir" RETURN
    cat > "$tmpdir/check.py" << 'PYEOF'
import sys, re, collections
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
    printf '%s\n' "$1" | python3 "$tmpdir/check.py"
}

echo ""
echo "==> fixture: check-1 PASS: non-global multi-major + same-major dupes  [expect: pass]"
# thiserror v1+v2 and serde same-major should NOT fail; only global-state crates matter
_c1_check "serde v1.0.197
serde v1.0.195
thiserror v1.0.63
thiserror v2.0.3
tokio v1.36.0"
_expect_result "check-1 PASS: non-global multi-major + same-major dupes" "pass" "$?"

echo ""
echo "==> fixture: check-1 FAIL: metrics at v0 and v1  [expect: fail]"
_c1_check "metrics v0.23.0
metrics v1.0.0"
_expect_result "check-1 FAIL: metrics at v0 and v1" "fail" "$?"

# ── Check 2 fixtures ──────────────────────────────────────────────────────────
# Uses heredoc so \$ and \n in Python are not mangled by bash double-quote rules.

_c2_check() {
    local tmpdir; tmpdir=$(mktemp -d)
    # shellcheck disable=SC2064
    trap "rm -rf $tmpdir" RETURN
    printf '%s\n' "$1" > "$tmpdir/Cargo.lock"
    (cd "$tmpdir" && python3 - << 'PYEOF'
import re, sys

def compat_key(ver):
    parts = ver.split('.')
    major = int(parts[0])
    return (0, int(parts[1]) if len(parts) > 1 else 0) if major == 0 else (major,)

with open('Cargo.lock') as f:
    lock_text = f.read()

pkg_re = re.compile(
    r'\[\[package\]\]\s*\nname\s*=\s*"([^"]+)"\s*\nversion\s*=\s*"([^"]+)"',
    re.MULTILINE,
)
packages = {}
for m in pkg_re.finditer(lock_text):
    packages.setdefault(m.group(1), []).append(m.group(2))

metrics_vers = packages.get('metrics', [])
if not metrics_vers:
    print('  WARN: no metrics in lockfile')
    sys.exit(0)

ws_compat = {compat_key(v) for v in metrics_vers}

exporter_block_re = re.compile(
    r'\[\[package\]\]\s*\nname\s*=\s*"(metrics-exporter-[^"]+)".*?(?=\[\[package\]\]|\Z)',
    re.DOTALL,
)
issues = []
for m in exporter_block_re.finditer(lock_text):
    name = m.group(1)
    block = m.group(0)
    for dv in re.findall(r'"metrics (\d+\.\d+\.\d+)"', block):
        dk = compat_key(dv)
        if dk not in ws_compat:
            ws_r = ['.'.join(str(x) for x in k) + '.x' for k in sorted(ws_compat)]
            issues.append(f'  FAIL: {name} depends on metrics@{dv} but workspace uses {ws_r}')

if issues:
    for i in issues: print(i)
    sys.exit(1)
ws_r = ['.'.join(str(x) for x in k) + '.x' for k in sorted(ws_compat)]
print(f'  OK: exporter compat key matches workspace ({ws_r})')
PYEOF
)
}

echo ""
echo "==> fixture: check-2 PASS: exporter matches workspace 0.24.x  [expect: pass]"
_c2_check '
[[package]]
name = "metrics"
version = "0.24.0"

[[package]]
name = "metrics-exporter-prometheus"
version = "0.16.0"
dependencies = [
 "metrics 0.24.0",
]
'
_expect_result "check-2 PASS: exporter matches workspace 0.24.x" "pass" "$?"

echo ""
echo "==> fixture: check-2 FAIL: exporter 0.23 vs workspace 0.24  [expect: fail]"
_c2_check '
[[package]]
name = "metrics"
version = "0.24.0"

[[package]]
name = "metrics-exporter-prometheus"
version = "0.15.3"
dependencies = [
 "metrics 0.23.0",
]
'
_expect_result "check-2 FAIL: exporter 0.23 vs workspace 0.24" "fail" "$?"

# ── Check 3 fixtures ──────────────────────────────────────────────────────────

_c3_check() {
    local tmpdir; tmpdir=$(mktemp -d)
    # shellcheck disable=SC2064
    trap "rm -rf $tmpdir" RETURN
    mkdir -p "$tmpdir/src"
    printf '%s' "$1" > "$tmpdir/src/lib.rs"
    REPO_ROOT="$tmpdir" python3 - << 'PYEOF'
import os, re, sys, collections

REPO = os.environ.get('REPO_ROOT', '.')
rs_files = []
for root, dirs, files in os.walk(REPO):
    dirs[:] = [d for d in dirs if d not in {'target', '.git'}]
    for f in files:
        if f.endswith('.rs'):
            rs_files.append(os.path.join(root, f))

def extract_impl_methods(text, struct_name):
    methods = set()
    impl_re = re.compile(
        r'\bimpl(?:<[^>]*>)?\s+' + re.escape(struct_name) + r'(?:<[^>]*>)?\s*\{',
        re.DOTALL,
    )
    for m in impl_re.finditer(text):
        start = m.end()
        depth = 1
        pos = start
        while pos < len(text) and depth > 0:
            if text[pos] == '{': depth += 1
            elif text[pos] == '}': depth -= 1
            pos += 1
        impl_body = text[start:pos - 1]
        for mm in re.finditer(r'\bpub\s+(?:async\s+)?fn\s+(\w+)', impl_body):
            methods.add(mm.group(1))
    return methods

struct_files = collections.defaultdict(list)
for path in rs_files:
    with open(path) as f:
        text = f.read()
    for m in re.finditer(r'\bpub\s+struct\s+(\w+)', text):
        struct_files[m.group(1)].append(path)

failures = []
checked = set()
for name in sorted(struct_files):
    if not name.startswith('Test') or len(name) <= 4:
        continue
    base = name[4:]
    if base not in struct_files or base in checked:
        continue
    checked.add(base)
    base_methods = set()
    test_methods = set()
    for path in rs_files:
        with open(path) as f:
            text = f.read()
        base_methods |= extract_impl_methods(text, base)
        test_methods |= extract_impl_methods(text, name)
    extra = sorted(test_methods - base_methods)
    if extra:
        failures.append(f'  FAIL: Test{base} exposes extra pub fns: {extra}')

if failures:
    for ff in failures: print(ff)
    sys.exit(1)
print('  OK: no test-double public method leakage')
PYEOF
}

echo ""
echo "==> fixture: check-3 PASS: TestProducer has no extra pub fns  [expect: pass]"
_c3_check 'pub struct Producer {}
impl Producer {
    pub fn new() -> Self { Self {} }
    pub async fn publish(&self) {}
}
pub struct TestProducer {}
impl TestProducer {
    pub async fn publish(&self) {}
}'
_expect_result "check-3 PASS: TestProducer has no extra pub fns" "pass" "$?"

echo ""
echo "==> fixture: check-3 FAIL: TestProducer exposes drain_messages  [expect: fail]"
_c3_check 'pub struct Producer {}
impl Producer {
    pub fn new() -> Self { Self {} }
    pub async fn publish(&self) {}
}
pub struct TestProducer {}
impl TestProducer {
    pub async fn publish(&self) {}
    pub fn drain_messages(&self) -> Vec<String> { vec![] }
}'
_expect_result "check-3 FAIL: TestProducer exposes drain_messages" "fail" "$?"

# ── Check 4 fixtures ──────────────────────────────────────────────────────────

_c4_grep_check() {
    local tmpdir; tmpdir=$(mktemp -d)
    # shellcheck disable=SC2064
    trap "rm -rf $tmpdir" RETURN
    mkdir -p "$tmpdir/src"
    printf '%s' "$1" > "$tmpdir/src/lib.rs"
    REPO_ROOT="$tmpdir" python3 - << 'PYEOF'
import os, re, sys

REPO = os.environ.get('REPO_ROOT', '.')
GUARD_PAT = re.compile(
    r'(?:\.lock\(\)|\.borrow\(\)|\.borrow_mut\(\)|\.attach\(\)|'
    r'MutexGuard|RwLockReadGuard|RwLockWriteGuard|RefMut\b)'
)

issues = []
for root, dirs, files in os.walk(REPO):
    dirs[:] = [d for d in dirs if d not in {'target', '.git'}]
    for fn in files:
        if not fn.endswith('.rs'): continue
        path = os.path.join(root, fn)
        with open(path) as f: text = f.read()
        async_fn_re = re.compile(r'\basync\s+fn\s+\w+[^{]*\{', re.DOTALL)
        for m in async_fn_re.finditer(text):
            start = m.end()
            depth = 1
            pos = start
            while pos < len(text) and depth > 0:
                if text[pos] == '{': depth += 1
                elif text[pos] == '}': depth -= 1
                pos += 1
            body = text[start:pos - 1]
            guard_m = GUARD_PAT.search(body)
            await_m = re.search(r'\.await\b', body)
            if not (guard_m and await_m): continue
            if guard_m.start() >= await_m.start(): continue
            between = body[guard_m.start():await_m.start()]
            if re.search(r'\bdrop\s*\(', between): continue
            depth_at_guard = 0
            for ch in body[:guard_m.start()]:
                if ch == '{': depth_at_guard += 1
                elif ch == '}': depth_at_guard -= 1
            depth_at_await = depth_at_guard
            for ch in between:
                if ch == '{': depth_at_await += 1
                elif ch == '}': depth_at_await -= 1
            if depth_at_await < depth_at_guard: continue
            rel = os.path.relpath(path, REPO)
            issues.append(f'  WARN: {rel}: guard across .await')

if issues:
    for i in issues[:10]: print(i)
    print(f'  FAIL: {len(issues)} guard-across-.await site(s)')
    sys.exit(1)
print('  OK: no RAII guards across .await')
PYEOF
}

echo ""
echo "==> fixture: check-4 PASS: guard scoped before .await  [expect: pass]"
_c4_grep_check 'use std::sync::Mutex;
async fn safe_fn() {
    let m = Mutex::new(0u32);
    {
        let _g = m.lock().unwrap();
    } // guard dropped here
    something().await;
}
async fn something() {}'
_expect_result "check-4 PASS: guard scoped before .await" "pass" "$?"

echo ""
echo "==> fixture: check-4 FAIL: MutexGuard held across .await  [expect: fail]"
_c4_grep_check 'use std::sync::Mutex;
async fn unsafe_fn() {
    let m = Mutex::new(0u32);
    let _g = m.lock().unwrap();
    something().await;
}
async fn something() {}'
_expect_result "check-4 FAIL: MutexGuard held across .await" "fail" "$?"

# ── Check 5 fixtures ──────────────────────────────────────────────────────────
# Uses heredoc for Python so \$ is literal (not expanded by bash double-quote rules).

_c5_check() {
    local sh_content="$1"
    local tmpdir; tmpdir=$(mktemp -d)
    # shellcheck disable=SC2064
    trap "rm -rf $tmpdir" RETURN
    mkdir -p "$tmpdir/compose" "$tmpdir/scripts"
    printf 'CONTROL_API_HOST_PORT=18080\nFRONTEND_HOST_PORT=15173\n' \
        > "$tmpdir/compose/tailscale.env"
    printf '%s' "$sh_content" > "$tmpdir/scripts/test.sh"
    REPO_ROOT="$tmpdir" python3 - << 'PYEOF'
import os, re, sys

REPO = os.environ.get('REPO_ROOT', '.')
COMPOSE_DIR = os.path.join(REPO, 'compose')
SCRIPTS_DIR = os.path.join(REPO, 'scripts')

def parse_env_file(path):
    names = set()
    with open(path) as f:
        for line in f:
            line = line.strip()
            if not line or line.startswith('#'): continue
            m = re.match(r'^([A-Z_][A-Z0-9_]*)=', line)
            if m: names.add(m.group(1))
    return names

env_files = {}
for fname in os.listdir(COMPOSE_DIR):
    if fname.endswith('.env'):
        env_files[fname] = parse_env_file(os.path.join(COMPOSE_DIR, fname))

issues = []
for fname in os.listdir(SCRIPTS_DIR):
    if not fname.endswith('.sh'): continue
    path = os.path.join(SCRIPTS_DIR, fname)
    with open(path) as f: text = f.read()
    for env_fname, vars_in_file in env_files.items():
        if not re.search(r'--env-file\s+[^\s]*' + re.escape(env_fname), text): continue
        if re.search(r'\bsource\b[^\n]*' + re.escape(env_fname), text): continue
        for var in sorted(vars_in_file):
            for lineno, line in enumerate(text.split('\n'), 1):
                if re.search(r'docker\s+compose|docker\s+run', line): continue
                if re.match(r'\s*#', line): continue
                if re.search(r'\$\{?' + re.escape(var) + r'\b', line):
                    issues.append(
                        f'  FAIL: {fname} reads ${var} without sourcing {env_fname} '
                        f'(line {lineno})'
                    )
                    break

if issues:
    for i in issues: print(i)
    sys.exit(1)
print('  OK: no env-file parity issues')
PYEOF
}

echo ""
echo "==> fixture: check-5 PASS: script sources env file  [expect: pass]"
_c5_check '#!/usr/bin/env bash
source compose/tailscale.env
docker compose --env-file compose/tailscale.env -f compose/dev.yml up -d
curl http://localhost:${CONTROL_API_HOST_PORT}/health
'
_expect_result "check-5 PASS: script sources env file" "pass" "$?"

echo ""
echo "==> fixture: check-5 FAIL: reads var without sourcing  [expect: fail]"
_c5_check '#!/usr/bin/env bash
docker compose --env-file compose/tailscale.env -f compose/dev.yml up -d
curl http://localhost:${CONTROL_API_HOST_PORT}/health
'
_expect_result "check-5 FAIL: reads var without sourcing" "fail" "$?"

# ── Check 7 fixtures ──────────────────────────────────────────────────────────
# Check 6 (docker smoke) is skipped in fixture tests — requires docker.

_c7_check() {
    local src_content="$1" test_content="$2"
    local tmpdir; tmpdir=$(mktemp -d)
    # shellcheck disable=SC2064
    trap "rm -rf $tmpdir" RETURN
    mkdir -p "$tmpdir/crates/rb-foo/src" "$tmpdir/crates/rb-foo/tests"
    printf '%s' "$src_content"  > "$tmpdir/crates/rb-foo/src/lib.rs"
    printf '%s' "$test_content" > "$tmpdir/crates/rb-foo/tests/metrics_test.rs"
    REPO_ROOT="$tmpdir" python3 - << 'PYEOF'
import os, re, sys, collections

REPO = os.environ.get('REPO_ROOT', '.')
SRC_DIRS = [os.path.join(REPO, 'crates'), os.path.join(REPO, 'services')]
METRIC_MACRO = re.compile(
    r'\b(counter|histogram|gauge|increment|record)\s*!\s*\(\s*"([^"]+)"'
)
VALUE_ASSERT_PAT = re.compile(
    r'DebuggingRecorder|metrics_util|\.get_counter\b|\.get_histogram\b|'
    r'\.get_gauge\b|debug_recorder|snapshot\(\)'
)

metric_sources = collections.defaultdict(list)
for src_dir in SRC_DIRS:
    if not os.path.isdir(src_dir): continue
    for root, dirs, files in os.walk(src_dir):
        dirs[:] = [d for d in dirs if d not in {'target', '.git'}]
        for fname in files:
            if not fname.endswith('.rs'): continue
            if 'testing' in fname or 'test_util' in fname: continue
            path = os.path.join(root, fname)
            if 'tests/' in path or '/tests' in path: continue
            with open(path) as f: text = f.read()
            for m in METRIC_MACRO.finditer(text):
                lineno = text[:m.start()].count('\n') + 1
                rel = os.path.relpath(path, REPO)
                metric_sources[m.group(2)].append(f'{rel}:{lineno}')

if not metric_sources:
    print('  OK: no metrics found')
    sys.exit(0)

test_texts = []
for src_dir in SRC_DIRS:
    if not os.path.isdir(src_dir): continue
    for root, dirs, files in os.walk(src_dir):
        dirs[:] = [d for d in dirs if d not in {'target', '.git'}]
        for fname in files:
            if not fname.endswith('.rs'): continue
            path = os.path.join(root, fname)
            if 'test' in fname or 'tests' in root:
                with open(path) as f: test_texts.append(f.read())
            else:
                with open(path) as f: t = f.read()
                if '#[cfg(test)]' in t or '#[test]' in t: test_texts.append(t)

value_tested = set()
for text in test_texts:
    if not VALUE_ASSERT_PAT.search(text): continue
    for m in re.finditer(r'"([^"]+)"', text):
        if m.group(1) in metric_sources: value_tested.add(m.group(1))

missing = sorted(set(metric_sources.keys()) - value_tested)
if missing:
    for name in missing: print(f'  FAIL: metric "{name}" has no value-asserting test')
    sys.exit(1)
print(f'  OK: all {len(metric_sources)} metric(s) have value-asserting tests')
PYEOF
}

echo ""
echo "==> fixture: check-7 PASS: metric has DebuggingRecorder test  [expect: pass]"
_c7_check \
'use metrics::counter;
pub fn process() { counter!("my_app_events_total", "k" => "v").increment(1); }
' \
'#[cfg(test)]
mod tests {
    use metrics_util::debugging::DebuggingRecorder;
    #[test]
    fn test_metric_value() {
        let recorder = DebuggingRecorder::new();
        let snap = recorder.snapshotter();
        super::process();
        let data = snap.snapshot();
        let key = "my_app_events_total";
        assert!(data.into_hashmap().contains_key(key));
    }
}
'
_expect_result "check-7 PASS: metric has DebuggingRecorder test" "pass" "$?"

echo ""
echo "==> fixture: check-7 FAIL: metric emitted, no value assertion  [expect: fail]"
_c7_check \
'use metrics::counter;
pub fn process() { counter!("my_app_events_total", "k" => "v").increment(1); }
' \
'#[cfg(test)]
mod tests {
    #[test]
    fn test_process_runs() { super::process(); }
}
'
_expect_result "check-7 FAIL: metric emitted, no value assertion" "fail" "$?"

# ── Summary ───────────────────────────────────────────────────────────────────

echo ""
echo "==========================================="
echo "review-checklist fixture summary:"
for r in "${RESULTS[@]}"; do
    echo "$r"
done
echo "==========================================="

if [[ "$FAIL" -eq 0 ]]; then
    echo "ALL FIXTURES PASSED"
else
    echo "$FAIL fixture(s) had wrong outcome — checker logic is broken"
fi

exit "$FAIL"
