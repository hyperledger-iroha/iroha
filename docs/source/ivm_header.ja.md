<!-- Japanese translation of docs/source/ivm_header.md -->

---
lang: ja
direction: ltr
source: docs/source/ivm_header.md
status: complete
translator: manual
---

# IVM バイトコードヘッダー

本ページでは IVM バイトコードヘッダーの各フィールドと互換性ポリシーを説明します。内容は `ivm.md`（アーキテクチャ概説）を補完し、パイプラインやサンプル関連のドキュメントから参照されます。

## マジック
- 4 バイト: オフセット 0 に ASCII `IVM\0` を配置します。

## レイアウト（現行）
- オフセットとサイズ（合計 17 バイト）:
  - 0..4: マジック `IVM\0`
  - 4: `version_major: u8`
  - 5: `version_minor: u8`
  - 6: `mode: u8`（フィーチャビット。後述）
  - 7: `vector_length: u8`
  - 8..16: `max_cycles: u64`（リトルエンディアン）
  - 16: `abi_version: u8`

## モードビット
- `ZK = 0x01`, `VECTOR = 0x02`, `HTM = 0x04`（予約済み／機能ゲート対象）。

## フィールドの意味
- `version`: バイトコード形式のセマンティックバージョン（`version_major.version_minor`）。デコーダー互換性とガススケジュール選択に利用されます。
- `abi_version`: システムコール表とポインター ABI スキーマのバージョン。
- `mode`: ZK トレース／VECTOR／HTM のフィーチャビット。
- `vector_length`: ベクトル命令向けの論理的ベクトル長（0 は未設定を意味する）。
- `max_cycles`: ZK モードおよびアドミッションで利用される実行パディング上限。

## 備考
- エンディアンとレイアウトは実装によって定義され、`version` と結び付いています。上記は `crates/ivm_abi/src/metadata.rs` における現行実装を反映しています。
- 最小限のリーダーは現行アーティファクトに対してこのレイアウトに依存できますが、将来の変更に備え `version` によるゲーティングを実装すべきです。
- ハードウェアアクセラレーション（Metal/CUDA）は各ホストでオプトインです。ランタイムは `iroha_config` の `AccelerationConfig` から `enable_metal` と `enable_cuda` を読み取り、コンパイル済みであっても該当バックエンドを切り替えます。VM 作成前に `ivm::set_acceleration_config` を介して適用されます。
- オペレーターは診断目的で `IVM_DISABLE_METAL=1` または `IVM_DISABLE_CUDA=1` を設定し、特定バックエンドを強制的に無効化できます。これらの環境変数は設定を上書きし、VM を決定論的な CPU パスに固定します。

## Durable 状態ヘルパーと ABI サーフェス
- Durable 状態ヘルパーのシステムコール（0x50–0x5A: STATE_{GET,SET,DEL}, ENCODE/DECODE_INT, BUILD_PATH_* および JSON/SCHEMA エンコード／デコード）は V1 ABI の一部であり、`abi_hash` の計算に含まれます。
- CoreHost は STATE_{GET,SET,DEL} を WSV に裏付けられた永続スマートコントラクト状態へ接続します。dev/test ホストはオーバーレイやローカル永続化を使ってもよいが、観測可能な挙動は同一でなければなりません。

## バリデーション
- ノードアドミッションは `version_major = 1` かつ `version_minor = 0` のヘッダーのみを受け入れます。
- `mode` には既知のビット（`ZK`, `VECTOR`, `HTM`）のみを設定できます。未知のビットは拒否されます。
- `vector_length` は助言的な値であり、`VECTOR` ビットが未設定でも非ゼロになり得ます。アドミッションでは上限のみを検証します。
- サポートされる `abi_version` は初回リリース時点では `1` のみであり、他の値はアドミッションで拒否されます。

### ポリシー（自動生成）
以下のポリシー概要は実装から生成され、手動で編集してはいけません。

<!-- BEGIN GENERATED HEADER POLICY -->
| フィールド | ポリシー |
|---|---|
| version_major | 1 |
| version_minor | 0 |
| mode (known bits) | 0x07 (ZK=0x01, VECTOR=0x02, HTM=0x04) |
| abi_version | 1 |
| vector_length | 0 または 1..=64（助言用。VECTOR ビットと独立） |
<!-- END GENERATED HEADER POLICY -->

### ABI ハッシュ（自動生成）
以下の表は、サポートされるポリシーに対する正準 `abi_hash` を示します。

<!-- BEGIN GENERATED ABI HASHES -->
| ポリシー | abi_hash (hex) |
|---|---|
| ABI v1 | 377f7125a0f20d40f65ed5e3a179bc5e04d68a385570b54d67d961861e8d9f83 |
<!-- END GENERATED ABI HASHES -->

## 互換性ポリシー
- マイナー更新では `feature_bits` や予約済み opcode 領域の背後で命令を追加できます。メジャー更新ではプロトコルアップグレードと同時にのみエンコーディングの変更や廃止／置換を行います。
- システムコール番号のレンジは安定しており、アクティブな `abi_version` に存在しない番号は `E_SCALL_UNKNOWN` によって扱われます。

```rust
fn is_ivm_artifact(path: &std::path::Path) -> std::io::Result<bool> {
    let mut f = File::open(path)?;
    let mut magic = [0u8; 4];
    if f.read(&mut magic)? != 4 { return Ok(false); }
    if &magic != b"IVM\0" { return Ok(false); }
    let meta = std::fs::metadata(path)?;
    Ok(meta.len() >= 64)
}
```

**補足:** マジック以降の正確なヘッダーレイアウトはバージョン管理され実装依存です。安定したフィールド名と値を取得するには `ivm_tool inspect` の利用を優先してください。
