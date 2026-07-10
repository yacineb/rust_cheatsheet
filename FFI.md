# Rust FFI: crossing the boundary — quirks, API design, and language targets

> A companion to the [README cheat-sheet](./README.md) and [WASM.md](./WASM.md). Where WASM is about the
> *browser* boundary, this is about the **native** one: exposing Rust to C, C++, Swift, Kotlin/Java, and
> anything else that speaks the C ABI — and consuming C from Rust.
>
> The one-line mental model: **the C ABI is the only stable contract Rust has.** Rust's own type system —
> generics, lifetimes, trait objects, `String`, `Vec`, enums-with-data — *does not exist at the ABI level*,
> and its layout is deliberately unstable (the compiler reorders fields and changes enum layouts between
> builds). So *every* cross-language boundary is a **C-shaped waist**: `#[repr(C)]` data and `extern "C"`
> functions in the middle, rich Rust on one side, a rich foreign language on the other. Design the waist
> deliberately; everything else follows. It's the same boundary discipline as **pyo3** (Rust↔Python),
> pointed at C, Swift, and Kotlin instead.

---

## Table of contents

0. [Mental model: the C ABI is the only stable contract](#0-mental-model-the-c-abi-is-the-only-stable-contract)
1. [FFI-safe types and the quirks that bite](#1-ffi-safe-types-and-the-quirks-that-bite)
2. [Strings, slices, ownership, and memory](#2-strings-slices-ownership-and-memory)
3. [Panics, unwinding, and error handling](#3-panics-unwinding-and-error-handling)
4. [API design: the opaque-handle, C-shaped waist](#4-api-design-the-opaque-handle-c-shaped-waist)
5. [Callbacks, threads, and global state](#5-callbacks-threads-and-global-state)
6. [Building and generating bindings](#6-building-and-generating-bindings)
7. [Consuming from C and C++](#7-consuming-from-c-and-c)
8. [Android: JNI and Kotlin/Java](#8-android-jni-and-kotlinjava)
9. [Swift, iOS, and macOS](#9-swift-ios-and-macos)
10. [Kotlin: JVM and Multiplatform Native](#10-kotlin-jvm-and-multiplatform-native)
11. [The high-level path: uniffi and swift-bridge](#11-the-high-level-path-uniffi-and-swift-bridge)
12. [Checklist, crates, further reading](#12-checklist-crates-further-reading)

---

## 0. Mental model: the C ABI is the only stable contract

Three facts everything else derives from:

- **Rust has no stable ABI.** Struct field order, enum tags, and calling conventions can change between
  compiler versions and even builds. You may *never* pass a plain Rust type across the boundary by value.
  The stable subset is: `extern "C"` functions + `#[repr(C)]`/`#[repr(transparent)]` types + primitive
  scalars + raw pointers. Everything richer must be **reduced to something C-shaped, then reconstructed on
  the other side.**
- **The boundary is unsafe by construction.** The foreign side can't see Rust's ownership, lifetimes, or
  `Send`/`Sync`; the compiler can't check the other side of the call. FFI is where *you* uphold the
  invariants the borrow checker normally upholds for you — so the goal of good FFI design is to **shrink the
  unsafe surface to a thin, auditable shell** and keep the safe Rust core untouched.
- **Two directions, two tools.** *Exporting* Rust (build a `cdylib`/`staticlib`, generate a C header with
  `cbindgen`) vs. *importing* C (declare `extern "C"` blocks, or generate them with `bindgen`). Most real
  projects do both.

§1–§3 are the quirks (types, memory, panics), §4–§5 are API design, §6 is build & codegen, §7–§10 are the
consumer languages, §11 is "don't hand-roll it."

---

## 1. FFI-safe types and the quirks that bite

An **FFI-safe** type has a guaranteed, C-compatible layout. The compiler warns (`improper_ctypes`) on many
violations, but not all — and the warning is easy to `#[allow]` into a footgun.

| Type | FFI-safe? | Notes / what bites |
|---|---|---|
| `u8..u128`, `i8..i128`, `f32/f64` | ✅ | use `core::ffi::c_int`/`c_char`/… (or `libc`) to match C's platform-dependent widths |
| `bool` | ✅ | 1 byte, `0`/`1`; matches C `_Bool`. **Any other bit pattern is UB** |
| `char` | ⚠️ | 4-byte Unicode scalar — **not** C `char`. For a C character use `c_char` (`i8`/`u8`) |
| `#[repr(C)] struct` | ✅ | C field order + padding. **Default `repr(Rust)` layout is unspecified** — must annotate |
| `#[repr(transparent)] struct` | ✅ | same ABI as its single field — ideal for newtype handles over a pointer/int |
| `#[repr(C)]` / `#[repr(i32)]` fieldless enum | ✅ | defined discriminant type. **An out-of-range value read from C is instant UB** |
| plain `enum` with data | ❌ | tagged-union layout is unspecified; use `#[repr(C)]`/`#[repr(C, i32)]` or a manual union |
| `*const T` / `*mut T` | ✅ | the currency of FFI |
| `&T` / `&mut T` | ✅ | non-null; upholding aliasing/lifetime is *your* job |
| `Option<&T>`, `Option<Box<T>>`, `Option<NonNull<T>>`, `Option<extern fn>` | ✅ | **nullable pointer** via niche: `None` == `NULL`. The idiomatic FFI-safe "maybe pointer" |
| `extern "C" fn(..)` pointer | ✅ | callbacks; pair with a `void*` context (§5) |
| `str`, `&[T]` | ❌ | **fat pointers** (ptr + len) with no stable layout — pass as separate `(ptr, len)` |
| `String`, `Vec<T>`, `Box<[T]>` | ❌ | internal layout not guaranteed — decompose (§2), never pass by value |
| `Box<dyn Trait>`, `&dyn Trait` | ❌ | fat pointer (data + vtable) — not representable in C |
| `usize`/`isize` | ✅ | maps to `size_t`/`ssize_t`; width is platform-dependent (footgun on 32-bit consumers) |
| zero-sized types | ⚠️ | C has no ZSTs; avoid at the boundary |

The three mechanisms you'll reach for constantly:

- **`extern "C"`** — the C calling convention (also `extern "system"` for JNI/Win32, `extern "C-unwind"`
  when unwinding must cross, §3).
- **`#[no_mangle]`** — keep the symbol name verbatim so the host linker can find it (mangled names won't be).
- **`#[repr(C)]`** — predictable, C-compatible layout. Two rules that prevent most layout bugs:
  - **`#[repr(C)]` everything that crosses.** `repr(Rust)` is free to reorder for niche-packing; a struct or
    discriminant enum shared with C *must* be `#[repr(C)]` (or `transparent`).
  - **`#[repr(C, packed)]` is a loaded gun.** It drops padding to match a wire/C struct, but taking a
    reference to a misaligned field is **UB**. Read/write packed fields with `ptr::read_unaligned` /
    `addr_of!`, never `&packed.field` (see [PERFORMANCE.md §1.4](./PERFORMANCE.md)).

---

## 2. Strings, slices, ownership, and memory

This is where most real FFI bugs live: two languages, and often **two allocators**, disagreeing about who
owns what.

### 2.1 Strings

C strings are NUL-terminated `char*`; Rust strings are UTF-8 with an explicit length and **no NUL**. (Swift
`String` is UTF-8 but bridged; Java/Kotlin strings are UTF-16 — the JNI/uniffi layer transcodes.) Bridge
with [`CString`](https://doc.rust-lang.org/std/ffi/struct.CString.html) (owned, heap, NUL-terminated) and
[`CStr`](https://doc.rust-lang.org/std/ffi/struct.CStr.html) (borrowed view):

```rust
use std::ffi::{CStr, CString};
use std::os::raw::c_char;

// C → Rust: borrow a C string (caller keeps ownership). UNSAFE: trust ptr is valid + NUL-terminated.
unsafe fn from_c<'a>(ptr: *const c_char) -> Option<&'a str> {
    if ptr.is_null() { return None; }
    CStr::from_ptr(ptr).to_str().ok()      // .to_str() validates UTF-8 and can fail
}

// Rust → C: hand out an owned string the caller must free with OUR free fn (§2.3).
#[no_mangle]
pub extern "C" fn mylib_version() -> *mut c_char {
    CString::new("1.2.3").unwrap().into_raw()   // leak into C; reclaim later with from_raw
}

#[no_mangle]
pub unsafe extern "C" fn mylib_string_free(p: *mut c_char) {
    if !p.is_null() { drop(CString::from_raw(p)); }  // reconstruct → Drop frees it
}
```

**The #1 classic footgun:** `CString::new(s).unwrap().as_ptr()` returns a pointer into a temporary that is
**dropped at the end of the statement** — the pointer dangles immediately. Bind the `CString` to a variable
that outlives the call, or use `into_raw()` to transfer ownership. Also, `to_str()` validates UTF-8 and can
fail; C strings can be arbitrary bytes. Decide per-API whether you require UTF-8 or pass bytes through as
`(*const u8, usize)`.

### 2.2 Slices and buffers

Fat pointers don't cross. Pass a slice as an explicit **pointer + length**, and rebuild it on the Rust side:

```rust
#[no_mangle]
pub unsafe extern "C" fn mylib_sum(ptr: *const f64, len: usize) -> f64 {
    if ptr.is_null() || len == 0 { return 0.0; }
    std::slice::from_raw_parts(ptr, len).iter().sum()   // borrows caller memory; no copy
}
```

To **return** an owned buffer, hand back `(ptr, len, cap)` and provide a matching free. To move a `Vec` out
without copying, decompose it and `mem::forget` it, then reconstruct to free (this is what
`Vec::into_raw_parts` does; it's still unstable, so do it by hand):

```rust
#[repr(C)]
pub struct Buf { ptr: *mut u8, len: usize, cap: usize }

#[no_mangle]
pub extern "C" fn mylib_render() -> Buf {
    let mut v: Vec<u8> = render_bytes();
    let b = Buf { ptr: v.as_mut_ptr(), len: v.len(), cap: v.capacity() };
    std::mem::forget(v);                 // don't drop — ownership now lives in C
    b
}

#[no_mangle]
pub unsafe extern "C" fn mylib_buf_free(b: Buf) {
    drop(Vec::from_raw_parts(b.ptr, b.len, b.cap));   // MUST reuse the same cap
}
```

### 2.3 The ownership rules (memorize these)

- **Whoever allocates, frees — through the *same* allocator.** Rust's allocator ≠ libc `malloc`/`free` ≠
  the JVM/Swift allocator. **Never** let the consumer `free()` a pointer Rust allocated (or vice-versa) —
  that's instant heap corruption. Ship a paired `mylib_*_free` for everything you hand out, and document it.
- **`Box::into_raw` / `Box::from_raw`** is the canonical "move a heap value across and reclaim it" pair. One
  `into_raw` must be balanced by exactly one `from_raw` — no double-free, no leak.
- **`Drop` does not run across FFI.** A Rust value living behind a raw pointer in the host is a leak until
  *you* reconstruct and drop it. There is no RAII on the other side — you must expose destructors.
- **Borrow vs. transfer must be documented per parameter.** "Does this take ownership of the pointer, borrow
  it for the call, or borrow it beyond the call?" is the single most important thing your header comments
  must answer. Pick a convention and hold it across the whole API.

---

## 3. Panics, unwinding, and error handling

### 3.1 Never unwind into foreign code

A Rust panic unwinding out of an `extern "C"` function used to be **undefined behavior**. Since Rust 1.81
the language made it safe-by-default: a panic trying to escape a plain `extern "C"` function **aborts the
process** instead of unwinding into foreign frames. Memory-safe — but usually *not* what a library wants
(killing the host app, the JVM, or Xcode's process on a recoverable error is hostile). So the rule stands:
**catch panics at the boundary** and convert them to error values.

```rust
#[no_mangle]
pub extern "C" fn mylib_do(input: *const u8, len: usize) -> i32 {
    let result = std::panic::catch_unwind(|| {
        run(unsafe { std::slice::from_raw_parts(input, len) })   // real work; may panic
    });
    match result {
        Ok(Ok(()))  => 0,              // success
        Ok(Err(e))  => e.code(),       // domain error → negative code
        Err(_panic) => -1,             // panic → generic failure, process stays alive
    }
}
```

- `catch_unwind` requires the closure be `UnwindSafe`; wrap captured `&mut` in `AssertUnwindSafe` when you
  know it's fine. It does **nothing** under `panic = "abort"` — choose your panic strategy consciously (see
  [PERFORMANCE.md §6.2](./PERFORMANCE.md)).
- `extern "C-unwind"` (stable since 1.71) is the opt-in ABI that *allows* unwinding to propagate across the
  boundary — use it only when both sides cooperate (C++ with exceptions, or Rust→C→Rust unwinding). For
  ordinary bindings, stick with `extern "C"` + `catch_unwind`.

### 3.2 Error handling has no exceptions

The C ABI has no `Result` and no exceptions. Pick one discipline and apply it everywhere:

- **Integer status codes** — `0` = ok, negative = error class; return the *value* via an out-parameter.
  Simple, universal, works with every consumer.
- **Out-param + null return** — return the result pointer, `NULL` on failure.
- **Thread-local last-error** (errno style) — `mylib_last_error_message()` returns a string for the last
  failed call on this thread. Ergonomic, but stateful; document the threading contract.

Keep rich `Result<_, thiserror::Error>` internally; **map to stable codes only in the thin `extern "C"`
shell.** Don't leak Rust error *types* across the boundary — leak integer codes (and optionally a message
getter). `Result`/`Option` themselves don't cross; a `#[repr(C)]` tagged struct is the other option when a
plain code isn't enough.

```rust
#[no_mangle]
pub unsafe extern "C" fn mylib_parse(text: *const c_char, out: *mut *mut Doc) -> i32 {
    let Some(s) = from_c(text) else { return ERR_NULL };
    match Doc::parse(s) {                          // rich Result internally
        Ok(doc) => { *out = Box::into_raw(Box::new(doc)); OK }
        Err(e)  => { set_last_error(e); ERR_PARSE }   // stable code out, message stashed
    }
}
```

---

## 4. API design: the opaque-handle, C-shaped waist

The single most important pattern. **Don't expose your rich Rust struct's fields to the consumer** — expose
an *opaque pointer* (a handle) plus functions that operate on it. The consumer never sees the layout, so you
can evolve the Rust internals freely; the consumer just holds a `void*` (or a typed opaque pointer). **This
is exactly pyo3's `#[pyclass]` model: Python holds an opaque handle to a `Box`ed Rust object and calls
methods on it — same pattern, different host.**

```rust
pub struct Engine { /* whatever you like — private, may change every release */ }

// create → returns an opaque handle (heap Box leaked to the caller)
#[no_mangle]
pub extern "C" fn engine_new(seed: u64) -> *mut Engine {
    Box::into_raw(Box::new(Engine::new(seed)))
}

// operate → borrow through the handle; check null defensively
#[no_mangle]
pub unsafe extern "C" fn engine_step(h: *mut Engine, dt: f64, out: *mut f64) -> i32 {
    let Some(engine) = h.as_mut() else { return ERR_NULL };   // Option<&mut> from raw ptr
    *out = engine.step(dt);
    OK
}

// destroy → the one and only free; makes the destructor explicit
#[no_mangle]
pub unsafe extern "C" fn engine_free(h: *mut Engine) {
    if !h.is_null() { drop(Box::from_raw(h)); }
}
```

The header the consumer sees is just `typedef struct Engine Engine;` + the signatures — the struct is
**incomplete/opaque** on their side. Best practices that make such an API pleasant *and* safe:

- **Keep the surface small and flat.** Few functions, scalar/pointer arguments, no nested Rust types. Every
  type that crosses is a maintenance liability; prefer handles + accessor functions over exposing data.
- **A consistent prefix** (`engine_`, `mylib_`) — C has no namespaces, so avoid symbol collisions.
- **Symmetric lifecycle**: every `*_new`/`*_create` has exactly one `*_free`/`*_destroy`. Document ownership
  transfer on *every* pointer parameter and return (borrowed vs. owned).
- **All fallible calls return a status code**; deliver values via out-params. Reserve the return value for
  the error channel so consumers can wrap calls uniformly.
- **Nullability & thread-safety are part of the contract, and live only in docs** — the type system won't
  carry them across. State explicitly: "handle may be shared across threads iff …", "NULL is/isn't allowed
  here." If the Rust type isn't `Sync`, say the handle is single-thread-only.
- **Version your ABI for extensibility.** A struct passed by value can't gain fields without breaking its
  layout — so put a `size`/`version` field first (caller sets `sizeof`), and read new fields only when the
  caller's size covers them. Expose `mylib_abi_version()`.
- **Validate at the shell, trust in the core.** Do null checks, length checks, and UTF-8 validation in the
  `extern "C"` layer (parse-don't-validate), then call into ordinary safe Rust that assumes valid inputs.
  This confines `unsafe` to the boundary.

> The payoff: the entire `extern "C"` layer is a thin, boring, auditable shell of `unsafe` blocks; your real
> logic stays in normal, borrow-checked, testable Rust. Hand-writing this shell is fine for a small C API —
> but if you target several managed languages, generate it (§11).

> **Interview framing** — *"Design a Rust API to be called from Swift, Kotlin, and JS."* Answer with the
> shape: narrow C-ABI waist → opaque handle per object → explicit ownership/free contract → errors as
> codes/out-params (never panics across the line) → **generate** the bindings (`uniffi` for mobile,
> `wasm-bindgen` for web) rather than hand-rolling. Anchor it: *"I've shipped this exact discipline with
> pyo3 exposing a Rust core to Python — same boundary problem, different host."*

---

## 5. Callbacks, threads, and global state

**Callbacks** cross as `extern "C"` function pointers, plus an opaque `void*` "userdata" you thread through
so the callback can find its context (C has no closures). Wrap the callback body in `catch_unwind` if it
runs code that might panic — never let a panic escape through foreign frames.

```rust
pub type LogCb = extern "C" fn(level: i32, msg: *const c_char, user: *mut c_void);

#[no_mangle]
pub extern "C" fn mylib_set_logger(cb: LogCb, user: *mut c_void) { /* store cb + user */ }
```

- **Thread-safety is on you.** The foreign side doesn't know `Send`/`Sync`. If a handle can be used from
  multiple threads, the underlying type must actually be `Sync` (or you guard it with a `Mutex`/`Arc<Mutex>`
  internally) — and you must *say so* in the docs. If a callback may fire on a Rust-owned thread, the
  consumer must be prepared for that (especially the JVM — §8 — where you must attach the thread first).
  (On WASM you're effectively single-threaded, which *simplifies* this; on mobile you can't assume it.)
- **Global state**: use `std::sync::OnceLock`/`OnceCell` for one-time init — an explicit `mylib_init()`
  entry point beats lazy statics with hidden ordering. Beware **re-entrancy**: a callback that calls back
  into your library while you hold a lock will deadlock.

---

## 6. Building and generating bindings

**Crate types** (in `Cargo.toml`):

```toml
[lib]
crate-type = ["staticlib", "cdylib"]   # pick per target; often both
```

- **`cdylib`** → a shared library (`.so` / `.dylib` / `.dll`) loaded at runtime. What Android
  (`System.loadLibrary`) and dynamically-linked C apps want.
- **`staticlib`** → a `.a` archive linked at build time. What iOS/Swift (into an `.xcframework`) and static
  C builds want. Bundles the Rust std → larger; `strip = true` + `panic = "abort"` + LTO to trim.
- **`rlib`** (default) is Rust-only and *not* for FFI.

**Generating the boundary — pick your direction:**

| Task | Tool | Direction |
|---|---|---|
| Rust → C/C++ header | [`cbindgen`](https://github.com/mozilla/cbindgen) | reads your `extern "C"` + `#[repr(C)]`, emits `.h`/`.hpp` |
| C header → Rust bindings | [`bindgen`](https://github.com/rust-lang/rust-bindgen) | emits `extern "C"` blocks + `#[repr(C)]` structs |
| Safe **C++** interop (both ways) | [`cxx`](https://cxx.rs) | shared `#[cxx::bridge]`; generates matching C++/Rust glue, no raw pointers |
| Rust → Kotlin/Swift/Python | [`uniffi`](https://mozilla.github.io/uniffi-rs/) | §11 |
| Rust ↔ Swift (rich) | [`swift-bridge`](https://github.com/chinedufn/swift-bridge) | §11 |

Run `cbindgen` from `build.rs` or CI so the header never drifts from the code. `#[no_mangle]` keeps symbol
names verbatim; `#[export_name = "..."]` renames.

---

## 7. Consuming from C and C++

The baseline case; everything above applies directly.

- Expose `extern "C"` + `#[repr(C)]`, generate the header with **`cbindgen`**, ship `cdylib` or `staticlib`.
- Opaque types become **incomplete struct** typedefs (`typedef struct Engine Engine;`) — the C side holds
  only a pointer, exactly as you want.
- **For C++ specifically, prefer [`cxx`](https://cxx.rs)** over hand-rolled `extern "C"`: declare a shared
  `#[cxx::bridge] mod ffi { ... }` and it generates *both* sides with matching types — `String`, `Vec`,
  `Box`, `UniquePtr`, slices, and `Result`→exception mapping — with no raw pointers and compile-time
  checking that the two sides agree. It eliminates the whole class of layout/ownership mismatches for C++
  consumers. Reach for raw `extern "C"` only when the consumer is plain C or a codegen backend.

---

## 8. Android: JNI and Kotlin/Java

Android calls native code through **JNI**. Two routes:

1. **Direct JNI** with the [`jni`](https://docs.rs/jni) crate — export `Java_`-named functions taking a
   `JNIEnv`, marshalling `jstring`/`jlong`/… by hand.
2. **A plain C-ABI `cdylib` + a thin Kotlin shim** — or, better for anything non-trivial, **generate the
   whole thing with uniffi** (§11), the recommended path.

Direct-JNI essentials:

```rust
use jni::JNIEnv;
use jni::objects::{JClass, JString};
use jni::sys::{jlong, jstring};

// Name = Java_<package with _>_<Class>_<method>. Note extern "system" (JNICALL), not "C".
#[no_mangle]
pub extern "system" fn Java_com_example_Engine_nativeNew(_env: JNIEnv, _cls: JClass) -> jlong {
    Box::into_raw(Box::new(Engine::new())) as jlong        // store the handle in a Kotlin Long
}

#[no_mangle]
pub extern "system" fn Java_com_example_Engine_nativeStep(
    mut env: JNIEnv, _cls: JClass, handle: jlong, input: JString,
) -> jstring {
    let engine = unsafe { &mut *(handle as *mut Engine) };
    let s: String = env.get_string(&input).unwrap().into();    // jstring → Rust String
    env.new_string(engine.step(&s)).unwrap().into_raw()        // Rust String → jstring
}

#[no_mangle]
pub extern "system" fn Java_com_example_Engine_nativeFree(_e: JNIEnv, _c: JClass, handle: jlong) {
    if handle != 0 { drop(unsafe { Box::from_raw(handle as *mut Engine) }); }
}
```

On the Kotlin side you keep the `jlong` in the object and free it in `close()`:

```kotlin
class Engine : AutoCloseable {
    private val handle: Long = nativeNew()
    fun step(s: String): String = nativeStep(handle, s)
    override fun close() = nativeFree(handle)
    private external fun nativeStep(h: Long, s: String): String
    companion object { init { System.loadLibrary("mylib") }; @JvmStatic external fun nativeNew(): Long }
}
```

JNI-specific quirks:

- **`JNIEnv` is per-thread and `!Send`.** Use only the one handed to your function, on that thread. To call
  into the JVM from a Rust-spawned thread (e.g. a callback), keep the `JavaVM` (which *is* shareable) and
  `attach_current_thread()` for an env — detach before the thread exits.
- **Local refs are frame-scoped and limited** (~512). In loops use `with_local_frame` or delete refs;
  promote anything long-lived to a **global ref** (and delete it explicitly — the GC won't).
- **Panics must be caught** and turned into a thrown Java exception (`env.throw_new(...)`) — never let one
  unwind into the JVM (§3).
- **Cross-compile with [`cargo-ndk`](https://github.com/bbqsrc/cargo-ndk)** for the ABIs you ship —
  `aarch64-linux-android`, `armv7-linux-androideabi`, `x86_64-linux-android`, `i686-linux-android` — and
  drop the `.so`s into `src/main/jniLibs/<abi>/`.
- **Storing pointers as `jlong`** is the standard handle trick (64-bit holds any pointer). Guard against
  double-`close()` / use-after-close on the Kotlin side (null the handle).

---

## 9. Swift, iOS, and macOS

Swift has **first-class C interop**, so the shape is "expose a clean C API, import it into Swift, wrap it in
a Swift class." Two levels of tooling:

**Hand-rolled (C ABI + cbindgen):**

- Build a **`staticlib`** per Apple target and package them as an **`.xcframework`** (via
  `xcodebuild -create-xcframework`) bundling the `.a`s and the `cbindgen` header. Targets:
  `aarch64-apple-ios` (device), `aarch64-apple-ios-sim` (Apple-silicon simulator),
  `x86_64-apple-ios` (Intel sim), `aarch64-apple-darwin`/`x86_64-apple-darwin` (macOS).
  (`cargo-lipo` is deprecated — `.xcframework` is the modern multi-arch container.)
- Expose the header to Swift via a **module map** (`module.modulemap`) or bridging header. Opaque Rust
  pointers arrive in Swift as `OpaquePointer`.
- **Wrap the handle in a Swift `class` and call the Rust destructor in `deinit`** — Swift's ARC manages the
  *wrapper*; the wrapper's `deinit` is where you release the Rust-owned memory (ARC never sees inside):

  ```swift
  final class Engine {
      private let handle: OpaquePointer
      init(seed: UInt64) { handle = engine_new(seed) }
      func step(_ dt: Double) -> Double { var out = 0.0; _ = engine_step(handle, dt, &out); return out }
      deinit { engine_free(handle) }        // Rust free, driven by Swift ARC
  }
  ```

**Richer: [`swift-bridge`](https://github.com/chinedufn/swift-bridge).** A `#[swift_bridge::bridge]` module
generates matching Swift + Rust glue and supports passing `String`, `Vec`, `Option`, opaque types both ways,
and **`async`** (Rust futures ↔ Swift async/await) — far less boilerplate than raw C for Swift-only targets.

Quirks: watch **symbol stripping** in release Xcode builds (keep exported symbols referenced); and both the
simulator and device architectures must be in the `.xcframework` or you'll hit link errors.

---

## 10. Kotlin: JVM and Multiplatform Native

"Kotlin" spans several runtimes; the boundary tech differs, but the Rust side is the same clean C ABI from
§4:

- **Kotlin/JVM & Android** → **JNI** (§8) is the native path. Shim-free alternatives: **JNA** (dynamic,
  reflection-based — easiest, slowest), **JNR-FFI** (faster dynamic binding), or **Project Panama / the
  Foreign Function & Memory API** (`java.lang.foreign`, stable in JDK 22) — the modern JNI-free way for
  desktop JVM to call a C-ABI `cdylib` directly. For most app teams, **uniffi** (§11) generating idiomatic
  Kotlin over a C ABI is the sweet spot.
- **Kotlin/Native & Multiplatform (KMP)** → **cinterop**: write a `.def` file pointing at the `cbindgen`
  header + library, and the Kotlin/Native toolchain generates Kotlin bindings to the C ABI. One shared Rust
  core can back a KMP module across Android + iOS + desktop.

The differences are purely in *how the Kotlin side reaches the library* (JNI shim vs. FFM vs. cinterop) and
how you package the artifact (`.so` in `jniLibs` for Android, native lib for KMP).

---

## 11. The high-level path: uniffi and swift-bridge

Hand-writing the `extern "C"` shell **plus** a JNI shim **plus** a Swift wrapper — and keeping them in sync
by hand across releases — is a lot of error-prone boilerplate. If you target **managed languages**, generate
it.

**[`uniffi`](https://mozilla.github.io/uniffi-rs/) (Mozilla)** is the default recommendation for
multi-language SDKs. Describe your interface (proc-macros on your Rust types, or a UDL file) and it generates
**idiomatic Kotlin, Swift, Python** (plus community Ruby/others) over a C ABI — handling the parts this doc
spends pages on:

- **Object lifecycle & memory** — handles, destructors, and the free-through-the-right-allocator rule.
- **Type mapping** — strings, sequences, records, enums, `Option`, and `Result`→idiomatic errors
  (Swift `throws`, Kotlin exceptions).
- **Panics → exceptions**, and callback interfaces in both directions.
- **Async** (Rust `async fn` ↔ Swift/Kotlin coroutines) in recent versions.

One Rust interface → Android (Kotlin) + iOS (Swift) + a Python test harness, all consistent. The cost is a
constrained type system (you model the boundary in uniffi's terms) and a codegen step — almost always the
right trade for a cross-platform mobile SDK.

**When to use what:**

| Situation | Reach for |
|---|---|
| Cross-platform SDK (Kotlin + Swift + Python) | **uniffi** |
| Swift-only, want rich types / `async` | **swift-bridge** |
| C++ consumer, want safety both ways | **cxx** |
| Web / JS consumer | **wasm-bindgen** ([WASM.md](./WASM.md)) |
| Node.js native addon | **napi-rs** |
| Python consumer | **pyo3** + maturin |
| Plain C consumer, or a codegen target needs a stable header | hand-rolled `extern "C"` + **cbindgen** |
| Consuming an existing C library from Rust | **bindgen** |

Rule of thumb: **hand-roll the C ABI only when the consumer is C itself or another generator; for managed
languages, generate the bindings** and keep your hand-written `unsafe` surface near zero.

---

## 12. Checklist, crates, further reading

### FFI review checklist

1. **Everything crossing is `#[repr(C)]`/`transparent`** or a primitive/pointer — no bare Rust types (§1).
2. **`extern "C"` boundary functions `catch_unwind`** (or you've consciously accepted process-abort) (§3).
3. **Every allocation has a matching `*_free`**, freed through the *same* allocator; documented (§2.3).
4. **`Box::into_raw` balanced 1:1 with `from_raw`** — no double-free, no leak; `Drop` won't run for you.
5. **No dangling `CString::…as_ptr()`**; slices passed as `(ptr, len)` and null-checked (§2).
6. **Opaque handles, not exposed fields**; small, flat, prefixed API; ownership documented per param (§4).
7. **Errors are stable integer codes** (or errno-style / tagged struct), not leaked Rust error types (§3.2).
8. **Thread-safety & nullability stated in docs** — the type system won't carry them across (§4, §5).
9. **ABI versioned** (size/version field, `abi_version()`) so structs can evolve (§4).
10. **Header generated by `cbindgen` in CI** so it never drifts; symbols `#[no_mangle]` (§6).
11. **Targeting managed languages? Generate with uniffi** instead of hand-rolling shims (§11).

### Crates & tools

| Need | Reach for |
|---|---|
| Rust → C/C++ header | `cbindgen` |
| C → Rust bindings | `bindgen` |
| Safe C++ interop | `cxx`, `autocxx` |
| Kotlin/Swift/Python bindings | `uniffi` |
| Rich Swift interop / async | `swift-bridge` |
| Raw JNI | `jni` |
| Web / Node / Python hosts | `wasm-bindgen`, `napi-rs`, `pyo3` + `maturin` |
| C types / libc | `core::ffi`, `libc` |
| Nullable pointers | `Option<NonNull<T>>`, `std::ptr::NonNull` |
| Android cross-compile | `cargo-ndk` |
| Apple packaging | `xcodebuild -create-xcframework`, `cargo-xcframework` |
| Error boilerplate (internal) | `thiserror` |
| Other generators | `flapigen`, `diplomat` |

### Further reading

- [The Rustonomicon — FFI](https://doc.rust-lang.org/nomicon/ffi.html) — the canonical unsafe-FFI reference
- [Rust Reference: type layout & `repr`](https://doc.rust-lang.org/reference/type-layout.html)
- [Rust FFI Omnibus](http://jakegoulding.com/rust-ffi-omnibus/) — worked examples per language
- [`cxx` book](https://cxx.rs) · [`uniffi` book](https://mozilla.github.io/uniffi-rs/) ·
  [`bindgen` book](https://rust-lang.github.io/rust-bindgen/)
- [`jni` crate docs](https://docs.rs/jni) · [`cargo-ndk`](https://github.com/bbqsrc/cargo-ndk) ·
  [`swift-bridge`](https://github.com/chinedufn/swift-bridge)
- [PERFORMANCE.md §6.2](./PERFORMANCE.md) — panic policy at boundaries · [WASM.md](./WASM.md) — the browser boundary
