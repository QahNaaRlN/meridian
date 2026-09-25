#!/usr/bin/env bash
# Smoke of one PACKAGED release artifact (`meridian-rust-migration-program-plan.md`
# §5.24.4 items 6–7, §5.24.5), never of `cargo run`:
#
#   scripts/release/smoke.sh <artifact> <target-triple> [<SHA256SUMS>]
#   scripts/release/smoke.sh --self-test
#
# It unpacks the artifact, checks its exact content (the executable and
# LICENSE, no runtime files) and, when given, its SHA-256 against
# SHA256SUMS; then drives every command of the one executable against this
# Kernel checkout and the synthetic frozen-Instance fixture
# (`meridian-cli/tests/fixtures/frozen-instance`): init creates both SQLite
# databases, doctor, validate (Kernel-only and DB-backed), resolve, export,
# both import kinds, and the migration operations plan, apply (dry run and
# real), verify and rollback.
#
# Every call is held to the contract of its exit class (`meridian --help`):
#   0/1 with --format json — stdout is exactly one JSON document
#       {"command", "status", "result"} with the expected command, status
#       "ok" (0) or "fail" (1) and the result's mandatory fields; stderr is
#       empty, also for the negative result 1;
#   0 human (--help) — text on stdout, empty stderr;
#   2/3 — empty stdout, a message on stderr.
# Repeated twice on the same state, with the same code and byte-identical
# stdout required: every read-only call (doctor, Kernel-only and DB-backed
# validate, resolve, export of the empty and of the imported state,
# migration plan, migration verify, the dry-run apply) and every repeat that
# must not change state (the already-applied apply, the refused second
# rollback). A state-changing call (init, the first apply, the first
# rollback, each import) runs once; its effect is pinned instead by a
# byte comparison of the export before and after (rollback restores the
# empty export, a repeated import leaves the export unchanged, the
# canonical round trip reproduces it). --help, the usage/environment
# errors and the import refused for a wrong confirmation run once.
#
# The executable runs with a PATH that holds nothing but the Git adapter it
# is allowed to call; the smoke proves by enumeration that no `node`,
# `npm`, `npx` or `psql` (with any Windows executable extension) is
# reachable through it. MERIDIAN_INSTANCE is unset and network proxies point
# nowhere. The contract checker needs a Python interpreter on the host; the
# executable never gets one.
#
# `--self-test` feeds the checks deliberately wrong outputs and a PATH that
# still reaches a runtime, and passes only if every one is rejected.

set -euo pipefail

root=$(cd "$(dirname "$0")/../.." && pwd)
fail() { echo "SMOKE FAIL: $*" >&2; exit 1; }

python=
for candidate in python3 python; do
  if command -v "$candidate" >/dev/null 2>&1 && "$candidate" -c 'import json' >/dev/null 2>&1; then
    python=$candidate
    break
  fi
done
[ -n "$python" ] || fail "the contract checker needs python3 or python on the host"

# contract <out> <err> <code> <kind> [<command> <status> <result-shape>]:
# prints a reason and returns 1 when the call breaks its exit-class contract.
#   kind json:  <command> <status> and <result-shape> "list" or
#               "field,field,…" (the mandatory fields of an object result)
#   kind text:  a human result on stdout
#   kind error: exit 2/3
contract() {
  "$python" - "$@" <<'PY'
import json, sys
out_path, err_path, code, kind = sys.argv[1:5]
rest = sys.argv[5:]
out = open(out_path, encoding="utf-8", errors="strict").read()
err = open(err_path, encoding="utf-8", errors="strict").read()
def no(reason):
    print(reason)
    sys.exit(1)
if kind == "error":
    if code not in ("2", "3"):
        no(f"exit {code} is not an error class")
    if out:
        no("an error wrote to stdout")
    if not err.strip():
        no("an error wrote nothing to stderr")
    sys.exit(0)
if err:
    no(f"exit {code} wrote to stderr: {err[:200]!r}")
if kind == "text":
    if code != "0" or not out.strip():
        no("a human result must be exit 0 with text on stdout")
    sys.exit(0)
command, status, shape = rest
try:
    doc = json.loads(out)
except ValueError as e:
    no(f"stdout is not exactly one JSON document: {e}")
if not isinstance(doc, dict):
    no("the JSON document is not an object")
if set(doc) != {"command", "status", "result"}:
    no(f"envelope keys are {sorted(doc)}, expected command, result, status")
expected_status = {"0": "ok", "1": "fail"}.get(code)
if expected_status is None:
    no(f"exit {code} cannot carry a JSON result")
if doc["status"] != status or status != expected_status:
    no(f"status {doc['status']!r} with exit {code}; expected {expected_status!r}")
if doc["command"] != command:
    no(f"command {doc['command']!r}, expected {command!r}")
result = doc["result"]
if shape == "list":
    if not isinstance(result, list):
        no("result is not a list")
else:
    if not isinstance(result, dict):
        no("result is not an object")
    missing = [f for f in shape.split(",") if f not in result]
    if missing:
        no(f"result lacks {missing}")
PY
}

