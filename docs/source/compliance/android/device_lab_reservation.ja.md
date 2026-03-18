---
lang: ja
direction: ltr
source: docs/source/compliance/android/device_lab_reservation.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 05dc578338882ddfcdf2410b0643774ceb8212f28739ba94ac83edf087b9b5dc
source_last_modified: "2026-01-03T18:07:59.245516+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Android Device Lab予約手順（AND6/AND7）

このプレイブックでは、Android チームがデバイスを予約、確認、監査する方法について説明します。
マイルストーン **AND6** (CI およびコンプライアンスの強化) および **AND7** のラボ時間
(可観測性の準備)。不測の事態に備えたログインを補完します
容量確保による`docs/source/compliance/android/device_lab_contingency.md`
欠品はまず避けられます。

## 1. 目標と範囲

- StrongBox + 一般デバイス プールをロードマップで義務付けられている 80% 以上に維持する
  フリーズウィンドウ全体の容量目標。
- 決定論的なカレンダーを提供するため、CI、認証スイープ、カオスが発生しません。
  リハーサルでは同じハードウェアを争うことはありません。
- フィードする監査可能な証跡 (リクエスト、承認、実行後のメモ) をキャプチャします。
  AND6 準拠チェックリストと証拠ログ。

この手順では、専用ピクセル レーン、共有フォールバック プール、および
ロードマップで参照されている外部の StrongBox ラボ リテイナー。アドホックエミュレータ
使用方法は対象外となります。

## 2. 予約受付期間

|プール/レーン |ハードウェア |デフォルトのスロット長 |予約リードタイム |オーナー |
|---------------|----------|----------|----------|----------|
| `pixel8pro-strongbox-a` | Pixel8Pro (StrongBox) | 4時間 | 3営業日 |ハードウェア ラボ リーダー |
| `pixel8a-ci-b` | Pixel8a (CI全般) | 2時間 | 2営業日 | Android 基盤 TL |
| `pixel7-fallback` | Pixel7 共有プール | 2時間 | 1営業日 |リリースエンジニアリング |
| `firebase-burst` | Firebase Test Lab の煙キュー | 1時間 | 1営業日 | Android 基盤 TL |
| `strongbox-external` |外部 StrongBox ラボリテーナー | 8時間 | 7 暦日 |プログラムリーダー |

スロットは UTC で予約されます。重複する予約には明示的な承認が必要です
ハードウェアラボリーダーより。

## 3. リクエストのワークフロー

1. **コンテキストを準備します**
   - `docs/source/sdk/android/android_strongbox_device_matrix.md` を次のように更新します
     演習する予定のデバイスと Readiness タグ
     (`attestation`、`ci`、`chaos`、`partner`)。
   - 最新の容量スナップショットを収集します。
     `docs/source/sdk/android/android_strongbox_capture_status.md`。
2. **リクエストを送信**
   - のテンプレートを使用して、`_android-device-lab` キューにチケットをファイルします。
     `docs/examples/android_device_lab_request.md` (所有者、日付、ワークロード、
     フォールバック要件）。
   - 規制上の依存関係を添付します (例: AND6 認証スイープ、AND7)
     テレメトリ ドリル) と、関連するロードマップ エントリへのリンク。
3. **承認**
   - ハードウェア ラボ リーダーが 1 営業日以内にレビューし、スロットの確認を行います。
     共有カレンダー (`Android Device Lab – Reservations`)、および
     `device_lab_capacity_pct`列
     `docs/source/compliance/android/evidence_log.csv`。
4. **実行**
   - スケジュールされたジョブを実行します。 Buildkite の実行 ID またはツールのログを記録します。
   - 逸脱（ハードウェア交換、オーバーラン）があればメモしてください。
5. **終了**
   - アーティファクト/リンクを含むチケットにコメントします。
   - 実行がコンプライアンス関連だった場合は、更新します
     `docs/source/compliance/android/and6_compliance_checklist.md` と行を追加します
     `evidence_log.csv`まで。

パートナー デモ (AND8) に影響を与えるリクエストは、パートナー エンジニアリングに cc する必要があります。

## 4. 変更・キャンセル- **再スケジュール:** 元のチケットを再度開き、新しいスロットを提案し、
  カレンダーのエントリー。新しいスロットが 24 時間以内の場合は、Hardware Lab Lead + SRE に ping を送信します。
  直接的に。
- **緊急キャンセル:** 緊急時対応計画に従ってください
  (`device_lab_contingency.md`) トリガー/アクション/フォローアップ行を記録します。
- **オーバーラン:** 実行がスロットを 15 分以上超えた場合は、更新を投稿して確認します
  次の予約を続行できるかどうか。それ以外の場合はフォールバックにハンドオフします
  プールまたは Firebase バースト レーン。

## 5. 証拠と監査

