#!/usr/bin/env python3
import json
from pathlib import Path

def read_estimate(path: Path):
    try:
        data = json.loads(path.read_text())
        mean = float(data.get('mean', {}).get('point_estimate', 0.0))
        std = float(data.get('std_dev', {}).get('point_estimate', 0.0))
        var = std * std
        median = float(data.get('median', {}).get('point_estimate', 0.0))
        mad = float(data.get('median_abs_dev', {}).get('point_estimate', 0.0))
        return mean, std, var, median, mad
        median = float(data.get('median', {}).get('point_estimate', 0.0))
        mad = float(data.get('median_abs_dev', {}).get('point_estimate', 0.0))
    except Exception:
        return None

def cat_name(name: str) -> str:
    # Prefer longer stream tags when present
    for pfx in [
        'vec_stream', 'hashmap_stream', 'btreemap_stream', 'enum_stream', 'ncb_stream', 'crc64_stream', 'enum', 'ncb', 'crc64', 'codec', 'parity', 'iroha_tx'
    ]:
        if name.startswith(pfx):
            return pfx
    # Fallback: first token before underscore
    return name.split('_', 1)[0]

def categories(root: Path):
    cats = {}
    for bench in root.glob('*'):
        est_new = bench / 'new' / 'estimates.json'
        if not est_new.exists():
            continue
        name = bench.name
        cat = cat_name(name)
        res_new = read_estimate(est_new)
        if res_new is None:
            continue
        mean, std, var, median, mad = res_new
        # Optional base for deltas
        est_base = bench / 'base' / 'estimates.json'
        delta = None
        if est_base.exists():
            res_base = read_estimate(est_base)
            if res_base is not None and res_base[0] > 0.0:
                base_mean = res_base[0]
                delta = (mean - base_mean) / base_mean * 100.0
        cats.setdefault(cat, []).append((name, mean, std, var, median, mad, delta))
    for cat in cats:
        cats[cat].sort(key=lambda x: x[0])
    return dict(sorted(cats.items(), key=lambda x: x[0]))

def fmt_ns(x):
    return f"{x*1e9:,.0f}"

def main():
    root = Path('target/criterion')
    if not root.exists():
        print('No criterion output found.')
        return
    cats = categories(root)
    if not cats:
        print('No estimates found.')
        return
    for cat, rows in cats.items():
        print(f"## {cat}")
        print("| Benchmark | Mean (ns) | Std Dev (ns) | Var (ns^2) | Median (ns) | MAD (ns) | Δ vs base (%) |")
        print("|---|---:|---:|---:|---:|---:|---:|")
        for name, mean, std, var, median, mad, delta in rows:
            delta_str = f"{delta:+.2f}%" if delta is not None else "—"
            print(f"| {name} | {fmt_ns(mean)} | {fmt_ns(std)} | {fmt_ns(var)} | {fmt_ns(median)} | {fmt_ns(mad)} | {delta_str} |")
        print()
    # Highlight parallel Stage-1 speedups if present
    par_group = root / 'json_stage1_parallel'
    seq_est = par_group / 'sequential' / 'new' / 'estimates.json'
    par_est = par_group / 'parallel' / 'new' / 'estimates.json'
    if seq_est.exists() and par_est.exists():
        seq = read_estimate(seq_est)
        par = read_estimate(par_est)
        if seq and par and seq[0] > 0:
            speedup = seq[0] / par[0]
            print('## Parallel Stage-1 Speedup')
            print(f'Sequential mean: {fmt_ns(seq[0])} ns')
            print(f'Parallel mean:   {fmt_ns(par[0])} ns')
            print(f'Speedup: {speedup:.2f}×')
    par_group = Path('target/criterion') / 'json_stage1_parallel'
    seq_est = par_group / 'sequential' / 'new' / 'estimates.json'
    par_est = par_group / 'parallel' / 'new' / 'estimates.json'
    if seq_est.exists() and par_est.exists():
        seq = read_estimate(seq_est); par = read_estimate(par_est)
        if seq and par and seq[0] > 0:
            speedup = seq[0] / par[0]
            print('## Parallel Stage-1 Speedup')
            print(f'Sequential mean: {fmt_ns(seq[0])} ns')
            print(f'Parallel mean:   {fmt_ns(par[0])} ns')
            print(f'Speedup: {speedup:.2f}×')

if __name__ == '__main__':
    main()
