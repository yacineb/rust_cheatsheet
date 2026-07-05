"""Correctness / reproducibility gate.

This is the part most candidates skip and the job posting explicitly asks for:
"Maintain numerical stability, correctness, and reproducibility."

We pin the Rust output against a numpy reference within tolerance. The tolerance
is tight (rtol=1e-4) *because* we accumulate stats in f64 on the Rust side — if
you switch the Welford accumulators to f32 you will watch this test fail, which
is the whole lesson.

Run:  pytest python/test_correctness.py -q
"""

import numpy as np
import pytest

import tabular_accel as ta


def np_standardize(x):
    mu = x.mean(axis=0)
    sd = x.std(axis=0)
    sd = np.where(sd < 1e-8, 1.0, sd)
    return ((x - mu) / sd).astype(np.float32)


def np_softmax(x):
    m = x.max(axis=1, keepdims=True)
    e = np.exp(x - m)
    return (e / e.sum(axis=1, keepdims=True)).astype(np.float32)


@pytest.mark.parametrize("shape", [(1, 8), (1000, 64), (50_000, 16)])
def test_standardize_matches_numpy(shape):
    rng = np.random.default_rng(0)
    x = rng.standard_normal(shape).astype(np.float32)
    got = ta.standardize_columns(x)
    want = np_standardize(x)
    np.testing.assert_allclose(got, want, rtol=1e-4, atol=1e-4)


def test_standardize_constant_column():
    # A constant column has std 0; we must not divide by zero → output all zeros.
    x = np.ones((100, 4), dtype=np.float32)
    got = ta.standardize_columns(x)
    assert np.all(np.isfinite(got))
    np.testing.assert_allclose(got, np.zeros_like(got), atol=1e-6)


@pytest.mark.parametrize("shape", [(1, 10), (1000, 64), (50_000, 16)])
def test_softmax_matches_scipy(shape):
    rng = np.random.default_rng(1)
    x = rng.standard_normal(shape).astype(np.float32)
    got = ta.row_softmax(x)
    want = np_softmax(x)
    np.testing.assert_allclose(got, want, rtol=1e-4, atol=1e-5)
    # rows must sum to 1
    np.testing.assert_allclose(got.sum(axis=1), np.ones(shape[0]), rtol=1e-4)


def test_softmax_overflow_stability():
    # Without the max-shift trick this overflows to nan/inf.
    x = np.array([[1000.0, 1001.0, 1002.0]], dtype=np.float32)
    got = ta.row_softmax(x)
    assert np.all(np.isfinite(got))
    np.testing.assert_allclose(got.sum(axis=1), [1.0], rtol=1e-5)


def test_reproducibility():
    # Same input → bitwise-identical output across runs (no data races, no
    # nondeterministic reductions in the parallel section).
    rng = np.random.default_rng(2)
    x = rng.standard_normal((10_000, 32)).astype(np.float32)
    a = ta.standardize_columns(x)
    b = ta.standardize_columns(x)
    assert np.array_equal(a, b)
