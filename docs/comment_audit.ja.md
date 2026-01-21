<!-- Japanese translation of docs/comment_audit.md -->

---
lang: ja
direction: ltr
source: docs/comment_audit.md
status: complete
translator: manual
---

# コメント監査メモ

以下の IVM モジュールについて、インラインコメントおよびドキュメントコメントが現行の挙動と一致していることを確認しました（コードの変更は不要です）。

- `crates/ivm/src/runtime.rs`

- `crates/ivm/src/memory.rs`
- `crates/ivm/src/register_file.rs`
- `crates/ivm/src/registers.rs`
- `crates/ivm/src/decoder.rs`
- `crates/ivm/src/core_host.rs`
- `crates/ivm/src/ivm.rs`
- `crates/ivm/src/vector.rs`
- `crates/ivm/src/host.rs`
- `crates/ivm_abi/src/syscalls.rs`
- `crates/ivm/src/parallel.rs`
- `crates/ivm/src/mock_wsv.rs`
- `crates/ivm/src/axt.rs`
- `crates/ivm/src/byte_merkle_tree.rs`
- `crates/ivm/src/merkle_utils.rs`
- `crates/ivm/src/gas.rs`
- `crates/ivm/src/error.rs`
- `crates/ivm/src/instruction.rs`
- `crates/ivm/src/encoding.rs`
- `crates/ivm/src/iso20022.rs`
- `crates/ivm/src/halo2.rs`
- `crates/ivm/src/signature.rs`
- `crates/ivm_abi/src/pointer_abi.rs`
- `crates/ivm/src/ivm_cache.rs`

ワークスペース内の残りのクレートについても、今後コメントを更新する必要が生じた場合は改めて監査を行います。
