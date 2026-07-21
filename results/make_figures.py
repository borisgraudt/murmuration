#!/usr/bin/env python3
"""Publication figures for the Elysium routing benchmark.

Reads the tagged CSVs written by `cargo run --release --bin benchmark` and emits
SVG (for the paper) and PNG (for slides/README) into `results/figures/`.

    python3 results/make_figures.py

Colours come from a validated categorical palette; series are assigned to fixed
slots by *entity*, so a strategy keeps its colour across every figure.
"""

from __future__ import annotations

import csv
import math
import os
from collections import defaultdict

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt
from matplotlib.ticker import FuncFormatter

HERE = os.path.dirname(os.path.abspath(__file__))
FIGDIR = os.path.join(HERE, "figures")

# ─── Palette (validated categorical slots) ───────────────────────────────────
# Fixed slot per strategy — colour follows the entity, never its rank.
COLOR = {
    "ucb1": "#2a78d6",            # slot 1 blue   — the shipped router
    "q_routing": "#eb6834",       # slot 2 orange — the proposed fix
    "agnostic_limit": "#1baf7a",  # slot 3 aqua   — the bound
    "heuristic": "#eda100",       # slot 4 yellow
    "random": "#e87ba4",          # slot 5 magenta
    "oracle": "#4a3aa7",          # slot 7 violet
    "flooding": "#e34948",        # slot 8 red
    "ucb1_dest": "#898781",       # muted — a negative result, deliberately recessive
}
LABEL = {
    "ucb1": "UCB1 (shipped)",
    "q_routing": "Q-routing",
    "agnostic_limit": "Agnostic ceiling",
    "heuristic": "Heuristic",
    "random": "Random",
    "oracle": "Oracle",
    "flooding": "Flooding",
    "ucb1_dest": "UCB1 per-destination",
}

INK = "#0b0b0b"
MUTED = "#898781"
GRID = "#e1e0d9"
AXIS = "#c3c2b7"
SURFACE = "#fcfcfb"

plt.rcParams.update({
    "figure.facecolor": SURFACE,
    "axes.facecolor": SURFACE,
    "savefig.facecolor": SURFACE,
    "font.family": ["DejaVu Sans"],
    "font.size": 9,
    "axes.edgecolor": AXIS,
    "axes.labelcolor": INK,
    "axes.titlesize": 10.5,
    "axes.titleweight": "bold",
    "axes.titlecolor": INK,
    "xtick.color": MUTED,
    "ytick.color": MUTED,
    "xtick.labelcolor": INK,
    "ytick.labelcolor": INK,
    "grid.color": GRID,
    "grid.linewidth": 0.6,
    "legend.frameon": False,
    "svg.fonttype": "none",
})

PCT = FuncFormatter(lambda v, _: f"{v:.0f}%")


def style(ax, *, ygrid=True):
    """Recessive chrome: no top/right spines, hairline horizontal grid only."""
    for side in ("top", "right"):
        ax.spines[side].set_visible(False)
    for side in ("left", "bottom"):
        ax.spines[side].set_linewidth(0.8)
    if ygrid:
        ax.set_axisbelow(True)
        ax.yaxis.grid(True)
        ax.xaxis.grid(False)


def mean_ci(xs):
    n = len(xs)
    if n == 0:
        return float("nan"), 0.0
    m = sum(xs) / n
    if n < 2:
        return m, 0.0
    var = sum((x - m) ** 2 for x in xs) / (n - 1)
    return m, 1.96 * math.sqrt(var / n)


def load(tag):
    """Aggregate a tagged benchmark CSV to {(strategy, n, p): (mean%, ci%)}."""
    path = os.path.join(HERE, f"benchmark_{tag}.csv")
    if not os.path.exists(path):
        raise SystemExit(f"missing {path} — run the benchmark with BENCH_TAG={tag}")
    buckets = defaultdict(list)
    extra = defaultdict(list)
    with open(path) as fh:
        for row in csv.DictReader(fh):
            key = (row["strategy"], int(row["n_nodes"]), float(row["link_prob"]))
            buckets[key].append(float(row["delivery_ratio"]) * 100)
            extra[key].append(float(row["transmissions_per_msg"]))
    delivery = {k: mean_ci(v) for k, v in buckets.items()}
    tx = {k: mean_ci(v) for k, v in extra.items()}
    return delivery, tx


def save(fig, name):
    os.makedirs(FIGDIR, exist_ok=True)
    for ext in ("svg", "png"):
        fig.savefig(os.path.join(FIGDIR, f"{name}.{ext}"), dpi=200, bbox_inches="tight")
    plt.close(fig)
    print(f"  figures/{name}.svg + .png")


# ─── Figure 1 — the ceiling ──────────────────────────────────────────────────

