use lib1::{
    mod_a::say_hello,
    mod_iter::{Repeat, RepeatIterator},
};

fn main() {
    for data in Repeat::new(10, &|_| "ERR") {
        println!("{}", data);
    }

    say_hello();

    let u = RepeatIterator::n_items(10);
    let v: Vec<i32> = u.collect();
    assert_eq!(v, vec![0; 10]);
}
