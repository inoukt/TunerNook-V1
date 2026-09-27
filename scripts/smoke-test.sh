#!/usr/bin/env bash
# TunerNook runtime smoke test — no Git metadata, no test framework required.
# Usage: scripts/smoke-test.sh [--app]
#   TUNERNOOK_BIN_DIR can select a build output directory (default: target/release).
#   --app  additionally launch the release GUI, verify it stays live with
#          empty stdout/stderr, and close only the process it launched (Windows only).
# Exits nonzero if any check fails. Safe to rerun; never writes inside the
# repository except through tuner-cli's explicit-output rules (temp dir).

set -u

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BIN_DIR="${TUNERNOOK_BIN_DIR:-$ROOT/target/release}"
FIXTURES="$ROOT/Test bin and xdf"
SRC_BIN="$FIXTURES/SCGa05_cal.bin"
SRC_XDF="$FIXTURES/SCGa05_cal.xdf"
RUN_APP=0
[ "${1:-}" = "--app" ] && RUN_APP=1

PASS=0
FAIL=0
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

ok()   { PASS=$((PASS + 1)); echo "  PASS: $1"; }
bad()  { FAIL=$((FAIL + 1)); echo "  FAIL: $1"; }
need() { bad "$1"; }

section() { echo; echo "== $1 =="; }

json_field() { # json_field <json-text> <python-expr over d>
  printf '%s' "$1" | python -c "
import json, sys
try:
    d = json.loads(sys.stdin.read())
    v = $2
    print('TRUE' if v else 'FALSE')
except Exception:
    print('FALSE')
"

}

expect_field_true() { # expect_field_true <name> <json> <expr>
  if [ "$(json_field "$2" "$3")" = "TRUE" ]; then ok "$1"; else need "$1 -- got: $(printf '%s' "$2" | head -c 200)"; fi
}

cli() { "$BIN_DIR/tuner-cli.exe" "$@"; }

section "Preflight"
for exe in tuner-cli tuner-api tuner-app; do
  if [ -x "$BIN_DIR/$exe.exe" ]; then ok "$exe.exe present"; else bad "$exe.exe missing — run: cargo build --release"; fi
done
if [ -f "$SRC_BIN" ] && [ -f "$SRC_XDF" ]; then ok "fixtures present"; else bad "fixtures missing under 'Test bin and xdf/'"; fi
if command -v python >/dev/null 2>&1; then ok "python available"; else bad "python not on PATH"; fi
if [ "$PASS" -eq 0 ] || [ "$FAIL" -gt 0 ]; then
  echo "Preflight failed; aborting."
  exit 1
fi

SOURCE_SHA="$(sha256sum "$SRC_BIN" | cut -d' ' -f1)"

section "tuner-cli"
R="$(cli inspect "$SRC_BIN" 2>/dev/null)"
expect_field_true "inspect: status ok, size+sha reported" "$R" "d.get('status')=='ok' and d.get('size_bytes',0)>0 and d.get('sha256')=='$SOURCE_SHA'"

R="$(cli inspect-xdf "$SRC_XDF" 2>/dev/null)"
expect_field_true "inspect-xdf: parameters and categories parsed" "$R" "d.get('status')=='ok' and d.get('parameter_count',0)>0 and d.get('category_count',0)>0"

R="$(cli xdf-validate "$SRC_XDF" "$SRC_BIN" 2>/dev/null)"
expect_field_true "xdf-validate: definition fits the BIN" "$R" "d.get('valid') is True"

R="$(cli compare "$SRC_BIN" "$SRC_BIN" 2>/dev/null)"
expect_field_true "compare: self-comparison is clean" "$R" "d.get('status')=='ok'"

R="$(cli plan-byte "$SRC_BIN" --offset 0x120 --value 0x7f 2>/dev/null)"
expect_field_true "plan-byte: dry run with operation id, no write" "$R" "d.get('dry_run') is True and bool(d.get('operation_id'))"

ERR_OUT="$TMP/cli.err"
cli inspect "$SRC_BIN" 2>"$ERR_OUT" >/dev/null
if grep -q '"schema":"tuner-log/v1"' "$ERR_OUT"; then ok "diagnostics: tuner-log/v1 records on stderr"; else need "diagnostics: no tuner-log/v1 on stderr"; fi

