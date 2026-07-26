<!--
  SPDX-License-Identifier: Apache-2.0
-->

---
lang: ja
direction: ltr
source: docs/source/crypto/sm_operator_rollout.md
status: complete
translator: manual
---

# SM 機能ロールアウト & テレメトリ チェックリスト

本チェックリストは、監査およびコンプライアンスのゲートをクリアした後に SM (SM2/SM3/SM4) 機能を安全に有効化するために、SRE とオペレーター チームを支援します。`docs/source/crypto/sm_program.md` の構成ガイドと `docs/source/crypto/sm_compliance_brief.md` の法務/輸出ガイダンスと併せて参照してください。

## 1. 事前準備
- [ ] ワークスペースのリリースノートに、対象リリースで `sm` が「検証のみ」か「署名あり」かが明示されていることを確認する。
- [ ] フリートが SM のテレメトリと構成ノブを含むコミットからビルドされたバイナリを実行していることを確認する（対象リリースはロールアウトチケットで追跡）。
- [ ] ターゲットごとのアーキテクチャで `scripts/sm_perf.sh --tolerance 0.25` をステージングノード上で実行し、サマリー出力を保管する。スクリプトは自動的にスカラー基準値を選択し、加速モードとの比較を行う（SM3 NEON 作業が完了するまで `--compare-tolerance` は 5.25）。一次もしくは比較側のガードに違反した場合は、調査が終わるまでロールアウトを停止する。Linux/aarch64 Neoverse ハードウェアで計測する場合は `--baseline crates/iroha_crypto/benches/sm_perf_baseline_aarch64_unknown_linux_gnu_<mode>.json --write-baseline` を指定して、`m3-pro-native` 由来の中央値をホスト計測で上書きしてから出荷する。
- [ ] `status.md` とロールアウトチケットに、各司法管轄で求められるコンプライアンス申請の状況を記録する。
- [ ] バリデータが SM 署名鍵を HSM/KMS に保管する場合は、必要な更新を準備する。

## 2. 構成変更
1. xtask ヘルパーを使って SM2 の鍵インベントリとスニペットを同時に生成する:
   ```bash
   cargo xtask sm-operator-snippet \
     --distid CN12345678901234 \
     --json-out sm2-key.json \
     --snippet-out client-sm2.toml
   ```
   `--snippet-out -`（必要に応じて `--json-out -`）を指定すると、結果を標準出力に直接表示できる。
   より低レベルな CLI 手順を使いたい場合は、以下のコマンドを実行する:
   ```bash
   cargo run -p iroha_cli --features sm -- \
     crypto sm2 keygen \
     --distid CN12345678901234 \
     --output sm2-key.json

   cargo run -p iroha_cli --features sm -- \
     crypto sm2 export \
     --private-key-hex "$(jq -r .private_key_hex sm2-key.json)" \
     --distid CN12345678901234 \
     --snippet-output client-sm2.toml \
     --emit-json --quiet
   ```
   `jq` が利用できない場合は `sm2-key.json` を開いて `private_key_hex` をコピーし、`crypto sm2 export` に直接渡す。
2. 得られたスニペットを各ノードの設定に追加する（以下は検証フェーズ向けの例。環境に合わせて調整する）:
```toml
[crypto]
default_hash = "sm3-256"
allowed_signing = ["ed25519", "sm2"]   # 検証専用に留める場合は "sm2" を外す
sm2_distid_default = "1234567812345678"
# enable_sm_openssl_preview = true  # オプション: OpenSSL/Tongsuo 経路を展開する場合のみ
```
3. ノードを再起動し、`crypto.sm_helpers_available` と `crypto.sm_openssl_preview_enabled` が以下の場所で期待どおり表示されることを確認する:
   - `/status` の JSON (`"crypto":{"sm_helpers_available":true,"sm_openssl_preview_enabled":true,...}`)
   - 各ノードの生成済み `config.toml`
4. 署名フェーズで SM を許可する場合は、マニフェスト/ジェネシスにも SM アルゴリズムを追加する。`--genesis-manifest-json` のみで起動する場合、`irohad` はマニフェストの `crypto` ブロックからランタイム暗号設定を直接読み込むため、ロールアウト前にマニフェストを変更計画内に含めておく。

## 3. テレメトリと監視
- Prometheus エンドポイントをスクレイプし、以下のカウンター/ゲージが出力されていることを確認する:
  - `iroha_sm_syscall_total{kind="verify"}`
  - `iroha_sm_syscall_total{kind="hash"}`
  - `iroha_sm_syscall_total{kind="seal|open",mode="gcm|ccm"}`
  - `iroha_sm_openssl_preview`（プレビュー切り替え状態を表す 0/1 ゲージ）
  - `iroha_sm_syscall_failures_total{kind="verify|hash|seal|open",reason="..."}`
- SM2 署名を有効化した場合は `iroha_sm_sign_total` と `iroha_sm_sign_failures_total` も収集する。
- Grafana ダッシュボードとアラートを以下の用途で作成する:
  - 障害カウンターの急増（5 分窓）
  - SM システムコールのスループット低下
  - ノード間での有効化状態の不一致

