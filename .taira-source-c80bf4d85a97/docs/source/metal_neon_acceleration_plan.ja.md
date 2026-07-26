<!-- Japanese translation of docs/source/metal_neon_acceleration_plan.md -->

---
lang: ja
direction: ltr
source: docs/source/metal_neon_acceleration_plan.md
status: complete
translator: manual
---

# Metal & NEON アクセラレーション計画（Swift と Rust）

本ドキュメントは、Rust ワークスペースと Swift SDK 全体で決定論的なハードウェアアクセラレーション（Metal GPU、NEON / Accelerate SIMD、StrongBox 連携）を実現するための共有計画をまとめたものです。**Hardware Acceleration Workstream (macOS/iOS)** で追跡されているロードマップ項目をカバーし、Rust IVM チーム・Swift ブリッジ担当・テレメトリ担当者の間で共通のハンドオフ資料となることを目指しています。

> 最終更新日: 2026-01-12  
> オーナー: IVM パフォーマンス TL、Swift SDK リード

## ゴール

1. Apple ハードウェア上で Rust の GPU カーネル（Poseidon / BN254 / CRC64）を再利用できるようにし、CPU パスと決定論的パリティを保つ。
2. Swift アプリが Metal / NEON / StrongBox を選択的に利用できるようにしつつ、ABI とパリティの保証を保つため、エンドツーエンドでアクセラレーション設定（`AccelerationConfig`）を公開する。
3. CI とダッシュボードでパリティ／ベンチマークデータを観測できるように instrumentation を追加し、CPU と GPU/SIMD の差異を自動的に検出する。
4. Android（AND2）と Swift（IOS4）で得た StrongBox / Secure Enclave の知見を共有し、署名フローの決定論的一致を維持する。

## 成果物と担当

| マイルストーン | 成果物 | 担当者 | 目標時期 |
|----------------|--------|--------|----------|
| Rust WP2-A/B | CUDA カーネルと同等の Metal シェーダーインターフェイス | IVM パフォーマンス TL | 2026 年 2 月 |
| Rust WP2-C | Metal BN254 パリティテスト + CI レーン | IVM パフォーマンス TL | 2026 年第 2 四半期 |
| Swift IOS6 | ブリッジトグル（`connect_norito_set_acceleration_config`）、SDK API、サンプル | Swift ブリッジ担当 | 完了（2026 年 1 月） |
| Swift IOS5 | 設定使用例を示すサンプルアプリ／ドキュメント | Swift DX TL | 2026 年第 2 四半期 |
| Telemetry | アクセラレーションのパリティ／ベンチマーク指標を出力するダッシュボード | Swift Program PM / テレメトリ担当 | 2026 年第 2 四半期パイロット |
| CI | CPU vs Metal/NEON をデバイスポールで検証する XCFramework スモークハーネス | Swift QA リード | 2026 年第 2 四半期 |
| StrongBox | ハードウェア署名のパリティテスト（共有ベクタ） | Android 暗号 TL / Swift セキュリティ | 2026 年第 3 四半期 |

## インターフェイスと API 契約

### Rust（`ivm::AccelerationConfig`）
- 既存フィールド（`enable_metal`, `enable_cuda`, `max_gpus`, 各種閾値）を維持。
- 初回使用時のレイテンシーを避けるため Metal ウォームアップを追加（Rust issue #15875）。
- ダッシュボード向けのパリティ API を提供（例: `ivm::vector::metal_status()` → {enabled, parity, last_error}）。
- ベンチマーク指標（Merkle ツリー処理時間、CRC スループットなど）をテレメトリフック経由で出力し、`ci/xcode-swift-parity` から収集できるようにする。

### C FFI（`connect_norito_bridge`）
- 新構造体 `connect_norito_acceleration_config`（実装済み）。
- 設定のみを取得する `connect_norito_get_acceleration_config` と、設定＋パリティを返す `connect_norito_get_acceleration_state` の両 getter を公開し、setter と対になるよう整備済み。
- SPM / CocoaPods 利用者向けにヘッダーコメントで構造体レイアウトを明記する。

### Swift（`AccelerationSettings`）
- デフォルト: Metal 有効、CUDA 無効、閾値は継承（`nil`）。
- 負値は無視し、`apply()` は `IrohaSDK` が自動的に呼び出す。
- Rust がパリティ診断を公開したら、それに合わせて Swift 側でも状態を照らし合わせる。
- サンプルアプリと README にトグル／テレメトリ連携の例を追記する。

### テレメトリ（ダッシュボードおよびエクスポータ）
- パリティフィード（`mobile_parity.json`）:
  - `acceleration.metal/neon/strongbox` → {enabled, parity, perf_delta_pct}
  - `perf_delta_pct` は CPU と GPU の比較を示す。
- CI フィード（`mobile_ci.json`）:
  - `acceleration_bench.metal_vs_cpu_merkle_ms` → {cpu, metal}
  - `acceleration_bench.neon_crc64_throughput_mb_s` → Double
- エクスポータは Rust ベンチマークや CI 実行結果をソースとし、`ci/xcode-swift-parity` で Metal / CPU のマイクロベンチを実行する。

## テスト戦略

1. **ユニットパリティテスト（Rust）**: Metal カーネルが CPU の出力と一致することをベクトルで検証し、`cargo test -p ivm --features metal` で実行する。
2. **Swift スモークハーネス**: IOS6 テストランナーを拡張し、エミュレータと StrongBox 対応デバイス双方で CPU vs Metal（Merkle / CRC64）を実行して結果とパリティステータスを記録する。
3. **CI**: `norito_bridge_ios.yml`（既に `make swift-ci` を呼び出し）を更新し、アクセラレーションメトリクスをアーティファクトへ出力しつつ、Buildkite の `ci/xcframework-smoke:<lane>:device_tag` メタデータが有効であることを確認する。パリティやベンチマークの逸脱で失敗させる。
4. **ダッシュボード**: 新フィールドが CLI 出力に反映されることを確認し、エクスポータが本番化する前にデータが生成されるようにする。

## 未解決事項

1. **Metal リソース解放**: Rust の `warm_up_metal()` が冪等であり、Swift アプリのライフサイクル（バックグラウンド／フォアグラウンド遷移）でもリークしないことを確認する。
2. **ベンチマーク基準**: Metal vs CPU の目標性能差をどう定義するか（どの程度の差異をリグレッションと見なすか）を決め、ダッシュボードアラートに反映する。
3. **StrongBox フォールバック**: Android と Swift で StrongBox アテステーション失敗時の挙動を統一し、フォールバックの鍵導出ドキュメントを共有する。
4. **テレメトリ保存**: エクスポータが出力する JSON フィードをどこに保管するか（S3、ビルドアーティファクトなど）を決定する。

## 次のステップ（2026 年第 1 四半期）

- [ ] Rust: Metal カーネルインターフェイス文書を完成させ、Swift チームとヘッダーを共有する。
- [x] Swift: SDK レベルのアクセラレーション設定公開（2026 年 1 月に完了）。
- [ ] テレメトリ: 新しいダッシュボードフィールドに対応するエクスポータ実装と CI 連携を検討する。
- [ ] Swift QA: アクセラレーションカバレッジを含むスモークハーネス計画を拡張する。
- [ ] エクスポータが実データを出力し始めたら `status.md` を更新する（2026 年第 2 四半期を目標）。
