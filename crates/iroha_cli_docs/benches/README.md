# Command-documentation benchmark

From the workspace root, using the ordinary warm Cargo target:

```sh
cargo iroha-fast -- bench -p iroha_cli_docs --bench render
```

The standard-library harness builds 100 and 1,000 leaf commands under ten groups.
Each leaf has a required positional, flags, aliases, defaults, possible values,
multiline help and a repeated-value option. Fixture construction happens outside
measurement. Every invocation includes one fresh command-tree clone. The harness
reports nine-batch median time and all samples; each batch has 20 renders for the
small tree or three for the large tree.
Full sampling requires `--bench`, which `cargo bench` supplies. Invoking the
executable without arguments uses one render per fixture as a fast smoke check,
so Cargo's all-target test runner cannot accidentally start the full benchmark.
An explicit `--test` also selects smoke mode, even if `--bench` is present.

The checked-in benchmark is entirely safe Rust and measures timing only. A
separate standalone allocation comparison can wrap `System` to count `alloc`,
`alloc_zeroed` and `realloc`, resetting counters immediately before each render
and reading them before reporting. Include the same fresh command clone in both
renderers. Sum requested sizes, including replacement sizes on reallocation;
these totals are not peak/live memory or RSS. Keep allocator instrumentation
outside this crate and use it identically for both single-threaded renderers.
Use `workload::tree(100)` and `workload::tree(1000)` for that standalone comparison.
Concurrent machine load can distort timing even when allocation counts remain
stable; instrumented and uninstrumented timing are different experiments.

For an external historical comparison, reuse `render/workload.rs` in a temporary
standalone Rust harness with two `workload::Renderer` entries. The local callback is
`|tree| iroha_cli_docs::render_markdown(tree.clone())`; the historical callback
must likewise clone the tree before rendering. For the removed `clap-markdown`
0.1.5 baseline, truncate its final attribution footer beginning with `<hr/>`, then
normalize its trailing newlines to exactly one before returning the String.
The harness then checks byte-for-byte equality
before measuring and alternates renderer order between batches.

Compile the historical source to an external rlib using `rustc --crate-type rlib
-O`, and compile the local library and comparison harness with the same
optimization flags, Rust toolchain, and `--extern clap=<rlib>` /
`-L dependency=<directory>` inputs. Pass the historical library only through the
standalone harness's `--extern clap_markdown=<rlib>` flag. Keep those sources and
artifacts outside the checkout; do not add a manifest, patch or dev-dependency for
the removed package. Cached debug Clap artifacts and a release Clap build are
different experiments and must not be compared as if they were the same run.
