#! /usr/bin/env nix-shell
#! nix-shell --pure -i python3 -p 'python3.withPackages (pypkgs: with pypkgs; [matplotlib])'

# Add `pkgs.texlive.combined.scheme-full` as a dependency for PGF export.

import sys
import json
import re
import itertools
import matplotlib
matplotlib.use('Agg') 
import matplotlib.pyplot as plt
from types import SimpleNamespace

#matplotlib.use("pgf")
#matplotlib.rcParams.update({
#    "pgf.texsystem": "pdflatex",
#    'font.family': 'serif',
#    'text.usetex': True,
#    'pgf.rcfonts': False,
#})

def plot_benchmark_means(benchmark_name, json_data):
    threads = []
    mean_times = []
    parse_iters = None

    print("Parsing data...")
    for bmark_out in json_data:
        if bmark_out["reason"] != "benchmark-complete":
            continue

        benchmark_id_matches = re.search(r"llhttp_parse/llhttp_parse_([0-9]+)req_threads/([0-9]+)", bmark_out["id"])
        bmark_parse_iters = int(benchmark_id_matches.group(1))
        if parse_iters != None and parse_iters != bmark_parse_iters:
            raise ValueError(f"Different parse iterations between benchmarks: {parse_iters} vs. {bmark_parse_iters}")
        parse_iters = bmark_parse_iters

        threads.append(int(benchmark_id_matches.group(2)))
        mean_times.append(bmark_out["mean"]["estimate"] / parse_iters)

    print("Plotting...")
    plt.figure(figsize=(3.5, 2))
    plt.xlabel("Number of Threads")
    plt.ylabel("Parse Time (ns)")
    plt.tight_layout()
    plt.grid(True)

    plt.plot(
        threads,
        mean_times,
        marker='p',
        markersize=6,
        linestyle='-',
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
    data = []
    print(f"Reading data from file {sys.argv[2]}")
    with open(sys.argv[2], 'r') as f:
        for line in f:
            if line.strip() != "":
                data.append(json.loads(line))
    plot_benchmark_means(sys.argv[1], data)

except FileNotFoundError:
    print(f"Error: '{sys.argv[2]}' not found. Please provide a valid file path.")
except json.JSONDecodeError:
    print("Error: Invalid JSON format in the file.")
