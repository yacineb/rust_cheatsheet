/// An example reimplement of std::iter::repeat reimplemented
pub struct RepeatIterator<'a, T> {
    idx: usize,
    length: usize,
    factory: Box<&'a dyn Fn(usize) -> T>,
}

impl<'a, T> RepeatIterator<'a, T> {
    pub fn new(factory: &'a dyn Fn(usize) -> T, length: usize) -> Self {
        Self {
            idx: 0,
            length,
            factory: Box::new(factory),
        }
    }

    pub fn n_items(length: usize) -> Self
    where
        T: Default,
    {
        Self::new(&|_| Default::default(), length)
    }
}

impl<'a, T> Iterator for RepeatIterator<'a, T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        if self.idx >= self.length {
            None
        } else {
            self.idx += 1;

            Some((self.factory)(self.idx))
        }
    }
}

pub struct Repeat<'a, T> {
    length: usize,
    factory: Option<&'a dyn Fn(usize) -> T>,
}

impl<'a, T> Repeat<'a, T> {
    pub fn new(length: usize, factory: &'a dyn Fn(usize) -> T) -> Self {
        Self {
            length,
            factory: Some(factory),
        }
    }
}

impl<'a, T> IntoIterator for Repeat<'a, T> {
    type IntoIter = RepeatIterator<'a, T>;
    type Item = <RepeatIterator<'a, T> as Iterator>::Item;

    fn into_iter(mut self) -> Self::IntoIter {
        RepeatIterator::new(self.factory.take().unwrap(), self.length)
    }
}
