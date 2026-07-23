use std::{env, fs, process};

fn main() {
    let path = env::args().nth(1).expect("artifact path");
    let bytes = fs::read(path).expect("read artifact");
    match ivm::verify_contract_artifact(&bytes) {
        Ok(verified) => {
            println!("code_hash_hex={}", verified.code_hash);
            println!("abi_hash_hex={}", verified.abi_hash);
            println!("header_len={}", verified.header_len);
            println!("code_offset={}", verified.code_offset);
            println!(
                "entrypoint_count={}",
                verified.contract_interface.entrypoints.len()
            );
        }
        Err(error) => {
            eprintln!("{error}");
            process::exit(2);
        }
    }
}
