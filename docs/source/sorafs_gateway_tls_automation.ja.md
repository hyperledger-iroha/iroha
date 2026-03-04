---
lang: ja
direction: ltr
source: docs/source/sorafs_gateway_tls_automation.md
status: complete
translation_last_reviewed: 2026-02-18
title: SoraFS ゲートウェイ TLS と ECH オペレーターガイド
summary: SF-5b 証明書自動化の構成、テレメトリ、インシデント対応、ガバナンス要件を解説します。
---

# SoraFS ゲートウェイ TLS と ECH オペレーターガイド

## 範囲

本ガイドは、SF-5b 証明書自動化スタックが有効になっている状態で Torii の SoraFS ゲートウェイを運用する方法をまとめています。従来の計画メモを置き換え、秘密情報の準備、ACME/ECH の設定、テレメトリの読み方、ガバナンス要件の満たし方、認定済みのインシデント対応プレイブックの実行手順といった日常業務に焦点を当てます。

ゲートウェイは `iroha_torii::sorafs::gateway` にある自動化コントローラーを含み、バスティオンホストには `cargo xtask sorafs-self-cert` がインストールされていることを前提とします。

## クイックスタートチェックリスト

1. ACME 資格情報と GAR 成果物を KMS や Vault などの `sorafs_gateway_tls/` シークレットストアに格納し、バスティオンホストへ同期してください。
2. `torii.sorafs_gateway.acme` 設定に DNS-01/TLS-ALPN-01 チャレンジの優先度、ECH の有効化可否、更新しきい値を入力してください。
3. `sorafs-gateway-acme@<environment>.service` ユニットをデプロイまたは有効化し、自動化コントローラーを常時稼働させてください。
4. `cargo xtask sorafs-self-cert --check-tls` を実行してベースラインのアテステーションバンドルを取得し、ヘッダーとメトリクスを検証してください。
5. **テレメトリリファレンス** セクションで定義したメトリクス向けにダッシュボードとアラートを公開し、当番チームを登録してください。
6. インシデント対応プレイブックを PagerDuty や Notion などのランブックツールに取り込み、**演習頻度とエビデンス** に記載した演習をスケジュールしてください。

## 設定手順

### 前提条件

| 要件 | 重要な理由 |
|-------------|----------------|
| `sorafs_gateway_tls/` に保管した ACME アカウント資格情報 (KMS/Vault) | 手動発注、失効、更新トークンの管理に必須。 |
| 最新の `torii.sorafs_gateway` 設定バンドルのオフラインコピー | `ech_enabled`、ホスト一覧、リトライタイミングを構成管理の待ち時間なく修正できる。 |
| ガバナンスマニフェスト (`manifest_signatures.json`、GAR エンベロープ) へのアクセス | 証明書フィンガープリントの更新やカノニカルホストのローテーション時に必要。 |
| バスティオンホスト上の `cargo xtask` ツールチェーン | `sorafs-gateway-attest` / `sorafs-self-cert` を利用した検証に必要。 |
| プレイブックテンプレートを PagerDuty/Notion に保存 | 本ガイドのチェックリストへワンクリックでアクセスできるようにする。 |

### 秘密情報とファイル配置

- ACME アカウント情報は `/etc/iroha/sorafs_gateway_tls/` に `iroha:iroha` 所有・`0600` 権限で配置してください。
- 手動ローテーションに備え、KMS や Vault の名前空間に暗号化済みバックアップを保持してください。
- 自動化コントローラーは `/var/lib/sorafs_gateway_tls/automation.json` に状態を保存するため、バックアップ対象に含めてリニューアルのジッターやリトライウィンドウを再起動後も維持してください。

### Torii 設定

Torii の設定バンドル (例: `configs/production.toml`) に以下を追加または更新してください。

```toml
[torii.sorafs_gateway.acme]
account_email = "tls-ops@example.com"
directory_url = "https://acme-v02.api.letsencrypt.org/directory"
dns_provider = "route53"
renewal_threshold_seconds = 2_592_000  # 30 days
retry_backoff_seconds = 900
retry_jitter_seconds = 120
use_tls_alpn_challenge = true
ech_enabled = true
```

- `dns_provider` は `acme::DnsProvider` に登録された実装へマッピングします。Route53 が同梱されており、追加プロバイダーはゲートウェイの再ビルドなしで拡張できます。
- `ech_enabled = true` の場合、証明書と同時に ECH 設定を生成します。上流 CDN やクライアントが非対応の場合は `false` に設定します (後述のフォールバック手順参照)。
- `renewal_threshold_seconds` は新規オーダーを開始するタイミングを制御します。既定値は 30 日ですが、リスクが高い運用では短縮できます。
- 設定パーサーは `iroha_torii::sorafs::gateway::config` にあり、検証エラーは Torii 起動時にログへ出力されます。

