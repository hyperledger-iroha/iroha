---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/chunker-profile-authoring.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: チャンカープロファイルオーサリング
タイトル: Руководство по авторингу профилей chunker SoraFS
サイドバーラベル: Авторинг チャンカー
説明: Чеклист для предложения новых профилей chunker SoraFS およびフィクスチャ。
---

:::note Канонический источник
Эта страница отражает `docs/source/sorafs/chunker_profile_authoring.md`.スフィンクスの正体は、スフィンクスだ。
:::

# Руководство по авторингу профилей チャンカー SoraFS

Это руководство объясняет、какпредлагать и публиковать новые профили chunker для SoraFS.
Оно дополняет архитектурный RFC (SF-1) および справочник реестра (SF-2a)
конкретными требованиями к авторингу, вагами валидации и заблонами предложений.
В качестве канонического примера см。
`docs/source/sorafs/proposals/sorafs_sf1_profile_v1.json`
または、予行演習を行ってください。
`docs/source/sorafs/reports/sf1_determinism.md`。

## Обзор

Каждый профиль, попадающий в реестр, должен:

- объявлять детерминированные параметры CDC и настройки マルチハッシュ、одинаковые на всех
  архитектурах;
- フィクスチャ (JSON Rust/Go/TS + ファズ コーパス + PoR 監視) の機能
  ダウンストリーム SDK はツールを提供します。
- メソッド、ガバナンス (名前空間、名前、セムバー)、および также рекомендации
  по миграции и окна совместимости;や
- 差分スイートを確認してください。

Следуйте чеклисту ниже, чтобы подготовить предложение, удовлетворяющее этим правилам.

## Снимок чартеров рестра

Перед подготовкой предложения убедитесь, что оно соответствует чартеру реестра,
который обеспечивает `sorafs_manifest::chunker_registry::ensure_charter_compliance()`:

- ID профилей — положительные целые числа, монотонно возрастающие без пропусков.
- Канонический ハンドル (`namespace.name@semver`) の別名と名前
- Ни один エイリアス не должен конфликтовать с другим каноническим ハンドル и повторяться。
- 別名 должны быть непустыми и без пробелов по краям。

CLI の例:

```bash
# JSON список всех зарегистрированных дескрипторов (ids, handles, aliases, multihash)
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles

# Эмитить метаданные для кандидата на профиль по умолчанию (канонический handle + aliases)
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
  --promote-profile=sorafs.sf1@1.0.0 --json-out=-
```

Эти команды держат предложения согласованными с чартером реестра и дают канонические
政府の統治。

## Требуемые метаданные

| Поле | Описание | Пример (`sorafs.sf1@1.0.0`) |
|------|----------|----------------------------|
| `namespace` | Логическая группировка связанных профилей。 | `sorafs` |
| `name` | Читаемая человеком метка。 | `sf1` |
| `semver` | Строка семантической версии для набора параметров. | `1.0.0` |
| `profile_id` | Монотонный числовой идентификатор、назначаемый после попадания профиля。 Резервируйте следующий ID, но не переиспользуйте существующие номера. | `1` |
| `profile.min_size` |バイト数を超えます。 | `65536` |
| `profile.target_size` | Целевая длина чанка в バイト。 | `262144` |
| `profile.max_size` |バイト数を超えます。 | `524288` |
| `profile.break_mask` |ローリング ハッシュ (16 進数)。 | `0x0000ffff` |
| `profile.polynomial` | Константа ギア полинома (16 進数)。 | `0x3da3358b4dc173` |
| `gear_seed` |シード для вычисления 64 KiB gear таблицы. | `sorafs-v1-gear` |
| `chunk_multihash.code` |マルチハッシュのダイジェスト。 | `0x1f` (ブレイク3-256) |
| `chunk_multihash.digest` |ダイジェスト канонического バンドル フィクスチャ。 | `13fa...c482` |
| `fixtures_root` | Относительный каталог с регенерированными 備品。 | `fixtures/sorafs_chunker/sorafs.sf1@1.0.0/` |
| `por_seed` |シードは、PoR выборки (`splitmix64`) です。 | `0xfeedbeefcafebabe` (直訳) |

Метаданные должны присутствовать как в документе предложения, так и внутри сгенерированных
フィクスチャ、インターフェイス、CLI ツール、ガバナンスなどの機能を備えています。
ручных сверок。チャンクストアとマニフェスト CLI の `--json-out=-`、
Їтобы стримить вычисленные метаданные в заметки ревью.

### Точки взаимодействия CLI と реестра

- `sorafs_manifest_chunk_store --profile=<handle>` — ログインしてください。
  マニフェスト ダイジェストと PoR の詳細。
