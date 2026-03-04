---
lang: ja
direction: ltr
source: docs/portal/docs/nexus/nexus-bootstrap-plan.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: nexus-bootstrap-plan
title: Sora Nexus بوٹ اسٹریپ اور آبزرویبیلٹی
説明: Nexus ٩ے بنیادی バリデーター クラスター کو آن لائن لانے سے پہلے SoraFS اور SoraNet خدمات شامل کرنے کا और देखें
---

:::note ٩ینونیکل ماخذ
یہ صفحہ `docs/source/soranexus_bootstrap_plan.md` کی عکاسی کرتا ہے۔ پکلائزڈ ورژنز پورٹل تک پہنچنے تک دونوں نقول ہم آہنگ رکھیں۔
:::

# Sora Nexus ブートストラップ & 可観測性計画

## ああ
- Torii API のコンセンサス監視、Sora Nexus バリデータ/オブザーバーの検証
- テスト (Torii コンセンサス、永続性) SoraFS/SoraNet ピギーバック デプロイメント テスト テスト
- CI/CD ワークフロー、可観測性ダッシュボード/アラート、監視、監視、監視、監視

## پیشگی شرائط
- ガバナンス キー マテリアル (評議会マルチシグ委員会キー) HSM ボールト メッセージ
- 分析 (Kubernetes クラスター、ベアメタル ノード) 分析 / 分析、分析、分析、分析、分析、分析、分析、分析。
- ブートストラップ構成 (`configs/nexus/bootstrap/*.toml`) コンセンサス パラメーターの説明

## और देखें
- Nexus 環境のプレフィックスの数:
- **Sora Nexus (メインネット)** - プレフィックス `nexus` 正規ガバナンス SoraFS/SoraNet ピギーバック サービス میزبان بناتا (チェーン ID `0x02F1` / UUID `00000000-0000-0000-0000-000000000753`)。
- **Sora Testus (テストネット)** - ステージング プレフィックス `testus` メインネット構成 統合テスト リリース前検証 ミラー ミラー (チェーン UUID `809574f5-fee7-5e69-bfcf-52451e42d50f`)。
- 環境、ジェネシス ファイル、ガバナンス キー、インフラストラクチャのフットプリントTestus テスト SoraFS/SoraNet ロールアウト テスト テスト グラウンド Nexus プロモーション テスト پہلے۔
- CI/CD パイプライン Testus のデプロイ 自動スモーク テスト 自動チェック Nexus マニュアル プロモーション
- リファレンス構成バンドル `configs/soranexus/nexus/` (メインネット) `configs/soranexus/testus/` (テストネット) `config.toml`、`genesis.json` Torii 入学ディレクトリ شامل ہیں۔

## 1 - 構成のレビュー
1. 文書化と監査:
   - `docs/source/nexus/architecture.md` (コンセンサス、Torii レイアウト)。
   - `docs/source/nexus/deployment_checklist.md` (インフラ要件)。
   - `docs/source/nexus/governance_keys.md` (鍵保管手順)。
2. Genesis ファイル (`configs/nexus/genesis/*.json`) 検証、検証、検証、ステーキング ウェイト、整列
3. パラメータの説明:
   - コンセンサス委員会の規模と定足数。
   - ブロック間隔/ファイナリティのしきい値。
   - Torii サービス ポートおよび TLS 証明書。

## 2 - ブートストラップ クラスターのデプロイメント
1. バリデーターノードのプロビジョニング:
   - `irohad` インスタンス (バリデータ) 永続ボリュームのデプロイ
   - ファイアウォール ルール - 許可 - Torii トラフィック ノード - 許可 - コンセンサス - Torii トラフィック ノード - 許可 -
2. バリデータ Torii サービス (REST/WebSocket) TLS の検証
3. 復元力の強化 オブザーバー ノード (読み取り専用) のデプロイ
4. ブートストラップ スクリプト (`scripts/nexus_bootstrap.sh`) ジェネシス コンセンサス ノード ノード
5. 煙テスト:
   - Torii テスト トランザクションが送信されます (`iroha_cli tx submit`)。
   - テレメトリーブロック生成/ファイナリティ検証
   - バリデーター/オブザーバー - 台帳複製 - 検証者/オブザーバー

## 3 - ガバナンスと主要な管理
1. 評議会のマルチシグ設定ガバナンス提案の提出 批准 ہو سکتی ہیں۔
2. コンセンサス/委員会の鍵アクセスログ 自動バックアップ 構成
3. 緊急キーローテーション手順 (`docs/source/nexus/key_rotation.md`) ランブックの確認

## 4 - CI/CD の統合
1. パイプラインは以下を構成します。
   - Validator/Torii イメージをビルドし、公開します (GitHub Actions、GitLab CI)。
   - 自動構成検証 (lint 生成、署名の検証)。
   - デプロイメント パイプライン (Helm/KusTOMize)、本番クラスターのステージング。
2. CI میں スモーク テスト شامل کریں (一時クラスター اٹھائیں، 正規トランザクション スイート چلائیں)。
3. デプロイメントとロールバック スクリプト、ランブック、およびランブック## 5 - 可観測性アラート
1. 監視スタック (Prometheus + Grafana + Alertmanager) リージョンのデプロイ
2. コア指標の概要:
  - `nexus_consensus_height`、`nexus_finality_lag`、`torii_request_duration_seconds`、`validator_peer_count`。
   - Torii コンセンサス サービス Loki/ELK ログ
3. ダッシュボード:
   - コンセンサスの健全性 (ブロックの高さ、ファイナリティ、ピアのステータス)。
   - Torii API 遅延/エラー率。
   - ガバナンス取引と提案ステータス。
4. アラート:
   - ブロック生産停止 (>2 ブロック間隔)。
   - ピア数定足数 سے نیچے گر جائے۔
   - Torii エラー率のスパイク。
   - ガバナンス提案キューのバックログ。

## 6 - 検証と引き継ぎ
1. エンドツーエンドの検証:
   - ガバナンス提案を提出します (パラメータ変更)。
   - 理事会の承認プロセスとガバナンス パイプラインの承認
   - 台帳の状態の差分と一貫性の確認
2. オンコール ランブック (インシデント対応、フェイルオーバー、スケーリング)。
3. SoraFS/SoraNet ٹیموں کو 準備完了ですピギーバック デプロイメント Nexus ノードのポイント数

## 実装チェックリスト
- [ ] 生成/構成の監査
- [ ] バリデーター、オブザーバー ノードのデプロイ、コンセンサス、コンセンサス
- [ ] ガバナンス キー فوڈ ہوئے، 提案テスト ہوا۔
- [ ] CI/CD パイプライン (ビルド + デプロイ + スモーク テスト)。
- [ ] 可観測性ダッシュボード فعال ہیں اور アラート موجود ہے۔
- [ ] ハンドオフ ドキュメントのダウンストリーム ٹیموں کو دے دی گئی۔