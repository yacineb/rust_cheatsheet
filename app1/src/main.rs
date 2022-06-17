use std::thread;

#[derive(Clone, Debug)]
struct Toto {}

fn main() {
    let x = "Hello".to_owned();

    let x = thread::spawn(move || println!("{}", x));
    x.join().unwrap();
}