# runtime_reachable <path-list>: prints the first node/npm/npx/psql the
# colon-separated list reaches, in any executable form.
runtime_reachable() {
  local dir tool ext
  local IFS=:
  for dir in $1; do
    [ -n "$dir" ] || continue
    for tool in node npm npx psql; do
      for ext in "" .exe .cmd .bat .ps1 .com; do
        if [ -e "$dir/$tool$ext" ]; then
          echo "$dir/$tool$ext"
          return 0
        fi
      done
    done
  done
  return 1
}

if [ "${1:-}" = "--self-test" ]; then
  [ "$#" -eq 1 ] || { echo "usage: $0 --self-test" >&2; exit 2; }
  t=$(mktemp -d)
  trap 'rm -rf "$t"' EXIT
  rejected=0
  expect() { # expect accept|reject <label> <stdout> <stderr> <code> <kind> [args]
    local want=$1 label=$2 out=$3 err=$4
    shift 4
    printf '%s' "$out" > "$t/out"
    printf '%s' "$err" > "$t/err"
    if reason=$(contract "$t/out" "$t/err" "$@"); then got=accept; else got=reject; fi
    [ "$got" = "$want" ] || fail "self-test \"$label\": the check would $got it (${reason:-no reason})"
    [ "$got" = accept ] || rejected=$((rejected + 1))
    echo "ok   self-test: $label — ${got}ed${reason:+ ($reason)}"
  }
  ok='{"command":"init","status":"ok","result":{"kernel":"k","tool_db":"t"}}'
  expect accept "a valid positive result" "$ok" "" 0 json init ok kernel,tool_db
  expect accept "a valid negative result" '{"command":"validate","status":"fail","result":{"ok":false}}' "" 1 json validate fail ok
  expect accept "a usage error" "" "error: no command given" 2 error
  expect accept "the human help" "USAGE:" "" 0 text
  expect reject "invalid JSON" '{"command":"init",' "" 0 json init ok kernel
  expect reject "two JSON documents" "$ok$ok" "" 0 json init ok kernel
  expect reject "text before the JSON" "note: $ok" "" 0 json init ok kernel
  expect reject "a JSON array" '[1]' "" 0 json init ok kernel
  expect reject "an extra envelope key" '{"command":"init","status":"ok","result":{"kernel":1},"x":1}' "" 0 json init ok kernel
  expect reject "the wrong command" "$ok" "" 0 json doctor ok kernel
  expect reject "status ok with exit 1" "$ok" "" 1 json init ok kernel
  expect reject "status fail with exit 0" '{"command":"init","status":"fail","result":{"kernel":1}}' "" 0 json init fail kernel
  expect reject "a missing mandatory field" "$ok" "" 0 json init ok kernel,workspace_db
  expect reject "an object where a list is due" "$ok" "" 0 json init ok list
  expect reject "stderr on a positive result" "$ok" "warning: x" 0 json init ok kernel
  expect reject "stderr on a negative result" '{"command":"validate","status":"fail","result":{"ok":false}}' "FAIL x" 1 json validate fail ok
  expect reject "stdout on an error" "{}" "error: x" 3 error
  expect reject "an error with empty stderr" "" "" 3 error
  expect reject "exit 1 as an error class" "" "error: x" 1 error
  expect reject "empty help" "" "" 0 text
  mkdir -p "$t/a" "$t/b"
  : > "$t/b/node.exe"
  if found=$(runtime_reachable "$t/a:$t/b"); then
    echo "ok   self-test: a PATH reaching node.exe — rejected ($found)"
    rejected=$((rejected + 1))
  else
    fail "self-test: node.exe further in PATH was not found"
  fi
  for tool in npm.cmd npx psql; do
    rm -f "$t/b/"*
    : > "$t/b/$tool"
    runtime_reachable "$t/a:$t/b" >/dev/null || fail "self-test: $tool in PATH was not found"
    rejected=$((rejected + 1))
    echo "ok   self-test: a PATH reaching $tool — rejected"
  done
  rm -f "$t/b/"*
  ! runtime_reachable "$t/a:$t/b" >/dev/null || fail "self-test: a clean PATH was rejected"
  echo "ok   self-test: a PATH without a runtime — accepted"
  echo "SELF-TEST PASSED: $rejected wrong inputs rejected"
  exit 0