EDITED="$TMP/edited.bin"
R="$(cli edit-byte "$SRC_BIN" --offset 0x120 --value 0x7f --output "$EDITED" 2>/dev/null)"
expect_field_true "edit-byte: status ok" "$R" "d.get('status')=='ok'"
if [ -f "$EDITED" ]; then
  EDIT_SHA="$(sha256sum "$EDITED" | cut -d' ' -f1)"
  [ "$EDIT_SHA" != "$SOURCE_SHA" ] && ok "edit-byte: output differs from input" || need "edit-byte: output identical to input"
else
  need "edit-byte: no output file"
fi
[ "$(sha256sum "$SRC_BIN" | cut -d' ' -f1)" = "$SOURCE_SHA" ] && ok "safety: input BIN unmodified" || need "safety: input BIN was modified!"

section "tuner-api JSONL session"
LOG="$TMP/api-run.jsonl"
# Paths inside JSON are not converted by MSYS, so run from the project root
# with fixture-relative paths (bare CLI arguments above get auto-converted).
FIXTURES_REL="Test bin and xdf"
( cd "$ROOT" && printf '%s\n' \
  '{"action":"capabilities","request_id":"r-1"}' \
  "{\"action\":\"inspect\",\"request_id\":\"r-2\",\"input\":\"$FIXTURES_REL/SCGa05_cal.bin\"}" \
  "{\"action\":\"inspect_xdf\",\"request_id\":\"r-3\",\"xdf\":\"$FIXTURES_REL/SCGa05_cal.xdf\"}" \
  "{\"action\":\"xdf_validate\",\"request_id\":\"r-4\",\"xdf\":\"$FIXTURES_REL/SCGa05_cal.xdf\",\"bin\":\"$FIXTURES_REL/SCGa05_cal.bin\"}" \
  "{\"action\":\"plan_edit\",\"request_id\":\"r-5\",\"input\":\"$FIXTURES_REL/SCGa05_cal.bin\",\"offset\":288,\"value\":127}" \
  '{"action":"definitely_not_an_action","request_id":"r-6"}' \
  | "$BIN_DIR/tuner-api.exe" --log-file "$(cygpath -m "$LOG" 2>/dev/null || echo "$LOG")" 2>/dev/null > "$TMP/api.out" )

API_JSON="$TMP/api.out"
api_expr() { # api_expr <python-expr over results dict> ; results: request_id -> parsed object
  cat "$API_JSON" | python -c "
import json, sys
text = sys.stdin.read()
results = {}
for line in text.splitlines():
    line = line.strip()
    if line:
        obj = json.loads(line)
        results[obj.get('request_id')] = obj
try:
    v = $1
    print('TRUE' if v else 'FALSE')
except Exception:
    print('FALSE')
"
}

if [ "$(api_expr "all(results.get('r-%d' % i, {}).get('status') == 'ok' for i in range(1, 6))")" = "TRUE" ]; then
  ok "api: all five valid actions ok"
else
  need "api: expected r-1..r-5 ok -- got: $(head -c 300 "$API_JSON")"
fi

if [ "$(api_expr "results.get('r-6', {}).get('status') == 'error'")" = "TRUE" ]; then
  ok "api: unknown action rejected with error"
else
  need "api: unknown action was not rejected"
fi

if [ -f "$LOG" ] && grep -q '"schema":"tuner-log/v1"' "$LOG"; then
  ok "api: --log-file captured tuner-log/v1 diagnostics"
else
  need "api: log file missing or wrong schema"
fi

if [ "$RUN_APP" -eq 1 ]; then
  section "tuner-app (release GUI)"
  if ! command -v powershell.exe >/dev/null 2>&1 || ! command -v cygpath >/dev/null 2>&1; then
  need "app: --app requires Windows PowerShell and cygpath"
  else
    export TUNERNOOK_SMOKE_APP="$(cygpath -w "$BIN_DIR/tuner-app.exe")"
    mkdir -p "$TMP/appdata"
    export TUNERNOOK_SMOKE_APPDATA="$(cygpath -w "$TMP/appdata")"
    export TUNERNOOK_SMOKE_STDOUT="$(cygpath -w "$TMP/app.out")"
    export TUNERNOOK_SMOKE_STDERR="$(cygpath -w "$TMP/app.err")"
    APP_RESULT="$(powershell.exe -NoProfile -NonInteractive -Command \
      '$ErrorActionPreference = "Stop"; Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;
