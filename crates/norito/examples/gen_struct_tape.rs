//! Generate structural tape (offsets) from JSON input, printed to stdout.
//! Usage: cargo run -p norito --example gen_struct_tape --features json -- <file.json>

use std::{env, fs};

fn build_struct_index_scalar(input: &str) -> Vec<u32> {
    let b = input.as_bytes();
    let mut offsets = Vec::new();
    let mut i = 0usize;
    let mut in_str = false;
    while i < b.len() {
        let c = b[i];
        if in_str {
            if c == b'\\' {
                i = i.saturating_add(2);
                continue;
            }
            if c == b'"' {
                in_str = false;
                offsets.push(i as u32);
                i += 1;
                continue;
            }
            i += 1;
            continue;
        } else {
            match c {
                b'"' => {
                    in_str = true;
                    offsets.push(i as u32);
                    i += 1;
                }
                b'{' | b'}' | b'[' | b']' | b':' | b',' => {
                    offsets.push(i as u32);
                    i += 1;
                }
                _ => {
                    i += 1;
                }
            }
        }
    }
    offsets
}

fn main() {
    let mut args = env::args().skip(1).collect::<Vec<_>>();
    if args.is_empty() {
        eprintln!("usage: gen_struct_tape <file.json> [<file2.json> ...]");
        std::process::exit(2);
    }
    for path in args.drain(..) {
        let s = fs::read_to_string(&path).expect("read");
        let tape = build_struct_index_scalar(&s);
        println!("# {path}");
        for off in tape {
            println!("{off}");
        }
    }
}