fi

if [ "$#" -lt 2 ] || [ "$#" -gt 3 ]; then
  echo "usage: $0 <artifact> <target-triple> [<SHA256SUMS>] | $0 --self-test" >&2
  exit 2
fi
artifact=$(cd "$(dirname "$1")" && pwd)/$(basename "$1")
target=$2
sums=${3:-}

version=$(tr -d '[:space:]' < "$root/VERSION")
name="meridian-$version-$target"
fixture="$root/meridian-cli/tests/fixtures/frozen-instance"

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
steps=0
sha256() {
  if command -v sha256sum >/dev/null 2>&1; then sha256sum "$1" | cut -d' ' -f1
  else shasum -a 256 "$1" | cut -d' ' -f1; fi
}

# --- the artifact -------------------------------------------------------
case "$target" in
  *-windows-*) exe=meridian.exe; expected_archive="$name.zip" ;;
  *) exe=meridian; expected_archive="$name.tar.gz" ;;
esac
[ "$(basename "$artifact")" = "$expected_archive" ] \
  || fail "artifact is named $(basename "$artifact"), expected $expected_archive"
if [ -n "$sums" ]; then
  line=$(grep " \*\?$expected_archive\$" "$sums" || true)
  [ -n "$line" ] || fail "$expected_archive is not listed in $sums"
  [ "${line%% *}" = "$(sha256 "$artifact")" ] || fail "SHA-256 of $expected_archive differs from $sums"
  echo "ok   sha256 $expected_archive matches SHA256SUMS"
fi
mkdir "$work/unpacked"
case "$expected_archive" in
  *.zip)
    if command -v unzip >/dev/null 2>&1; then unzip -q "$artifact" -d "$work/unpacked"
    else 7z x -bso0 -bsp0 "-o$work/unpacked" "$artifact"; fi ;;
  *) tar -xzf "$artifact" -C "$work/unpacked" ;;
esac
listing=$(cd "$work/unpacked" && find . -mindepth 1 | sed 's|^\./||' | sort | tr '\n' ' ')
[ "$listing" = "$name $name/LICENSE $name/$exe " ] \
  || fail "artifact content is [$listing], expected exactly $name/{$exe,LICENSE}"
bin="$work/unpacked/$name/$exe"
[ -x "$bin" ] || fail "$exe is not executable"
echo "ok   artifact holds exactly $name/$exe and $name/LICENSE"

