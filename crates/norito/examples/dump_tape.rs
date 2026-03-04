#![cfg(feature = "json")]
//! Dump structural offsets (tape) for a JSON document to stdout, one offset per line.
//! Usage: cargo run -p norito --example dump_tape --features json -- path/to.json > path/to.tape

use std::{env, fs};

fn main() {
    let mut args = env::args().skip(1);
    let path = args.next().expect("usage: dump_tape <file.json>");
    let s = fs::read_to_string(&path).expect("read json");
    let tape = norito::json::build_struct_index(&s);
    for off in tape.offsets {
        println!("{off}");
    }
}
