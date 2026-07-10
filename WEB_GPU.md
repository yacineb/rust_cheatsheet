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