def fig_ceiling(delivery):
    """UCB1 tracks the agnostic ceiling; both sit far under the oracle."""
    conds = [(10, 0.30), (20, 0.20), (50, 0.10), (50, 0.20), (100, 0.06)]
    xs = list(range(len(conds)))
    xlabels = [f"n={n}\np={p:g}" for n, p in conds]

    fig, ax = plt.subplots(figsize=(6.4, 3.6))
    series = ["oracle", "agnostic_limit", "ucb1", "heuristic", "random"]
    for strat in series:
        ys, es = [], []
        for c in conds:
            m, ci = delivery.get((strat, *c), (float("nan"), 0))
            ys.append(m)
            es.append(ci)
        dashed = strat == "agnostic_limit"
        ax.errorbar(
            xs, ys, yerr=es,
            color=COLOR[strat], lw=2, marker="o", ms=5,
            markeredgecolor=SURFACE, markeredgewidth=1.2,
            linestyle="--" if dashed else "-",
            capsize=2.5, elinewidth=1, label=LABEL[strat],
        )

    # Shade the unreachable band: everything above the agnostic ceiling.
    ceil = [delivery.get(("agnostic_limit", *c), (0, 0))[0] for c in conds]
    orac = [delivery.get(("oracle", *c), (0, 0))[0] for c in conds]
    ax.fill_between(xs, ceil, orac, color=COLOR["agnostic_limit"], alpha=0.07, lw=0)
    ax.annotate(
        "unreachable by any\ndestination-agnostic policy",
        xy=(2.0, 40), color=MUTED, fontsize=8, ha="center", va="center",
    )

    ax.set_xticks(xs)
    ax.set_xticklabels(xlabels)
    ax.set_ylabel("Delivery rate")
    ax.yaxis.set_major_formatter(PCT)
    ax.set_ylim(0, 88)
    ax.set_title("UCB1 saturates its formulation; the ceiling is the problem")
    # Legend below the axes: the oracle peaks at ~78% and would sit under an
    # in-axes legend at upper right.
    ax.legend(loc="upper center", bbox_to_anchor=(0.5, -0.16), ncol=5, fontsize=8)
    style(ax)
    save(fig, "fig1_ceiling")


# ─── Figure 2 — the Zipf crossover (headline) ────────────────────────────────

def fig_zipf():
    """Concentrated traffic flips the ranking: the money figure."""
    ss = [0.0, 0.8, 1.2]
    data = {}
    for s in ss:
        d, _ = load(f"zipf{s}")
        data[s] = d

    fig, ax = plt.subplots(figsize=(6.0, 3.6))
    cond = (50, 0.20)
    for strat in ["agnostic_limit", "q_routing", "ucb1"]:
        ys = [data[s].get((strat, *cond), (float("nan"), 0))[0] for s in ss]
        es = [data[s].get((strat, *cond), (0, 0))[1] for s in ss]
        ax.errorbar(
            ss, ys, yerr=es,
            color=COLOR[strat], lw=2.2, marker="o", ms=6,
            markeredgecolor=SURFACE, markeredgewidth=1.2,
            linestyle="--" if strat == "agnostic_limit" else "-",
            capsize=3, elinewidth=1, label=LABEL[strat],
        )
        ax.annotate(
            f"{ys[-1]:.1f}%", xy=(ss[-1], ys[-1]), xytext=(6, 0),
            textcoords="offset points", color=INK, fontsize=8.5,
            va="center", fontweight="bold",
        )

    ax.set_xlabel("Destination concentration  (Zipf exponent $s$)")
    ax.set_ylabel("Delivery rate")
    ax.yaxis.set_major_formatter(PCT)
    ax.set_xticks(ss)
    ax.set_xticklabels(["0.0\nuniform", "0.8", "1.2\nconcentrated"])
    ax.set_xlim(-0.08, 1.42)
    ax.set_ylim(0, 19)
    ax.set_title("Bootstrapping overtakes the ceiling as traffic concentrates")
    ax.legend(loc="upper left", fontsize=8)
    style(ax)
    save(fig, "fig2_zipf_crossover")


# ─── Figure 3 — convergence ──────────────────────────────────────────────────

def fig_convergence():
    """Q-routing needs ~20x more traffic than a bandit to converge."""
    # Measured at n=50, p=0.20 (see RESULTS.md finding 6).
    msgs = [2_000, 10_000, 40_000, 150_000]
    q = [4.5, 6.0, 8.3, 14.1]
    u = [8.0, 8.1, 9.9, 11.7]
    ceiling = 13.4

    fig, ax = plt.subplots(figsize=(6.0, 3.4))
    ax.axhline(ceiling, color=COLOR["agnostic_limit"], lw=2, ls="--",
               label=LABEL["agnostic_limit"])
    ax.plot(msgs, q, color=COLOR["q_routing"], lw=2.2, marker="o", ms=6,
            markeredgecolor=SURFACE, markeredgewidth=1.2, label=LABEL["q_routing"])
    ax.plot(msgs, u, color=COLOR["ucb1"], lw=2.2, marker="o", ms=6,
            markeredgecolor=SURFACE, markeredgewidth=1.2, label=LABEL["ucb1"])

    ax.set_xscale("log")
    ax.set_xlabel("Messages per run  (log scale)")
    ax.set_ylabel("Delivery rate")
    ax.yaxis.set_major_formatter(PCT)
    ax.set_ylim(0, 17)
    ax.xaxis.set_major_formatter(FuncFormatter(
        lambda v, _: f"{v/1000:.0f}k" if v >= 1000 else f"{v:.0f}"))
    ax.set_xticks(msgs)
    ax.set_title("Q-routing converges slowly, then crosses the ceiling")
    ax.legend(loc="upper left", fontsize=8)
    style(ax)
    save(fig, "fig3_convergence")


