# Rust → WebAssembly: quirks, what doesn't work, and the JS boundary

> A companion to the [README cheat-sheet](./README.md). Where the README is language notes, this doc is
> about the **platform**: what breaks when you retarget Rust to `wasm32`, *why* it breaks, and how the
> **Rust ↔ JavaScript boundary** actually works under `wasm-bindgen`.
>
> The one-line mental model: **`wasm32-unknown-unknown` is a freestanding, single-threaded, 32-bit,
> sandboxed target with no OS underneath it.** Anything in `std` that needs a syscall — clock, files,
> sockets, threads, randomness, env — is either a compile error, a link error, or a runtime panic. The
> browser (or a WASI host) is the OS, and you reach it only across a **copy boundary** made of numbers.

---

## Table of contents

0. [First decision: which wasm target?](#0-first-decision-which-wasm-target)
1. [What does *not* work on `wasm32-unknown-unknown`](#1-what-does-not-work-on-wasm32-unknown-unknown)
2. [Panics, aborts, and getting errors to show up](#2-panics-aborts-and-getting-errors-to-show-up)
3. [The 32-bit, numbers-only ABI](#3-the-32-bit-numbers-only-abi)
4. [The JS boundary — `wasm-bindgen` walkthrough](#4-the-js-boundary--wasm-bindgen-walkthrough)
5. [Async across the boundary](#5-async-across-the-boundary)
6. [Threads (if you really must)](#6-threads-if-you-really-must)
7. [Build, size, and performance quirks](#7-build-size-and-performance-quirks)
8. [WASI: server-side wasm has an OS again](#8-wasi-server-side-wasm-has-an-os-again)
9. [Checklist, crates, further reading](#9-checklist-crates-further-reading)

---

## 0. First decision: which wasm target?

`wasm32` is not one target. The single biggest source of "why doesn't `std` work" is picking the wrong one.

| Target | Host / use | `std::fs`, `net`, clock, random, env, args | How you call out |
|---|---|---|---|
| `wasm32-unknown-unknown` | **Browser / JS** (via `wasm-bindgen`, `wasm-pack`, `trunk`) | ❌ stubbed → error/panic | JS imports (`wasm-bindgen`) |
| `wasm32-wasip1` (was `wasm32-wasi`) | **Server-side** (Wasmtime, Wasmer, WasmEdge, Spin) | ✅ via WASI syscalls | WASI + component imports |
| `wasm32-wasip2` | WASI 0.2 / **Component Model** | ✅ + typed components | WIT interfaces |
| `wasm32-unknown-emscripten` | Legacy C/C++ interop, POSIX emulation | ⚠️ partial (emulated) | Emscripten JS glue |

**Everything in §1–§7 is about `wasm32-unknown-unknown`** (the browser target) because that's where the
surprises live. `unknown-unknown` means *"unknown vendor, unknown OS"* — literally no OS. `std` still
compiles (unlike `no_std`), but the OS-backed parts are stubs that fail. If you're doing server-side wasm,
jump to §8 — WASI gives you most of `std` back.

---

## 1. What does *not* work on `wasm32-unknown-unknown`

The high-value section. Most of these **compile fine** and fail at **runtime** (panic/trap) or **link
time**, which is exactly why they surprise people.

| You wrote | What happens | Do this instead |
|---|---|---|
| `Instant::now()` / `SystemTime::now()` | **runtime panic** "time not implemented on this platform" | `web-time` (drop-in `Instant`/`SystemTime`), `chrono` (`Utc::now()` via `js_sys::Date`), or `performance.now()` |
| `thread::spawn(..)` | **unsupported** — no threads by default | `wasm-bindgen-futures` tasks; Web Workers; `wasm-bindgen-rayon` (§6) |
| `thread::sleep(d)` | busy-loop / unsupported; **blocks the only thread** (freezes the UI) | `gloo_timers::future::sleep(d).await`, or `setTimeout` |
| `std::fs::*` | returns `Err`/unsupported (no filesystem) | `fetch`, `File`/`Blob` via `web_sys`; or WASI (§8) |
| `std::net::*` (`TcpStream`, `UdpSocket`) | not available / errors | `fetch` (`web_sys`), `WebSocket`, WebRTC |
| `std::process::*`, `Command` | unsupported | — (no processes in the sandbox) |
| `std::env::var` / `args()` | empty / errors — no environment | pass config across the JS boundary |
| `println!` / `print!` | **goes nowhere** — stdout is not wired up | `web_sys::console::log_1(&…)`, or the `log` + `console_log` crates |
| `getrandom` (used by `rand`, `uuid`, `HashMap` seeding) | **compile/link error** by default | enable the JS backend (see below) |
| `catch_unwind` | doesn't catch — panics `abort` → trap (§2) | design for `Result`; don't rely on unwinding |
| Deep recursion / big stack arrays | **trap** ("memory out of bounds"), no nice message | fixed link-time stack (default ~1 MiB); iterate, box, or raise `-z stack-size` |
| C-dependency crates (`openssl`, `ring` C parts, anything with `build.rs` + `cc`) | fail to build (no C toolchain for this target) | pure-Rust alternatives: `rustls`, `ring`'s wasm path, `getrandom` js |
| `mmap`, `dlopen`, dynamic linking, inline asm for other ISAs | unsupported | — |
| `tokio` runtime / timers / net / fs | multi-thread rt, timers, I/O don't run (won't even compile with default features) | `wasm-bindgen-futures` + `gloo` as the runtime; keep tokio's `sync`+`macros` subset (§5.1) |

### 1.1 Time

`Instant`/`SystemTime` have **no clock source** on `unknown-unknown`, so `::now()` panics at runtime — a
nasty one because it compiles clean and only blows up when hit. Two clocks exist, both via JS:

```rust
// Wall clock (ms since Unix epoch) — can jump backwards (NTP/user changes it).
let ms: f64 = js_sys::Date::now();

// Monotonic-ish, sub-ms — for measuring durations. Needs web_sys "Performance" + "Window" features.
let t: f64 = web_sys::window().unwrap().performance().unwrap().now();
```

The pragmatic fix is the [`web-time`](https://docs.rs/web-time) crate: it re-exports `Instant`/`SystemTime`
that work on wasm and are the real `std` types everywhere else — so your `Duration` math compiles once and
runs on both.

For **calendar dates/times**, [`chrono`](https://docs.rs/chrono) works on wasm out of the box:
`Utc::now()` and `Local::now()` are transparently backed by `js_sys::Date` through chrono's **`wasmbind`**
feature (pulled in by the default `clock` feature), and it reads the browser's local timezone offset the
same way — no manual `Date` plumbing. This is the ergonomic win: the *same* `chrono` code runs natively and
on wasm. One gotcha: if you set `default-features = false`, **re-add `wasmbind`** explicitly, otherwise
`now()` falls back to `SystemTime::now()` and you're back to the runtime panic above. (The other date crate,
`time` 0.3, needs its `wasm-bindgen` feature for the same reason.)

### 1.2 Randomness (the `getrandom` wall)

`rand`, `uuid`, `ahash`, and `HashMap`'s DoS-resistant seeding all funnel through `getrandom`, which has
**no OS entropy source** here. By default you get a build error telling you to pick a backend:

```toml
# getrandom 0.2 — the common case. Routes to crypto.getRandomValues() in the browser.
getrandom = { version = "0.2", features = ["js"] }
```

```bash
# getrandom 0.3 changed the mechanism to a build cfg (set in .cargo/config.toml or RUSTFLAGS):
RUSTFLAGS='--cfg getrandom_backend="wasm_js"' cargo build --target wasm32-unknown-unknown
```

Miss this and you don't just lose `rand` — `HashMap::new()` can fail to link too, because its default
hasher wants a random seed. (Swapping in a fixed-seed hasher like `FxHashMap` sidesteps that.)

### 1.3 Blocking is a footgun, not just "slow"

There is **one thread** and it's usually the browser's main/UI thread. Anything that blocks it — a spin
loop, `thread::sleep`, a synchronous `block_on`, a tight compute loop — **freezes the entire tab**: no
rendering, no events, "page unresponsive." There is no true `block_on` because there's no other thread to
make progress. Long work must either be chunked and yielded (`await` a timer between chunks) or moved to a
Web Worker (§6). This is the single biggest mindset shift coming from native Rust.

---

## 2. Panics, aborts, and getting errors to show up

- **Panic = trap, by default.** wasm has no stack unwinding in the stable/default configuration, so a
  panic calls `abort`, which becomes a wasm **trap**. Out of the box that surfaces in the JS console as an
  opaque `RuntimeError: unreachable` with **no message and no backtrace** — useless for debugging.
- **Fix it once:** add [`console_error_panic_hook`](https://docs.rs/console_error_panic_hook) and install
  it at startup so panics print a real message + JS stack:

  ```rust
  #[wasm_bindgen(start)]
  pub fn start() {
      console_error_panic_hook::set_once();
  }
  ```
- **`catch_unwind` does not work** — there's nothing to unwind, `abort` skips right past it. Don't build
  error handling on catching panics; use `Result` and let it cross the boundary as a JS exception (§4.6).
- A panic **cannot unwind into JS** and a JS exception cannot unwind through wasm frames cleanly — the
  boundary is a hard wall. Convert to values on each side.
- **Integer overflow** panics in debug (→ trap) and wraps in release, same as native — but the trap is
  again messageless without the hook. **Array/slice out-of-bounds**, **stack overflow**, and
  `unreachable!()` all become the same generic trap.
- There *is* a wasm exception-handling proposal (and `panic=unwind` support is maturing), but assume
  **abort semantics** unless you've deliberately opted in.

---

## 3. The 32-bit, numbers-only ABI

Two hardware facts leak into your API design:

**wasm32 is a 32-bit machine.** `usize`/`isize` are **32 bits**, pointers are 32 bits, and a module's
linear memory maxes out around **4 GiB** (the `memory64` proposal lifts this but isn't the default). Code
that assumes `usize == u64`, or that indexes past 4 GiB, or that transmutes pointer-sized values, breaks.
`u64`/`i64` arithmetic itself works fine (lowered to multiple 32-bit ops).

**The wasm ABI only speaks numbers.** A raw wasm function's arguments/returns are `i32`, `i64`, `f32`,
`f64` — nothing else. So *every* richer type crossing the boundary is either encoded into numbers or passed
as a **pointer + length into linear memory**. Consequences:

- JS `Number` is an IEEE-754 `f64`, exact only up to 2^53. `wasm-bindgen` therefore maps Rust **`u64`/`i64`
  to JS `BigInt`**, not `Number` — a common surprise when you index or bit-twiddle on the JS side.
- There is **no shared heap.** JS objects live in the JS GC heap; Rust values live in wasm linear memory.
  Neither side can dereference the other's pointers. Everything is **copy or handle** (§4).

---

## 4. The JS boundary — `wasm-bindgen` walkthrough

The raw ABI is unusable by hand, so [`wasm-bindgen`](https://rustwasm.github.io/wasm-bindgen/) generates
the glue: JS shims that marshal types, plus a `.d.ts`. `wasm-pack` / `trunk` drive it. Mental model:

> **The boundary is a copy boundary made of numbers.** Cross it *rarely* and in *bulk*. Passing a
> million elements one call at a time is death by a thousand shims; pass one big buffer once.

### 4.1 The basics

```rust
use wasm_bindgen::prelude::*;

#[wasm_bindgen]                         // export a free function → becomes a JS function
pub fn add(a: i32, b: i32) -> i32 { a + b }

#[wasm_bindgen]
extern "C" {                            // import something from JS
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);

    #[wasm_bindgen(js_name = alert)]    // rename to a JS identifier
    fn alert(s: &str);
}
```

### 4.2 What crosses cheaply vs. what copies

| Rust type | Crosses as | Cost |
|---|---|---|
| `i32`,`u32`,`f32`,`f64`,`bool`,`char` | JS `Number`/`Boolean` | **free** (register) |
| `i64`,`u64` | JS **`BigInt`** | cheap, but not a `Number` |
| `&str`, `String` | JS string | **copy + UTF‑8 ↔ UTF‑16 re-encode** |
| `&[u8]`, `Vec<u8>`, `&[f64]`, … | `Uint8Array`/`Float64Array` | **copy** out of linear memory |
| `Option<T>` | value or `undefined`/`null` | as inner |
| `Result<T, E>` | `T`, or **throws** `E` (§4.6) | as inner |
| `JsValue` | any JS value, opaquely | handle (no copy) |
| `#[wasm_bindgen] struct` | **JS class holding a pointer** into wasm memory | handle + must `free()` (§4.5) |
| arbitrary `Serialize`/`Deserialize` | JS object via `serde-wasm-bindgen` | copy + (de)serialize |

Key point: **`String` and slices are copied** across the boundary, and strings also pay a UTF‑8↔UTF‑16
transcode (Rust is UTF‑8, JS is UTF‑16). Chatty stringly APIs are slow; batch.

### 4.3 `JsValue` — the opaque handle

`JsValue` is a typed handle to *any* JS value living in the JS heap; Rust holds an index into a side table,
not a pointer. You can't inspect it directly — you go through `js_sys` (ECMAScript built-ins: `Array`,
`Object`, `Reflect`, `Date`, `Promise`, `Map`, typed arrays…) or `web_sys` (DOM/Web APIs). `web_sys` is
**feature-gated per interface** — enable exactly what you touch, which also keeps binary size down:

```toml
web-sys = { version = "0.3", features = ["Window", "Document", "Element", "console"] }
```

### 4.4 Zero-copy views — and the footgun that eats beginners

To avoid copying a big buffer, you can hand JS a **view** directly into wasm linear memory:

```rust
let data: Vec<u8> = compute();
// SAFETY: the view aliases wasm memory; it is only valid until memory changes.
let view = unsafe { js_sys::Uint8Array::view(&data) };
```

**The trap:** the view is backed by the `WebAssembly.Memory`'s `ArrayBuffer`. The moment wasm memory
**grows** (any allocation can trigger `memory.grow`), the old `ArrayBuffer` is **detached** and every
retained view becomes **empty/detached** — silently reading zeros or throwing. So:

- **Never keep a `.view()` across an `await`, a callback, or any Rust code that might allocate.**
- Use it, copy it immediately (`.to_vec()` / `.slice()`), or hand it straight to a synchronous consumer.
- When in doubt, take the copy — `Uint8Array::from(&data[..])` copies and is always safe.

### 4.5 Exported structs are pointers you must free

Put `#[wasm_bindgen]` on a `struct`/`impl` and JS gets a **class whose instance is a pointer** into wasm
memory:

```rust
#[wasm_bindgen]
pub struct Engine { /* fields live in wasm linear memory */ }

#[wasm_bindgen]
impl Engine {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Engine { Engine { /* … */ } }

    pub fn step(&mut self, dt: f64) -> f64 { /* … */ 0.0 }

    #[wasm_bindgen(getter)]
    pub fn score(&self) -> u32 { 0 }
}
```

```js
const e = new Engine();
e.step(0.016);
e.free();               // ← REQUIRED: JS GC does not own wasm memory
```

- The JS object is a thin handle; the real bytes are Rust-owned in linear memory. **JS garbage collection
  will not reclaim them** — forgetting `free()` **leaks** wasm memory (which only ever grows, §7).
- Modern `wasm-bindgen` registers a `FinalizationRegistry` to auto-`free` when the JS wrapper is collected,
  but finalizers are **best-effort and untimed** — don't rely on them for anything hot or bounded. Call
  `free()` explicitly.
- Passing an owned struct *into* a JS-bound function **moves** it (consumes the pointer); using the JS
  handle afterward throws "null pointer passed to rust". `&self`/`&mut self` methods borrow instead.
- Only `pub` fields of `Copy`-ish type auto-expose as getters/setters; everything else goes through methods.

### 4.6 Errors and callbacks

**`Result` → exception.** Return `Result<T, JsValue>` (or any `Into<JsValue>` error) and the `Err` is
**thrown** as a JS exception; JS uses normal `try/catch`. Going the other way, mark an imported JS function
that can throw with `catch` so it returns a `Result`:

```rust
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(catch)]
    fn risky() -> Result<JsValue, JsValue>;   // JS throw → Rust Err
}
```

**Closures need to outlive the call.** To hand Rust a callback JS will invoke *later* (event listener,
`setTimeout`), wrap it in a `Closure`. Its lifetime is the footgun:

```rust
let cb = Closure::<dyn FnMut()>::new(move || { /* … */ });
window.set_onclick(Some(cb.as_ref().unchecked_ref()));
// If `cb` drops here, JS calling it later throws:
//   "closure invoked recursively or after being dropped"
cb.forget();   // leak it to keep it alive forever — or store it somewhere with a matching lifetime
```

`.forget()` is a deliberate leak; the clean alternative is to **store the `Closure` in a struct that lives
as long as the subscription** and drop it to unsubscribe.

### 4.7 Passing structured data

For rich objects, don't hand-marshal fields — use
[`serde-wasm-bindgen`](https://docs.rs/serde-wasm-bindgen) to convert any `Serialize`/`Deserialize` type to
a real JS object/value (faster and more correct than the old `JsValue::from_serde` JSON-string path):

```rust
#[derive(serde::Serialize, serde::Deserialize)]
pub struct Config { name: String, retries: u32 }

#[wasm_bindgen]
pub fn make() -> Result<JsValue, JsValue> {
    serde_wasm_bindgen::to_value(&Config { name: "x".into(), retries: 3 }).map_err(Into::into)
}
```

---

## 5. Async across the boundary

There is **no native async runtime** — no reactor, no timer wheel, no I/O driver. The event loop *is* the
JS event loop. `wasm-bindgen-futures` bridges the two worlds:

- **`JsFuture`**: wrap a JS `Promise` as a Rust `Future` and `.await` it.
- **`spawn_local(fut)`**: drive a `'static` Rust future to completion on the JS microtask queue (no
  `Send` bound — everything is single-threaded, so `!Send` futures are fine, unlike tokio).
- Return a Rust `async fn` and `wasm-bindgen` hands JS back a real **`Promise`**.

```rust
use wasm_bindgen_futures::JsFuture;

#[wasm_bindgen]
pub async fn get(url: String) -> Result<JsValue, JsValue> {
    let win = web_sys::window().unwrap();
    let resp = JsFuture::from(win.fetch_with_str(&url)).await?;   // await a JS Promise
    let resp: web_sys::Response = resp.dyn_into()?;
    JsFuture::from(resp.json()?).await                            // → resolves to JS value
}
```

### 5.1 Tokio on the wasm target — a walkthrough

Tokio *does* compile for wasm, but only a **runtime-agnostic subset**, and you must strip default features
(the `net`/`fs`/`process`/`signal` drivers don't build). The only feature set tokio permits on `wasm32-*`:

```toml
# Default features WILL fail to compile on wasm — disable them and opt back in:
tokio = { version = "1", default-features = false, features = [
    "sync",      # Mutex, RwLock, mpsc, oneshot, watch, broadcast, Notify, Semaphore
    "macros",    # select!, join!, try_join!, #[tokio::test]
    "io-util",   # AsyncReadExt / AsyncWriteExt combinators
    "rt",        # current-thread runtime ONLY (never rt-multi-thread)
    "time",      # compiles — but timers are unbacked in the browser, see below
] }
```

What each tier actually does **in the browser** (`wasm32-unknown-unknown`):

- ✅ **`sync` + `macros` are the real win.** `tokio::sync::{mpsc, oneshot, watch, Notify, Semaphore}` and
  `select!`/`join!` are pure state machines with **no OS dependency** — they work perfectly and are
  genuinely useful for wiring together futures. This is the 90% case: use tokio as a *library of async
  primitives*, not as a runtime.
- ⚠️ **`time` compiles but timers are unbacked on `unknown-unknown`.** Tokio's timer wheel needs a clock
  and a wakeup source; the browser target has neither, so `tokio::time::sleep` / `Instant::now()`
  **panic** (same root cause as §1.1). Timers *do* work under `wasm32-wasi`. In the browser, use
  `gloo-timers` instead.
- ⚠️ **`rt` is current-thread only, and `block_on` is a trap on the main thread.** You can build a
  `new_current_thread()` runtime, but `Runtime::block_on` blocks the calling thread — and in the browser
  that thread *is* the event loop you're running on, so it deadlocks/freezes the tab (§1.3). In practice
  you don't spin up a tokio runtime in the browser at all: **`wasm-bindgen-futures::spawn_local` is your
  executor**, and tokio supplies the primitives that run *inside* those tasks.
- ❌ **No I/O reactor, ever.** `tokio::net`, `tokio::fs`, `tokio::process`, `tokio::signal`, and
  `rt-multi-thread` aren't supported on any wasm target — they don't even compile, which is *why* you have
  to disable default features.

The practical recipe for browser wasm — tokio primitives, wasm-bindgen runtime, gloo timers:

```rust
use tokio::sync::mpsc;
use wasm_bindgen_futures::spawn_local;

let (tx, mut rx) = mpsc::channel::<Job>(32);         // tokio primitive: fine on wasm

spawn_local(async move {                             // wasm-bindgen drives the future
    loop {
        tokio::select! {                             // tokio macro: fine on wasm
            Some(job) = rx.recv() => handle(job).await,
            () = gloo_timers::future::sleep(          // gloo timer — NOT tokio::time::sleep
                    std::time::Duration::from_millis(16)) => tick().await,
            else => break,
        }
    }
});
```

**Rule of thumb:** on `wasm32-unknown-unknown`, treat tokio as `tokio::sync` + `tokio::select!`, let
`wasm-bindgen-futures` be the runtime, and use `gloo-timers` for time. Under `wasm32-wasi`, tokio's
current-thread runtime and timers additionally work. `block_on` is never viable on the browser main thread —
you cannot synchronously wait on the event loop that's currently running your code.

---

## 6. Threads (if you really must)

Default wasm is single-threaded and that's the happy path. Real threads exist but are a heavy opt-in:

- Need `SharedArrayBuffer` + the wasm **atomics/bulk-memory** features, built with
  `-C target-feature=+atomics,+bulk-memory,+mutable-globals` and a **custom std build**
  (`-Z build-std`, **nightly**).
- Threads are backed by **Web Workers**; `wasm-bindgen-rayon` gives you a Rayon pool on top.
- The page must be **cross-origin isolated** — served with `Cross-Origin-Opener-Policy: same-origin` and
  `Cross-Origin-Embedder-Policy: require-corp`. Without those headers, `SharedArrayBuffer` is unavailable
  and it silently won't work.

For most apps, prefer: keep the main thread responsive, push heavy compute into a Worker, and message
results back. Reach for shared-memory threads only for genuinely parallel number-crunching.

---

## 7. Build, size, and performance quirks

**Toolchains.** `wasm-pack build` (targets: `bundler` | `web` | `nodejs` | `no-modules`) for libraries/npm;
[`trunk`](https://trunkrs.dev) for whole web apps (bundles HTML/CSS/assets); or raw
`cargo build --target wasm32-unknown-unknown` + `wasm-bindgen` CLI. Post-process with
[`wasm-opt`](https://github.com/WebAssembly/binaryen) (Binaryen) — it routinely shaves 15–40%.

**Binary size is a product concern** (it's shipped over the wire and compiled in the browser). Wins:

```toml
[profile.release]
opt-level = "z"     # or "s" — optimize for size
lto = true
codegen-units = 1
panic = "abort"     # drop landing pads (already effectively abort on wasm)
strip = true
```

- **Panic strings and formatting (`fmt`) machinery bloat the binary** — every `unwrap`/`panic!` embeds a
  message + location. `console_error_panic_hook` in dev; consider trimming panics for prod.
- **Monomorphization** bloats wasm just like native; generic-heavy code = big modules.
- Profile size with [`twiggy`](https://github.com/rustwasm/twiggy) (and `wasm-opt -O`); the old `wee_alloc`
  size hack is now **unmaintained** — prefer the default `dlmalloc`, or shrink elsewhere.

**Performance quirks:**

- **Linear memory only grows, never shrinks.** Freeing Rust allocations returns memory to the wasm
  allocator, but the module's total footprint stays at its high-water mark for the page's life. A leak (a
  forgotten `free()`, a `.forget()` closure) is permanent. Watch peak usage.
- **Boundary crossings have overhead** — each call runs a JS shim. Chatty per-element APIs dominate real
  workloads; **batch data and minimize crossings**.
- **SIMD is opt-in:** `-C target-feature=+simd128` (plus `wasm-opt`), or `std::simd`/`wide`. Not on by
  default, and older runtimes may lack it.
- **No direct GPU/threads/syscalls** — you go through Web APIs (WebGPU/WebGL via `web_sys`), which means
  every frame's data crosses the boundary. Design the data flow, not just the kernels.
- Wasm is generally **~1.2–2× native** for compute-bound code and can be much slower when boundary-bound.
  Treat the JS boundary the way you'd treat a syscall or a cache miss: amortize it.

---

## 8. WASI: server-side wasm has an OS again

Target `wasm32-wasip1` (formerly `wasm32-wasi`) or `wasm32-wasip2` and run under Wasmtime/Wasmer/WasmEdge,
and most of §1 reverses: WASI provides **capability-scoped** syscalls, so `std::fs`, `std::env`, clocks,
random, and `stdin/stdout/stderr` **work** (subject to what the host grants — e.g. preopened directories).
Still missing/limited: threads (until `wasi-threads`), full networking (evolving via `wasi-sockets`), and
`std::process`. The **Component Model** (`wasm32-wasip2` + WIT) replaces the ad-hoc JS boundary with typed,
language-agnostic interfaces. If your goal is plugins, edge functions, or sandboxed compute rather than a
browser UI, WASI is the target you want — and the JS-boundary machinery in §4 doesn't apply.

---

## 9. Checklist, crates, further reading

### Pre-flight checklist for `wasm32-unknown-unknown`

1. **Right target?** Browser → `unknown-unknown`; server/sandbox → WASI (§0).
2. **`console_error_panic_hook` installed** at startup, or every panic is a mystery trap (§2).
3. **`getrandom` js backend enabled** if anything uses randomness or `HashMap` seeding (§1.2).
4. **Time via `web-time`/`chrono`** (or `Date`/`performance`), never bare `Instant::now()` (§1.1).
5. **Nothing blocks the main thread** — no `sleep`, no `block_on`, chunk long compute (§1.3).
6. **`.free()` every exported struct**; audit `Closure` lifetimes (`.forget()` = leak) (§4.5–4.6).
7. **No `.view()` kept across `await`/allocation** — memory growth detaches it (§4.4).
8. **Batch across the boundary**; strings/slices copy, `u64` becomes `BigInt` (§3–4).
9. **Size profile** (`twiggy`, `wasm-opt`, `opt-level="z"`, `strip`) before shipping (§7).
10. **No C-dependency crates**; pick pure-Rust (`rustls` over `openssl`, etc.) (§1).

### Crates & tools

| Need | Reach for |
|---|---|
| JS glue / bindings | `wasm-bindgen`, `js-sys`, `web-sys` |
| Build / bundle | `wasm-pack`, `trunk`, `wasm-bindgen-cli`, `wasm-opt` (Binaryen) |
| Async / promises | `wasm-bindgen-futures`, `gloo` (`gloo-timers`, `gloo-net`, `gloo-events`) |
| Time / dates | `chrono` (default `wasmbind` → `js_sys::Date`), `web-time` (`Instant`/`SystemTime`), `time` 0.3 (`wasm-bindgen` feature) |
| Randomness | `getrandom` (`js` feature / `wasm_js` backend) |
| Panics / logging | `console_error_panic_hook`, `log` + `console_log`, `tracing-wasm` |
| Structured data | `serde-wasm-bindgen`, `serde` |
| Threads / parallelism | `wasm-bindgen-rayon` (needs COOP/COEP + atomics) |
| Framework (SPA in Rust) | `leptos`, `yew`, `dioxus`, `sycamore` |
| Size profiling | `twiggy`, `wasm-opt -O` |
| Server-side / WASI | `wasmtime`, `wasmer`, `wasi`, `cargo-component` |

### Further reading

- [The `wasm-bindgen` Guide](https://rustwasm.github.io/wasm-bindgen/) — the reference for the JS boundary
- [Rust and WebAssembly Book](https://rustwasm.github.io/docs/book/) — end-to-end tutorial
- [`web-sys` / `js-sys` docs](https://rustwasm.github.io/wasm-bindgen/api/web_sys/) — every Web API binding
- [WebAssembly features roadmap](https://webassembly.org/features/) — what's shipped (SIMD, threads, GC…)
- [WASI](https://wasi.dev/) and the [Component Model](https://component-model.bytecodealliance.org/)
- [`twiggy`](https://github.com/rustwasm/twiggy) — code-size profiler for wasm
