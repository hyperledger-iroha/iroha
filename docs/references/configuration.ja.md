<!-- Japanese translation of docs/references/configuration.md -->

---
lang: ja
direction: ltr
source: docs/references/configuration.md
status: complete
translator: manual
---

# アクセラレーション

`[accel]` セクションは IVM と補助機能向けの任意ハードウェアアクセラレーションを制御します。すべてのアクセラレート経路には決定論的な CPU フォールバックがあり、バックエンドがランタイムのゴールデンセルフテストに失敗した場合は自動的に無効化され、CPU 実行に切り替わります。

- `enable_cuda`（既定値: true） – コンパイル済みで利用可能な場合に CUDA を使用。
- `enable_metal`（既定値: true） – macOS で利用可能な場合に Metal を使用。
- `max_gpus`（既定値: 0） – 初期化する GPU の上限。`0` は自動／制限なしを意味する。
- `merkle_min_leaves_gpu`（既定値: 8192） – GPU にオフロードする Merkle 葉ハッシュの最小リーフ数。非常に高速な GPU でのみこれより小さくする。
- 上級設定（任意。通常は既定値で十分）:
  - `merkle_min_leaves_metal`（既定値: `merkle_min_leaves_gpu` を継承）
  - `merkle_min_leaves_cuda`（既定値: `merkle_min_leaves_gpu` を継承）
  - `prefer_cpu_sha2_max_leaves_aarch64`（既定値: 32768） – SHA2 命令を備えた ARMv8 で、このリーフ数までは CPU SHA-2 を優先。
  - `prefer_cpu_sha2_max_leaves_x86`（既定値: 32768） – x86/x86_64 で SHA-NI を備える場合、このリーフ数までは CPU SHA-2 を優先。

注記
- 最優先は決定性: アクセラレーションは観測可能な出力を決して変えません。バックエンドは初期化時にゴールデンテストを実行し、不一致が検出されればスカラ／SIMD 実装にフォールバックします。
- 設定は `iroha_config` 経由で行い、本番環境では環境変数の利用を避けてください。
