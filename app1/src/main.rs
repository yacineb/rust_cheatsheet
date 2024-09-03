mod graph;

use graph::{Graph as _, GraphAdjacencyMatrix};

fn main() {
    /*
     2 -> 1
       -> 3
     1 -> 4

     0 -> 1
       -> 2
    */
    let graph = GraphAdjacencyMatrix::new([(0, vec![1, 2]), (2, vec![1, 3]), (1, vec![4])]);

    for item in graph.depth_first(0) {
        println!("{item}");
    }
    println!("End of main");
}
