//! tabular_accel — a *minimal but realistic* PyO3 extension that accelerates two
//! hot paths you find in tabular ML systems:
//!
//!   1. `standardize_columns` — feature preprocessing (z-score per column).
//!      Demonstrates: numerically-stable Welford stats accumulated in f64,
//!      two-pass over a zero-copy numpy view, rayon parallelism on pass 2.
//!
//!   2. `row_softmax` — an inference primitive (softmax over each row).
//!      Demonstrates: the max-shift numerical-stability trick, embarrassingly
//!      parallel rows via `rayon`, GIL released for the whole compute.
//!
//! The point of this crate is not the maths — numpy already does both. The point
//! is to show the *boundary* done correctly: zero-copy in, GIL released, parallel
//! compute, one allocation out. See COURSE.md for the full walkthrough.

use numpy::ndarray::{Array2, ArrayView2};
use numpy::{IntoPyArray, PyArray2, PyReadonlyArray2};
use pyo3::prelude::*;
use rayon::prelude::*;

/// Column-wise standardization: `(x - mean) / std` computed per column.
///
/// - Input is taken as a **read-only, zero-copy** view of the caller's numpy
///   array (`PyReadonlyArray2`). No data is copied on the way in.
/// - Statistics are accumulated in **f64** even though the data is f32 — this is
///   the cheap, correct default that avoids catastrophic cancellation and lets
///   us match numpy within tight tolerance. (COURSE.md §4.5)
/// - The whole computation runs inside `py.allow_threads`, so the GIL is
///   released and other Python threads can run while rayon uses all cores.
#[pyfunction]
fn standardize_columns<'py>(
    py: Python<'py>,
    x: PyReadonlyArray2<'py, f32>,
) -> PyResult<Bound<'py, PyArray2<f32>>> {
    // Zero-copy view; its lifetime is tied to the local `x` binding, not `'py`.
    let x: ArrayView2<'_, f32> = x.as_array();
    let (nrows, ncols) = x.dim();

    // Everything CPU-bound happens with the GIL released.
    let out_vec: Vec<f32> = py.allow_threads(|| {
        // --- Pass 1: streaming mean/variance via Welford, in f64. ---
        // We iterate ROW by row (cache-friendly on C-contiguous arrays) and
        // update all columns' running stats in lock-step.
        let mut mean = vec![0f64; ncols];
        let mut m2 = vec![0f64; ncols];
        let mut count = 0f64;
        for row in x.rows() {
            count += 1.0;
            for (j, &v) in row.iter().enumerate() {
                let v = v as f64;
                let delta = v - mean[j];
                mean[j] += delta / count;
                m2[j] += delta * (v - mean[j]);
            }
        }
        // Population std (ddof = 0) to match numpy's default.
        let inv_std: Vec<f64> = m2
            .iter()
            .map(|&s| {
                let var = if count > 0.0 { s / count } else { 0.0 };
                let std = var.sqrt();
                // Guard constant columns: divide by 1.0 instead of 0.0.
                if std < 1e-8 {
                    1.0
                } else {
                    1.0 / std
                }
            })
            .collect();

        // --- Pass 2: standardize, parallel over rows. ---
        let mut out = vec![0f32; nrows * ncols];
        out.par_chunks_mut(ncols).enumerate().for_each(|(i, row_out)| {
            let row = x.row(i);
            for j in 0..ncols {
                row_out[j] = ((row[j] as f64 - mean[j]) * inv_std[j]) as f32;
            }
        });
        out
    });

    // One allocation crosses back: reshape the flat Vec into (nrows, ncols) and
    // hand ownership to numpy WITHOUT copying (`into_pyarray_bound` moves it).
    let arr = Array2::from_shape_vec((nrows, ncols), out_vec)
        .expect("shape matches by construction");
    Ok(arr.into_pyarray_bound(py))
}

/// Numerically-stable softmax applied independently to each row.
///
/// Each row is independent → embarrassingly parallel. We subtract the row max
/// before `exp` to avoid overflow (`exp(large)` → inf). GIL released throughout.
#[pyfunction]
fn row_softmax<'py>(
    py: Python<'py>,
    x: PyReadonlyArray2<'py, f32>,
) -> PyResult<Bound<'py, PyArray2<f32>>> {
    let x: ArrayView2<'_, f32> = x.as_array();
    let (nrows, ncols) = x.dim();

    let out_vec: Vec<f32> = py.allow_threads(|| {
        let mut out = vec![0f32; nrows * ncols];
        out.par_chunks_mut(ncols).enumerate().for_each(|(i, row_out)| {
            let row = x.row(i);

            // 1) max-shift for numerical stability.
            let m = row.iter().copied().fold(f32::NEG_INFINITY, f32::max);

            // 2) exp(x - m), accumulate denominator.
            let mut sum = 0f32;
            for (o, &v) in row_out.iter_mut().zip(row.iter()) {
                let e = (v - m).exp();
                *o = e;
                sum += e;
            }

            // 3) normalize. Guard the all-(-inf)/empty edge case.
            let inv = if sum > 0.0 { 1.0 / sum } else { 0.0 };
            for o in row_out.iter_mut() {
                *o *= inv;
            }
        });
        out
    });

    let arr = Array2::from_shape_vec((nrows, ncols), out_vec)
        .expect("shape matches by construction");
    Ok(arr.into_pyarray_bound(py))
}

/// The module name MUST equal the `[lib] name` in Cargo.toml and the name Python
/// imports (`import tabular_accel`).
#[pymodule]
fn tabular_accel(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(standardize_columns, m)?)?;
    m.add_function(wrap_pyfunction!(row_softmax, m)?)?;
    Ok(())
}