### 設定リファレンス

| キー | 既定値 | 本番想定 | コンプライアンス要件 |
|-----|--------|----------|------------------------|
| `torii.sorafs_gateway.acme.enabled` | `true` | 演習やインシデント対応時を除き有効化したままにする。 | 無効化した場合は変更ログへインシデント/チケット ID を記録する。 |
| `torii.sorafs_gateway.acme.directory_url` | Let’s Encrypt v2 | CA を切り替える場合のみ更新する。 | 選定した CA を GAR マニフェストで公開する義務がある。 |
| `torii.sorafs_gateway.acme.dns_provider` | `route53` | 自動化コントローラーに登録済みのプロバイダー名を指定する。 | プロバイダーの IAM ポリシーを四半期ごとにレビューする。 |
| `torii.sorafs_gateway.acme.renewal_threshold_seconds` | `2_592_000` (30 日) | リスクプロファイルに合わせて調整するが 14 日未満にはしない。 | 閾値変更にはリスクオーナーの承認が必要。 |
| `torii.sorafs_gateway.acme.retry_backoff_seconds` | `900` | GAR の復旧 SLA を満たすため 900 以下を維持する。 | 900 を超える設定はガバナンスログに例外として記録する。 |
| `torii.sorafs_gateway.acme.ech_enabled` | `true` | プレイブックに従ってのみ切り替える。 | 切り替え時は理由と証跡をコンプライアンス台帳へ記録する。 |
| `torii.sorafs_gateway.acme.telemetry_topic` | `sorafs-tls` | 特別なログルーティングが必要な場合のみ変更する。 | 変更した場合は観測チームに新しいチャンネルと保持ポリシーを登録する。 |

手動で設定を変更する場合は `iroha_config::actual::sorafs_gateway` (または貴社の設定管理システム) を更新し、変更差分をチケットへ添付したうえで、デプロイ後に最新の self-cert バンドルをアップロードしてください。

### 自動化サービス

- 自動化バンドルに付属する systemd テンプレートユニットを有効化してください。

  ```bash
  systemctl enable --now sorafs-gateway-acme@production.service
  ```

- ログは `sorafs_gateway_tls_automation` ジャーナルターゲットへ出力されます。ログ集約基盤に転送し、環境タグを付けてインシデント調査を迅速化してください。
- 実装状況: `iroha_torii::sorafs::gateway::acme::AcmeAutomation` が DNS-01/TLS-ALPN-01 チャレンジの決定論的処理、ジッター/バックオフ設定、更新状態の永続化を提供します。

### 検証とアテステーション

1. 設定を適用したら Torii をリロードしてください。

   ```bash
   systemctl reload iroha-torii.service
   ```

2. 自己証明ハーネスを実行し、ヘッダー、メトリクス、GAR ポリシー統合を確認してください。

   ```bash
   cargo xtask sorafs-self-cert --check-tls \
     --gateway https://gateway.example.com \
     --out artifacts/sorafs_gateway_self_cert
   ```

3. 生成されたレポートを GAR エンベロープとともに保管し、トレーサビリティを確保してください。
4. 実装状況: `iroha_torii::sorafs::gateway::telemetry` が `SORA_TLS_STATE_HEADER`、`Metrics::set_sorafs_tls_state`、`Metrics::record_sorafs_tls_renewal` を公開し、ハーネスから利用されています。

## テレメトリリファレンス

| 種別 | 名前 | 説明 | アラート / 対応 |
|---------|------|-------------|----------------|
| メトリクス | `sorafs_gateway_tls_cert_expiry_seconds` | 現在の証明書が失効するまでの残り秒数。 | `< 1_209_600` (14 日) でオンコールに通知。 |
| メトリクス | `sorafs_gateway_tls_renewal_total{result}` | 成功/失敗でラベル付けされた更新試行カウンター。 | 1 時間あたり失敗率が 5% を超えたら調査。 |
| メトリクス | `sorafs_gateway_tls_ech_enabled` | 現在の ECH 状態を表す 0/1 ゲージ。 | 想定外に `0` になったらアラート。 |
| メトリクス | `torii_sorafs_gar_violations_total{reason,detail}` | GAR ポリシー違反カウンター。 | ガバナンスへ即時エスカレーションし、ログを添付。 |
| ヘッダー | `X-Sora-TLS-State` | レスポンスに付与される状態 (例: `ech-enabled;expiry=2025-06-12T12:00:00Z`)。 | 合成監視で確認し、`ech-disabled` や `degraded` が出たら本プレイブックを適用。 |
| ログ | `journalctl -u sorafs-gateway-acme@*.service` | 更新の痕跡、チャレンジ失敗、手動操作を記録。 | インシデントチケットや演習時に必ず保存。 |

