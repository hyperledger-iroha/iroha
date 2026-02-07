---
lang: ja
direction: ltr
source: docs/source/compliance/android/device_lab_failover_runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 473b2b49d32c32d2b884b670ba35e9aa3d0606cfd451d441a7ca927c1160311d
source_last_modified: "2026-01-03T18:07:59.262670+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Android Device Lab フェイルオーバー ドリル ランブック (AND6/AND7)

このランブックには、手順、証拠要件、連絡先マトリックスが記載されています
で参照されている **デバイスラボ緊急時対応計画** を実行するときに使用されます。
`roadmap.md` (§「規制上のアーティファクトの承認とラボの緊急事態」)。補完する
予約ワークフロー (`device_lab_reservation.md`) とインシデント ログ
(`device_lab_contingency.md`) コンプライアンス審査担当者、法律顧問、SRE
フェイルオーバーの準備が整っているかどうかを検証する方法について、唯一の信頼できる情報源があります。

## 目的と頻度

- Android StrongBox + 一般的なデバイス プールがフェイルオーバーできることを実証します。
  フォールバック Pixel レーン、共有プール、Firebase Test Lab バースト キュー、および
  AND6/AND7 SLA を欠落させることなく、外部 StrongBox リテーナーを装着できます。
- 法務担当者が ETSI/FISC 提出物に添付できる証拠バンドルを作成します。
  2月のコンプライアンスレビューに先立って。
- 四半期に少なくとも 1 回、およびラボ ハードウェアの名簿が変更されるたびに実行します
  (新しいデバイス、廃止、または 24 時間を超えるメンテナンス)。

|ドリルID |日付 |シナリオ |証拠バンドル |ステータス |
|----------|------|----------|------|----------|
| DR-2026-02-Q1 | 2026-02-20 | AND7 テレメトリ リハーサルによる Pixel8Pro レーン停止 + 認証バックログのシミュレート | `artifacts/android/device_lab_contingency/20260220-failover-drill/` | ✅ 完了 — `docs/source/compliance/android/evidence_log.csv` に記録されたバンドル ハッシュ。 |
| DR-2026-05-Q2 | 2026-05-22 (予定) | StrongBox メンテナンス重複 + Nexus リハーサル | `artifacts/android/device_lab_contingency/20260522-failover-drill/` *(保留中)* — `_android-device-lab` チケット **AND6-DR-202605** は予約を保持します。バンドルはドリル後に設定されます。 | 🗓 スケジュール済み — AND6 リズムごとにカレンダー ブロックが「Android Device Lab – 予約」に追加されます。 |

## 手順

### 1. ドリル前の準備

1. `docs/source/sdk/android/android_strongbox_capture_status.md` でベースライン容量を確認します。
2. 対象の ISO 週の予約カレンダーをエクスポートします。
   `python3 scripts/android_device_lab_export.py --week <ISO week>`。
3. ファイル `_android-device-lab` チケット
   `AND6-DR-<YYYYMM>` スコープ (「フェイルオーバー ドリル」)、計画されたスロット、および影響を受ける
   ワークロード (構成証明、CI スモーク、テレメトリの混乱)。
4. `device_lab_contingency.md` の緊急時ログ テンプレートを更新します。
   訓練日のプレースホルダー行。

### 2. 故障状態をシミュレートする

1. ラボ内のプライマリ レーン (`pixel8pro-strongbox-a`) を無効にするかプール解除します。
   スケジューラを作成し、予約エントリに「ドリル」というタグを付けます。
2. PagerDuty (`AND6-device-lab` サービス) で模擬停止アラートをトリガーし、
   証拠バンドルの通知エクスポートをキャプチャします。
3. 通常レーンを消費する Buildkite ジョブに注釈を付ける
   (`android-strongbox-attestation`、`android-ci-e2e`) をドリル ID に置き換えます。

### 3. フェイルオーバーの実行1. フォールバック Pixel7 レーンをプライマリ CI ターゲットにプロモートし、
   それに対して計画されたワークロード。
2. `firebase-burst` レーン経由で Firebase Test Lab バースト スイートをトリガーします。
   小売店のウォレットのスモークテストが行われる一方、StrongBox の対象範囲は共有領域に移行します。
   レーン。 CLI 呼び出し (またはコンソール エクスポート) を監査用のチケットにキャプチャします。
   パリティ。