|アーティファクト |場所 |メモ |
|----------|----------|----------|
|予約チケット | `_android-device-lab` キュー (Jira) |週ごとのサマリーをエクスポートします。証拠ログ内のチケット ID をリンクします。 |
|カレンダーのエクスポート | `artifacts/android/device_lab/<YYYY-WW>-calendar.{ics,json}` |毎週金曜日に `scripts/android_device_lab_export.py --ics-url <calendar_ics_feed>` を実行します。ヘルパーは、フィルタリングされた `.ics` ファイルと ISO 週の JSON サマリーを保存するため、監査では手動でダウンロードせずに両方のアーティファクトを添付できます。 |
|容量スナップショット | `docs/source/compliance/android/evidence_log.csv` |予約/終了ごとに更新します。 |
|実行後のメモ | `docs/source/compliance/android/device_lab_contingency.md` (不測の事態の場合) またはチケット コメント |監査のために必要です。 |

四半期ごとのコンプライアンス レビュー中に、カレンダーのエクスポート、チケットの概要、
AND6 チェックリスト提出の証拠ログの抜粋。

### カレンダーのエクスポートの自動化

1. 「Android Device Lab – 予約」の ICS フィード URL を取得します (または `.ics` ファイルをダウンロードします)。
2. 実行

   ```bash
   python3 scripts/android_device_lab_export.py \
     --ics-url "https://calendar.example/ical/export" \
     --week <ISO week, defaults to current>
   ```

   スクリプトは両方の `artifacts/android/device_lab/<YYYY-WW>-calendar.ics` を書き込みます。
   および `...-calendar.json`、選択された ISO 週をキャプチャします。
3. 生成されたファイルを毎週の証拠パケットとともにアップロードし、
   `docs/source/compliance/android/evidence_log.csv` の JSON 概要
   ロギングデバイスとラボの容量。

## 6. エスカレーションラダー

1. ハードウェア ラボ リーダー (プライマリ)
2. Android 基盤 TL
3. プログラム リード / リリース エンジニアリング (フリーズ ウィンドウ用)
4. 外部 StrongBox ラボ連絡先 (リテイナーが呼び出されたとき)

エスカレーションはチケットに記録し、毎週の Android に反映する必要があります
ステータスメール。

## 7. 関連文書

- `docs/source/compliance/android/device_lab_contingency.md` — インシデントログ
  容量不足。
- `docs/source/compliance/android/and6_compliance_checklist.md` — マスター
  成果物のチェックリスト。
- `docs/source/sdk/android/android_strongbox_device_matrix.md` — ハードウェア
  カバレッジトラッカー。
- `docs/source/sdk/android/android_strongbox_attestation_run_log.md` —
  AND6/AND7 によって参照される StrongBox の証明証拠。

この予約手順を維持することで、ロードマップのアクション項目「定義」が満たされます。
デバイス ラボ予約手順」を使用し、パートナー向けのコンプライアンス アーティファクトを保持します
Android 準備計画の残りの部分と同期します。

## 8. フェイルオーバー訓練の手順と連絡先

ロードマップ項目 AND6 では、四半期ごとのフェイルオーバーのリハーサルも必要です。いっぱい、
段階的な手順が記載されています
`docs/source/compliance/android/device_lab_failover_runbook.md`、しかし高い
レベルのワークフローは以下にまとめられているため、要求者は並行して訓練を計画できます。
定期的な予約。1. **訓練のスケジュールを設定します:** 影響を受ける車線をブロックします (`pixel8pro-strongbox-a`、
   共有内のフォールバック プール、`firebase-burst`、外部 StrongBox リテーナー)
   カレンダーと `_android-device-lab` は訓練の少なくとも 7 日前にキューに入れてください。
2. **停止をシミュレートします:** プライマリ レーンをデプールし、PagerDuty をトリガーします。
   (`AND6-device-lab`) インシデントを検出し、依存する Buildkite ジョブに注釈を付けます。
   ランブックに記載されているドリル ID。
3. **フェイルオーバー:** Pixel7 フォールバック レーンを昇格し、Firebase バーストを開始します
   スイートにアクセスし、6 時間以内に外部の StrongBox パートナーと連携します。キャプチャ
   Buildkite の実行 URL、Firebase エクスポート、およびリテイナーの確認応答。
4. **検証と復元:** 構成証明 + CI ランタイムを検証し、
   元のレーン、および `device_lab_contingency.md` と証拠ログを更新します
   バンドル パス + チェックサムを使用します。

### 連絡先とエスカレーションのリファレンス

|役割 |主な連絡先 |チャンネル |エスカレーション順序 |
|------|---------------|-----------|------|
|ハードウェア ラボ リーダー |プリヤ・ラマナタン | `@android-lab` スラック · +81-3-5550-1234 | 1 |
|デバイス ラボ運用 |マテオ・クルーズ | `_android-device-lab` キュー | 2 |
| Android 基盤 TL |エレナ・ヴォロベワ | `@android-foundations` スラック | 3 |
|リリースエンジニアリング |アレクセイ・モロゾフ | `release-eng@iroha.org` | 4 |
|外部ストロングボックスラボ |サクラ楽器 NOC | `noc@sakura.example` · +81-3-5550-9876 | 5 |

ドリルによって障害となる問題が明らかになった場合、またはフォールバックが発生した場合は、順次エスカレーションします。
レーンを 30 分以内にオンラインにすることはできません。エスカレーションを常に記録する
`_android-device-lab` チケット内のメモを作成し、緊急事態ログにミラーリングします。