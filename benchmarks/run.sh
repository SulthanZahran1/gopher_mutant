#!/usr/bin/env bash
# GOAL-3 criterion 7: benchmark gopher_mutant vs gremlins on uuid/cobra.
# Like-for-like operator set: gremlins' 5 defaults map to AOR, ROR, INC
# (INVERT_NEGATIVES has no gopher_mutant equivalent; AOR covers its binary
# half). Same machine, real wall clocks, mutant counts documented.
set -u

BIN="${GOPHER_MUTANT_BIN:-$(cd "$(dirname "$0")/.." && pwd)/target/release/gopher_mutant}"
GREMLINS="${GREMLINS_BIN:-$HOME/go/bin/gremlins}"
OUT_DIR="$(cd "$(dirname "$0")" && pwd)/results"
mkdir -p "$OUT_DIR"

echo "gopher_mutant: $BIN"
echo "gremlins:      $GREMLINS"
"$BIN" --list-operators >/dev/null || { echo "gopher_mutant binary missing — build release first"; exit 1; }
"$GREMLINS" --version >/dev/null 2>&1 || { echo "gremlins missing"; exit 1; }

run_one() {
  local name="$1" dir="$2" ops="$3"
  echo "=== $name ==="
  cd "$dir"

  # Fairness: warm the Go build cache for BOTH tools with a baseline run
  # (also validates the target). Then clear only gopher_mutant's own
  # content-addressed cache so its timing is honest.
  go test ./... >/dev/null 2>&1
  rm -rf /tmp/gopher-mutant-cache

  # gopher_mutant (L4L ops, adaptive timeout, routing on)
  local start end gm_secs gm_total
  start=$(date +%s.%N)
  "$BIN" --path "$dir" --json --timeout 2 --operators "$ops" --parallel 2 \
    > "$OUT_DIR/$name-gopher.json" 2>/dev/null
  end=$(date +%s.%N)
  gm_secs=$(echo "$end $start" | awk '{printf "%.1f", $1-$2}')
  gm_total=$(python3 -c "import json; print(json.load(open('$OUT_DIR/$name-gopher.json'))['report']['total'])")

  # gremlins (5 default mutators, minus INVERT_NEGATIVES which has no
  # gopher_mutant equivalent — L4L parity). --timeout-coefficient 20 is
  # REQUIRED: the default 0 times out every mutant instantly (97/123
  # TIMED OUT on uuid — bogus "fast" times that never run tests).
  start=$(date +%s.%N)
  "$GREMLINS" unleash "$dir" --workers 2 --invert-negatives=false --timeout-coefficient 20 --output "$OUT_DIR/$name-gremlins.json" >/dev/null 2>&1
  end=$(date +%s.%N)
  local gr_secs gr_total
  gr_secs=$(echo "$end $start" | awk '{printf "%.1f", $1-$2}')
  gr_total=$(python3 -c "
import json
d = json.load(open('$OUT_DIR/$name-gremlins.json'))
print(sum(len(f.get('mutations', [])) for f in d.get('files', [])))
" 2>/dev/null || echo "?")

  echo "$name: gopher_mutant ${gm_secs}s (${gm_total} mutants) | gremlins ${gr_secs}s (${gr_total} mutants)"
  echo "$name|$gm_secs|$gm_total|$gr_secs|$gr_total" >> "$OUT_DIR/timings.tsv"
}

rm -f "$OUT_DIR/timings.tsv"
run_one "uuid"  "/tmp/gm-bench/uuid"  "AOR,ROR,INC"
run_one "cobra" "/tmp/gm-bench/cobra" "AOR,ROR,INC"

echo
echo "=== SUMMARY ==="
column -t -s'|' "$OUT_DIR/timings.tsv" 2>/dev/null || cat "$OUT_DIR/timings.tsv"
