# Crash Course: PyO3/maturin, the Python↔Rust Perf Boundary, and ML/NN Survival

**Audience:** a strong production-Rust engineer with no deep ML background, prepping
for an *Applied AI Engineer* role whose real job is: *profile a Python/PyTorch ML
pipeline, find the hot path, and rewrite it in Rust via PyO3 — without breaking
numerical correctness.*

**How to use this:** read §1 for the mental model, work §2–§3 with the repo open
(`src/lib.rs` implements everything discussed), skim §4 to stop being scared of the
ML vocabulary, and study §5 because "walk me through inference optimizations" is the
most likely deep-dive question. §6 is interview Q&A. §7 is a 1-week plan.

---

## Table of contents

1. The mental model of the job
2. PyO3 & maturin depth
3. The perf story on the boundary
4. ML/NN survival guide (for a systems engineer)
5. Inference optimization techniques — the walkthrough
6. Likely interview questions + crisp answers
7. A 1-week plan

---

## 1. The mental model of the job

Fundamental builds **NEXUS**, a "Large Tabular Model" — a big neural network for
structured/tabular business data. The research is in Python/PyTorch. Your job lives
at the seam:

```
   ML researchers (Python/PyTorch)                 You (Rust)
   ┌──────────────────────────────┐   profile      ┌───────────────────────────┐
   │ model, training loop, data    │  ───────────▶  │ find hot path             │
   │ pipeline, inference           │                │ rewrite in Rust via PyO3  │
   │ "it's too slow / too much RAM" │  ◀───────────  │ same numbers, more speed  │
   └──────────────────────────────┘   drop-in .so   └───────────────────────────┘
```

Three competencies, in the order they'll be tested:

1. **The boundary (PyO3/maturin).** Move data across Python↔Rust with no copies,
   release the GIL, return results cleanly. This is the *required* skill and where
   most "good Rust devs" are thin.
2. **Profiling-driven perf.** Prove where time/RAM goes across a *mixed* pipeline,
   then make the right thing faster. Not micro-benchmarking Rust in isolation.
3. **ML literacy.** Enough NN/numerics to talk to researchers and not break
   correctness. You don't need to derive backprop; you need to not be scared of it.

The senior signal throughout: **you can say exactly where a speedup comes from, and
prove the output is still correct.**

---

## 2. PyO3 & maturin depth

PyO3 is the Rust↔CPython bridge. maturin is the build tool that turns your Rust crate
into an installable Python wheel. Everything below is implemented in `src/lib.rs`.

### 2.1 Project anatomy

```
pyo3-tabular-accel/
├── Cargo.toml         # crate-type = ["cdylib"], pyo3 with "extension-module"
├── pyproject.toml     # build-backend = "maturin"
├── src/lib.rs         # #[pymodule] + #[pyfunction]s
└── python/            # benchmark + tests that import the built module
```

The three names that MUST agree: `[lib].name` in Cargo.toml == the `#[pymodule] fn`
name == what Python does `import`. Here that's `tabular_accel`.

