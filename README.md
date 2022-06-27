# A very basic Rust workspace project skeleton

## Further readings
[About workspace and members](https://doc.rust-lang.org/book/ch14-03-cargo-workspaces.html)  
[About filestruct](https://doc.rust-lang.org/stable/rust-by-example/mod.html)  
[Mods and "submods"](https://doc.rust-lang.org/stable/rust-by-example/mod/split.html)  
[Visibility and Privacy](https://doc.rust-lang.org/reference/visibility-and-privacy.html)


## Rust cheat sheet

### Bootstrap environment

- `cargo build` to build-install deps
- `cargo fmt` to format the code
- `cargo test` to test
- etc.

Testing setup:

- `rustup update stable` get latest stable release of rust

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
-  Box::clone : Is Clone only when underlying type is Clone.
- Implements Deref and DerefMut.



### Drop

- Dropping order for struct fields (by their index 'order of declaration' in their memory alignment).
- Dropping order for Variables: same than for stack frames popping. reverse order.
- Inside of containers (array, table) : first to last
- If a field value is moved behind &mut self, then another value must be left in place (see Default trait, std::mem::take/swap pattern)


### Lifetime variance

if 'b: 'a ('b outlives 'a), then 'b is a subtype of 'a. This is obviously not the formal definition, but it gets close enough to be
of practical use.

Any type that provides mutability is generally invariant for the same reason—for example, Cell<T> is invariant in T.

Fn(T) is contravariant in T