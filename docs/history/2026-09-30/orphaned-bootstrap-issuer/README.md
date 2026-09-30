# Orphaned bootstrap issuer prototype

The unit-test repair audit on September 30 found these two source files outside every Rust build target. No module declaration includes the issuer and no caller references its types. The issuer declares a Unix test module, but that module was never present in the introducing commit (`88bb6a28a1`) or the current tree. The prototype therefore supplies no runtime capability or unit-test evidence.

The exact originals are retained below as non-executable design history. The unowned source files are retired. Current KAGEMUSHA bootstrap packages, signatures, replay pins and SDK validation remain owned by the data model and canonical SDKs. A future server issuer must establish a real build owner, qualified clock and custody inputs, durable-store tests and a reviewed caller before it becomes implementation surface.

| Original path | Exact original | SHA-256 |
| --- | --- | --- |
| `crates/iroha_torii/src/kagemusha_mobile_bootstrap_issuer_v1.rs` | [kagemusha_mobile_bootstrap_issuer_v1.rs.txt](kagemusha_mobile_bootstrap_issuer_v1.rs.txt) | `0b1924e7b3d46d120bfe2ddd7ecc84508125bf6fb0252d11f00b688343551639` |
| `crates/iroha_torii/src/kagemusha_mobile_bootstrap_issuer_v1/store.rs` | [store.rs.txt](store.rs.txt) | `99e14a61834048166e16a4bdd4a549a7af0415cd57a776cf8d299c0b224eb65a` |
