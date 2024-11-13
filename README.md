# A very basic Rust workspace project skeleton

## Rust cheat sheet

### Bootstrap environment

- `cargo build` to build-install deps
- `cargo fmt` to format the code
- `cargo test` to test
- etc.

Testing setup:

- `rustup update stable` get latest stable release of rust

Clean cargo cache:

- rm -rf ~/.cargo/.package-cache
- rm -rf ~/.cargo/registry/index/*

### PartialEq vs Eq

- Not all sets have a total ordering. float types are an example with NaN, (+/-)Inf, cases when comparison has undefined result.
- `Eq` is for total ordering: reflexive, symmetric and transitive.

### Borrowing

- each `&T` is trivially `Copy` (and `Clone` of course).
- In a given block scope, `&mut T` , is exclusive and `!Clone`

### Copy vs Clone

- Copy is supposed to be cheap. It's implicit. Any 'move' is turned into copy when the type is Copy.
- Clone is a supertrait for Copy. any type that is Copy is also Clone.
- Clone is explicit.

### Box

- owns a value in the heap.
- Is not Copy, can only be moved or borrowed
- Box is Clone only when underlying type is Clone.
- Implements Deref and DerefMut.
- Box::leak is an elegant way to get a &'static mut

### Drop

- Dropping order for struct fields (by their index 'order of declaration' in their memory alignment).
- Dropping order for Variables: same than for stack frames popping (LIFO order).
- Inside of containers (array, table) : first to last
- If a field value is moved behind &mut self, then another value must be left in place (see Default trait, std::mem::take/swap pattern)

## Variance

- Types providing interior mutability of an inner T are usually invariant: `UnsafeCell<T>`, `Cell<T>`.
- `&'a T` is covariant in both `T` and `'a`
- `*mut T` is invariant in T.
- `*const T` is covariant in T.
- fn(T) -> U is covariant in U and contravariant in T.

### Lifetimes

- if 'b: 'a ('b outlives 'a), then 'b is a subtype of 'a. This is obviously not the formal definition, but it gets close enough to be
of practical use.

- Any type that provides mutability is generally invariant for the same reason—for example, `Cell<T>` is invariant in T.

- `Fn(T)` is contravariant in T

- T is a superset of both &T and &mut T

- &T and &mut T are disjoint sets

- T: 'static should be read as "T is bounded by a 'static lifetime"

- if T: 'static then T can be a borrowed type with a 'static lifetime or an owned type
        - since T: 'static includes owned types that means T
        - can be dynamically allocated at run-time
        - does not have to be valid for the entire program
        - can be safely and freely mutated
        - can be dynamically dropped at run-time
        - can have lifetimes of different durations

- T:'a is more general and more flexible than &'a T, &'a T implies T:'a

- T: 'a accepts owned types, owned types which contain references, and references

- &'a T only accepts references

- if T: 'static then T: 'a since 'static >= 'a for all 'a

- Rust compiler error messages suggest fixes which will make your program compile which is not that same as fixes which will make you program compile and best suit the

- lifetimes cannot grow or shrink or change in any way at run-time and they are statically checked at compile time

- Rust borrow checker will always choose the shortest possible lifetime for a variable assuming all code paths can be taken

- try not to re-borrow mut refs as shared refs, or you're gonna have a bad time

- re-borrowing a mut ref doesn't end its lifetime, even if the ref is dropped

### Global allocator

- jemalloc used to be the default allocation in rust. and now it's the default system allocator. The default system allocation is well-known, and jemalloc had compatibility issues especially with windows. another reason is that it does not work with valgrind and it has binary size overhead (300kb). For most of scenarios the default system alloator is OK.
- For heavy memory workloads, jemalloc has a better performance.

### Rust conversions

Generic Implementations

- `From<T> for U` implies [`Into`]`<U> for T`
- `From` is reflexive, which means that `From<T> for T` is implemented
- `IntoIterator` is also reflexive, which means that `IntoIterator` for `T` is implemented when `T: Iterator`

### Static lifetime promotion

Any stuff which is known at compile-time : literals, const. are defacto `'static`.

### Functions

Any Fn **which does not capture any variable**:

- Is zero-sized.
- coerces to `fn(_) -> _` (a function pointer)
- is `'static`

Function pointers are regular pointers and have same size than `usize`.

### Generics

- Use trait with associated type when only and only 1 implementation is expected for a type for that generic parameter. (for iterators for example, whtever the iteratee type is, the iterator impl is the same)

### ZST

Those types occupy no space in memory. They exist purely for type-level information or as markers.
Multiple instances of a ZST are indistinguishable because they all occupy zero space. Therefore, copying or moving a ZST is essentially a no-op.

Some example of Zero-sized types:

- Non capturing lambdas.
- unit `()` type
- `PhantomData<T>` and PhantomPinned.
- Any struct with no field.
- any [T; N] where T is ZST (all of the array items have the same address)

Rust’s compiler optimizes ZSTs heavily. For instance, in a generic context, if a type is a ZST, the compiler can remove any associated memory allocations or computations related to that type, leading to more efficient code.

### Aliasing

Event though it's possible to have 2 Box instance pointing to the same `*mut T`, that does not mean it guarantees aliasing. The reason is that rust compiler emit `#[noalias]` for Box, which means that compiler could suppress aliasing an do some optimization stuff (inlining for example if the memory lauyout fits)

The only clean way to wrap Box with `MaybeUninit<T>` which suppresses `#[noalias]`

## Threading

### Smart Pointers

- `Rc<T> is !Send and !Sync` whatever T is, because Rc is not thread safe (reference counting results in a race condition). Rc provides ability to have multiple owners
- `RefCell<T> and Cell<T>` allow **single owner**, they are Send if T: Send
- `Rc<T>` allow only build-time check immutable borrowing, emulates multiple owners, cheap Clone, `RefCell<T>` has dynamic (mutable/immutable) borrowing rules, runtime-checked
- `Rc<T>` does not allow mutating T unless T has interior mutability semantics (eg. RefCell)

- `Arc<T>` is a thread-safe version of `Rc<T>`. if `T: Send, Sync` then `Arc<T>: Send, Sync`

### Raw pointers

- All of raw pointers `*mut T` `*const T` are marked as `!Send` and `!Sync`.

If you need to send a raw pointer, create newtype `struct Ptr(*const u8)` and unsafe impl Send for Ptr {}. Just ensure you may send it.

Prefer `NonNull<T>` over *mut T for those reasons:

- Covariance with T.
- Clearer intent and type safety.
- Dereferencing is explicit and can only be done inside an unsafe block.
- Is Send, Sync if T: Send, T:Sync

- `UnsafeCell<T>` is the only idiomatic way in rust at the moment, to get a mutable access to a shared reference

### Spinlocks

Spinlocks should be avoided when possible <https://matklad.github.io/2020/01/02/spinlocks-considered-harmful.html>

### Tricky threading cases

`T` is `!Send` but i need to "send it" across threads (example `*mut U` for any type U is automatically !Send)

**"Safe" Solutions**

- **Solution 1**: if T creation is cheap and has very few side effects, or if it's serializable or marshallable then don't move it and recreate one when needed.
- **Solution 2**: store T in thread_local slots. this way you'll never need to move T between threads. Disadvantages: memory overhead, drop() for T can only be ran by threads owning slot of T
- **Solution 3**: case when having multiple instances of T is not possible or T has a thread affinity. then you'll have no other choice than using messaging/signalling (mpsc::channel or other constructs). Disadvantage: tricky to implement, this is common in ui libraries such as *gtk+*

**Unsafe solutions**

- `unsafe impl Send for T {}`: You might have a scenario where it's ok to Send T event if it's !Send. You'll then need to check the implementation of T.

### Lock-free / wait-free

Atomics and memory ordering

- `compare_exchange` is fairly expensive operation, spinning on `compare_exchange` can lead to "owners bounce"
- prefer `compares_exchange_weak` if it's a condition in a loop (more efficient on ARM especially), that doesnot generate nested loop.
- `wait free` means no compare-and-swap loop, no spinning

### Mutex

- std::sync:Mutex provides:
        - poisoning: if the lock holder panics then the mutex becomes poinsed (any attempt to take the lock will panic)
        - uses system mutex, kernel space
        - Guarantees fairness: all of the threads have the same chance to take the lock.
        - The thread is block if waiting of (parked). That causes context-swicth overhead.
        - Does not provide reentrance.

- Mutex from parking lot:
  - Much less overhead, more efficient for cases when locks are taken briefly. Active wait, yielding mechanism minimises context-switch. much likely to run in use space.
  - no fairness gurantees.
  - Less memory overhead.
  - Provides reentrance and timeouts

### JoinSet and LocalSet

- A JoinSet can be used to await the completion of some or all of the tasks in the set. The set is not ordered, and the tasks will be returned in the order they complete.

- LocalSet: In some cases, it is necessary to run one or more futures that do not implement Send and thus are unsafe to send between threads. In these cases, a local task set may be used to schedule one or more !Send futures to run together on the same thread.

### Other sync primitives

- Barrier: A barrier will block n-1 threads which call wait() and then wake up all threads at once when the nth thread calls wait(). Used for preventing interleaving in some scenarios and do coordination of computation.

- CondVar

- Notify

- OneShot

## Pinning and self-referential structs

- For self referential structs you might use the excellent: <https://github.com/someguynamedjosh/ouroboros>

## Performance optimization hints

### Profilers

- cachegrind
- valgrind
- dhat

### Compilation options

> `IMPORTANT`: you should make sure for CI/CD that the compilation environment is closest possible to the runtime environment (Os, CPU arch)

- Don't forget `--release` flag when doing cargo build for production or performance benchmarking
- `llvm` compiler options such as [lto](https://doc.rust-lang.org/cargo/reference/profiles.html#lto) (Link-Time Optimization). It's a whole-program optimization that could optimize performances up to 20%
- Target newest CPU instructions: If you do not care that much about the compatibility of your binary on older (or other types of) processors, you can tell the compiler to generate the newest (and potentially fastest) instructions specific to a certain CPU architecture. As an example `$ RUSTFLAGS="-C target-cpu=native" cargo build --release`
- inlining: usually you don't need to explicitly annotate function with `#[inline()]` (because compiler is smart enought) but in some scenarios that could make a difference

### Reduce binary size

- Default `panic!` behaviour is unwinding, this has overhead in terms of code and processing. One optimization is to make panic! abort directly:

```toml
[profile.release]
panic = 'abort'
```

- print dependencies: `cargo tree`

- Binary size profiler: <https://github.com/google/bloaty>. For wasm use `twiggy`: <https://github.com/rustwasm/twiggy>

### Speed up build

- For Generics: Use an Inner Non-Generic Function If you have a generic function, it will be compiled for every type you use it with. This can be a problem if you have a lot of different types. A common solution is to use an inner non-generic function. This way, the compiler will only compile the inner function once. This is a trick often used in the standard library. For example, the implementation of read_to_string.
- Wraps deps as dylib: <https://github.com/rksm/cargo-add-dynamic>
- Use `lld` as drop-in LLVM linker. for macos: <https://davidlattimore.github.io/posts/2024/02/04/speeding-up-the-rust-edit-build-run-cycle.html>
- For ci, use <https://github.com/Swatinem/rust-cache> github action.
- For github actions use faster runners: <https://www.ubicloud.com/use-cases/github-actions>
- For ci 2: disable incremental build `CARGO_INCREMENTAL: 0`
- deny warnings in release mode : `RUSTFLAGS: -D warnings`
- Tweak codegen options: <https://doc.rust-lang.org/stable/rustc/codegen-options/>
- Use the new complier frontend, parallel build:

```toml
[build]
rustflags = ["-Z", "threads=8"]
```

- Profil build time: `cargo build --timings`

- Find the most expense functions in terms of compilation: `cargo llvm-lines | head -20`

- Split into workspaces, smaller code units lead to better build speed (incrementality)

- Turn off non-required features : <https://github.com/ToBinio/cargo-features-manager>

- Use the amazing utility for easy features toggling: <https://docs.rs/cfg_aliases/latest/cfg_aliases/>

- Speed up local development on macos:

```toml
[profile.dev]
split-debuginfo = "unpacked"
```

- Example of optimized compilation options on `Bevy` project:

```toml
# Add the contents of this file to `config.toml` to enable "fast build" configuration. Please read the notes below.

# NOTE: For maximum performance, build using a nightly compiler
# If you are using rust stable, remove the "-Zshare-generics=y" below (as well as "-Csplit-debuginfo=unpacked" when building on macOS).

[target.x86_64-unknown-linux-gnu]
linker = "/usr/bin/clang"
rustflags = ["-Clink-arg=-fuse-ld=lld", "-Zshare-generics=y"]

# NOTE: you must manually install https://github.com/michaeleisel/zld on mac. you can easily do this with the "brew" package manager:
# `brew install michaeleisel/zld/zld`
[target.x86_64-apple-darwin]
rustflags = ["-C", "link-arg=-fuse-ld=/usr/local/bin/zld", "-Zshare-generics=y", "-Csplit-debuginfo=unpacked"]

[target.x86_64-pc-windows-msvc]
linker = "rust-lld.exe"
rustflags = ["-Zshare-generics=y"]

# Optional: Uncommenting the following improves compile times, but reduces the amount of debug info to 'line number tables only'
# In most cases the gains are negligible, but if you are on macos and have slow compile times you should see significant gains.
#[profile.dev]
#debug = 1
````

### PGO

Profile-guided optimization [PGO](https://doc.rust-lang.org/rustc/profile-guided-optimization.html) is a compilation model where you compile your program, run it on sample data while collecting profiling data, and then use that profiling data to guide a second compilation of the program.

### Follow linting recommendations and write idiomatic rust

- Some warnings raised by `cargo clippy` might suggest much better coding idioms which lead to better compiler optimization opportunities
- use Default on types when needed instead of explicitly writing trivial boilerplate code for defaults (cans ave stack allocations and make maintainability better)

### Hashing

HashSet and HashMap are two widely-used types. The default hashing algorithm is not specified, but at the time of writing the default is an algorithm called SipHash 1-3. This algorithm is high quality—it provides high protection against collisions—but is relatively slow, particularly for short keys such as integers.

If profiling shows that hashing is hot, and HashDoS attacks are not a concern for your application, the use of hash tables with faster hash algorithms can provide large speed wins. There are many alternatives in the Rust crates.

> Some keys don’t need hashing. For example, integer keys which are almost random. For such a such case, the distribution of the hashed values won’t be that different to the distribution of the values themselves. In this case the `nohash_hasher` crate can be useful.

### Heap allocations

**For Vecs and strings:**

- Preallocate with capacity if capacity is known or a size hint is bounded
- Use `small string optimization` (similar than the one in C++), for scenarios where vecs or string are much likely to be small. (see `smartstring` `smallvec` and `smallstring` crates)

**Cow:**

Sometimes you have some borrowed data, such as a &str, that is mostly read-only but occasionally needs to be modified. Cloning the data every time would be wasteful. Instead you can use “clone-on-write” semantics via the Cow type, which can represent both borrowed and owned data.

**Reduce types size:**

Smaller integers: it is often possible to shrink types by using smaller integer types. For example, while it is most natural to use usize for indices, it is often reasonable to stores indices as u32, u16, or even u8, and then coerce to usize at use points. Example 1, Example 2.

Wrap large types inside `Box`

**Vec<T> vs Rc<[T]>, Box<[T]>**

In scenarios where the vector never needs to be mutated, you might prefer Rc or Box , because they have lower memory overhead (no `capacity` field). Depending on scenarios, they might be better alternatives.

**Advanced scenarios:**

- You migh consider looking into the generated assembly code using this AMAZING [tool]<https://godbolt.org/> --> <https://github.com/compiler-explorer/compiler-explorer>
- Using an alternative Allocator such as jemalloc or mimalloc

### overhead of async

In critical low latency apps, async can bloat performance and add overhead and complexity. Of course that depends!

- Async can rely on a scheduler (ie tokio, async-std runtimes etc..) to manage tasks the overhead of scheduling can include context switching and event loop handling.
- Async functions are converted to state machines, managing that state across await points can introduce overhead.
- memory access patterns can be more complex and that might impact cache performance.

Ideally for those kind of apps, i'd adopt hybrid approch where i use async for I/O-bound tasks (fetching data from exchanges, http requests etc..) and use synchronous code
for decision making tasks that require low latency and predictable execution.

Of course , the benefits of async in rust overweights its cost, and its cost is still low.

### Design recommendations

- Type-driven design: Leverage type system to enforce invariants. For example if you except an integer to be strictly positive, use NonZeroU32 instead of u32.
- Parse-don't validate , use the `Newtype pattern`
- Leverage state type pattern , using ZSTs and generics: <https://cliffle.com/blog/rust-typestate/>

### Other general-puropose performance recommendations

- Avoid when possible dynamic dispatch/vtable (&dyn Trait)
- For threading sync primitives prefer using `parking_lot` crate over std implementations.
- Eliminate bound checks on arrays/vecs, numbers computation [cookbook](<https://github.com/Shnatsel/bounds-check-cookbook/>)
- Logging/debugging can slow down a program
- Make expensive computations/allocations lazy
- Cache efficieny/cache locality. prefer contiguous data access, data that fits cache lines has much faster memory access.
- Leverage std::mem::take and std::mem::swap for types implementiung Default in some scenario can avoid unnessary memory allocations especially when mutating.

## Further readings

[Rust cheat sheet](https://cheats.rs/)
[Rust containers cheat sheet](https://docs.google.com/presentation/d/13IgYIal8xClkBGJz-3WSIGduZEPhnSCOq29Ssrg5QZc/edit?usp=sharing)
[Rust perf book](https://nnethercote.github.io/perf-book)
[About workspace and members](https://doc.rust-lang.org/book/ch14-03-cargo-workspaces.html)
[About filestruct](https://doc.rust-lang.org/stable/rust-by-example/mod.html)
[Mods and "submods"](https://doc.rust-lang.org/stable/rust-by-example/mod/split.html)
[Visibility and Privacy](https://doc.rust-lang.org/reference/visibility-and-privacy.html)
[Best of rust tools](https://blessed.rs/crates)