case "$target" in
  *-linux-musl)
    if command -v file >/dev/null 2>&1; then
      file -L "$bin" | grep -Eq 'statically linked|static-pie linked' \
        || fail "the Linux executable is not statically linked: $(file -L "$bin")"
      echo "ok   statically linked"
    fi ;;
  *-apple-darwin)
    foreign=$(otool -L "$bin" | tail -n +2 | awk '{print $1}' | grep -Ev '^/usr/lib/|^/System/' || true)
    [ -z "$foreign" ] || fail "links outside the system ABI: $foreign"
    echo "ok   links only the macOS system libraries" ;;
esac

# --- an environment without a runtime -----------------------------------
git_path=$(command -v git) || fail "git, the one adapter the executable may call, is not on the host"
case "$(uname -s)" in
  MINGW*|MSYS*|CYGWIN*)
    # Git for Windows: its own executable directory, verified below to hold
    # no runtime.
    allowed=$(dirname "$git_path") ;;
  *)
    mkdir "$work/allowed"
    ln -s "$git_path" "$work/allowed/git"
    allowed="$work/allowed" ;;
esac
if found=$(runtime_reachable "$allowed"); then
  fail "the executable's PATH still reaches a runtime: $found"
fi
env PATH="$allowed" git --version >/dev/null || fail "git is not callable through the executable's PATH"
echo "ok   the executable's PATH reaches git and no node, npm, npx or psql"
unset MERIDIAN_INSTANCE MERIDIAN_KERNEL
export http_proxy=http://127.0.0.1:9 https_proxy=http://127.0.0.1:9 all_proxy=http://127.0.0.1:9
# The executable's own directory and the working directory hold no runtime
# either (Windows searches both when it starts a program).
cd "$work"

# call <code> <label> <kind> [<command> <shape>] -- <args…>: one call held to
# its exit-class contract; stdout in $work/out, stderr in $work/err.
call() {
  local want=$1 label=$2 kind=$3
  shift 3
  local spec=()
  while [ "$1" != "--" ]; do spec+=("$1"); shift; done
  shift
  set +e
  env PATH="$allowed" "$bin" "$@" > "$work/out" 2> "$work/err"
  local code=$?
  set -e
  steps=$((steps + 1))
  [ "$code" = "$want" ] || fail "$label: exit $code, expected $want; stderr: $(head -c 2000 "$work/err")"
  local status=ok
  [ "$code" = 1 ] && status=fail
  local reason
  if [ "$kind" = json ]; then
    reason=$(contract "$work/out" "$work/err" "$code" json "${spec[0]}" "$status" "${spec[1]}") \
      || fail "$label: $reason"
  else
    reason=$(contract "$work/out" "$work/err" "$code" "$kind") || fail "$label: $reason"
  fi
  echo "ok   $label (exit $code, $kind contract)"
}
# twice: the same JSON call twice; the same code and byte-identical stdout.
twice() {
  call "$@"
  cp "$work/out" "$work/first"
  call "$@"
  cmp -s "$work/first" "$work/out" || fail "$2: two runs on the same state differ"
  echo "ok   $2 is deterministic"
}
json_field() { sed -n "s/.*\"$1\": *\"\([^\"]*\)\".*/\1/p" "$2" | head -n 1; }

kernel="$root"
INIT=kernel,kernel_edition,tool_db,tool_db_created,workspace,workspace_db,workspace_db_created
DOCTOR=healthy,kernel,kernel_git,tool_db,workspace,workspace_db
VALIDATE=checked_files,failures,kernel,ok,stats,warnings
RESOLVE=applicable_norms,applicable_protocols,conflicts,unresolved_items
PLAN=bundle_revision,canonical_export,diagnostics,plan,source,verdict
APPLY=already_applied,dry_run,effects,plan_fingerprint,run_id,status
VERIFY=applicability,changed,duplicate,expected,extra,imported,missing,plan_fingerprint,status
ROLLBACK=checkpoint_digest,reason,run_id,status
IMPORT_FROZEN=apply,bundle_revision,input_digest,kind,source,status,verify
IMPORT_CANONICAL=input_digest,kind,status,tool,workspace

