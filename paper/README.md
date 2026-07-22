# Paper

`murmuration.tex` — workshop draft (ACM `acmart`, `sigconf`). Full prose over the
settled numbers in `../results/RESULTS.md`; `OUTLINE.md` maps sections to
evidence.

## Build

```bash
# 1. Figures are SVG in ../results/figures/. Convert the ones the paper uses to PDF:
for f in fig1_ceiling fig6_hyperparameter_sweep; do
  rsvg-convert -f pdf -o ../results/figures/$f.pdf ../results/figures/$f.svg
done
# (or: inkscape --export-type=pdf ../results/figures/$f.svg)

# 2. Build
latexmk -pdf murmuration.tex
```

Needs a TeX distribution with `acmart` (TeX Live / MacTeX). If `acmart` is
unavailable, switch the first line to `\documentclass[11pt]{article}` for a quick
local preview — the content is class-agnostic.

## Status
- Prose: abstract, intro, related work, model, all findings, limitations,
  conclusion — drafted.
- Numbers/figures: final (from `results/`).
- TODO before submission: real CRAWDAD trace datapoint (\S6), live-node Q-routing
  validation (\S8 limitation → result), and a pass in the author's own voice.
