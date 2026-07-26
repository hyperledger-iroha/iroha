---
lang: ja
direction: ltr
source: docs/portal/docs/nexus/operations.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: ネクサスオペレーション
タイトル: ランブック по операциям Nexus
説明: Практичный полевой обзор рабочего процесса оператора Nexus、отражающий `docs/source/nexus_operations.md`。
---

Используйте эту страницу как быстрый справочник к `docs/source/nexus_operations.md`. Она концентрирует операционный чек-лист, точки управления изменениями и требования к телеметрии, которым должны следовать операторы Nexus。

## Чек-лист жизненного цикла

| Этап | Действия | Доказательства |
|----------|----------|----------|
| Предстарт | Проверить хэли/подписи релиза, подтвердить `profile = "iroha3"` и подготовить заблоны конфигурации. | `scripts/select_release_profile.py`、チェックサム、バンドル メッセージ。 |
| Выравнивание каталога | Обновить каталог `[nexus]`, политику марловрутизации и пороги DA по манифесту совета, затем сохранить `--trace-config`。 | `irohad --sora --config ... --trace-config`、オンボーディングを完了しました。 |
|スモークとカットオーバー | `irohad --sora --config ... --trace-config`、CLI スモーク (`FindNetworkStatus`)、入場料を支払います。 |スモークテスト + アラートマネージャー。 |
| Штатный режим |ダッシュボード/アラート、ガバナンス、構成/ランブックなどの機能を備えています。 | Протоколы квартального обзора、скринсоты дазбордов、ID тикетов ротации。 |

Подробный オンボーディング (замена ключей、заблоны марлофиля релиза) остается в `docs/source/sora_nexus_operator_onboarding.md`。

## Управление изменениями

1. **Обновления релиза** - Объявлениями в `status.md`/`roadmap.md`;オンボーディングの PR 機能を確認してください。
2. **レーン** - バンドルとスペース ディレクトリ、`docs/source/project_tracker/nexus_config_deltas/` の両方。
3. ** レーン/データスペースの** - каждое изменение `config/config.toml` требует тикет сссылкой на / data-space. Сохраняйте редактированную копию эффективной конфигурации при に参加/アップグレードしてください。
4. **ロールバック** - 停止/復元/スモーク; `docs/source/project_tracker/nexus_config_deltas/<date>-rollback.md` を参照してください。
5. **コンプライアンス** - プライベート/CBDC レーンでコンプライアンスを確認し、DA ノブを設定します。 (最低 `docs/source/cbdc_lane_playbook.md`)。

## Телеметрия と SLO

- 説明: `dashboards/grafana/nexus_lanes.json`、`nexus_settlement.json`、SDK の詳細 (например、`android_operator_console.json`)。
- Алерты: `dashboards/alerts/nexus_audit_rules.yml` および Torii/Norito トランスポート (`dashboards/alerts/torii_norito_rpc_rules.yml`)。
- Метрики для мониторинга:
  - `nexus_lane_height{lane_id}` - ログインしてください。
  - `nexus_da_backlog_chunks{lane_id}` - レーン (公共 64 / 民間 8)。
  - `nexus_settlement_latency_seconds{lane_id}` - 900 ミリ秒 (パブリック) または 1200 ミリ秒 (プライベート)。
  - `torii_request_failures_total{scheme="norito_rpc"}` - 5 分以内に 2% 以上。
  - `telemetry_redaction_override_total` - セクション 2 немедленно;つまり、что はコンプライアンスをオーバーライドします。
- Выполняйте чек-лист телеметрии из [Nexus テレメトリ修復計画](./nexus-telemetry-remediation) минимум ежеквартально и прикладывайте заполненную форму к заметкам операционного обзора.

## Матрица инцидентов

|重大度 | Определение | Ответ |
|----------|-----------|----------|
|セクション 1 |データスペース、決済 >15 分以内のガバナンスを実現します。 | Пейджинг Nexus プライマリ + リリース エンジニアリング + コンプライアンス、入場許可、資格認定、資格認定 <=60 分、RCA <=5 рабочих日々。 |
|セクション 2 | SLA バックログ レーンを確認し、30 分を超えるまでにロールアウトします。 | Nexus プライマリ + SRE、4 日以内、2 日以内にフォローアップします。 |
|セクション 3 | Не блокирующий дрейф (ドキュメント、アラート)。 |トラッカーとサービスを利用できます。 |

ID レーン/データスペース、манифестов、таймлайн、поддерживающие метрики/логи、フォローアップ задачи/ответственных。

## Архив доказательств

- バンドル/マニフェスト/パッケージを `artifacts/nexus/<lane>/<date>/` で確認します。
- configs + вывод `--trace-config` для каждого релиза を設定します。
- Прикладывайте протоколы совета + подписанные резмения при внесении изменений конфигурации или манифеста.
- スナップショット Prometheus と Nexus を 12 分に取得します。
- `docs/source/project_tracker/nexus_config_deltas/README.md` の Runbook を参照してください。

## Связанные материалы- 説明: [Nexus 概要](./nexus-overview)
- Спецификация: [Nexus 仕様](./nexus-spec)
- レーンの例: [Nexus レーン モデル](./nexus-lane-model)
- ルーティング シムの説明: [Nexus 移行ノート](./nexus-transition-notes)
- オンボーディング: [Sora Nexus オペレーター オンボーディング](./nexus-operator-onboarding)
- メッセージ: [Nexus テレメトリ修復計画](./nexus-telemetry-remediation)