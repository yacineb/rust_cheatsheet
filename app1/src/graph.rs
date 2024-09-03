use std::{
    collections::{HashMap, HashSet, VecDeque},
    hash::Hash,
};

#[derive(Debug)]
pub struct GraphAdjacencyMatrix<T> {
    adjacency: HashMap<T, Vec<T>>,
}

pub trait Graph<T>
where
    T: Hash + Eq + Copy,
{
    fn neighbours(&self, node: &T) -> Option<&Vec<T>>;

    fn depth_first(&self, root: T) -> NodesIterator<'_, T, true>
    where
        T: std::fmt::Debug;

    fn breadth_first(&self, root: T) -> NodesIterator<'_, T, false>
    where
        T: std::fmt::Debug;
}

pub struct NodesIterator<'a, T, const ORDERING: bool> {
    stack: VecDeque<T>,
    visited: HashSet<T>,
    graph: &'a GraphAdjacencyMatrix<T>,
}

impl<T, const ORDERING: bool> Iterator for NodesIterator<'_, T, ORDERING>
where
    T: Hash + Eq + Copy,
{
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let node = self.stack.pop_front()?;
            if self.visited.insert(node) {
                // if node have never been visited
                if let Some(neighbours) = self.graph.neighbours(&node) {
                    neighbours.iter().for_each(|n| {
                        if ORDERING {
                            self.stack.push_front(*n);
                        } else {
                            self.stack.push_back(*n);
                        }
                    })
                }

                // yield that node
                break Some(node);
            }
        }
    }
}

impl<'a, T, const ORDERING: bool> NodesIterator<'a, T, ORDERING> {
    pub fn new(root: T, graph: &'a GraphAdjacencyMatrix<T>) -> Self {
        Self {
            stack: [root].into(),
            visited: Default::default(),
            graph,
        }
    }
}

impl<T> GraphAdjacencyMatrix<T>
where
    T: Hash + Eq + Copy,
{
    pub fn new(input: impl IntoIterator<Item = (T, Vec<T>)>) -> Self {
        Self {
            adjacency: HashMap::from_iter(input),
        }
    }
}

impl<T> Graph<T> for GraphAdjacencyMatrix<T>
where
    T: Hash + Eq + Copy,
{
    fn neighbours(&self, node: &T) -> Option<&Vec<T>> {
        self.adjacency.get(node)
    }

    fn depth_first(&self, root: T) -> NodesIterator<'_, T, true>
    where
        T: std::fmt::Debug,
    {
        NodesIterator::new(root, self)
    }
    fn breadth_first(&self, root: T) -> NodesIterator<'_, T, false>
    where
        T: std::fmt::Debug,
    {
        NodesIterator::new(root, self)
    }
}
