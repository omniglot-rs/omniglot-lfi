#! /usr/bin/env nix-shell
#! nix-shell --pure -i python3 -p 'python3.withPackages (pypkgs: with pypkgs; [matplotlib])'

# Add `pkgs.texlive.combined.scheme-full` as a dependency for PGF export. 

import sys
import json
import re
import itertools
import matplotlib
import matplotlib.pyplot as plt
from types import SimpleNamespace

#matplotlib.use("pgf")
#matplotlib.rcParams.update({
#    "pgf.texsystem": "pdflatex",
#    'font.family': 'serif',
#    'text.usetex': True,
#    'pgf.rcfonts': False,
#})

def plot_benchmark_means(benchmark_name, lfi_json_data, mpk_json_data):
    benchmarks = {}

    print("Parsing data...")
    for benchmark_series_name, benchmark_series in [("lfi", lfi_json_data), ("mpk", mpk_json_data)]:
        for bmark_out in benchmark_series:
            if bmark_out["reason"] != "benchmark-complete":
                continue

            benchmark_id_matches = re.search(r"(.*)/(.*)/(.*)", bmark_out["id"])
            benchmark_series = benchmark_series_name + "_" + benchmark_id_matches.group(2)

            if benchmark_series not in benchmarks:
                if benchmark_series == "lfi_unsafe" or benchmark_series == "mpk_unsafe":
                    label = f"Unsafe FFI ({benchmark_series_name.upper()})"
                elif benchmark_series == "lfi_og_lfi":
                    label = "OG\\textsubscript{LFI}"
                elif benchmark_series == "mpk_og_mpk":
                    label = "OG\\textsubscript{MPK}"
                elif benchmark_series == "mpk_sandcrust":
                    label = "Sandcrust"
                else:
                    raise ValueError(f"Unknown benchmark series: {benchmark_series}")

                print(f"Found benchmark series {benchmark_series}, label {label}")
                benchmarks[benchmark_series] = SimpleNamespace(
                    label=label,
                    mean_times=[],
                    throughputs=[],
                )

            benchmark = benchmarks[benchmark_series]

            try:
                benchmark.mean_times.append(bmark_out["mean"]["estimate"] / 1e6)
                benchmark.throughputs.append(bmark_out["throughput"][0]["per_iteration"] / 1e6)
            except KeyError:
                print(f"Skipping invalid JSON object: {bmark_out}")

    print("Plotting...")
    plt.figure(figsize=(3.5, 2))
    plt.xlabel("Decoded Image Size (MB)")
    plt.ylabel("Decode Time (ms)")
    plt.tight_layout()
    plt.grid(True)

    for ((_, benchmark), marker) in zip(benchmarks.items(), itertools.cycle('p+*')):
        plt.plot(
            benchmark.throughputs,
            benchmark.mean_times,
            marker=marker,
            markersize=6,
            linestyle='-',
            label=benchmark.label
        )

    plt.legend(fontsize="small")

    print(f"Writing output to {benchmark_name}.pdf")
    plt.savefig(f"{benchmark_name}.pdf")

    # Requires a LaTeX installation:
    #plt.savefig(f"{benchmark_name}.pgf")

if len(sys.argv) != 3:
    print(f"USAGE: {sys.argv[0]} <benchmark-name> <benchmark-output>")
    sys.exit(1)

try:
    print(f"Reading LFI data from file {sys.argv[2]}")
    lfi_data = []
    with open(sys.argv[2], 'r') as f:
        for line in f:
            if line.strip() != "":
                lfi_data.append(json.loads(line))

    mpk_file = "mpk_libpng_checked_1765902617.json"
    print(f"Reading MPK data from file {mpk_file}")
    mpk_data = []
    with open(mpk_file, 'r') as f:
        for line in f:
            if line.strip() != "":
                mpk_data.append(json.loads(line))

    plot_benchmark_means(sys.argv[1], lfi_data, mpk_data)

except FileNotFoundError:
    print(f"Error: '{sys.argv[2]}' not found. Please provide a valid file path.")
except json.JSONDecodeError:
    print("Error: Invalid JSON format in the file.")