## 4. ロールアウト段階
| フェーズ | 対応事項 | 備考 |
|----------|-----------|------|
| 検証のみ | `crypto.default_hash` を `sm3-256` に更新し、`allowed_signing` から `"sm2"` を除外した状態で検証カウンターを監視。 | 目的: コンセンサスに影響を与えず SM 検証経路を運用する。 |
| 署名パイロット（混在） | 限定的なバリデータに SM 署名を許可し、署名カウンターとレイテンシを監視。 | Ed25519 へのフォールバックを常に確保。テレメトリが不一致を示した場合は停止。 |
| 本番署名 (GA) | `allowed_signing` に `"sm2"` を追加し、マニフェスト/SDK を更新。最終ランブックを公開。 | 監査完了、各司法管轄のコンプライアンス更新、安定したテレメトリが必須。 |

### レディネスレビュー
- **検証フェーズ準備完了 (SM-RR1)**: Release Eng、Crypto WG、Ops、Legal とレビュー。必要項目:
  - `status.md` にコンプライアンス申請の状況と OpenSSL の由来情報が記録されている。
  - `docs/source/crypto/sm_program.md`、`sm_compliance_brief.md`、本チェックリストが直近のリリースウィンドウで更新済み。
  - 既定または環境別の genesis マニフェストに `crypto.allowed_signing = ["ed25519", "sm2"]` と `crypto.default_hash = "sm3-256"`（検証段階では `sm2` を含まないバリエーションでも可）が設定されている。
  - `scripts/sm_openssl_smoke.sh` と `scripts/sm_interop_matrix.sh` のログがロールアウトチケットに添付されている。
  - `iroha_sm_*` テレメトリダッシュボードが平常動作であることを確認。
- **署名パイロット準備完了 (SM-RR2)**: 追加要件:
  - RustCrypto SM スタックの監査報告書がクローズ済み、または補完的統制の RFC がセキュリティチームに承認されている。
  - 各拠点向けのオペレーターランブックが、署名フォールバック／ロールバック手順を含めて更新済み。
  - パイロット対象の genesis マニフェストに `allowed_signing = ["ed25519","sm2"]` が設定され、同じ値が全ノードの設定に反映されている。
  - ロールバック計画が文書化されている（`allowed_signing` から `"sm2"` を削除、マニフェスト復旧、ダッシュボードをリセット）。
- **本番準備完了 (SM-RR3)**: パイロットの承認済みレポート、全バリデータ地域のコンプライアンス更新、署名付きテレメトリ ベースライン、Release Eng + Crypto WG + Ops/Legal トライアドの承認が必要。

## 5. パッケージングとコンプライアンス チェックリスト
- **OpenSSL/Tongsuo アーティファクトを同梱する。** 各バリデータパッケージに OpenSSL/Tongsuo 3.0 以上の `libcrypto` / `libssl` を含めるか、正確なシステム依存関係を明記する。バージョン、ビルドフラグ、SHA256 チェックサムをリリースマニフェストに記録し、サプライチェーンを追跡可能にする。
- **CI で検証する。** すべての標的プラットフォームに対して `scripts/sm_openssl_smoke.sh` をパッケージされたアーティファクトに対して実行する CI ステップを追加する。プレビュー フラグが有効で provider が初期化できない場合（ヘッダー欠如、未対応アルゴリズム等）は必ず失敗させる。
- **コンプライアンスノートを公表する。** リリースノート／`status.md` に、同梱された provider のバージョン、輸出規制（GM/T、GB/T）への言及、SM アルゴリズムに関する地域固有の要件を記載する。
- **オペレーターランブックを更新する。** 以下のアップグレード手順を文書化する: 新しい共有ライブラリを配置 → `crypto.enable_sm_openssl_preview = true` でノードを再起動 → `/status` のフィールドと `iroha_sm_openssl_preview` ゲージが `true` になることを確認 → テレメトリに差異が出た場合のロールバック（フラグの無効化またはパッケージのロールバック）を用意。
- **証跡を保管する。** OpenSSL/Tongsuo パッケージのビルドログと署名証跡を各バリデータリリースに添えて保管し、将来の監査でも証明できるようにする。

## 6. インシデント対応
- **検証失敗が急増した場合:** SM 非対応ビルドに戻すか、`allowed_signing` から `"sm2"` を削除（必要に応じて `default_hash` を元に戻す）して前バージョンに切り戻す。失敗したペイロード、比較結果、ノードログを収集する。
- **性能劣化:** SM メトリクスを Ed25519/SHA2 の基準と比較する。ARM の命令最適化が原因の場合は `crypto.sm_intrinsics = "force-disable"` を設定し、結果を報告する。
- **テレメトリ欠落:** カウンターが欠落／更新されない場合は Release Engineering にチケットを起票し、修正が完了するまでロールアウトを停止する。

## 7. チェックリスト テンプレート
- [ ] 設定を反映し、ピアを再起動した。
- [ ] テレメトリカウンターが可視化され、ダッシュボードが構成された。
- [ ] コンプライアンス／法務手順を記録した。
- [ ] ロールアウト段階が Crypto WG / Release TL に承認された。
- [ ] ロールアウト後レビューが完了し、所見を記録した。

このチェックリストはロールアウトチケットで管理し、フリートが段階を移行するたびに `status.md` を更新してください。
