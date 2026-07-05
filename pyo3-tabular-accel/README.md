# tabular_accel

A small, **realistic** PyO3 + maturin + rayon + numpy extension that accelerates
two hot paths from tabular ML systems, built as a portfolio piece for the
Fundamental "Applied AI Engineer" role. It exists to demonstrate the thing that
job is actually about: **taking a Python/numpy hot path and rewriting it in Rust
across the boundary, correctly and fast.**

Read [`COURSE.md`](./COURSE.md) alongside this — it's the crash course this repo
was built to illustrate.

## What it shows (the interview talking points)

| Technique | Where |
|---|---|
| Zero-copy numpy input (`PyReadonlyArray2`) | `src/lib.rs` |
| GIL released for all compute (`py.allow_threads`) | `src/lib.rs` |
| Data parallelism (`rayon::par_chunks_mut`) | `src/lib.rs` |
| Numerical stability (f64 Welford, softmax max-shift) | `src/lib.rs` |
| One allocation back, no copy (`into_pyarray_bound`) | `src/lib.rs` |
| Correctness pinned to numpy + reproducibility test | `python/test_correctness.py` |
| Honest benchmark vs *vectorized* numpy | `python/benchmark.py` |
| Release profile that makes the benchmark fair (LTO, cgu=1) | `Cargo.toml` |

## Quickstart

```bash
# 1. Toolchain
pip install maturin
# (or: pipx install maturin)

# 2. Build the extension into the current venv, optimized
cd pyo3-tabular-accel
python -m venv .venv && source .venv/bin/activate
pip install numpy scipy pytest
maturin develop --release      # compiles Rust, installs `tabular_accel` into the venv

# 3. Verify correctness
pytest python/test_correctness.py -q

# 4. Benchmark vs numpy
python python/benchmark.py

# Control parallelism to show the scaling story:
RAYON_NUM_THREADS=1 python python/benchmark.py   # single-threaded Rust
RAYON_NUM_THREADS=8 python python/benchmark.py
```

## What honest results look like

- On **small** arrays numpy may win or tie — FFI + allocation overhead dominates
  and numpy's C loops are already excellent. This is the point, not a failure.
- On **large** arrays the rayon version pulls ahead because numpy's elementwise
  ops are single-threaded and materialize temporaries; you fused the passes and
  used all cores.
- With `RAYON_NUM_THREADS=1` most of the win disappears — proving the win is
  *parallelism*, not magic. Being able to say exactly where your speedup comes
  from is the senior signal.

See `COURSE.md` §3 for how to read and defend these numbers.
