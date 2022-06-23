use lib1::mod_iter::RepeatIterator;

fn main() {
    let u = RepeatIterator::n_items(10);
    let v: Vec<i32> = u.collect();
    assert_eq!(v, vec![0; 10]);
}
