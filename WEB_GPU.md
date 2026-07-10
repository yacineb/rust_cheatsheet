# Technical Crash Course

Four modules for the cross-platform / Engine interview: **FFI · Graphics fundamentals · wgpu · CRDTs.** Each ends with *what they'll ask* and *how to answer*. Study order and time budget at the bottom.

The framing to hold throughout: Photoroom writes a **Rust core** that is compiled to **WASM for web** and exposed via **FFI to iOS (Swift) and Android (Kotlin)**, rendered through **wgpu**, with a **CRDT-based** live-collaboration layer. Your job in the interview is to show you understand the *boundaries* between these worlds and the failure modes each one hides.
---

## Graphics fundamentals (the mental model, not the math)

You don't need to write shaders live. You need to not be *lost* when a graphics engineer talks. Here's the model.

### Why a GPU at all
CPU = few fast cores optimized for latency and branching. GPU = thousands of tiny cores optimized for **throughput** — the same small program run in parallel over millions of data points (pixels, vertices). Image editing is embarrassingly parallel (every pixel is independent), which is why Photoroom lives on the GPU.

### The render pipeline (classic path)
Data flows one direction through fixed and programmable stages:
1. **Vertex buffer** — your geometry (positions, UVs) uploaded to GPU memory.
2. **Vertex shader** — runs once per vertex; transforms it into screen space. *Programmable.*
3. **Rasterization** — fixed-function; figures out which pixels a triangle covers.
4. **Fragment (pixel) shader** — runs once per covered pixel; computes its color. *Programmable.* This is where sampling textures, blending, and filters happen.
5. **Framebuffer / render target** — where the result lands (the screen, or an offscreen texture).

### The compute pipeline (matters more for an image editor)
For filters, effects, and transformations you often skip the geometry path entirely and run a **compute shader**: a general parallel program over a texture/buffer, no triangles involved. Blur, color grading, background removal post-processing — compute shader territory. Mention this; it signals you understand Photoroom's actual workload, not just "3D games."

### The mental model that wins points
- **The GPU is a separate, asynchronous machine.** The CPU records a list of commands (a *command buffer*) and **submits** it. The GPU runs later. You do not call the GPU synchronously.
- **Memory is separate and explicit.** Data must be *uploaded* (buffers, textures) across the bus. Reading results *back* to the CPU is a stall — expensive. **Minimizing CPU↔GPU synchronization is the whole performance game.**
- **Textures vs. buffers.** Textures are for image data (sampled, filtered, blended). Buffers are for structured data (vertices, uniforms, compute I/O).
- **Blend modes / compositing.** An image editor stacks layers; the fragment stage combines them (normal, multiply, screen...). "Compositing layers with blend modes" is the domain vocabulary.
- **Offscreen render targets.** You render intermediate results into textures, not the screen — the basis of multi-pass effects and non-destructive editing.

> **What they'll ask:** *"How would you apply a filter to an image efficiently?"* or *"Walk me through what happens from an edit to pixels on screen."*
> **How to answer:** Upload the image as a texture → run a compute (or fragment) shader over it in parallel → write to an offscreen target → composite → present. Emphasize keeping work on-GPU and avoiding readbacks. Admit the boundary of your experience honestly, then reason from the throughput/sync model.

---

## Module 3 — wgpu (the concrete API over that model)

### What it is
**wgpu** is a safe, pure-Rust implementation of the **WebGPU** standard. One API that translates to **Vulkan (Linux/Android), Metal (macOS/iOS), D3D12 (Windows)** natively, and to **WebGPU or WebGL2** when compiled to WASM. This is *why Photoroom chose it*: one renderer, every platform. It's the graphics half of "write the core once."

### The object graph (memorize this — it's the backbone of any wgpu question)
```
Instance                      // entry point; picks backends
  └─ Adapter                  // a physical GPU
       └─ Device + Queue      // Device = resource factory; Queue = submits work
Surface                       // the thing you present to (window / canvas)
```
- **Instance** → request an **Adapter** (a GPU) → request a **Device** (your handle for creating resources) and a **Queue** (where you submit commands).
- **Surface** is the presentation target, configured with a format and size; on web it's derived from a `<canvas>`.
- Handles are **reference-counted and cloneable** — creating a resource that references another keeps the dependency alive automatically. (Good detail to drop.)

### Resources
- **Buffer** — GPU memory for vertices, uniforms, compute data.
- **Texture** / **TextureView** — image data and a typed view into it.
- **Sampler** — how a texture is filtered/wrapped when sampled.
- **BindGroup** + **BindGroupLayout** — how you bind resources (textures, buffers, samplers) so shaders can see them. The layout is the contract; the bind group is the concrete set.

### Pipelines and shaders
- **RenderPipeline** / **ComputePipeline** — the configured state (shaders, layouts, formats) for a draw or a dispatch.
- **Shaders are written in WGSL** (WebGPU Shading Language). **Naga** is wgpu's shader translator — it converts WGSL to Metal/SPIR-V/HLSL/GLSL per backend. On native-web it passes WGSL to the browser; with the `webgl` feature it translates WGSL→GLSL.

### Command flow (every frame)
```
CommandEncoder                     // records commands
  ├─ begin_render_pass(...) / begin_compute_pass(...)
  │     set_pipeline, set_bind_group, draw()/dispatch()
  └─ finish() -> CommandBuffer
Queue::submit([command_buffer])    // hand it to the GPU
surface.present()                  // show the frame
```
This is the concrete instance of "record → submit → present" from Module 2.

