//! Repeatable timing measurements for command documentation.
//!
//! Run `cargo iroha-fast -- bench -p iroha_cli_docs --bench render` from the workspace root.
//! See `benches/README.md` for the workload, accounting limits and external baseline comparison.

#[path = "render/workload.rs"]
mod workload;

fn main() {
    workload::run(&[workload::Renderer {
        name: "local",
        render: |tree| iroha_cli_docs::render_markdown(tree.clone()),
    }]);
}
