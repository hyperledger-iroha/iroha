//! Build script to extract git hash of Iroha build

fn main() {
    vergen::EmitBuilder::builder().git_sha(true).emit().unwrap()
}
