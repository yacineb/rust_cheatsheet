"""End-to-end benchmark: tabular_accel (Rust) vs numpy/scipy baselines.

Run after `maturin develop --release`:

    python python/benchmark.py

The numbers you should care about are NOT "Rust is faster" in the abstract.
Care about:
  * Does Rust beat *vectorized numpy* (the real competitor, not a Python loop)?
  * Where does the win come from — parallelism, avoided copies, or fusion?
  * At what array size does the boundary/FFI overhead stop mattering?
"""

import time

import numpy as np

import tabular_accel as ta


def bench(fn, *args, iters=50, warmup=5):
    for _ in range(warmup):
        fn(*args)
    t0 = time.perf_counter()
    for _ in range(iters):
        out = fn(*args)
    dt = (time.perf_counter() - t0) / iters
    return dt, out


# ---- numpy baselines (vectorized — the honest competitor) ----
def np_standardize(x):
    mu = x.mean(axis=0)
    sd = x.std(axis=0)
    sd = np.where(sd < 1e-8, 1.0, sd)
    return ((x - mu) / sd).astype(np.float32)


def np_softmax(x):
    m = x.max(axis=1, keepdims=True)
    e = np.exp(x - m)
    return (e / e.sum(axis=1, keepdims=True)).astype(np.float32)


def report(name, rust_dt, np_dt):
    speedup = np_dt / rust_dt
    print(f"  {name:24s}  numpy={np_dt*1e3:8.3f} ms   rust={rust_dt*1e3:8.3f} ms   speedup={speedup:5.2f}x")


if __name__ == "__main__":
    print(f"threads available to rayon: set RAYON_NUM_THREADS to control\n")
    for (nrows, ncols) in [(1_000, 64), (100_000, 64), (1_000_000, 32)]:
        print(f"shape = ({nrows:,}, {ncols})")
        # C-contiguous float32 — the layout the Rust side assumes is cheap.
        x = np.random.randn(nrows, ncols).astype(np.float32)

        r_dt, _ = bench(ta.standardize_columns, x)
        n_dt, _ = bench(np_standardize, x)
        report("standardize_columns", r_dt, n_dt)

        r_dt, _ = bench(ta.row_softmax, x)
        n_dt, _ = bench(np_softmax, x)
        report("row_softmax", r_dt, n_dt)
        print()
