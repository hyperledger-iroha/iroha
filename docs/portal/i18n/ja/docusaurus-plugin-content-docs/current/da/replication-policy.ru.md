---
lang: ja
direction: ltr
source: docs/portal/docs/da/replication-policy.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::note Канонический источник
Эта страница отражает `docs/source/da/replication_policy.md`. жержите обе версии
:::

# データの可用性 (DA-4)

_Статус: Вработе — Владельцы: コア プロトコル WG / ストレージ チーム / SRE_

DA の取り込みパイプラインの取得と保持の維持
класса blob、описанного в `roadmap.md` (ワークストリーム DA-4)。 Torii отказывается
保存エンベロープ、発信者、メッセージの保存
настроенной политикой, гарантируя, что каждый валидатор/узел хранения удерживает
необходимое число эпох и реплик без опоры на намерения отправителя.

## Политика по умолчанию

| Класс ブロブ |保温性 |保冷力 | Требуемые реплики | Класс хранения |ガバナンスタグ |
|-----------|---------------|----------------|---------------------|----------------|--------------|
| `taikai_segment` | 24時間 | 14日 | 5 | `hot` | `da.taikai.live` |
| `nexus_lane_sidecar` | 6時間 | 7日間 | 4 | `warm` | `da.sidecar` |
| `governance_artifact` | 12時間 | 180日 | 3 | `cold` | `da.governance` |
| _デフォルト (デフォルト)_ | 6時間 | 30日 | 3 | `warm` | `da.default` |

Эти значения встроены в `torii.da_ingest.replication_policy` и применяются ко
`/v2/da/ingest` です。 Torii マニフェストの確認
保持期間と発信者数を確認する
SDK を使用すると、SDK が更新されます。

### Классы доступности Taikai

Taikai ルーティング マニフェスト (`taikai.trm`) объявляют `availability_class`
(`hot`、`warm`、`cold`)。 Torii チャンク処理で、
чтобы операторы могли масbolо реплик по stream без редактирования
глобальной таблицы。デフォルト:

| Класс доступности |保温性 |保冷力 | Требуемые реплики | Класс хранения |ガバナンスタグ |
|-------------------|-----------------|----------------|-------------------|----------------|----------------|
| `hot` | 24時間 | 14日 | 5 | `hot` | `da.taikai.live` |
| `warm` | 6時間 | 30日 | 4 | `warm` | `da.taikai.warm` |
| `cold` | 1時間 | 180日 | 3 | `cold` | `da.taikai.archive` |

Если подсказок нет, используется `hot`, чтобы 生放送 удерживали самый
必要があります。デフォルト設定です
`torii.da_ingest.replication_policy.taikai_availability`、最高です
другие цели。

## Конфигурация

Политика находится под `torii.da_ingest.replication_policy` と предоставляет
*デフォルト* ваблон плюс массив は、для каждого класса をオーバーライドします。 Идентификаторы классов
регистронезависимы и принимают `taikai_segment`、`nexus_lane_sidecar`、
`governance_artifact`、`custom:<u16>` は、 одобренных управлением です。
`hot`、`warm`、`cold` を参照してください。

```toml
[torii.da_ingest.replication_policy.default_retention]
hot_retention_secs = 21600          # 6 h
cold_retention_secs = 2592000       # 30 d
required_replicas = 3
storage_class = "warm"
governance_tag = "da.default"

[[torii.da_ingest.replication_policy.overrides]]
class = "taikai_segment"
[torii.da_ingest.replication_policy.overrides.retention]
hot_retention_secs = 86400          # 24 h
cold_retention_secs = 1209600       # 14 d
required_replicas = 5
storage_class = "hot"
governance_tag = "da.taikai.live"
```

デフォルトでは、デフォルトが表示されます。 Чтобы ужесточить
класс、обновите соответствующий オーバーライド。 Їтобы изменить базу для новых классов,
отредактируйте `default_retention`。Taikai の利用可能クラス можно переопределять отдельно через
`torii.da_ingest.replication_policy.taikai_availability`:

```toml
[[torii.da_ingest.replication_policy.taikai_availability]]
availability_class = "cold"
[torii.da_ingest.replication_policy.taikai_availability.retention]
hot_retention_secs = 3600          # 1 h
cold_retention_secs = 15552000     # 180 d
required_replicas = 3
storage_class = "cold"
governance_tag = "da.taikai.archive"
```

## Семантика の施行

- Torii заменяет пользовательский `RetentionPolicy` на принудительный профиль
  チャンク化とマニフェスト。
- マニフェストのマニフェスト、保持期間の確認、
  отклоняются с `400 schema mismatch`, чтобы устаревлие клиенты не могли ослабить
  контракт。
- オーバーライド логируется (`blob_class`、отправленная политика vs
  ожидаемая)、非準拠の呼び出し元をロールアウトします。

[データ可用性取り込み計画](ingest-plan.md) (検証チェックリスト) の日付
обновленного ゲート、покрывающего 執行保持。

## ワークフロー (フォローアップ DA-4)

執行保持 — лизь первый заг. Операторы также должны доказать, что ライブ
マニフェストとレプリケーションの順序 остаются согласованными с настроенной политикой,
SoraFS ブロブを再レプリケートします。

1. **ドリフトと同じです。** Torii です。
   `overriding DA retention policy to match configured network baseline` ビデオ
   発信者は保持期間を延長します。 Сопоставляйте этот лог с
   телеметрией `torii_sorafs_replication_*`, 不足分 реплик
   再配置。
2. **インテントとライブ レプリカを使用します。** 監査ヘルパーの機能:

   ```bash
   cargo xtask da-replication-audit \
     --config configs/iroha/torii.toml \
     --manifest spool/da/manifests/*.json \
     --replication-order artifacts/da/orders/*.norito \
     --json-out artifacts/da/replication_audit.json
   ```

   Команда загружает `torii.da_ingest.replication_policy` из конфигурации,
   декодирует каждый マニフェスト (JSON または Norito)、и опционально сопоставляет
   ペイロード `ReplicationOrderV1` はダイジェスト マニフェストです。必要な日付:

   - `policy_mismatch` - 保持マニフェストの保持
     профилем (такого не должно быть, если Torii настроен корректно)。
   - `replica_shortfall` - ライブ レプリケーションの順序、чем
     `RetentionPolicy.required_replicas`, или выдает меньзе назначений, чем цель.

   Ненулевой код выхода означает активный 不足、чтобы CI/オンコール автоматизация
   могла немедленно пейджить。 Приложите JSON отчет к пакету
   `docs/examples/da_manifest_review_template.md` 日です。
3. **再レプリケーションを実行します。** 不足がある場合は再レプリケーションを実行します。
   новый `ReplicationOrderV1` через инструменты управления, описанные в
   [SoraFS ストレージ容量マーケットプレイス](../sorafs/storage-capacity-marketplace.md)、
   и повторяйте аудит, пока набор реплик не сойдется.緊急オーバーライド
   CLI を使用して `iroha app da prove-availability`、SRE を実行します。
   ダイジェストと PDP 証拠。

回帰カバレッジ - `integration_tests/tests/da/replication_policy.rs`;
スイート отправляет несовпадающую политику 保持期間 в `/v2/da/ingest` および проверяет,
マニフェストのマニフェスト、インテントの呼び出し元。