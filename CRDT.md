# CRDTs (the live-collaboration layer)

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