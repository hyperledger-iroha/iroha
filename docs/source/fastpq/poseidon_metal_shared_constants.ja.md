---
lang: ja
direction: ltr
source: docs/source/fastpq/poseidon_metal_shared_constants.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4cbbc93e4212320422b8cbfcd8c563419d5ddaf5dad9e84a7878a439892ed081
source_last_modified: "2026-01-03T18:07:57.621942+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# ポセイドンメタルの共通定数

Metal カーネル、CUDA カーネル、Rust 証明者、およびすべての SDK フィクスチャは共有する必要があります
ハードウェアアクセラレーションを維持するために、まったく同じ Poseidon2 パラメータ
ハッシュ決定論的。このドキュメントでは、正規スナップショットとその方法を記録します。
それを再生成する方法と、GPU パイプラインがデータを取り込む方法について説明します。

## スナップショットマニフェスト

パラメーターは、`PoseidonSnapshot` RON ドキュメントとして公開されています。コピーは
バージョン管理下に置かれるため、GPU ツールチェーンと SDK はビルド時間に依存しません。
コード生成。

|パス |目的 | SHA-256 |
|------|--------|----------|
| `artifacts/offline_poseidon/constants.ron` | `fastpq_isi::poseidon::{ROUND_CONSTANTS, MDS}` から生成された正規スナップショット。 GPU ビルドの信頼できる情報源。 | `99bef7760fcc80c2d4c47e720cf28a156f106a0fa389f2be55a34493a0ca4c21` |
| `IrohaSwift/Fixtures/offline_poseidon/constants.ron` |正規のスナップショットをミラーリングするため、Swift 単体テストと XCFramework スモーク ハーネスは、Metal カーネルが期待するのと同じ定数をロードします。 | `99bef7760fcc80c2d4c47e720cf28a156f106a0fa389f2be55a34493a0ca4c21` |
| `java/iroha_android/src/test/resources/offline_poseidon/constants.ron` | Android/Kotlin フィクスチャは、パリティ テストとシリアル化テスト用に同一のマニフェストを共有します。 | `99bef7760fcc80c2d4c47e720cf28a156f106a0fa389f2be55a34493a0ca4c21` |

すべてのコンシューマは、定数を GPU に接続する前にハッシュを検証する必要があります
パイプライン。マニフェストが変更されると (新しいパラメーター セットまたはプロファイル)、SHA と
ダウンストリームミラーはロックステップで更新する必要があります。

## 再生

マニフェストは、`xtask` を実行することで Rust ソースから生成されます。
ヘルパー。このコマンドは、正規ファイルと SDK ミラーの両方を書き込みます。

```bash
cargo xtask offline-poseidon-fixtures --tag iroha.offline.receipt.merkle.v1
```

`--constants <path>`/`--vectors <path>` を使用して宛先をオーバーライドするか、
正規スナップショットのみを再生成する場合は `--no-sdk-mirror`。ヘルパーは、
フラグが省略された場合、アーティファクトを Swift および Android ツリーにミラーリングします。
これにより、ハッシュが CI に合わせて調整されます。

## メタル/CUDA ビルドのフィード

- `crates/fastpq_prover/metal/kernels/poseidon2.metal` および
  `crates/fastpq_prover/cuda/fastpq_cuda.cu` は、
  テーブルが変更されるたびにマニフェストされます。
- 丸められた MDS 定数は、連続した `MTLBuffer`/`__constant` にステージングされます。
  マニフェスト レイアウトに一致するセグメント: `round_constants[round][state_width]`
  次に 3x3 MDS マトリックスが続きます。
- `fastpq_prover::poseidon_manifest()` は、スナップショットをロードして検証します。
  実行時（メタルのウォームアップ中）、診断ツールが次のことをアサートできるようにします。
  シェーダ定数はパブリッシュされたハッシュと一致します。
  `fastpq_prover::poseidon_manifest_sha256()`。
- SDK フィクスチャ リーダー (Swift `PoseidonSnapshot`、Android `PoseidonSnapshot`) および
  Norito オフライン ツールは同じマニフェストに依存しているため、GPU のみの使用が防止されます。
  パラメータフォーク。

## 検証

1. マニフェストを再生成した後、`cargo test -p xtask` を実行して、
   ポセイドン フィクスチャ生成の単体テスト。
2. このドキュメントおよび監視するダッシュボードに新しい SHA-256 を記録します。
   GPU アーティファクト。
3. `cargo test -p fastpq_prover poseidon_manifest_consistency` 解析
   `poseidon2.metal` および `fastpq_cuda.cu` はビルド時に、それらの
   シリアル化された定数はマニフェストと一致し、CUDA/Metal テーブルと
   ロックステップの正規スナップショット。マニフェストを GPU ビルド手順と一緒に保持すると、Metal/CUDA が提供されます。
決定論的なハンドシェイクのワークフロー: カーネルはメモリを自由に最適化できます。
共有定数 BLOB を取り込み、ハッシュを公開する限り、レイアウト
パリティチェックのためのテレメトリ。