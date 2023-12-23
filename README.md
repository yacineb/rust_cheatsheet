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

### Borrowing

- each &T is trivially Copy.
- In a given block scope, &mut T , is exclusive

### Copy vs Clone

- Copy is supposed to be cheap. It's implicit. Any 'move' is turned into copy when the type is Copy.
- Clone is a supertrait for Copy. any type that is Copy is also Clone.
- Clone is explicit

### Box

- owns a value in the heap.
- Is not Copy, can only be moved or borrowed
- Box::clone : Is Clone only when underlying type is Clone.
- Implements Deref and DerefMut.
- Box::leak is an elegant way to get a &'static mut

### Drop

- Dropping order for struct fields (by their index 'order of declaration' in their memory alignment).
- Dropping order for Variables: same than for stack frames popping. reverse order.
- Inside of containers (array, table) : first to last
- If a field value is moved behind &mut self, then another value must be left in place (see Default trait, std::mem::take/swap pattern)

### Lifetime variance

if 'b: 'a ('b outlives 'a), then 'b is a subtype of 'a. This is obviously not the formal definition, but it gets close enough to be
of practical use.

Any type that provides mutability is generally invariant for the same reason—for example, `Cell<T>` is invariant in T.

Fn(T) is contravariant in T

### Rust conversions

Generic Implementations

- `From<T> for U` implies [`Into`]`<U> for T`
- `From` is reflexive, which means that `From<T> for T` is implemented
- `IntoIterator` is also reflexive, which means that `IntoIterator` for `T` is implemented when `T: Iterator`

### Aliasing

Event though it's possible to have 2 Box instance pointing to the same `*mut T`, that does not mean it guarantees aliasing. The reas is that rust compiler emit `#[noalias]` for Box, which means that compiler could suppress aliasing an do some optimization stuff (inlining for example if the memory lauyout fits)

The only clean way to wrap Box with `MaybeUninit<T>` which suppresses `#[noalias]`

## Threading

### Smart Pointers

- `Rc<T> is !Send and !Sync` whatever T is, because Rc is not thread safe (reference counting results in a race condition). Rc provides ability to have multiple owners
- `RefCell<T> and Cell<T>` allow single owner, they are Send if T: Send
- `Rc<T>` allow only build-time check immutable borrowing, `RefCell<T>` has dynamic (mutable/immutable) borrowing rules, runtime-checked
- `Rc<T>` does not allow mutating T unless T has interior mutability semantics (eg. RefCell)

- `Arc<T>` is a thread-safe version of `Rc<T>`. if `T: Send, Sync` then

### Raw pointers

- All of raw pointers `*mut T` `*const T` are marked as `!Send` and `!Sync` If you need to send a raw pointer, create newtype `struct Ptr(*const u8)` and unsafe impl Send for Ptr {}. Just ensure you may send it.

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

- `unsafe impl Send for T {}`: that needs enabling of unstable rust channel. You might have a scenario where it's ok to Send T event if it's !Send. You'll then need to check the implementation of T.

### Atomics and memory ordering

- `compare_exchange` is fairly expensive operation, spinning on `compare_exchange` can lead to "owners bounce"
- prefer ``compares_exchange_weak`if it's a condition in a loop (more efficient on ARM especially), that doesnot generate nested loop.
- `wait free` means no compare-and-swap loop, no spinning

## Performance optimization hints

### Profilers

- cachegrind
- dhat

### Compilation options

> `IMPORTANT`: you should make sure for CI/CD that the compilation environment is closest possible to the runtime environment (Os, CPU arch)

- Don't forget `--release` flag when doing cargo build for production or performance benchmarking
- `llvm` compiler options such as [lto](https://doc.rust-lang.org/cargo/reference/profiles.html#lto) (Link-Time Optimization). It's a whole-program optimization that could optimize performances up to 20%
- Target newest CPU instructions: If you do not care that much about the compatibility of your binary on older (or other types of) processors, you can tell the compiler to generate the newest (and potentially fastest) instructions specific to a certain CPU architecture. As an example `$ RUSTFLAGS="-C target-cpu=native" cargo build --release`
- inlining: usually you don't need to explicitly annotate function with `#[inline()]` (because compiler is smart enought) but in some scenarios that could make a difference

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

**Advanced scenarios:**

- You migh consider looking into the generated assembly code using this AMAZING [tool]<https://godbolt.org/> --> <https://github.com/compiler-explorer/compiler-explorer>
- Using an alternative Allocator such as jemalloc or mimalloc

### Other general-puropose recommendations

- Avoid when possible dynamic dispatch/vtable (&dyn Trait)
- For threading sync primitives prefer using `parking_lot` crate over std implementations.
- Eliminate bound checks on arrays/vecs, numbers computation [cookbook](<https://github.com/Shnatsel/bounds-check-cookbook/>)
- Logging/debugging can slow down a program
- Make expensive computations/allocations lazy
- Cache efficieny/cache locality. prefer contiguous data access, data that fits cache lines has much faster memory access

## Further readings

[Rust cheat sheet](https://cheats.rs/)
[Rust containers cheat sheet](https://docs.google.com/presentation/d/13IgYIal8xClkBGJz-3WSIGduZEPhnSCOq29Ssrg5QZc/edit?usp=sharing)
[Rust perf book](https://nnethercote.github.io/perf-book)
[About workspace and members](https://doc.rust-lang.org/book/ch14-03-cargo-workspaces.html)  
[About filestruct](https://doc.rust-lang.org/stable/rust-by-example/mod.html)  
[Mods and "submods"](https://doc.rust-lang.org/stable/rust-by-example/mod/split.html)  
[Visibility and Privacy](https://doc.rust-lang.org/reference/visibility-and-privacy.html)
