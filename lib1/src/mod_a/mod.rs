pub fn say_hello() {
    println!("hello from A!");
}

/// A private Trait (will not be in module exports)
pub(crate) trait PrivateTraitA {}

mod sealed {
    /// Marker for sealed traits
    pub trait Sealed {}
}

/// Public trait which can't be implemented by dependant crates (definitely sealed)
pub trait CanUseCannotImplement: sealed::Sealed {}

mod sealed_except_clone {
    /// Marker for sealed traits
    pub trait Sealed {}

    impl<T> Sealed for T where T: Clone {}
}

/// Public trait which can't be implemented by dependant crates execpt when implementing type has
/// a trait bound Clone
pub trait CanUseCannotImplementExceptWhenClonable: sealed_except_clone::Sealed {}