dbs() { # dbs <label>: fresh tool/workspace databases
  local d="$work/$1"
  mkdir -p "$d"
  call 0 "init ($1)" json init "$INIT" -- init --kernel "$kernel" --workspace "$d/workspace" \
    --tool-db "$d/tool.sqlite3" --workspace-db "$d/workspace.sqlite3" --format json
  [ -f "$d/tool.sqlite3" ] && [ -f "$d/workspace.sqlite3" ] || fail "init did not create both databases"
}

# --- the synthetic frozen source ----------------------------------------
src="$work/source"
mkdir -p "$src" "$work/no-hooks"
g() {
  git -C "$src" -c core.autocrlf=false -c commit.gpgsign=false -c core.hooksPath="$work/no-hooks" \
    -c user.name='Meridian Fixture' -c user.email=fixture@meridian.invalid "$@"
}
export GIT_AUTHOR_NAME='Meridian Fixture' GIT_AUTHOR_EMAIL=fixture@meridian.invalid \
  GIT_AUTHOR_DATE='2026-01-01T00:00:00+00:00' GIT_COMMITTER_NAME='Meridian Fixture' \
  GIT_COMMITTER_EMAIL=fixture@meridian.invalid GIT_COMMITTER_DATE='2026-01-01T00:00:00+00:00'
g init -q
cp -R "$fixture/source/." "$src/"
g add -A
g commit -q -m 'frozen instance fixture'
[ "$(g rev-parse HEAD)" = "$(json_field revision "$fixture/expected.json")" ] \
  || fail "the fixture commit is not reproducible here (line endings?)"
mkdir -p "$src/migration/instance-data"
cp -R "$fixture/bundle/." "$src/migration/instance-data/"
g add -A
g commit -q -m 'accepted bundle'
fingerprint=$(json_field plan_fingerprint "$fixture/expected.json")

# --- every command of the one executable --------------------------------
call 0 "--help" text -- --help
grep -q '^USAGE:' "$work/out" || fail "--help printed no usage"
call 2 "a command line without a command" error --
call 2 "an unknown flag" error -- validate --kernel "$kernel" --no-such-flag x
call 3 "a Kernel that does not exist" error -- validate --kernel "$work/no-kernel" --format json

dbs a
d="$work/a"
twice 0 "doctor" json doctor "$DOCTOR" -- doctor --kernel "$kernel" --workspace "$d/workspace" \
  --tool-db "$d/tool.sqlite3" --workspace-db "$d/workspace.sqlite3" --format json
twice 0 "validate (Kernel only)" json validate "$VALIDATE" -- validate --kernel "$kernel" --format json
cat > "$work/request.json" <<'JSON'
{"work_item":{"repository_id":"sample-repo","work_kind":"operation","candidate_paths":[],"changed_paths":[]},
 "repository_inventory":[{"id":"sample-repo"}],
 "applicability":{"$schema":"../../registries/rule-resolution/applicability.schema.json","schema_version":1,"records":[]}}
JSON
twice 0 "resolve" json resolve "$RESOLVE" -- resolve --kernel "$kernel" --request "$work/request.json" --format json
twice 0 "export (empty)" json export list -- export --kernel "$kernel" --tool-db "$d/tool.sqlite3" \
  --workspace-db "$d/workspace.sqlite3" --format json
cp "$work/out" "$work/export-pre.json"

twice 0 "migration plan" json "migration plan" "$PLAN" -- migration plan --kernel "$kernel" --source "$src" --format json
twice 0 "migration apply (dry run)" json "migration apply" "$APPLY" -- migration apply --kernel "$kernel" --source "$src" \
  --workspace-db "$d/workspace.sqlite3" --format json
call 0 "migration apply" json "migration apply" "$APPLY" -- migration apply --kernel "$kernel" --source "$src" \
  --workspace-db "$d/workspace.sqlite3" --dry-run false --confirm "$fingerprint" --format json
run_id=$(json_field run_id "$work/out")
[ -n "$run_id" ] || fail "migration apply reported no run_id"
twice 0 "migration apply (repeat: already applied)" json "migration apply" "$APPLY" -- migration apply --kernel "$kernel" \
  --source "$src" --workspace-db "$d/workspace.sqlite3" --dry-run false --confirm "$fingerprint" --format json
