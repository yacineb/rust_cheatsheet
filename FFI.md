## FFI 


### The core problem
Rust's type system — generics, lifetimes, trait objects, `String`, `Vec`, enums-with-data — **does not exist at the ABI level.** The only stable, language-agnostic contract is the **C ABI**. Everything that crosses a boundary to Swift/Kotlin/JS must be reduced to something C-shaped, then reconstructed on the other side. This is the same discipline as pyo3, just targeting a different host.

### What crosses cleanly vs. what doesn't
- **Crosses:** primitives (`i32`, `f64`, `bool`, ...), raw pointers, `#[repr(C)]` structs, C-compatible enums (`#[repr(C)]` or `#[repr(i32)]`), function pointers.
- **Does NOT cross:** Rust `String`/`str`, `Vec<T>`, `Option`/`Result` and other data-carrying enums, trait objects (`dyn`), generics, anything with a lifetime, anything relying on Rust's non-`#[repr(C)]` (default) memory layout. These must be **converted** at the boundary.

### The key mechanisms
- `extern "C"` — declares the C calling convention.
- `#[no_mangle]` — stops the compiler from renaming the symbol so the host linker can find it.
- `#[repr(C)]` — forces predictable, C-compatible struct layout (Rust's default layout is unspecified and may be reordered).

### Ownership across the boundary — the thing they'll probe
Rust's ownership model stops at the FFI line. You have to answer, explicitly, **who allocates and who frees.**
- `Box::into_raw(b)` hands a heap pointer to the host and **leaks it from Rust's view** (Rust forgets it). The host now owns it.
- The host must later call back into a Rust `free_*` function that does `Box::from_raw(ptr)` to reclaim and drop it. **Never free Rust-allocated memory with the host's allocator, or vice versa** — that's instant heap corruption.

### The opaque handle pattern (the idiomatic answer)
Don't expose your rich Rust struct's fields across FFI. Instead:
1. `Box` the struct, hand out an opaque `*mut MyThing` pointer.
2. Every operation is a C function taking that pointer + primitive args.
3. A `myThing_free(ptr)` finalizer reclaims it.

The host treats the pointer as an opaque token. This keeps the ABI surface tiny and lets you evolve the Rust internals freely. (This is exactly pyo3's `PyClass` model — say that.)

### Strings
- Rust strings are UTF-8, not null-terminated, with a length. C expects null-terminated. Swift `String` is UTF-8-ish but bridged; JS strings are UTF-16.
- Cross with `CString`/`CStr`, or pass `(ptr, len)` pairs. Watch ownership: a `CString` handed out must be freed by Rust later; a borrowed `&CStr` must not outlive its owner.

### Errors and panics — the trap
- **A Rust panic unwinding across an FFI boundary is undefined behavior.** You must not let it happen. Either compile with `panic = "abort"`, or wrap boundary functions in `std::panic::catch_unwind` and translate to an error code.
- `Result`/`Option` don't cross. Convert to: integer status codes, out-parameters (`*mut T` the host provides), or a `#[repr(C)]` tagged result struct. Reserve a null pointer or a sentinel as "error."

### Thread safety at the boundary
`Send`/`Sync` are Rust concepts the host can't see. If the host may call from multiple threads, your handle must actually be thread-safe (`Arc<Mutex<...>>` internally) — the compiler won't protect you once a `*mut` escapes. On web/WASM you're effectively single-threaded, which *simplifies* this; on mobile you can't assume that.

### The tooling that generates all this
You rarely hand-write the above at scale. Know the landscape:
- **uniffi** (Mozilla) — you write Rust + an interface definition; it generates idiomatic **Swift and Kotlin** bindings. The mobile answer.
- **wasm-bindgen** — generates the JS↔Rust glue for the **web** target; handles marshalling of strings, closures, and JS objects. The web answer.
- **cbindgen** — generates C headers from your Rust. **cxx** — safe Rust↔C++ interop. **napi-rs** — Node addons.
- Hand-rolled `extern "C"` — the fallback / lowest common denominator everything else builds on.

> **What they'll ask:** *"Design a Rust API meant to be called from Swift, Kotlin, and JS."*
> **How to answer:** Narrow C-ABI surface → opaque handle pattern → explicit ownership/free contract → errors as codes/tagged structs, never panics across the line → `uniffi` for mobile, `wasm-bindgen` for web. Anchor it in pyo3: *"I've shipped this boundary discipline with pyo3 exposing a Rust core to Python — same problem, different host."*