3. 外部の StrongBox ラボ リテイナーを使用して、短い認証スイープを実行します。
   以下で説明するように、連絡の確認をログに記録します。
4. すべての Buildkite 実行 ID、Firebase ジョブ URL、およびリテイナーのトランスクリプトを記録します。
   `_android-device-lab` チケットと証拠バンドルのマニフェスト。

### 4. 検証とロールバック

1. アテステーション/CI ランタイムをベースラインと比較します。フラグデルタ>10%
   ハードウェアラボのリーダー。
2. プライマリ レーンを復元し、キャパシティ スナップショットと準備状況を更新します。
   検証に合格した後の行列。
3. トリガー、アクション、および
   そしてフォローアップ。
4. `docs/source/compliance/android/evidence_log.csv` を次のように更新します。
   バンドル パス、SHA-256 マニフェスト、Buildkite 実行 ID、PagerDuty エクスポート ハッシュ、および
   査読者のサインオフ。

## 証拠バンドルのレイアウト

|ファイル |説明 |
|------|---------------|
| `README.md` |概要 (ドリル ID、スコープ、所有者、タイムライン)。 |
| `bundle-manifest.json` |バンドル内のすべてのファイルの SHA-256 マップ。 |
| `calendar-export.{ics,json}` |エクスポート スクリプトからの ISO 週間予約カレンダー。 |
| `pagerduty/incident_<id>.json` |アラートと確認のタイムラインを示す PagerDuty インシデントのエクスポート。 |
| `buildkite/<job>.txt` | Buildkite は、影響を受けるジョブの URL とログを実行します。 |
| `firebase/burst_report.json` | Firebase Test Lab のバースト実行の概要。 |
| `retainer/acknowledgement.eml` |外部の StrongBox ラボからの確認。 |
| `photos/` |ハードウェアが再ケーブル接続されている場合は、ラボ トポロジのオプションの写真/スクリーンショット。 |

バンドルを次の場所に保管します
`artifacts/android/device_lab_contingency/<YYYYMMDD>-failover-drill/` と記録
証拠ログ内のマニフェスト チェックサムと AND6 準拠チェックリスト。

## 連絡先とエスカレーション マトリックス

|役割 |主な連絡先 |チャンネル |メモ |
|------|------|-----------|------|
|ハードウェア ラボ リーダー |プリヤ・ラマナタン | `@android-lab` スラック · +81-3-5550-1234 |オンサイトのアクションとカレンダーの更新を管理します。 |
|デバイス ラボ運用 |マテオ・クルーズ | `_android-device-lab` キュー |予約チケット + バンドルのアップロードを調整します。 |
|リリースエンジニアリング |アレクセイ・モロゾフ |リリース英語 Slack · `release-eng@iroha.org` | Buildkite の証拠を検証し、ハッシュを公開します。 |
|外部ストロングボックスラボ |サクラ楽器 NOC | `noc@sakura.example` · +81-3-5550-9876 |リテーナーコンタクト。 6 時間以内に空き状況を確認してください。 |
| Firebase バースト コーディネーター |テッサ・ライト | `@android-ci` スラック |フォールバックが必要な場合に Firebase Test Lab の自動化をトリガーします。 |

ドリルによって障害となる問題が明らかになった場合は、次の順序でエスカレーションします。
1. ハードウェアラボリーダー
2. Android 基盤 TL
3. プログラムリード / リリースエンジニアリング
4. コンプライアンス責任者 + 法律顧問 (訓練により規制リスクが明らかになった場合)

## 報告とフォローアップ- 参照する場合は常に、このランブックを予約手順と並べてリンクします。
  `roadmap.md`、`status.md`、およびガバナンス パケットのフェールオーバー準備状況。
- 四半期ごとの訓練の要約を証拠バンドルとともにコンプライアンス + 法務部門に電子メールで送信します
  ハッシュ テーブルを作成し、`_android-device-lab` チケット エクスポートを添付します。
- ミラーの主要なメトリクス (フェールオーバーまでの時間、復元されたワークロード、未処理のアクション)
  `status.md` と AND7 ホットリスト トラッカー内にあるため、レビュー担当者は
  具体的なリハーサルへの依存。