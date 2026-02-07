---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/chunker-registry-charter.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: チャンカーレジストリチャーター
タイトル: Хартия реестра chunker SoraFS
Sidebar_label: チャンカーの説明
説明: Хартия управления для подачи и утверждения профилей chunker。
---

:::note Канонический источник
Эта страница отражает `docs/source/sorafs/chunker_registry_charter.md`.スフィンクスの正体は、スフィンクスだ。
:::

# チャンカー SoraFS

> **Ратифицировано:** 2025-10-29 ソラ議会インフラパネル (см.
> `docs/source/sorafs/council_minutes_2025-10-29.md`)。 Любые поправки требуют
> 統治。 команды внедрения должны считать этот документ
> нормативным、пока не будет утверждена новая хартия。

チャンカー SoraFS が見つかりました。
Она дополняет [Руководство по авторингу профилей chunker](./chunker-profile-authoring.md), описывая, как новые
профили предлагаются、рассматриваются、ратифицируются и в итоге выводятся из обращения。

## Область

Хартия применяется каждой записи в `sorafs_manifest::chunker_registry` и
клюбому ツール、который потребляет реестр (マニフェスト CLI、プロバイダー広告 CLI、
SDK)。 Она фиксирует инварианты エイリアスとハンドル、проверяемые
`chunker_registry::ensure_charter_compliance()`:

- ID профилей — положительные целые числа, монотонно возрастающие.
- Канонический ハンドル `namespace.name@semver` **должен** быть первой записью
- Строки エイリアス обрезаны、уникальны и не конфликтуют с каноническими ハンドル других записей。

## Роли

- **Автор(ы)** – 試合、試合、試合の試合
  доказательства детерминизма。
- **ツーリング ワーキング グループ (TWG)** – ツール ワーキング グループ (TWG)
  Їеклистам и убеждается, что инварианты реестра соблюдены.
- **ガバナンス評議会 (GC)** – рассматривает отчет TWG、подписывает конверт предложения
  и утверждает сроки публикации/депрекации.
- **ストレージ チーム** – チームのメンバーです。
  обновления документации。

## Жизненный цикл

1. **Подача предложения**
   - Автор запускает чеклист валидации из руководства по авторингу и создает
     JSON `ChunkerProfileProposalV1` ×
     `docs/source/sorafs/proposals/`。
   - CLI の説明:
     ```bash
     cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles
     cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
       --promote-profile=<handle> --json-out=-
     cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- \
       --chunker-profile=<handle> --json-out=-
     ```
   - Отправляет PR, содержащий 試合, предложение, отчет о детерминизме и
     обновления реестра。

2. **Ревью ツール (TWG)**
   - Повторяет чеклист валидации (フィクスチャ、ファズ、パイプライン マニフェスト/PoR)。
   - Запускает `cargo test -p sorafs_car --chunker-registry` と убеждается、что
     `ensure_charter_compliance()` は、 новой записью です。
   - Проверяет、что поведение CLI (`--list-profiles`、`--promote-profile`、ストリーミング)
     `--json-out=-`) отражает обновленные エイリアスとハンドル。
   - 合格/不合格を確認します。

3. **Одобрение совета (GC)**
   - Рассматривает отчет TWG とметаданные предложения.
   - ダイジェスト版 (`blake3("sorafs-chunker-profile-v1" || bytes)`)
     と、試合の試合、試合の試合。
   - ガバナンスを強化する必要があります。4. **Публикация**
   - 広報担当者:
     - `sorafs_manifest::chunker_registry_data`。
     - Документацию (`chunker_registry.md`、руководства по авторингу/соответствию)。
     - 備品と日付。
   - SDK のロールアウトを確認します。

5. **Депрекация / Закат**
   - Предложения、заменяющие существующий профиль、должны включать окно двойной публикации
     (грейс-периоды) アップグレード。
     移行台帳を確認します。

6. **Экстренные изменения**
   - ホットフィックスを更新してください。
   - TWG は、オンラインでの受信を可能にします。

## Ожидания от ツール

- `sorafs_manifest_chunk_store` および `sorafs_manifest_stub` の詳細:
  - `--list-profiles` が表示されます。
  - `--promote-profile=<handle>` для генерации канонического блока метаданных,
    жении профиля です。
  - `--json-out=-` の標準出力、標準出力の標準出力
    логи ревью。
- `ensure_charter_compliance()` вызывается при запуске релевантных бинарников
  (`manifest_chunk_store`、`provider_advert_stub`)。 CI тесты должны падать, если
  новые записи нарузают хартию。

## Документирование

- `docs/source/sorafs/reports/` を使用してください。
- Протоколы совета с резениями по chunker находятся в
  `docs/source/sorafs/migration_ledger.md`。
- `roadmap.md` と `status.md` は、ждого крупного изменения реестра です。

## Ссылки

- Руководство по авторингу: [Руководство по авторингу профилей chunker](./chunker-profile-authoring.md)
- Чеклист соответствия: `docs/source/sorafs/chunker_conformance.md`
- Справочник рестра: [Реестр профилей chunker](./chunker-registry.md)