# vega-prover: Low-latency client-side ZK proving over signed data

This repository implements the ZK provers of [Vega](https://eprint.iacr.org/2025/2094) in Rust. They are used for client-side ZK proving of statements over signed data. We focus on optimizing low ZK proving latency, and on settings where statements are proven repeatedly over the same signed data.

This [blog post](https://www.microsoft.com/en-us/research/blog/vega-zero-knowledge-proofs-for-digital-identity-in-the-age-of-ai/) describes the typical application this library targets.

The [Vega Prover Book](https://microsoft.github.io/vega-prover/) documents how the proof system is designed, walks through its building blocks, and develops an implementable specification of the multi-circuit prover.

## Performance
The prover in this repository is the fastest proof system for client-side ZK proving of statements over signed data: it generates a zero-knowledge proof of age from a typical mobile driver's license of about 2 KB in about 92 ms on a commodity client device. The resulting proof is about 108 KB, and it can be verified in 23 ms. No trusted setup is required. The prover key is 464 KB; it fits comfortably on any phone. For smaller credentials, proving drops to 62 ms, with 83 KB proofs, and 17 ms verification.

> [!IMPORTANT]
> We optimize for low ZK proving latency on signed messages typical in practice — not for raw throughput.

The `benches/` directory contains SHA-256 benchmarks using [Criterion](https://github.com/bheisler/criterion.rs). Each benchmark measures setup, prep_prove, prove, and verify times across multiple iterations and thread counts, and reports proof sizes.

```bash
# Single-circuit (SC) prover: SHA-256 over 1 KiB and 2 KiB messages
RUSTFLAGS="-C target-cpu=native" cargo bench --bench sha256_vega_sc

# Multi-circuit (MC) prover: 32 SHA-256 step circuits (2048 bytes total)
RUSTFLAGS="-C target-cpu=native" cargo bench --bench sha256_vega_mc_zkp
```

Override thread counts with `BENCH_THREADS` (comma-separated):

```bash
BENCH_THREADS=1,8 RUSTFLAGS="-C target-cpu=native" cargo bench --bench sha256_vega_sc
```

## Python reference implementation

A small pure-Python reference implementation of the multi-circuit (MC) prover lives in [`reference/`](reference/), mirroring the book's specification with readable, unoptimized code and cross-conformance tested against this Rust implementation in both directions.

> [!NOTE]
> The reference implementation is **preliminary**: it currently proves only the fixed cubic example, is optimized for clarity rather than performance or coverage, and has not been independently security-audited. Its internals may change.

## References

[Vega: Low-latency zero-knowledge proofs over existing credentials](https://eprint.iacr.org/2025/2094) \
Darya Kaviani, Srinath Setty \
IEEE S&P 2026

## Contributing

This project welcomes contributions and suggestions.  Most contributions require you to agree to a
Contributor License Agreement (CLA) declaring that you have the right to, and actually do, grant us
the rights to use your contribution. For details, visit https://cla.opensource.microsoft.com.

When you submit a pull request, a CLA bot will automatically determine whether you need to provide
a CLA and decorate the PR appropriately (e.g., status check, comment). Simply follow the instructions
provided by the bot. You will only need to do this once across all repos using our CLA.

This project has adopted the [Microsoft Open Source Code of Conduct](https://opensource.microsoft.com/codeofconduct/).
For more information see the [Code of Conduct FAQ](https://opensource.microsoft.com/codeofconduct/faq/) or
contact [opencode@microsoft.com](mailto:opencode@microsoft.com) with any additional questions or comments.

## Trademarks

This project may contain trademarks or logos for projects, products, or services. Authorized use of Microsoft 
trademarks or logos is subject to and must follow 
[Microsoft's Trademark & Brand Guidelines](https://www.microsoft.com/en-us/legal/intellectualproperty/trademarks/usage/general).
Use of Microsoft trademarks or logos in modified versions of this project must not cause confusion or imply Microsoft sponsorship.
Any use of third-party trademarks or logos are subject to those third-party's policies.
