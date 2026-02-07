---
lang: ja
direction: ltr
source: docs/portal/docs/nexus/nexus-bootstrap-plan.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: nexus-bootstrap-plan
title: ソラ Nexus のブートストラップと観察可能性
説明: 有効なクラスターの中心となる運用計画 Nexus および統合サービス SoraFS および SoraNet。
---

:::ノート フエンテ カノニカ
エスタページナリフレジャ`docs/source/soranexus_bootstrap_plan.md`。 Manten ambas alineadas hasta que las versiones localizadas lleguen al portal。
:::

# ブートストラップとソラの観察を計画 Nexus

## オブジェクト
- Levantar は、Sora Nexus の有効な検証/監視の赤い基地、Gobernanza の API、Torii の監視およびコンセンサスの監視を行います。
- 中央サービス (Torii、コンセンサス、永続化) は、SoraFS/SoraNet に便乗して利用可能です。
- CI/CD およびダッシュボード/アラートのワークフローを監視し、安全な状態で監視します。

## 前提条件
- HSM または Vault で管理される資料 (multisig del consejo、llaves de comite)。
- インフラストラクチャ ベース (クラスタ Kubernetes またはノード ベアメタル) プライマリ/セカンダリア リージョン。
- ブートストラップの実際の構成 (`configs/nexus/bootstrap/*.toml`) は、コンセンサスの究極のパラメータを参照します。

## エントルノス・デ・レッド
- 赤いディスティントのオペレーター Nexus:
- **Sora Nexus (メインネット)** - `nexus` の生産準備、SoraFS/SoraNet のピギーバック サービスの提供 (チェーン ID `0x02F1` / UUID) `00000000-0000-0000-0000-000000000753`)。
- **Sora Testus (テストネット)** - ステージング `testus` の赤、リリース前の統合と検証におけるメインネットの構成の確認 (チェーン UUID `809574f5-fee7-5e69-bfcf-52451e42d50f`)。
- セパラドスの発生源、ラベス デ ゴベルナンザ、およびインフラストラクチャーの管理を管理します。 Testus actua como el banco de pruebas de todos los rollouts SoraFS/SoraNet antes de promover a Nexus。
- テスト用の CI/CD のパイプライン、自動排煙テスト、プロモーション マニュアル、Nexus の検査が必要です。
- `configs/soranexus/nexus/` (メインネット) および `configs/soranexus/testus/` (テストネット) の参照用バンドルが失われ、`config.toml`、`genesis.json` および管理用ディレクトリ Torii が見つかりません。

## 手順 1 - 構成の改訂
1. 存在する文書の監査:
   - `docs/source/nexus/architecture.md` (コンセンサス、レイアウト Torii)。
   - `docs/source/nexus/deployment_checklist.md` (インフラストラクチャの要件)。
   - `docs/source/nexus/governance_keys.md` (ラベスの管理手順)。
2. Validar que los archives Genesis (`configs/nexus/genesis/*.json`) は、実際の Validadores と Los pesos de sking の名簿を確認します。
3. 赤のパラメータを確認します。
   - 玉野委員会の合意と定足数。
   - ブロック間/最終決定のアンブラル。
   - プエルトス デ サービス Torii および TLS 証明書。

## パソ 2 - クラスタ ブートストラップのデスリーグ
1. 暫定的なノードの有効性:
   - Desplegar instancias `irohad` (validadores) con volumenes が持続します。
   - ファイアウォールの規制は、Torii エントリのコンセンサストラフィックを許可します。
2. TLS の検証サービス Torii (REST/WebSocket) を開始します。
3. Desplegar nodos observadores (solo lectura) para resiliencia adicional。
4. ブートストラップのイジェクタ スクリプト (`scripts/nexus_bootstrap.sh`) は、配布元の生成、初期のコンセンサス、レジストラ ノードに含まれます。
5. エジェクターの煙テスト:
   - Torii (`iroha_cli tx submit`) 経由のプルエバ トランザクションを参照してください。
   - 中央テレメトリの製造/最終ブロックの検証。
   - 検証/観測所の台帳の複製を改訂します。

## Paso 3 - Gobernanza y gestion de llaves
1. マルチシグデルコンセホの設定を行う。承認者は、ゴベルナンザ・セ・プエダンの羨望の的であり、批准者であることを確認します。
2. 合意/委員会の形成のためのアルマセナール;構成バックアップは、ログ記録に応じて自動的に実行されます。
3. 緊急時の緊急対応手順 (`docs/source/nexus/key_rotation.md`) と検証用 Runbook を構成します。

## 手順 4 - CI/CD の統合
1. パイプラインを構成します。
   - 画像検証バリデータ/Torii の公開をビルドします (GitHub Actions または GitLab CI)。
   - 自動構成の検証 (生成、企業の検証)。
   - クラスターのステージングと制作のパイプライン (Helm/KusTOMize)。
2. CI でスモーク テストを実装します (levantar クラスター efimero、correr suite canonica de transacciones)。
3. フォールドやドキュメント Runbook のロールバック用スクリプトを統合。## パソ 5 - 監視と警告
1. リージョンの監視スタック (Prometheus + Grafana + Alertmanager) を削除します。
2. 中心的な指標を再コピーします。
  - `nexus_consensus_height`、`nexus_finality_lag`、`torii_request_duration_seconds`、`validator_peer_count`。
   - Loki/ELK パラサービス Torii y コンセンサスを介したログ。
3. ダッシュボード:
   - 合意の表明 (ブロックの合意、ファイナリザシオン、ピアの確立)。
   - Torii API の遅延/エラー。
   - 知事とプロプエスタの取引。
4. アラート:
   - ブロックの生産パロ (ブロックの間隔が 2 つを超える)。
   - 定足数に対する仲間のコンテオ。
   - Torii でエラーが発生しました。
   - 政府の資金調達のバックログ。

## パソ 6 - 検証と引き継ぎ
1. Ejecutar のエンドツーエンド検証:
   - Enviar una propuesta de gobernanza (p. ej.、cambio de parametro)。
   - 安全なパイプラインの管理による手続き。
   - 一貫性のある帳簿の差分を記録します。
2. オンコールでのランブックの文書化 (インシデント、フェイルオーバー、エスカレードの対応)。
3. SoraFS/SoraNet のロス・エクイポス通信。確認者 que los despliegues ピギーバック puedan apuntar a nodos Nexus。

## 実装のチェックリスト
- [ ] オーディオの生成/構成が完了しました。
- [ ] ノドスの有効性と観察性は、合意に基づいて評価できるものです。
- [ ] Llaves de gobernanza cargadas、propuesta probada。
- [ ] パイプライン CI/CD コリエンド (ビルド + デプロイ + スモーク テスト)。
- [ ] アラートを監視するダッシュボード。
- [ ] ダウンストリームでのハンドオフに関する文書。