grep -q '"status":"already-applied"' "$work/out" || fail "a repeated apply is not reported as already applied"
twice 0 "migration verify" json "migration verify" "$VERIFY" -- migration verify --kernel "$kernel" --source "$src" \
  --workspace-db "$d/workspace.sqlite3" --format json
call 0 "migration rollback" json "migration rollback" "$ROLLBACK" -- migration rollback --kernel "$kernel" --source "$src" \
  --workspace-db "$d/workspace.sqlite3" --run "$run_id" --confirm "$run_id" --format json
call 0 "export (after rollback)" json export list -- export --kernel "$kernel" --tool-db "$d/tool.sqlite3" \
  --workspace-db "$d/workspace.sqlite3" --format json
cmp -s "$work/export-pre.json" "$work/out" || fail "rollback did not return the export to its pre-state"
twice 1 "migration rollback (repeat: refused)" json "migration rollback" "$ROLLBACK,refusal" -- migration rollback \
  --kernel "$kernel" --source "$src" --workspace-db "$d/workspace.sqlite3" --run "$run_id" --confirm "$run_id" --format json

dbs b
d="$work/b"
call 0 "import --kind frozen-instance" json import "$IMPORT_FROZEN" -- import --kernel "$kernel" --kind frozen-instance \
  --source "$src" --tool-db "$d/tool.sqlite3" --workspace-db "$d/workspace.sqlite3" --confirm "$fingerprint" --format json
twice 0 "export (imported)" json export list -- export --kernel "$kernel" --tool-db "$d/tool.sqlite3" \
  --workspace-db "$d/workspace.sqlite3" --format json
cp "$work/out" "$work/export-imported.json"
call 0 "import --kind frozen-instance (repeat)" json import "$IMPORT_FROZEN" -- import --kernel "$kernel" \
  --kind frozen-instance --source "$src" --tool-db "$d/tool.sqlite3" --workspace-db "$d/workspace.sqlite3" \
  --confirm "$fingerprint" --format json
call 0 "export (after repeated import)" json export list -- export --kernel "$kernel" --tool-db "$d/tool.sqlite3" \
  --workspace-db "$d/workspace.sqlite3" --format json
cmp -s "$work/export-imported.json" "$work/out" || fail "a repeated import changed the export"
# The synthetic Instance carries no product record: a negative result.
twice 1 "validate --workspace-db (negative result)" json validate "$VALIDATE,workspace_state" -- validate \
  --kernel "$kernel" --workspace-db "$d/workspace.sqlite3" --format json
call 1 "import with a confirmation that does not match" json import kind,status -- import --kernel "$kernel" \
  --kind frozen-instance --source "$src" --tool-db "$d/tool.sqlite3" --workspace-db "$d/workspace.sqlite3" \
  --confirm "$(printf '0%.0s' $(seq 64))" --format json

dbs c
d="$work/c"
call 0 "import --kind canonical-records" json import "$IMPORT_CANONICAL" -- import --kernel "$kernel" \
  --kind canonical-records --input "$work/export-imported.json" --tool-db "$d/tool.sqlite3" \
  --workspace-db "$d/workspace.sqlite3" --confirm "$(sha256 "$work/export-imported.json")" --format json
call 0 "export (round trip)" json export list -- export --kernel "$kernel" --tool-db "$d/tool.sqlite3" \
  --workspace-db "$d/workspace.sqlite3" --format json
cmp -s "$work/export-imported.json" "$work/out" || fail "import → export → import → export is not byte-identical"
call 3 "a canonical-records input that does not exist" error -- import --kernel "$kernel" --kind canonical-records \
  --input "$work/request.json.missing" --tool-db "$d/tool.sqlite3" --workspace-db "$d/workspace.sqlite3" \
  --confirm "$(printf '0%.0s' $(seq 64))" --format json

echo "SMOKE PASSED: $name — $steps calls"