Prometheus や OpenTelemetry にメトリクスを公開し、失効・更新トレンド向けダッシュボードを構築してください。`X-Sora-TLS-State` を 1 時間ごとに検証する合成プローブも用意してください。

### アラート配線

- **有効期限の猶予:** `sorafs_gateway_tls_cert_expiry_seconds` が 14 日未満になったら警告、7 日未満でクリティカルアラートを発報し、**緊急証明書ローテーション** プレイブックへ誘導します。
- **更新失敗:** `sorafs_gateway_tls_renewal_total{result="error"}` が 6 時間で 3 回以上増加した場合にインシデントを起票し、自動化ログを添付します。
- **GAR 違反:** `torii_sorafs_gar_violations_total` のアラートをガバナンス評議会チャンネルへ直接ルーティングし、例外承認やトラフィック切替の判断を迅速化します。
- **ECH 状態のずれ:** `sorafs_gateway_tls_ech_enabled` が予定外に `1` から `0` へ変化した際は開発者体験チームへ通知し、SDK/運用チームへの周知を行います。

## GAR ポリシーフック

- ゲートウェイのポリシー拒否時は `torii_sorafs_gar_violations_total{policy_reason,policy_detail}` がインクリメントされ、Prometheus/Alertmanager でガバナンス用アラームが自動発火します。
- Torii は `DataEvent::Sorafs(GarViolation)` を発行し、プロバイダー識別子、拒否理由、レートリミット状況を SSE や Webhook で購読できます。ガバナンスのコンプライアンスパイプラインへ接続してください。
- リクエストログに `policy_reason`、`policy_detail`、`provider_id_hex` の構造化フィールドが追加され、フォレンジック調査と監査証跡の収集が容易になります。

## コンプライアンスとガバナンス

運用者は Nexus ガバナンスと整合させるため次の義務を満たす必要があります。

- **GAR 整合性:** 更新完了ごとに GAR マニフェストへ新しい証明書フィンガープリントを公開し、オートメーションログ、自己証明バンドル、アテステーションフィンガープリントをガバナンス評議会へ提出する。
- **ポリシーロギング:** GAR 違反ログを最低 180 日間保管し、四半期ごとのコンプライアンス報告に含める。
- **アテステーション保管:** `cargo xtask sorafs-self-cert` の出力を `artifacts/sorafs_gateway_tls/<YYYYMMDD>/` に保存し、監査人に閲覧権限を付与する。
- **設定変更管理:** `torii.sorafs_gateway` の変更を変更管理システムに記録し、`ech_enabled` の切り替えや更新閾値の調整理由を明記する。
- **演習実施:** 本ガイドで定義した演習を実行し、3 営業日以内に結果を記録する。

### コンプライアンス証跡チェックリスト

| 義務 | 収集する証跡 | 保持期間 | 担当 |
|------|---------------|----------|------|
| GAR 整合性 | 更新後の GAR マニフェスト、署名済みフィンガープリントバンドル、関連チケットへのリンク。 | 3 年 | ガバナンスリエゾン |
| ポリシーログ | `torii_sorafs_gar_violations_total` のエクスポート、構造化ログ、Alertmanager 通知。 | 180 日 | 観測チーム |
| アテステーション保管 | `cargo xtask sorafs-self-cert` レポート、TLS ヘッダースナップショット、OpenSSL フィンガープリント。 | 3 年 | ゲートウェイ運用チーム |
| 変更管理 | `torii.sorafs_gateway` の差分、承認記録、デプロイ時刻。 | 2 年 | 変更管理責任者 |
| 演習記録 | 演習トラッカーの記録、参加者一覧、フォローアップ課題。 | 2 年 | カオスコーディネーター |
| ECH 切替イベント | 設定変更ログ、SDK/運用チーム向け署名済み告知、前後のテレメトリスナップショット。 | 2 年 | 開発者体験チーム |

## 運用プレイブック

### 緊急証明書ローテーション