# ─── Figure 4 — delivery vs overhead ─────────────────────────────────────────

def fig_overhead(delivery, tx):
    """Flooding buys delivery with two orders of magnitude more transmissions."""
    cond = (50, 0.20)
    fig, ax = plt.subplots(figsize=(6.0, 3.6))

    # The single-path strategies bunch between 2.9 and 6.6 transmissions, so label
    # offsets are placed by hand rather than left to collide.
    OFFSET = {
        "random": (-9, -1, "right"),
        "q_routing": (-6, 11, "right"),
        "ucb1": (7, -4, "left"),
        "heuristic": (-2, 13, "right"),
        "agnostic_limit": (9, 2, "left"),
        "oracle": (0, 12, "center"),
        "flooding": (0, 12, "center"),
    }
    for strat in ["random", "ucb1", "heuristic", "agnostic_limit", "q_routing",
                  "oracle", "flooding"]:
        d = delivery.get((strat, *cond))
        t = tx.get((strat, *cond))
        if not d or not t:
            continue
        ax.scatter(t[0], d[0], s=70, color=COLOR[strat], zorder=3,
                   edgecolor=SURFACE, linewidth=1.4)
        dx, dy, ha = OFFSET[strat]
        # Direct labels: identity never rests on colour alone.
        ax.annotate(LABEL[strat], xy=(t[0], d[0]), xytext=(dx, dy),
                    textcoords="offset points", ha=ha,
                    color=INK, fontsize=8)

    ax.set_xscale("log")
    ax.set_xlabel("Transmissions per message  (log scale)")
    ax.set_ylabel("Delivery rate")
    ax.yaxis.set_major_formatter(PCT)
    ax.set_ylim(-6, 114)
    ax.set_xlim(2.2, 3200)
    ax.set_title("Flooding's delivery costs ~250× the transmissions  (n=50, p=0.2)")
    style(ax)
    save(fig, "fig4_delivery_vs_overhead")


# ─── Figure 5 — regret curves ────────────────────────────────────────────────

def fig_regret():
    """Linear regret = not learning. Slope is the diagnostic, not the height."""
    path = os.path.join(HERE, "learning_curve_grid.csv")
    if not os.path.exists(path):
        print("  (skipping fig5 — learning_curve_grid.csv not found)")
        return

    # Keyed by strategy → seed → {decisions: regret}. Seeds must be kept apart:
    # each run stops at a different decision count, so averaging over "whatever
    # seeds reached x" makes the mean fall as seeds drop out — and a *cumulative*
    # quantity that decreases is an artefact, not a finding.
    per_seed = defaultdict(lambda: defaultdict(dict))
    with open(path) as fh:
        for row in csv.DictReader(fh):
            if int(row["n_nodes"]) != 50 or float(row["link_prob"]) != 0.20:
                continue
            per_seed[row["strategy"]][row["seed"]][int(row["decisions"])] = float(
                row["cumulative_regret"])

    fig, ax = plt.subplots(figsize=(6.0, 3.4))
    for strat in ["random", "ucb1", "ucb1_dest", "heuristic"]:
        if strat not in per_seed:
            continue
        seeds_data = per_seed[strat]
        # Truncate to the shortest run so every plotted x averages the same seeds.
        cutoff = min(max(d) for d in seeds_data.values())
        xs = sorted(x for x in next(iter(seeds_data.values())) if x <= cutoff)
        ys = [
            sum(d[x] for d in seeds_data.values()) / len(seeds_data)
            for x in xs
            if all(x in d for d in seeds_data.values())
        ]
        xs = [x for x in xs if all(x in d for d in seeds_data.values())]
        ax.plot(xs, ys, color=COLOR[strat], lw=2, label=LABEL[strat])

    ax.set_xlabel("Routing decisions")
    ax.set_ylabel("Cumulative regret")
    ax.set_title("Regret grows linearly — the router is not converging")
    ax.legend(loc="upper left", fontsize=8)
    style(ax)
    save(fig, "fig5_regret")


def main():
    print("Building figures...")
    delivery, tx = load("grid")
    fig_ceiling(delivery)
    fig_zipf()
    fig_convergence()
    fig_overhead(delivery, tx)
    fig_regret()
    print(f"\nWrote to {FIGDIR}")


if __name__ == "__main__":
    main()
