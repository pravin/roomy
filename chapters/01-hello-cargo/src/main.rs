use std::env;
use std::fs;

fn main() {
    println!("Hello, Roomy!");

    // The first thing typed after the program's name, or "." (the current
    // folder) if there isn't one.
    let path = env::args().nth(1).unwrap_or_else(|| String::from("."));

    let meta = fs::metadata(&path).expect("couldn't look at that");
    if meta.is_dir() {
        println!("{path} is a folder");
    } else {
        println!("{path} is a file of {} bytes", meta.len());
    }
}