using System.Text;
public static class TunerNookSmokeWindow {
  public delegate bool EnumWindowsProc(IntPtr handle, IntPtr data);
  [DllImport("user32.dll")] static extern bool EnumWindows(EnumWindowsProc callback, IntPtr data);
  [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr handle, out uint processId);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] static extern int GetWindowText(IntPtr handle, StringBuilder text, int length);
  [DllImport("user32.dll")] static extern bool IsWindowVisible(IntPtr handle);
  [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr handle, uint message, IntPtr wParam, IntPtr lParam);
  public static IntPtr FindMainWindow(int wantedProcessId, string wantedTitle) {
    IntPtr found = IntPtr.Zero;
    EnumWindows((handle, data) => { uint processId; GetWindowThreadProcessId(handle, out processId); if (processId == (uint)wantedProcessId && IsWindowVisible(handle)) { var title = new StringBuilder(256); GetWindowText(handle, title, title.Capacity); if (title.ToString().StartsWith(wantedTitle, StringComparison.Ordinal)) { found = handle; return false; } } return true; }, IntPtr.Zero);
    return found;
  }
}
"@; $env:APPDATA = $env:TUNERNOOK_SMOKE_APPDATA; $p = Start-Process -FilePath $env:TUNERNOOK_SMOKE_APP -PassThru -RedirectStandardOutput $env:TUNERNOOK_SMOKE_STDOUT -RedirectStandardError $env:TUNERNOOK_SMOKE_STDERR; $id = $p.Id; Start-Sleep -Seconds 12; $p.Refresh(); $stayedLive = -not $p.HasExited; $closeStatus = "graceful"; if ($p.HasExited) { $closeStatus = "already-exited" } else { $window = [TunerNookSmokeWindow]::FindMainWindow($id, "TunerNook"); if ($window -eq [IntPtr]::Zero) { $closeStatus = "no-window" } else { $posted = [TunerNookSmokeWindow]::PostMessage($window, 0x0010, [IntPtr]::Zero, [IntPtr]::Zero); if (-not $posted) { $closeStatus = "close-failed" } elseif (-not $p.WaitForExit(4000)) { try { $p.Kill(); [void]$p.WaitForExit(4000); $p.Refresh(); $closeStatus = if ($p.HasExited) { "forced" } else { "stuck" } } catch { $closeStatus = "stuck" } } } }; "$id|$([int]$stayedLive)|$closeStatus"' \
      2>/dev/null | tr -d '\r')"
    IFS='|' read -r APP_PID APP_STAYED_LIVE APP_CLOSE_STATUS <<< "$APP_RESULT"

    if ! [[ "$APP_PID" =~ ^[0-9]+$ && "$APP_STAYED_LIVE" =~ ^[01]$ ]]; then
      need "app: launch did not return a Windows process ID"
    else
      if [ "$APP_STAYED_LIVE" = "1" ]; then
        ok "app: launched and stayed live (PID $APP_PID)"
      else
        need "app: process exited within 12s (PID $APP_PID)"
      fi
      [ -s "$TMP/app.out" ] && need "app: stdout not empty" || ok "app: stdout empty"
      [ -s "$TMP/app.err" ] && need "app: stderr not empty" || ok "app: stderr empty"

      case "$APP_CLOSE_STATUS" in
        graceful) ok "app: closed gracefully (PID $APP_PID)" ;;
        forced) need "app: did not close on WM_CLOSE; force cleanup was limited to its process handle (PID $APP_PID)" ;;
        no-window) need "app: could not find the visible TunerNook window for WM_CLOSE (PID $APP_PID)" ;;
        close-failed) need "app: Windows rejected WM_CLOSE for the visible TunerNook window (PID $APP_PID)" ;;
        already-exited) need "app: process exited before the close request (PID $APP_PID)" ;;
        *) need "app: process remained live after close and force-cleanup attempts (PID $APP_PID)" ;;
      esac
    fi
  fi
fi

echo
echo "== Summary: $PASS passed, $FAIL failed =="
[ "$FAIL" -eq 0 ]
