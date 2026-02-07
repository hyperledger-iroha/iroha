---
lang: ja
direction: ltr
source: docs/portal/docs/nexus/nexus-bootstrap-plan.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: nexus-bootstrap-plan
title: Bootstrap e observaviidade do Sora Nexus
説明: コロカル クラスター セントラル デ バリドーレス Nexus オンラインの高度なサービス SoraFS e SoraNet。
---

:::note フォンテ カノニカ
エスタページナリフレテ`docs/source/soranexus_bootstrap_plan.md`。デュアス・コピアス・アリンハダスとしてマンテンハは、ヴェルソエス・ローカルイザダス・チェゲム・アオポータルとしてクエリを食べました。
:::

# ソラ Nexus のブートストラップと観察を計画

## オブジェクト
- Levantar は、Sora Nexus com Chaves de Government、API Torii e Monitoramento de consenso を検証します。
- Validar servicos centrais (Torii、consenso、persistencia) antes de habilitar はピギーバック SoraFS/SoraNet を展開します。
- CI/CD およびダッシュボード/アラートのワークフローを確実に監視します。

## 前提条件
- HSM または Vault の管理資料 (multisig do conselho、chaves de comite)。
- インフラストラクチャ ベース (ベアメタルの Kubernetes クラスター) 地域のプライマリ/セカンダリア。
- 最近のコンセンサスに関するブートストラップの設定 (`configs/nexus/bootstrap/*.toml`) の OS パラメータが表示されます。

## アンビエンテス デ レッド
- オペレーターは環境 Nexus com 接頭辞を読み取ります:
- **Sora Nexus (メインネット)** - 生産物 `nexus` のプレフィックス、統治機関カノニカとサービスのピギーバック SoraFS/SoraNet (チェーン ID `0x02F1` / UUID) `00000000-0000-0000-0000-000000000753`)。
- **Sora Testus (テストネット)** - ステージング `testus` のプレフィックス、テスト統合のためのメインネットの設定、およびプレリリースの検証を実行します (チェーン UUID `809574f5-fee7-5e69-bfcf-52451e42d50f`)。
- 分離の起源、統治下の管理、および環境のインフラストラクチャーの足跡を管理します。 Testus atua como Campo de provas para rollout SoraFS/SoraNet antes de promover para Nexus。
- パイプラインの CI/CD 開発は、Testus と executar のスモーク テストを自動展開し、Nexus でのプロモーション マニュアルを実行して、OS をチェックします。
- `configs/soranexus/nexus/` (メインネット) および `configs/soranexus/testus/` (テストネット) の参照用設定バンドル、`config.toml`、`genesis.json` および管理者 Torii の例。

## Etapa 1 - 構成の見直し
1. 存在する監査文書:
   - `docs/source/nexus/architecture.md` (コンセンサス、レイアウト Torii)。
   - `docs/source/nexus/deployment_checklist.md` (インフラストラクチャの要件)。
   - `docs/source/nexus/governance_keys.md` (シャーベスの保管手順)。
2. Validar que arquivos Genesis (`configs/nexus/genesis/*.json`) アリナム・コム・ロスター・アチュアル・デ・バリドーレス・ペソ・デ・ステーキング。
3. 必要なパラメータを確認します。
   - タマンホは定足数のコンセンサス委員会を行います。
   - ブロックの間隔 / 最後の限界。
   - Torii 電子証明書 TLS を提供するポート。

## Etapa 2 - クラスターのブートストラップをデプロイする
1. 暫定的な有効期限:
   - 永続的なインスタンス `irohad` (validadores) com ボリュームをデプロイします。
   - Torii エントリ番号でファイアウォールの許可を保証します。
2. TLS の認証サービス Torii (REST/WebSocket) を開始します。
3. de nos observadores (somente leitura) para resiliencia adicional を展開します。
4. ブートストラップ実行スクリプト (`scripts/nexus_bootstrap.sh`) 配布元の生成、最初のコンセンサス、レジストラ番号。
5. エグゼキュータのスモークテスト:
   - Torii (`iroha_cli tx submit`) 経由の Enviar transacoes de teste。
   - テレメトリ経由での生産/最終的なブロックの検証。
   - 検証/観測所の台帳のレプリカを確認します。

## Etapa 3 - ガベルナンカとゲスタオ・デ・チャベス
1. コンセルホでマルチシグを設定する;政府の提案書と承認事項を確認します。
2. アルマゼナール・コム・セグランカ・チャベス・デ・コンセンサス/コミテ。構成バックアップは自動 OS COM ログ記録にアクセスします。
3. 緊急時の緊急対応手順 (`docs/source/nexus/key_rotation.md`) 検証ランブックを構成します。

## Etapa 4 - インテグラカオ CI/CD
1. パイプラインを構成します。
   - publicacao de imagens validator/Torii (GitHub Actions ou GitLab CI) をビルドします。
   - 自動設定の検証 (生成、実行の検証)。
   - クラスターのステージングと生産のためのデプロイメント (Helm/KusTOMize) のパイプライン。
2. CI なしのスモーク テストを実装します (subir クラスター efemero、rodar suite canonica de transacoes)。
3. ドキュメント Runbook のデプロイに伴うロールバック用の追加スクリプト。## Etapa 5 - 警報の監視
1. 監視スタック (Prometheus + Grafana + Alertmanager) を定期的に展開します。
2.コレタール・メトリカス・セントレイス：
  - `nexus_consensus_height`、`nexus_finality_lag`、`torii_request_duration_seconds`、`validator_peer_count`。
   - Loki/ELK パラサービス Torii とコンセンサスを介したログ。
3. ダッシュボード:
   - Saude do consenso (altura de bloco、finalidade、status depeers)。
   - API Torii のエラー分類群。
   - 統治機構と提案ステータス。
4. アラート:
   - ブロコ生産のパラダ (ブロコの間隔が 2 つ以上)。
   - 定足数を超えたピアの数はありません。
   - Torii の分類群。
   - 政府の提案に関するバックログ。

## Etapa 6 - Validacao とハンドオフ
1. エンドツーエンドの Rodar validacao:
   - Submeter proposta de Governmenta (例: mudanca de parametro)。
   - 統治機能のパイプラインを保証するための承認を行う処理者。
   - 保証の元帳の差分を記録します。
2. オンコールでのドキュメントまたはランブック (インシデントの復旧、フェイルオーバー、スケーリング)。
3. Comunicar prontidao は SoraFS/SoraNet を装備します。確認クエリは、Nexus のパラメタにピギーバック ポデムを展開します。

## 実装のチェックリスト
- [ ] オーディトリア・デ・ジェネシス/構成の結論。
- [ ] Nos validadores e observadoresdeployados com consenso saudavel。
- [ ] Chaves de Governmenta carregadas、proposta testada。
- [ ] パイプライン CI/CD ロダンド (ビルド + デプロイ + スモーク テスト)。
- [ ] アラートを監視するダッシュボード。
- [ ] ダウンストリームの AOS 時間に渡ってハンドオフを記録します。