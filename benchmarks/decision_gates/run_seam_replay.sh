#!/usr/bin/env bash
# Decision-gate measurement-seam runner (VOX-D-005..008).
#
# The seams under benchmarks/decision_gates/ are deliberately outside the cargo
# workspace: they drive shipped R-00047 harness types but must not become a
# published crate target. This script is the reproducible replacement for the
# hand-typed `rustc --test` line recorded in each evidence document.
#
# It builds the workspace rlibs, resolves their hashed filenames from cargo's
# JSON output (several feature-variants of the same crate coexist in
# target/debug/deps), compiles each seam as a test binary, and runs it with
# --nocapture so the replay tables reach stdout.
#
# Usage: benchmarks/decision_gates/run_seam_replay.sh [seam-name ...]
#        (no arguments = all four gate seams)
#
# SEAM_TOOLCHAIN overrides the rustup toolchain, so the same replay can be run
# on a second host architecture to show the trace hashes are host-independent:
#   SEAM_TOOLCHAIN=1.98.0-aarch64-apple-darwin \
#     SEAM_OUT_DIR=target/decision-gate-seams-aarch64 \
#     benchmarks/decision_gates/run_seam_replay.sh

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$repo_root"

out_dir="${SEAM_OUT_DIR:-target/decision-gate-seams}"
mkdir -p "$out_dir"

# `rustup run <toolchain>` bypasses the rust-toolchain.toml pin without editing
# it; with no override the pinned toolchain is used exactly as before.
toolchain="${SEAM_TOOLCHAIN:-}"
# `command` is a no-op prefix: bash 3.2 (macOS) treats an empty array under
# `set -u` as an unbound variable, so the unset case still needs one word.
if [ -n "$toolchain" ]; then
  run_tool=(rustup run "$toolchain")
else
  run_tool=(command)
fi

seams=("$@")
if [ ${#seams[@]} -eq 0 ]; then
  seams=(snapshot_cow streaming_backpressure spatial_collision migration_nodes)
fi

echo "host: $("${run_tool[@]}" rustc -vV | awk '/^host:/ {print $2}')"
echo "rustc: $("${run_tool[@]}" rustc --version)"
echo "seam-runner-commit: $(git rev-parse HEAD)"
echo

"${run_tool[@]}" cargo build --workspace --all-features \
  --target-dir "$out_dir/cargo" --message-format=json >"$out_dir/build.json"

resolve_rlib() {
  python3 - "$1" "$out_dir/build.json" <<'PY'
import json, sys
want, path = sys.argv[1], sys.argv[2]
hit = None
for line in open(path):
    try:
        msg = json.loads(line)
    except ValueError:
        continue
    if msg.get("reason") != "compiler-artifact":
        continue
    if msg["target"]["name"] != want or "lib" not in msg["target"]["kind"]:
        continue
    if msg["profile"]["test"]:
        continue
    for f in msg["filenames"]:
        if f.endswith(".rlib"):
            hit = f
if hit is None:
    sys.exit(f"no rlib artifact for {want}")
print(hit)
PY
}

contracts_rlib="$(resolve_rlib lumio_voxel_contracts)"
support_rlib="$(resolve_rlib lumio_voxel_test_support)"

status=0
for seam in "${seams[@]}"; do
  src="benchmarks/decision_gates/${seam}.rs"
  bin="$out_dir/${seam}"
  echo "=== seam ${seam}"
  "${run_tool[@]}" rustc --edition 2024 --test \
    --crate-name "vox_seam_${seam}" \
    -L "dependency=$out_dir/cargo/debug/deps" \
    --extern "lumio_voxel_contracts=${contracts_rlib}" \
    --extern "lumio_voxel_test_support=${support_rlib}" \
    -o "$bin" "$src"
  # Regenerate the VOX-D-008 raw-value file from the seam itself. Runs before
  # the assertions so the self-check compares against freshly emitted bytes.
  if [ "$seam" = migration_nodes ] && [ "${SEAM_EMIT_VOX_D_008:-0}" = 1 ]; then
    data=benchmarks/decision_gates/data/vox-d-008/measurements.txt
    "$bin" --nocapture --test-threads=1 --exact tests::print_measurements_text \
      | awk '/^---BEGIN vox-d-008 measurements.txt---$/{f=1;next} /^---END vox-d-008 measurements.txt---$/{f=0} f' \
      >"$data.new"
    test -s "$data.new"
    mv "$data.new" "$data"
    echo "regenerated $data"
    # include_str! baked the old bytes into $bin; rebuild before asserting.
    "${run_tool[@]}" rustc --edition 2024 --test \
      --crate-name "vox_seam_${seam}" \
      -L "dependency=$out_dir/cargo/debug/deps" \
      --extern "lumio_voxel_contracts=${contracts_rlib}" \
      --extern "lumio_voxel_test_support=${support_rlib}" \
      -o "$bin" "$src"
  fi

  if "$bin" --nocapture --test-threads=1; then
    echo "=== seam ${seam}: PASS"
  else
    echo "=== seam ${seam}: FAIL"
    status=1
  fi
  echo
done

exit "$status"
