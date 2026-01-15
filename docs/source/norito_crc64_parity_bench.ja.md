<!-- Japanese translation of docs/source/norito_crc64_parity_bench.md -->

---
lang: ja
direction: ltr
source: docs/source/norito_crc64_parity_bench.md
status: complete
translator: manual
---

# Norito CRC64: パリティ検証とベンチガイド

このガイドは、Norito の CRC64 がハードウェア・フォールバック間で一致することを確認し、CRC64 やエンコード／デコード性能を手元の環境で測定する手順をまとめたものです。

Norito は `crc64fast` クレートを用いて CRC64-XZ (`0x42F0E1EBA9EA3693`) を計算します。`hardware_crc64` と `crc64_fallback` は同一実装であり、プラットフォーム間で一貫した結果が得られます。

## 1) クイックパリティチェック

- Norito のユニット／統合テストでパリティを確認:

```bash
cargo test -p norito --test crc64
cargo test -p norito --test crc64_prop
```

- 対応 CPU でアクセラレーションを強制（最大速度のためのビルドフラグ・開発者向け）:

```bash
# x86_64（ローカルベンチ用。ノードランタイム設定ではない）
RUSTFLAGS='-C target-feature=+sse4.2,+pclmulqdq' cargo test -p norito --test crc64
RUSTFLAGS='-C target-feature=+sse4.2,+pclmulqdq' cargo test -p norito --test crc64_prop

# aarch64
RUSTFLAGS='-C target-feature=+neon,+aes' cargo test -p norito --test crc64
RUSTFLAGS='-C target-feature=+neon,+aes' cargo test -p norito --test crc64_prop
```

メモ:
- 繰り返し検証には個別テストを指定: `cargo test -p norito crc64_prop::random_lengths_parity_small -- --nocapture`

## 2) アドホックパリティ検証スニペット

```rust
use norito::{crc64_fallback, hardware_crc64};

fn main() {
    let mut ok = true;
    let sizes = [0usize, 1, 7, 8, 15, 16, 31, 32, 63, 64, 511, 512, 4095, 4096];
    for &n in &sizes {
        let mut buf = vec![0u8; n];
        let mut x: u64 = 0x9E37_79B9_7F4A_7C15;
        for b in &mut buf { x ^= x << 7; x ^= x >> 9; *b = (x as u8) ^ 0xA5; }
        let hw = hardware_crc64(&buf);
        let fb = crc64_fallback(&buf);
        if hw != fb { eprintln!("mismatch at n={n}: hw={hw:016X} fb={fb:016X}"); ok = false; break; }
    }
    if ok { println!("parity OK across sizes: {:?}", sizes); }
}
```

## 3) CRC64 スループットベンチマーク

Criterion ベンチを実行:

```bash
cargo bench -p norito -- benches::bench_crc64 -- --warm-up-time 1 --sample-size 30

# aarch64（Apple M シリーズなど）
RUSTFLAGS='-C target-feature=+neon,+aes' \
  cargo bench -p norito -- benches::bench_crc64 -- --warm-up-time 1 --sample-size 30
```

期待値:
- CPU 周波数やメモリ帯域に応じて結果が変化する。

## 4) エンドツーエンドのエンコード／デコードベンチ

`crates/norito/benches/codec.rs` は Norito と SCALE/bincode のエンコード／デコード性能を比較し、圧縮（zstd）も含む。

推奨実行:

```bash
cargo bench -p norito -- benches::bench_codec -- --warm-up-time 1 --sample-size 30

# x86_64
RUSTFLAGS='-C target-feature=+sse4.2,+pclmulqdq' cargo bench -p norito -- benches::bench_codec -- --warm-up-time 1 --sample-size 30
# aarch64
RUSTFLAGS='-C target-feature=+neon,+aes' cargo bench -p norito -- benches::bench_codec -- --warm-up-time 1 --sample-size 30
```

結果の読み方:
- Norito はヘッダ・スキーマハッシュ・CRC64 を含むため、非常に小さいペイロードでは速度／サイズが僅かに劣る一方、大きなペイロードでは競合する性能。
- 圧縮ベンチは圧縮処理を含むエンドツーエンド時間を測定。スループット要件に応じてレベル調整（既定は “fast”）。

## 5) トラブルシュートとヒント

- ビルド時間が長い場合はテスト対象を絞る: `cargo test -p norito --lib` や特定テスト。
- ネットワーク遮断時は `--offline` を使用。
- Apple Silicon はターゲット `aarch64-apple-darwin` を確認。
- 決定性: CRC 実装はプラットフォーム問わず安定。パリティテストが CI で保証。

---

ベンチ結果（Criterion レポートなど）を収集した場合、CPU 型番や OS、コンパイラバージョンと併せて記録しておくと将来の比較に役立ちます。
