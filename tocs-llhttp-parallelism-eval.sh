#! /usr/bin/env bash

set -Eeuo pipefail

echo "========== Omniglot-LFI TOCS LLHTTP Parallelism Evaluation Script =========="

# From https://unix.stackexchange.com/a/366655
printarr() { declare -n __p="$1"; for k in "${!__p[@]}"; do printf "%s=%s\n" "$k" "${__p[$k]}" ; done ;  }

declare -A BENCH_RESULTS
declare -A BENCH_RESULTS_EST
declare -A BENCH_RESULTS_UNIT

function run_benchmark() {
	BENCHMARK_LABEL="$1"
	CARGO_BENCH_NAME="$2"
	RESULTS_PREFIX="$3"
	CARGO_CRITERION_FLAGS="$4"
	ALLOW_FAIL="$5"
	CHECKPOINT_JSON="${BENCHMARK_LABEL}_checkpoint.json"
	if [ ! -f "$CHECKPOINT_JSON" ]; then
		OUTPUT_JSON="${BENCHMARK_LABEL}_$(date +%s).json"
		echo "==> Running \"$BENCHMARK_LABEL\" benchmark, saving results to $CHECKPOINT_JSON"
		if [ "$ALLOW_FAIL" == "1" ]; then
			(cargo criterion --bench "$CARGO_BENCH_NAME" --message-format=json $CARGO_CRITERION_FLAGS || true) | tee "$OUTPUT_JSON"
		else
			cargo criterion --bench "$CARGO_BENCH_NAME" --message-format=json $CARGO_CRITERION_FLAGS | tee "$OUTPUT_JSON"
		fi
		echo "Benchmark complete, saving checkpoint to $CHECKPOINT_JSON"
		cp "$OUTPUT_JSON" "$CHECKPOINT_JSON"
	else
		echo "==> Reusing \"$BENCHMARK_LABEL\" benchmark checkpoint $CHECKPOINT_JSON"
	fi

	# Analyze the $CHECKPOINT_JSON file, parsing all "benchmark-complete" messages and extracting their "typical" runtime:
	while IFS= read -r JSON_LINE; do
		if [ "$(echo "$JSON_LINE" | jq -r .reason)" == "benchmark-complete" ]; then
			BENCH_ID="$(echo "$JSON_LINE" | jq -r .id)"
			BENCH_RES_EST="$(echo "$JSON_LINE" | jq -r .typical.estimate)"
			BENCH_RES_UNIT="$(echo "$JSON_LINE" | jq -r .typical.unit)"
			BENCH_RESULTS_EST["${RESULTS_PREFIX}${BENCH_ID}"]="$BENCH_RES_EST"
			BENCH_RESULTS_UNIT["${RESULTS_PREFIX}${BENCH_ID}"]="$BENCH_RES_UNIT"
			BENCH_RESULTS["${RESULTS_PREFIX}${BENCH_ID}"]="$(printf "%13.3f %s" "$BENCH_RES_EST" "$BENCH_RES_UNIT")"
		fi
	done < "$CHECKPOINT_JSON"
}

run_benchmark "llhttp_parallelism" "llhttp_parallelism_bench" "" "" ""

echo
echo
echo "========== RESULTS =========="
echo
printarr BENCH_RESULTS

echo
echo
echo "========= FIGURE ???: LLHTTP parallelism =========="
echo
./generate-llhttp-parallelism-plot.py llhttp_parallelism_bench llhttp_parallelism_checkpoint.json