- `sorafs_manifest_chunk_store --json-out=-` — チャンクストアと標準出力
  автоматизированных сравнений。
- `sorafs_manifest_stub --chunker-profile=<handle>` — マニフェストと CAR
  ハンドルとエイリアスを指定します。
- `sorafs_manifest_stub --plan=-` — テスト `chunk_fetch_specs` テスト
  オフセット/ダイジェストも表示されます。

説明 (ダイジェスト、PoR ルート、マニフェスト ハッシュ) の説明、説明、説明
問題はありません。

## Чеклист детерминизма и валидации1. **試合の試合**
   ```bash
   cargo run --locked -p sorafs_chunker --bin export_vectors \
     --signature-out=fixtures/sorafs_chunker/manifest_signatures.json
   ```
2. **スイートの機能** — `cargo test -p sorafs_chunker` およびクロスランゲージ差分ハーネス
   (`crates/sorafs_chunker/tests/vectors.rs`) フィクスチャが追加されました。
3. **ファズ/バックプレッシャーコーパス** — `cargo fuzz list` およびストリーミング ハーネス
   (`fuzz/sorafs_chunker`) 資産。
4. **取得可能性証明証人を要求** — запустите
   `sorafs_manifest_chunk_store --por-sample=<n>` 最低、
   ルートはフィクスチャマニフェストです。
5. **CI 予行演習** — выполните `ci/check_sorafs_fixtures.sh` локально;必要な情報を提供します
   `manifest_signatures.json` のフィクスチャと существующим です。
6. **クロスランタイムの機能** — убедитесь、Go/TS バインディングの機能
   JSON とダイジェスト。

ツール WG がダイジェストを表示し、その内容を確認します。

### マニフェスト/PoR

フィクスチャのマニフェスト パイプライン、чтобы убедиться、что
CAR メソッドと PoR 証明の詳細:

```bash
# Проверить метаданные чанка + PoR с новым профилем
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
  --profile=sorafs.sf2@1.0.0 \
  --json-out=- --por-json-out=- fixtures/sorafs_chunker/input.bin

# Сгенерировать manifest + CAR и сохранить chunk fetch specs
cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- \
  fixtures/sorafs_chunker/input.bin \
  --chunker-profile=sorafs.sf2@1.0.0 \
  --chunk-fetch-plan-out=chunk_plan.json \
  --manifest-out=sf2.manifest \
  --car-out=sf2.car \
  --json-out=sf2.report.json

# Повторно запустить с сохраненным планом fetch (защищает от устаревших offsets)
cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- \
  fixtures/sorafs_chunker/input.bin \
  --chunker-profile=sorafs.sf2@1.0.0 \
  --plan=chunk_plan.json --json-out=-
```

試合の試合結果、試合結果、試合結果などをお知らせします。
(например、детерминированным потоком 1 GiB)、и приложите полученные ダイジェスト к предложению。

## Шаблон предложения

Предложения подаются как Norito записи `ChunkerProfileProposalV1` и фиксируются в
`docs/source/sorafs/proposals/`。 JSON は、 показывает ожидаемую форму を実行します。
(次のことを確認してください):


Предоставьте соответствующий Markdown отчет (`determinism_report`)、фиксирующий вывод
команд、ダイジェスト чанков и любые отклонения、обнаруженные при валидации。

## ガバナンスのワークフロー

1. **PR を取得するには + フィクスチャ。** Включите сгенерированные アセット、Norito
   `chunker_registry_data.rs` です。
2. **ツール WG.** ツール WG を作成し、
   что предложение соответствует правилам реестра (без повторного использования id,
   детерминизм достигнут）。
3. **Конверт совета.** После одобрения члены совета подписывают ダイジェスト предложения
   (`blake3("sorafs-chunker-profile-v1" || canonical_bytes)`) と добавляют подписи
   試合の試合、試合の試合。
4. **Публикация реестра.** ドキュメント、フィクスチャをマージします。 CLI を使用する
   政府は統治を維持し、統治を維持します。
5. **Отслеживание депрекации.** После окна миграции обновите реестр, отметив замененные

## Советы по авторингу

- チャンク処理を実行します。
- マルチハッシュ クラスのマニフェストとゲートウェイの組み合わせ。ビデオ
  заметку о совместимости.
- 種子のギアを確認してください。
- Сохраняйте артефакты бенчмаркинга (スループット、スループット) в
  `docs/source/sorafs/reports/` は、 будущих ссылок です。

ロールアウトを開始します。 × 移住台帳
(`docs/source/sorafs/migration_ledger.md`)。ランタイムを確認してください。 ×
`docs/source/sorafs/chunker_conformance.md`。