**発火条件**
- `sorafs_gateway_tls_cert_expiry_seconds` < 1,209,600 (14 日)。
- `sorafs_gateway_tls_renewal_total{result="failure"}` が更新ウィンドウ内で 2 回発生。
- `X-Sora-TLS-State` が `last-error=` を示す、またはクライアントから証明書不一致やハンドシェイク失敗が報告される。

**安定化手順**
1. SoraFS ゲートウェイ TLS オンコールにページを送り、`#sorafs-incident` でインシデントを立ち上げる。
2. 構成ロールアウトを一時停止し、現在の設定コミットハッシュをインシデントチケットへ記録する。
3. `torii.sorafs_gateway.acme.enabled = false` と設定し、変更をコミットしてゲートウェイデプロイを再起動し、自動化を停止する。

**新バンドルの発行とデプロイ**
1. 監査用の状態を採取する。
   ```bash
   curl -sD - https://gateway.example/status \
     | grep -i '^x-sora-tls-state'
   openssl s_client -connect gateway.example:443 -servername gateway.example \
     < /dev/null 2>/dev/null | openssl x509 -noout -fingerprint -sha256
   ```
2. リポジトリのラッパーを使用して決定的なバンドルを生成します。
   ```bash
   scripts/sorafs-gateway tls renew \
     --host gateway.example \
     --out /var/lib/sorafs_gateway_tls/pending \
     --account-email tls-ops@example.com \
     --directory-url https://acme-v02.api.letsencrypt.org/directory \
     --force
   ```
   `pending` ディレクトリに `fullchain.pem`、`privkey.pem`、`ech.json` が出力されます。
3. シークレットストアへ配置し、Torii をリロードしてください。
   ```bash
   install -m 600 /var/lib/sorafs_gateway_tls/pending/fullchain.pem /etc/iroha/sorafs_gateway_tls/fullchain.pem
   install -m 600 /var/lib/sorafs_gateway_tls/pending/privkey.pem  /etc/iroha/sorafs_gateway_tls/privkey.pem
   install -m 640 /var/lib/sorafs_gateway_tls/pending/ech.json     /etc/iroha/sorafs_gateway_tls/ech.json
   systemctl reload iroha-torii.service
   ```

**検証と自動化の復帰**
1. 自己証明ハーネスを実行してください。
   ```bash
   scripts/sorafs_gateway_self_cert.sh \
     --gateway https://gateway.example \
     --cert /etc/iroha/sorafs_gateway_tls/fullchain.pem \
     --ech-config /etc/iroha/sorafs_gateway_tls/ech.json \
     --out artifacts/sorafs_gateway_self_cert
   ```
2. テレメトリの回復を確認してください。
   - `sorafs_gateway_tls_cert_expiry_seconds` > 2,592,000 (30 日)。
   - `sorafs_gateway_tls_renewal_total{result="success"}` が 1 増加。
   - `X-Sora-TLS-State` が `ech-enabled;expiry=…;renewed-at=…` で `last-error` を含まない。
3. 新しいフィンガープリント (GAR マニフェスト + アテステーションバンドル) をガバナンス成果物に反映し、インシデントチケットへ添付してください。
4. `torii.sorafs_gateway.acme.enabled = true` に戻して Torii を再読み込みし、停止していたパイプラインを再開してください。3 営業日以内に事後報告を提出してください。

### ECH フォールバック／縮退モード

CDN やクライアントが ECH を処理できない場合に使用します。

1. `sorafs_gateway_tls_ech_enabled == 0`、`GREASE_ECH_MISMATCH` を含む顧客インシデント、またはガバナンスからの指示を検知してください。
2. ECH を無効化してください。
   ```toml
   [torii.sorafs_gateway.acme]
   ech_enabled = false
   ```
   設定を適用 (または `iroha_cli config apply` を使用) し、Torii を再起動してください。`X-Sora-TLS-State` が `ech-disabled` を示すことを確認してください。
3. ストレージオペレーター、SDK チーム、ガバナンスに通知し、レビュー期間を共有してください。
4. ECH 無効化中は `sorafs_gateway_tls_renewal_total{result="failure"}` を監視し、TLS 更新の揺らぎがないか確認してください。
5. 上流サービスが復旧したら、次のラッパーを使って決定的なバンドルを生成してください。
   ```bash
   scripts/sorafs-gateway tls renew \
     --host gateway.example \
     --out /var/lib/sorafs_gateway_tls/pending \
     --account-email tls-ops@example.com \
     --directory-url https://acme-v02.api.letsencrypt.org/directory \
     --force
   ```
   これにより `pending` ディレクトリに PEM ファイルが出力されます。Torii を更新し、自己証明ハーネスを再実行してから `ech_enabled = true` を戻し、最新テレメトリを共有してください。