- **`crate-type = ["cdylib"]`** — produce a C-ABI dynamic library (`.so`/`.pyd`),
  which is what CPython loads. Not `rlib` (that's for linking into other Rust).
- **`pyo3 features = ["extension-module"]`** — tells PyO3 *not* to link `libpython`.
  The interpreter supplies those symbols at import time. This is what makes wheels
  portable and also why you can't run the cdylib standalone.

### 2.2 The build loop (maturin)

```bash
maturin develop --release   # build + install into the *current* venv (dev loop)
maturin build --release     # produce a wheel in target/wheels/ (for CI/distribution)
```

- Always benchmark with `--release`. A debug PyO3 module can be 10–50× slower and
  will make you draw completely wrong conclusions.
- `maturin develop` is your inner loop: edit `lib.rs` → `maturin develop --release`
  → `pytest`. There's no separate "link" step to think about.

### 2.3 The GIL, and the two most important lines in the whole repo

CPython has a **Global Interpreter Lock**: only one thread runs Python bytecode at a
time. A lifetime-branded token `Python<'py>` (often written `py`) is your proof that
*you currently hold the GIL*. You need it to touch any Python object.

The single most important pattern for perf:

```rust
let result = py.allow_threads(|| {
    // GIL is RELEASED here. Pure-Rust, CPU-bound work.
    // Other Python threads can run; rayon can use all cores.
    heavy_compute()
});
// GIL re-acquired here; safe to build Python objects again.
```

Inside `allow_threads` you **cannot** touch any Python object (the closure must be
`Send` and can't capture the GIL token) — which is exactly the discipline you want:
convert to plain Rust data (or a zero-copy numpy view whose *buffer* is fine to read
without the GIL) *before* you enter, compute, then build the Python result *after*.

If you forget `allow_threads`, your Rust extension holds the GIL for its whole
runtime and **serializes every other Python thread** — a classic "why didn't my
Rust rewrite help the multithreaded server" bug.

### 2.4 The object model: Bound vs Py vs native Rust

Modern PyO3 (0.21+) uses the **Bound API**. Three ways to hold a Python value:

| Type | Meaning | Use when |
|---|---|---|
| `Bound<'py, T>` | a GIL-bound smart pointer to a live Python object | you're actively using it *now* (arguments, return values) |
| `Py<T>` (a.k.a. `PyObject`) | GIL-independent handle; needs `.bind(py)` to use | storing a Python object in a Rust struct / across `allow_threads` |
| plain Rust (`Vec`, `f32`, ndarray view) | not a Python object at all | the actual compute — this is where you want to be |

Rule of thumb: **spend as little time as possible holding `Bound`/`Py`.** Cross the
boundary once on the way in, drop to native Rust, compute, cross once on the way out.

`#[pyfunction]` auto-converts between Python and Rust types via the `FromPyObject`
(in) and `IntoPy` (out) traits — so a Rust `fn(x: f32, names: Vec<String>) -> Vec<f64>`
just works from Python. You only reach for `Bound`/`Py` when you need the Python
object itself (e.g. a numpy array, to read it zero-copy).

### 2.5 numpy interop — the part that actually matters here

`rust-numpy` bridges numpy arrays and the `ndarray` crate. The key types:

- **`PyReadonlyArray2<'py, f32>`** — a *borrowed, read-only, zero-copy* view of a
  caller's numpy array. `.as_array()` gives you an `ndarray::ArrayView2<f32>`
  pointing straight at numpy's buffer. **No copy.** This is your default input type.
- **`PyReadwriteArray2`** — mutable zero-copy view, if you want to write in place.
- Returning: build an `ndarray::Array2<f32>` (you own it) and call
  `.into_pyarray_bound(py)` — this *moves* the buffer into a numpy array with no copy.
  (`.to_pyarray_bound(py)` *copies*; prefer `into_` when you own the data.)

Two correctness traps at this boundary:

1. **dtype must match.** A `PyReadonlyArray2<f32>` extraction *fails* if the caller
   passed `float64`. Decide your contract (we assume `float32`) and either enforce it
   or accept both. Silent dtype mismatches are a top source of "why is it wrong."
2. **Contiguity & strides.** numpy arrays can be non-contiguous (a transpose or slice
   is just a strides change, no copy). `ArrayView` honors strides, so correctness is
   fine — but a strided access pattern destroys cache performance. If you need speed,
   assert C-contiguity (or call `.as_standard_layout()`), and *document* it. Our
   benchmark deliberately feeds C-contiguous arrays; see §3.

### 2.6 Errors across the boundary

Return `PyResult<T>` (= `Result<T, PyErr>`). A returned `Err(PyErr)` becomes a raised
Python exception. Any Rust `Error` type that implements `From<E> for PyErr` works with
`?`. You can construct specific exceptions:

```rust
use pyo3::exceptions::PyValueError;
if ncols == 0 {
    return Err(PyValueError::new_err("empty array"));
}
```

Design point: convert Rust errors into *the Python exception the caller expects*
(`ValueError`, `TypeError`, …), not a generic string, so your extension behaves like
a native Python library.

### 2.7 Panics: why NOT `panic = "abort"`

PyO3 wraps every `#[pyfunction]` body in `catch_unwind` and converts a Rust panic into
a Python `PanicException`. If you set `panic = "abort"` in `Cargo.toml`, that catch
can't run and a panic **kills the entire interpreter process**. So in an extension
module: leave panics as unwind (default), and still prefer returning `Err` over
panicking for expected error paths. (This is why the repo's `Cargo.toml` comments out
`panic = "abort"` even though it's tempting for binary size.)

### 2.8 ABI / distribution (know it exists)

- **abi3** (the "stable ABI", `features = ["abi3-py38"]`) lets one wheel work across
  many CPython versions (3.8+), at the cost of a few dynamic-only APIs. Great for
  shipping; you don't need it for local dev.
- Wheels are platform + Python-version specific unless abi3. CI usually builds a
  matrix (manylinux, macOS x86/arm, Windows) — maturin has GitHub Actions for this.
- You don't need this for the interview, but "I'd ship it as an abi3 manylinux wheel
  from a maturin CI matrix" is a good one-liner that shows production awareness.

---

## 3. The perf story on the boundary

This is the section that separates you from "I know Rust." The competitor is not a
Python `for` loop — it's **vectorized numpy calling optimized C/BLAS**. Beating that
takes understanding *why* you're faster.

### 3.1 The four traps that make a Rust rewrite SLOWER than numpy

1. **Copies at the boundary.** If you accept `Vec<Vec<f32>>` or call
   `.to_pyarray`, you copy the whole array in and/or out. For a 1M×64 array that copy
   can cost more than the compute. Fix: `PyReadonlyArray` in, `into_pyarray` out.
2. **Holding the GIL.** No `allow_threads` → your "parallel" Rust can't overlap with
   any Python thread, and if you spawn rayon while holding the GIL you gain nothing on
   a threaded workload. Fix: wrap compute in `py.allow_threads`.
3. **Losing to BLAS/SIMD.** numpy's elementwise ops and matmuls are already
   SIMD/BLAS-optimized C. A naive scalar Rust loop can be *slower*. You win by doing
   something numpy structurally can't: **fuse multiple passes**, **parallelize across
   cores** (numpy elementwise is single-threaded), or **avoid materializing
   temporaries** (`(x - mu) / sd` in numpy allocates several full-size arrays; you do
   it in one pass into one output).
4. **Per-call FFI + allocation overhead.** Crossing the boundary and allocating the
   output has a fixed cost (~microseconds). On tiny arrays that dominates and numpy
   wins. Amortize by operating on big batches, not per-row calls.

### 3.2 Where a Rust rewrite genuinely wins

- **Fusion:** collapse `(x - mean) / std`'s 3–4 numpy temporaries into one pass, one
  allocation. Less memory traffic = faster on memory-bound ops (most elementwise ops
  are memory-bound, not compute-bound).
- **Parallelism:** numpy elementwise ops don't use threads; `rayon` uses all cores.
  This is the biggest single lever for large arrays. Prove it: run with
  `RAYON_NUM_THREADS=1` and watch the win vanish.
- **Custom logic numpy expresses badly:** groupby-aggregate, target/mean encoding,
  irregular ragged ops, stateful passes — things that in pandas/numpy become many
  temporaries or Python-level loops. This is the real tabular-preprocessing sweet spot.
- **Memory:** streaming/in-place computation avoids doubling RAM on huge arrays — the
  posting explicitly lists "improve memory efficiency."

### 3.3 Reading the benchmark (`python/benchmark.py`)

Honest results you should *expect and be able to explain*:

- 1,000×64: numpy ties or wins → FFI/alloc overhead dominates. Correct, not a bug.
- 1,000,000×32: Rust wins, mostly from parallelism + fusion.
- `RAYON_NUM_THREADS=1`: most of the win disappears → confirms the source is
  parallelism. **Being able to attribute the speedup is the point.**

Golden rule: never present a benchmark you can't decompose into *why*.

### 3.4 Profiling the boundary (the toolkit to name-drop AND use)

You must profile the *mixed* pipeline, not just Rust.

- **`py-spy`** — sampling profiler for a live Python process, *including native
  frames*. `py-spy top --native -p <pid>` or `py-spy record -o out.svg -- python x.py`.
  This is the #1 tool for "where does my Python+Rust program spend time" and shows
  your Rust functions in the flamegraph. Learn this cold.
- **`cargo flamegraph`** / `perf record -g` — for the Rust side's hot instructions
  once you know which function matters.
- **`scalene`** — line-level Python profiler that separately attributes CPU vs native
  time and memory. Great for finding the hot path *before* you rewrite it.
- **`memory_profiler` / `tracemalloc` / massif / heaptrack** — for RAM. The posting
  says "memory efficiency, latency, throughput" — treat those as three separate axes
  you profile independently.
- **`perf stat`** — cycles, cache-misses, IPC. Use to show an op is memory-bound
  (high cache-miss, low IPC) which *justifies* a fusion/layout fix over a compute one.
- Rust micro-bench: **`criterion`** for the pure-Rust kernel in isolation, `divan` as
  a lighter alternative.

Methodology to state out loud in an interview: **profile first (find the hot path and
whether it's CPU/memory/latency-bound), form a hypothesis about *why*, change one
thing, re-measure, confirm the win came from where you predicted.** Never optimize by
vibes.

### 3.5 The kernel-level levers (once the right thing is identified)

- **Access pattern / cache:** iterate the contiguous dimension (rows on C-contiguous).
  A column-wise pass on row-major data strides through memory and tanks. (Our
  standardize does a row-major pass 1 on purpose.)
- **SIMD:** `-C target-cpu=native`, autovectorization (keep loops simple, no early
  exits), or `std::simd`/`wide` for explicit vectors. Usually try autovec first.
- **Parallelism:** `rayon` (`par_iter`, `par_chunks_mut`) for data parallelism.
  Chunk by rows so each task owns a disjoint output slice (no locking, no false
  sharing across cache lines when chunks are large).
- **Allocation:** allocate the output once; avoid per-iteration `Vec`s; reuse buffers
  across calls if you keep state.
- **Precision:** accumulate reductions in `f64` even for `f32` data (see §4.5). Cheap,
  and it's what keeps you correct.

---

## 4. ML/NN survival guide (for a systems engineer)

You don't need to *train* models. You need to (a) not be intimidated, (b) know where
the compute is, (c) not break the numerics. Here's the whole map.

### 4.1 A neural network is just typed linear algebra

A feed-forward layer is: `y = activation(x @ W + b)`.

- `x`: input, shape `(batch, in_features)`.
- `W`: weight matrix `(in_features, out_features)`, `b`: bias `(out_features,)`.
- `@`: matrix multiply (the dominant cost — this is where BLAS/GPUs earn their keep).
- `activation`: a cheap elementwise nonlinearity (ReLU = `max(0,x)`, GELU, etc.).

Stack a few of these and you have an MLP. **95% of the FLOPs are matmuls.** Everything
else (activations, norms, softmax) is cheap elementwise/reduction work — but it's
often *memory-bound* and can dominate wall-clock if not fused. That gap (cheap in
FLOPs, expensive in memory traffic) is exactly where a Rust/kernel person adds value.

### 4.2 Training vs inference (know the difference cold)

- **Forward pass:** compute outputs from inputs (this is inference).
- **Loss:** a number measuring wrongness vs the target.
- **Backward pass (backprop):** compute gradients of the loss w.r.t. every weight via
  the chain rule. Roughly **2× the cost of the forward pass** and needs to *stash
  activations* from the forward pass → this is why training is memory-hungry.
- **Optimizer step:** nudge weights down the gradient (SGD, Adam).

**Inference = forward pass only.** No gradients, no activation stashing, weights are
frozen. That unlocks a pile of optimizations (quantization, fusion, no autograd
bookkeeping) — see §5.

### 4.3 The training loop, and where time goes

```python
for batch in dataloader:          # (A) DATA: load, decode, augment, collate
    x, y = batch
    pred = model(x)               # (B) FORWARD: matmuls dominate
    loss = loss_fn(pred, y)
    loss.backward()               # (C) BACKWARD: ~2x forward
    optimizer.step()              # (D) OPTIMIZER: elementwise over params
    optimizer.zero_grad()
```

Where a Rust person is asked to help, in likelihood order:

- **(A) the data pipeline.** Very often the real bottleneck: parsing/decoding,
  feature engineering, tokenization/encoding, collation. CPU-bound, Python-loop-heavy,
  perfect for a Rust rewrite that feeds the GPU without starving it. On tabular data
  this is preprocessing (encoding, normalization, missing-value handling, joins).
- **(B/C)** custom operators / kernels when a specific op is slow (see §5.7).
- **(D)** rarely.

If asked "the GPU is only 40% utilized, what's wrong?" the textbook answer is **the
data pipeline can't keep up** — CPU preprocessing/IO is starving the GPU. That's your
territory.

### 4.4 Tabular models specifically (the NEXUS domain)

Tabular = rows × columns of mixed types (numeric, categorical, dates). Historically
gradient-boosted trees (XGBoost/LightGBM) beat neural nets here; recent work closes the
gap. Vocabulary you should recognize:

- **Categorical embeddings / learned embeddings:** map each category value to a learned
  vector (like word embeddings). A big **embedding lookup table**; lookups are
  gather-heavy, memory-bound — a systems concern.
- **Feature preprocessing:** normalization/standardization (our `standardize_columns`),
  quantile/rank transforms, missing-value imputation, target encoding. CPU-heavy,
  Rust-friendly.
- **Transformers over features/rows:** attention treating columns (or rows) as tokens.
  Attention cost is ~O(n²) in sequence length → a known optimization target.
- **Tree/NN hybrids:** models mixing decision-tree structure with differentiable NN
  parts. You don't need to build them; recognize the term.
- **"Large Tabular Model" (NEXUS):** a big pretrained model for tabular data, à la a
  foundation model but for tables. The scaling → the perf work → the reason they're
  hiring you.

### 4.5 Numerical stability — the correctness landmines

The posting says "numerical stability, correctness, reproducibility." Know these:

- **Softmax overflow:** `exp(x)` overflows for x ≳ 88 in f32. Always subtract the row
  max first: `softmax(x) = softmax(x - max(x))` (mathematically identical, numerically
  safe). Implemented in `row_softmax`. Also see **log-sum-exp**: `log(Σ exp(x_i)) =
  m + log(Σ exp(x_i - m))`.
- **Catastrophic cancellation:** subtracting near-equal large numbers loses precision.
  In variance, the naive `E[x²] - E[x]²` cancels badly → use **Welford's online
  algorithm** (what `standardize_columns` does).
- **Accumulate in higher precision:** summing many f32s accumulates rounding error;
  accumulate the *reduction* in f64 even when inputs/outputs are f32. Cheap insurance;
  it's why our stats match numpy within 1e-4. Flip it to f32 and the test fails — a
  deliberate lesson in `test_correctness.py`.
- **Reproducibility:** floating-point addition is **not associative**, so a *different
  reduction order* gives *different bits*. Parallel/nondeterministic reductions (random
  atomic add order, GPU nondeterminism, changing thread counts) break bitwise
  reproducibility. Our parallel section keeps each row's reduction sequential and
  independent → deterministic (`test_reproducibility`). If a researcher says "results
  changed after your rewrite," reduction order is suspect #1.
- **f32 vs f64 vs bf16/fp16:** know the tradeoff — lower precision = less memory + more
  throughput (and SIMD width) but more rounding. bf16 keeps f32's *range* (same
  exponent bits) with less mantissa, which is why training uses it.

---

## 5. Inference optimization techniques — the walkthrough

"Walk me through how you'd speed up inference" is the single most likely deep-dive.
Here's the structured answer, from methodology down to kernels. Lead with methodology;
never jump to a trick.

### 5.0 Methodology first (say this before any technique)

1. **Define the objective.** Latency (p50/p99 of one request), throughput
   (predictions/sec), or memory footprint? They optimize differently and often trade
   off. Pin the target and the SLA.
2. **Profile to find the bottleneck** (§3.4) and classify it: compute-bound (matmul,
   high IPC) vs memory-bound (elementwise/gather, high cache-miss) vs
   overhead-bound (Python glue, framework dispatch, tiny batches) vs IO/data-bound.
3. **Match the technique to the bottleneck.** Applying a compute trick to a
   memory-bound op wastes your week. The classification *is* the skill.
4. **Change one thing, re-measure, verify correctness within tolerance.**

### 5.1 Batching (usually the biggest, cheapest win)

Process many inputs in one call so the fixed per-call overhead (Python dispatch, kernel
launch, FFI) amortizes and matmuls hit efficient sizes.

- **Static batching:** fixed batch size offline/high-throughput.
- **Dynamic batching:** a server collects requests for a few ms and runs them as one
  batch — trades a little latency for large throughput gains. Standard in serving.
- Tradeoff: bigger batch → higher throughput + more latency + more memory. Tune to SLA.

### 5.2 Reduce precision: quantization & low precision

Run inference in fewer bits than training.

- **fp16 / bf16:** ~2× memory bandwidth and often ~2× throughput vs f32, minimal
  accuracy loss. Usually the first reach.
- **int8 quantization:** map weights/activations to 8-bit ints with a scale (and
  optional zero-point). ~4× smaller, big speedups on int8-capable hardware.
  - **Post-training quantization (PTQ):** quantize a trained model, calibrate scales on
    a small dataset. Cheap, sometimes small accuracy hit.
  - **Quantization-aware training (QAT):** simulate quantization during training for
    better accuracy. More expensive.
- Correctness caveat: quantization *changes outputs slightly* — you validate accuracy,
  not bit-equality. Know the dequant math (`x ≈ scale * (q - zero_point)`).

### 5.3 Operator fusion

Combine adjacent ops into one kernel so intermediates never hit main memory.
`Linear → BiasAdd → ReLU` as one pass reads inputs once and writes outputs once instead
of materializing two temporaries. Huge for **memory-bound** elementwise chains — the
same lever as §3.2 "fusion," now inside the model graph. This is what `torch.compile`,
TensorRT, and XLA do automatically; a hand-written fused Rust/CUDA kernel does it
manually when the compiler won't.

### 5.4 Graph-level compilation & export

Get out of eager Python and let a compiler optimize the whole graph.

- **`torch.compile`** (TorchInductor): JIT-fuses and optimizes a PyTorch model with one
  line; often free 1.3–2×.
- **ONNX + ONNX Runtime:** export the model to a portable graph, run it in an optimized
  C++ runtime (does fusion, constant folding, layout opt). Great for deploying without
  Python.
- **TensorRT** (NVIDIA): aggressive GPU-specific fusion + quantization; top latency on
  NVIDIA hardware.
- **TorchScript:** older trace/script export; still seen in the wild.

### 5.5 Kill Python/framework overhead

On small models or small batches, the dominant cost is *dispatch overhead*, not math:
Python interpreter, autograd bookkeeping (use `torch.no_grad()` / `inference_mode()`),
per-op framework dispatch, tiny kernel launches. Fixes: bigger batches (§5.1), graph
compile/export (§5.4), or **rewrite the glue/pre/post-processing in Rust** (your PyO3
lane). This is where your extension directly moves the needle even without touching the
model.

### 5.6 Memory & data movement

- **Layout:** ensure contiguous, cache-friendly layouts; avoid implicit transposes/
  copies between ops. (Same §3.5 lesson, model-scale.)
- **Zero-copy handoff:** move tensors between Python and your Rust code without copying
  (`PyReadonlyArray`, the DLPack/`__array_interface__` protocols, pinned host memory
  for CPU↔GPU).
- **KV cache** (autoregressive/transformer decoding): cache past keys/values so each new
  token doesn't recompute attention over the whole prefix. Less relevant for one-shot
  tabular prediction, but know the term.
- **Weight sharing / smaller dtype for weights:** cuts the memory-bandwidth bill that
  dominates low-batch inference.

### 5.7 Custom operators / kernels (the deepest lane)

When a specific op is slow and no library does it well, write the kernel:

- **CPU:** a Rust/PyO3 extension (this repo) with SIMD + rayon, or a custom C++ op.
- **GPU:** a custom CUDA/Triton kernel registered as a PyTorch custom operator.
- **Extend PyTorch** via `torch.autograd.Function` (define forward + backward) or the
  C++/CUDA extension API. The posting's "PyTorch internals / custom operator
  development" nice-to-have points straight here.

Say plainly: custom kernels are the *last* resort after batching, precision, fusion,
and compilation — highest effort, highest maintenance, only when profiling proves a
specific op is the wall.

### 5.8 The one-paragraph verbal answer (memorize the shape)

> "First I'd pin the objective — latency vs throughput vs memory — and profile to find
> and *classify* the bottleneck: compute-, memory-, overhead-, or data-bound. If it's
> overhead/small-batch, I batch (dynamic batching on a server) and cut Python/autograd
> overhead with `inference_mode` and a compiled/exported graph. If it's compute-bound,
> lower precision (bf16, then int8 with calibration) and lean on fused BLAS/GPU kernels.
> If it's memory-bound, fuse elementwise chains and fix layout so intermediates don't
> round-trip to memory. If a specific op is still the wall and no library does it well,
> I write a custom kernel — a PyO3/rayon/SIMD CPU op or a Triton/CUDA GPU op — and
> validate outputs against the reference within tolerance. Every step: measure, change
> one thing, re-measure, confirm the win came from where I predicted and the numbers
> still match."

---

## 6. Likely interview questions + crisp answers

**Q: Why is your Rust extension sometimes slower than numpy?**
Small arrays: FFI + allocation overhead dominates and numpy's C loop is already
optimal. Rust wins at scale via parallelism (numpy elementwise is single-threaded) and
fusion (one pass/one alloc vs numpy's temporaries). I can prove the source by toggling
`RAYON_NUM_THREADS`.

**Q: What does `allow_threads` do and why does it matter?**
Releases the GIL for a pure-Rust CPU section so other Python threads run and rayon
actually parallelizes. Forgetting it serializes the whole process on the GIL — a common
reason a Rust rewrite doesn't help a threaded server.

**Q: How do you pass a numpy array to Rust without copying?**
`PyReadonlyArray2<f32>` → `.as_array()` gives a zero-copy `ndarray` view over numpy's
buffer. Return by moving an owned `Array2` out via `into_pyarray_bound`. Watch dtype
(extraction fails on mismatch) and contiguity (strided = correct but cache-hostile).

**Q: The GPU is at 40% utilization. What's wrong?**
Almost always the data pipeline starving it — CPU preprocessing/IO can't feed the GPU.
Profile the dataloader; parallelize/prefetch; rewrite the hot preprocessing in Rust.

**Q: Softmax is producing NaNs. Why?**
`exp` overflowing in f32 (inputs ≳ 88). Subtract the row max before exp — identical
result, no overflow. Same idea generalizes to log-sum-exp.

**Q: Your rewrite changed the model's outputs slightly. Is it a bug?**
Depends on tolerance. Reduction order changes bits because FP addition isn't
associative; low-precision accumulation adds error. I validate within a tolerance
(`allclose`), accumulate reductions in f64, and keep reductions deterministic for
reproducibility. Only bit-exact when the algorithm/precision/order is identical.

**Q: How would you approach optimizing an inference pipeline you've never seen?**
The §5.8 paragraph: objective → profile → classify bottleneck → matched technique →
measure/verify. Never optimize by intuition.

**Q: When would you NOT rewrite in Rust?**
When the op is already BLAS/GPU-bound (you won't beat cuBLAS), when it's a small share
of runtime (Amdahl), or when a graph compiler / better batching gets the win at a
fraction of the maintenance cost. Rust is for CPU-bound custom logic, data pipelines,
and glue — not for out-muscling optimized matmul libraries.

---

## 7. A 1-week plan

**Day 1–2 — Own the repo.** Build it (`maturin develop --release`), run tests and the
benchmark, then *break things to learn*: remove `allow_threads` and re-benchmark;
switch the Welford accumulators to f32 and watch `test_correctness` fail; feed a
`float64` array and see the extraction error; feed a transposed (non-contiguous) array
and watch it slow down. Each break teaches one §2/§3 point you can now speak to.

**Day 3 — Profile for real.** `pip install py-spy scalene`. Profile `benchmark.py` with
`py-spy record --native` and `scalene`; find your Rust frames in the flamegraph; use
`perf stat` to show `standardize` is memory-bound. Screenshot a flamegraph for the repo
README — visual proof of the "profiling" nice-to-have.

**Day 4 — Add one tabular-flavored kernel.** Implement something numpy/pandas do badly:
a groupby-mean (target encoding) or a parallel quantile transform. This shows the
sweet-spot from §3.2 better than the two demo kernels and is very on-domain for NEXUS.

**Day 5 — ML literacy pass.** Re-read §4 and §5 until §5.8 is something you can say
cold. Skim: the PyTorch custom-op tutorial (autograd.Function), one blog on int8
quantization, one on `torch.compile`. You're aiming for fluency in the vocabulary, not
mastery.

**Day 6 — Package the story.** Polish the README with your benchmark table + flamegraph
+ a "why the speedup" paragraph and a "correctness/reproducibility" paragraph. This repo
*is* your cover letter — a DeepMind-alumni team reads code before prose.

**Day 7 — Application.** Rewrite your CV bullets in their language ("profile Python ML
pipelines; rewrite hot paths in Rust via PyO3/maturin with zero-copy numpy interop;
preserve numerical correctness"), link the repo, and keep the tokio/streams stuff to a
single "async Rust" line. Apply.

---

### Appendix: the mental checklist for any boundary rewrite

- [ ] Zero-copy in (`PyReadonlyArray`), one alloc out (`into_pyarray`).
- [ ] GIL released around the compute (`allow_threads`).
- [ ] Reductions accumulated in f64; softmax/logsumexp max-shifted.
- [ ] Access pattern follows memory layout (contiguous dim inner).
- [ ] Parallel tasks own disjoint output slices; deterministic reductions.
- [ ] Correctness pinned to the reference within tolerance; reproducible across runs.
- [ ] Benchmarked `--release` vs *vectorized* numpy; speedup attributable to a cause.
- [ ] Errors surface as the right Python exception; panics stay as unwind.