### The WASM specifics (cross-platform team will care)
- Surface comes from the canvas; you select a backend (WebGPU, or GL fallback via the `webgl` feature).
- `std::time::SystemTime` **panics in the browser** — you swap in `web-time`. This is the canonical example of "the WASM target isn't just Rust." Threads and blocking I/O are similarly constrained.
- Binary size and the `wasm-bindgen` glue boundary matter for load time.

> **What they'll ask:** *"Have you used wgpu? Walk me through setting up a minimal render."* or *"How does one wgpu codebase target iOS, Android, and web?"*
> **How to answer:** If you haven't shipped it, say so, then *demonstrate the mental model*: Instance→Adapter→Device/Queue, resources + bind groups, pipeline + WGSL, encoder→submit→present. For cross-platform: wgpu abstracts the backend; you isolate platform bits (surface creation, `#[cfg(target_arch = "wasm32")]`) at the edges and keep the render logic shared. Naga handles shader translation per platform.

---

## Module 4 — CRDTs (the live-collaboration layer)

### The problem
Multiple users edit the same document simultaneously, sometimes offline, and it must **converge to the same state everywhere without conflicts and without a central lock.** That's the Figma/Google-Docs problem.

### The idea
A **CRDT (Conflict-free Replicated Data Type)** is a data structure whose merge operation is **commutative, associative, and idempotent** — a mathematical join over a lattice. Because of those properties, replicas that have seen the same set of updates (in *any order, any number of times*) provably reach the **same state**. No coordination server required for correctness.

### Two families
- **State-based (CvRDT)** — replicas exchange their whole state and `merge()` via the lattice join. Simple, robust to duplication/reordering, but heavy to ship full state.
- **Op-based (CmRDT)** — replicas broadcast individual operations; requires reliable, causally-ordered delivery, but ops are small. Most practical collaborative editors are op-based (or delta-based, a hybrid).

### The building blocks (name these)
- **Counters:** G-Counter (grow-only), PN-Counter (inc/dec).
- **Sets:** G-Set, OR-Set (observed-remove, handles concurrent add/remove).
- **Registers:** LWW-Register (last-writer-wins via timestamps), MV-Register (multi-value).
- **Sequences** (the hard one — for text and ordered layer lists): RGA, Logoot, **YATA** (the algorithm behind Yjs), Fugue. These assign stable unique IDs to elements and use **tombstones** for deletes so concurrent inserts interleave deterministically.

### CRDT vs. OT — know the tradeoff
- **OT (Operational Transformation)** — what Google Docs historically used. Transforms concurrent ops against each other; needs a **central server** and correct transform functions (notoriously hard to get right).
- **CRDT** — **local-first**, works P2P and offline, no central authority needed for convergence. Cost: **metadata/tombstone overhead** (memory grows with edit history) — though modern libs (diamond-types, Yjs) have largely tamed this.

### Libraries (and the honest interview move)
- **Automerge** — Rust, general-purpose JSON-like CRDT, strong ecosystem.
- **yrs** — the Rust port of **Yjs**, battle-tested for editors.
- **diamond-types** — Rust, extremely fast text CRDT (Seph Gentle's work).
- The **presence/awareness** layer (cursors, who's-online) is usually a *separate, ephemeral* channel — **not** part of the CRDT, since it doesn't need to persist or converge.

### Mapping to Photoroom
The document is a canvas: layers, objects, transforms, properties. Model it as a CRDT (e.g. an OR-Set/map of objects + LWW or sequence CRDTs for ordering and properties). Each user edit is an op that merges deterministically. Live cursors ride the separate awareness channel.

> **What they'll ask:** *"How would you build the collaboration engine?"*
> **How to answer:** CRDT for convergent shared state, local-first for offline + responsiveness, op/delta-based for bandwidth; contrast with OT (central server, transform complexity). Then the maturity signal: *"I'd build on Automerge or yrs rather than roll a sequence CRDT — getting interleaving and tombstone GC right is a research-grade problem."* Keep presence out of the CRDT.

---

## How to weave it together (the senior signal)

The interviewer wants to see you connect the modules, because Photoroom's system *is* the connection:
- The **CRDT** produces authoritative document state (in the shared Rust core).
- The **FFI/WASM boundary** exposes that core to each platform's UI.
- **wgpu** renders the document, ideally with **compute shaders** for the image work.
- Every boundary has a failure mode — panics across FFI, readback stalls on the GPU, tombstone growth in the CRDT, `std::time` in WASM — and naming the failure mode (not just the pattern) is the coaching note you already know applies to you.

## Study order & time budget (before the interview)

1. **FFI — 1.5h.** Your highest-leverage, highest-confidence area. Rehearse the "opaque handle + ownership + uniffi/wasm-bindgen" answer out loud, anchored to pyo3. This is likely the deciding question.
2. **CRDT — 1h.** Concept + families + OT contrast + name three Rust libs. Cheap to sound credible.
3. **wgpu — 1h.** Memorize the object graph and command flow. Skim the "Learn Wgpu" pipeline-setup chapter purely for vocabulary.
4. **Graphics fundamentals — 0.5h.** The async-machine + minimize-sync model, plus "compute shader for filters."

Total ~4h converts your three honest gaps into "I have the mental model and the boundary discipline; I haven't shipped this specific stack yet, and here's how fast I ramp."