### 秘密鍵侵害時の失効

秘密鍵が漏洩した、または CA が誤発行を報告した場合に適用します。

1. 自動化を停止したまま、ストリームトークン署名鍵を運用ハンドブックに従ってローテーションし、資格情報の再利用を防いでください。
2. 次のコマンドで既存バンドルをアーカイブ付きで失効させてください (ファイルは `revoked/` に移動されます)。
   ```bash
   scripts/sorafs-gateway tls revoke \
     --out /var/lib/sorafs_gateway_tls \
     --reason keyCompromise
   ```
   生成される JSON 監査ファイルおよびアーカイブをインシデント記録に添付してください。
3. `scripts/sorafs-gateway tls renew --out /var/lib/sorafs_gateway_tls/pending --force` で新しいバンドルを発行し、緊急ローテーションと同じ検証手順に従ってください。
4. 更新済み GAR エンベロープと `manifest_signatures.json` を公開し、下流ノードが新フィンガープリントを採用できるようにしてください。
5. インシデント ID、失効日時、対処内容を添えてガバナンス評議会と SDK チームへ通知してください。

## 演習頻度とエビデンス

| 演習 | 頻度 | シナリオ | 成功基準 |
|-------|-----------|----------|------------------|
| `tls-renewal` | 四半期ごと | ステージング環境で緊急ローテーションを実施 (自動化停止/再開を含む)。 | 15 分未満で完了し、テレメトリが更新され、成果物が保管される。 |
| `ech-fallback` | 年 2 回 | ECH を 1 時間無効化して復旧。 | 通知が配信され、`X-Sora-TLS-State` が両状態を示し、未解決アラートが残らない。 |
| `tls-revocation` | 年 1 回 | 秘密鍵侵害を想定し、ステージング証明書を失効。 | 失効が確認され、新バンドルがデプロイされ、GAR が更新される。 |

各演習後の手順:
- 自動化ログ、Prometheus スナップショット、自己証明出力を `artifacts/sorafs_gateway_tls/<YYYYMMDD>/` に保管してください。
- `docs/source/sorafs_chaos_plan.md` の演習トラッカーを更新し、参加者、所要時間、観察事項、フォローアップを記録してください。
- 自動化の不具合はエビデンスを添えて SF-5 または SF-7 のロードマップフォローアップとして登録してください。

## トラブルシューティング

- **自動化ログで DNS-01 失敗が連続する:** 設定した `dns_provider` の IAM 権限を確認し、`dig _acme-challenge.gateway.example.com txt` で TXT レコード伝播を検証する。
- **`X-Sora-TLS-State` が欠落/不正:** 証明書配置後に Torii をリロードしたか確認し、`Metrics::set_sorafs_tls_state` が成功しているか (Torii ログの warn を確認) をチェックする。
- **GAR 違反カウンタが増加:** `torii_sorafs_gar_violations_total` の構造化ログを精査し、問題のプロバイダーまたはマニフェストを是正、ガバナンスへ通知した後にトラフィックを再開する。
- **ECH 無効化後もクライアント失敗が継続:** CDN (Cloudflare/CloudFront) のキャッシュを無効化し、利用者へフォールバックホスト名を共有する。

## 実装メモ

- **ACME クライアントライブラリ:** `letsencrypt-rs` を使用し、DNS-01/TLS-ALPN-01 の両チャレンジに対応します。既存の非同期エグゼキューターと統合され、Apache-2.0 で提供されます。
- **DNS プロバイダー抽象化:** デフォルトで Route53 をサポートします。`DnsProvider` トレイトを通じて Cloudflare や Google Cloud DNS を追加実装でき、コントローラー本体の変更は不要です。`acme.dns_provider = "<provider>"` で設定します。
- **テレメトリ命名整合:** テレメトリ表に示した名前が観測チームに承認済みであり、時間値は秒単位で公開されます。アップグレード後はダッシュボードのスキーマ変更を確認してください。
- **自己証明統合:** 証明書更新ごとに TLS 自動化フローが自己証明キットを起動します。`sorafs_gateway_tls_automation.yml` が `cargo xtask sorafs-self-cert --check-tls` を呼び出し、通知前に TLS/ECH の検証を自動化します。
