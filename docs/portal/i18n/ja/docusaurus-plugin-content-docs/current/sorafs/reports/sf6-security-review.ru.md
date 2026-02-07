---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/reports/sf6-security-review.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
タイトル: Отчет по безопасности SF-6
概要: キーレス署名、プルーフ ストリーミング、マニフェストの説明。
---

# SF-6 を完成させます

**Окно оценки:** 2026-02-10 → 2026-02-18  
**セキュリティ エンジニアリング ギルド (`@sec-eng`)、ツール ワーキング グループ (`@tooling-wg`)  
**説明:** SoraFS CLI/SDK (`sorafs_cli`、`sorafs_car`、`sorafs_manifest`)、プルーフ ストリーミング API、Torii のマニフェスト、 Sigstore/OIDC、CI リリース フック。  
** Артефакты:**  
- CLI および тесты (`crates/sorafs_car/src/bin/sorafs_cli.rs`)  
- Torii ハンドラーのマニフェスト/証明 (`crates/iroha_torii/src/sorafs/api.rs`)  
- リリースの自動化 (`ci/check_sorafs_cli_release.sh`、`scripts/release_sorafs_cli.sh`)  
- 確定的パリティ ハーネス (`crates/sorafs_car/tests/sorafs_cli.rs`、[Отчет о паритете GA SoraFS Orchestrator](./orchestrator-ga-parity.md))

## Методология

1. **脅威モデリング ワークショップ** セキュリティ ポリシー、Toriiうそ。  
2. **コード レビュー** сфокусировался на поверхностях учетных данных (обмен токенами OIDC、キーレス署名)、валидации Norito は、バックプレッシャーとプルーフ ストリーミングを示します。  
3. **動的テスト** フィクスチャ マニフェストとテスト (トークン リプレイ、マニフェスト改ざん、プルーフ ストリーム)、パリティ ハーネス、ファズ ドライブのテスト。  
4. **構成の検査** デフォルト `iroha_config`、CLI およびリリース スクリプト、アップデートの実行ね。  
5. **インタビューのプロセス** 修復フロー、エスカレーション パス、監査証拠を作成し、所有者ツール WG をリリースします。

## Сводка находок

| ID |重大度 |エリア |発見 |解像度 |
|----|----------|------|----------|------------|
| SF6-SR-01 |高 |キーレス署名 |デフォルトの OIDC トークンは、CI テンプレート、テナント間再生をサポートします。 | `--identity-token-audience`、リリース フック、CI テンプレート ([リリース プロセス](../developer-releases.md)、`docs/examples/sorafs_ci.md`)。 CI は、указана を表示します。 |
| SF6-SR-02 |中 |プルーフストリーミング |バック プレッシャー パスは、最も重要な問題を解決します。 | `sorafs_cli proof stream` ограничивает размеры каналов с детерминированным truncation, логирует Norito summary ит поток; Torii 応答チャンク (`crates/iroha_torii/src/sorafs/api.rs`) をミラーリングします。 |
| SF6-SR-03 |中 | Отправка マニフェスト | CLI は、チャンク プラン、когда `--plan` отсутствовал をマニフェストします。 | `sorafs_cli manifest submit` теперь пересчитывает и сравнивает CAR ダイジェスト、если не указан `--expect-plan-digest`、отклоняя の不一致と выдавая 修復ヒント。 (`crates/sorafs_car/tests/sorafs_cli.rs`) を参照してください。 |
| SF6-SR-04 |低い |監査証跡 |リリース チェックリストは、セキュリティ レビューを行うためのチェックリストです。 | Добавлен раздел [リリースプロセス](../developer-releases.md)、ハッシュメモレビュー、URL тикета サインオフ、GA。 |

高/中レベルのパリティ ハーネスです。 Критических скрытых проблем не осталось.

## Валидация контролей

- **資格情報の範囲:** CI テンプレートの対象者と発行者。 CLI およびリリース ヘルパー завербкой、если `--identity-token-audience` не указан вместе с `--identity-token-provider`。  
- **確定的リプレイ:** Обновленные тесты покрывают положительные/отрицательные потоки отправки マニフェスト、гарантируя、что 不一致のダイジェストостаются недетерминированными олыибками и выявляются до обращения к сети.  
- **プルーフ ストリーミング バック プレッシャー:** Torii PoR/PoTR アイテムの確認、CLI のサンプル レイテンシ + テスト結果概要をご覧ください。  
- **可観測性:** 証明ストリーミング (`torii_sorafs_proof_stream_*`) および CLI 概要、中止、監査パンくずリスト。  
- **ドキュメント:** セキュリティ関連のワークフローとエスカレーション ワークフロー。

## Дополнения к リリースチェックリスト

リリース マネージャー **обязаны** の概要 GA кандидата:1. ハッシュ メモ セキュリティ レビュー (этот документ)。  
2. Ссылка на тикет 修復 (например、`governance/tickets/SF6-SR-2026.md`)。  
3. `scripts/release_sorafs_cli.sh --manifest ... --bundle-out ... --signature-out ...` с явными аргументами 対象者/発行者を出力します。  
4. パリティ ハーネス (`cargo test -p sorafs_car -- --nocapture sorafs_cli::proof_stream::bounded_channels`)。  
5. リリース ノート Torii テレメトリ カウンターの制限付きプルーフ ストリーミング。

GA のサインオフを完了します。

**参照ハッシュ артефактов (承認 2026-02-20):**

- `sf6_security_review.md` — `66001d0b53d8e7ed5951a07453121c075dea931ca44c11f1fcd1571ed827342a`

## フォローアップ

- **脅威モデルの更新:** CLI を使用して、脅威モデルを更新します。  
- **ファジング カバレッジ:** 証明ストリーミング ファズ `fuzz/proof_stream_transport`、ペイロード ID、gzip、deflate、zstd を保証します。  
- **インシデントのリハーサル:** Запланировать операторское упражнение, симулирующее компрометацию токена и ロールバック マニフェスト, чтобы документация отражала отработанные процедуры。

## 承認

- セキュリティ エンジニアリング ギルド: @sec-eng (2026-02-20)  
- ツーリング ワーキング グループ: @tooling-wg (2026-02-20)

アーティファクト バンドルをリリースするために承認を取得します。