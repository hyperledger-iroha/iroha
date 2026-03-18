---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/chunker-profile-authoring.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: creación de perfiles fragmentados
título: Руководство по авторингу профилей fragmenter SoraFS
sidebar_label: fragmento de motor
descripción: Чеклист для предложения новых профилей fragmenter SoraFS y accesorios.
---

:::nota Канонический источник
Esta página está escrita `docs/source/sorafs/chunker_profile_authoring.md`. Deje copias sincronizadas, ya que la estrella de Sphinx no se preocupará por sus especificaciones.
:::

# Руководство по авторингу профилей fragmenter SoraFS

Esto es importante para publicar y publicar nuevos perfiles de fragmentación para SoraFS.
Uno de los arquitectos más populares RFC (SF-1) y un registro automático (SF-2a)
конкретными требованиями к авторингу, шагами валидации и шаблонами предложений.
В качестве канонического примера см.
`docs/source/sorafs/proposals/sorafs_sf1_profile_v1.json`
и соответствующий registro de funcionamiento en seco лог в
`docs/source/sorafs/reports/sf1_determinism.md`.

## Objeto

Каждый профиль, попадающий в рееестр, должен:

- объявлять детерминированные параметры CDC and настройки multihash, одинаковые на всех
  arquitectos;
- поставлять воспроизводимые accesorios (JSON Rust/Go/TS + fuzz corpora + testigo PoR), которые
  Los SDK posteriores pueden proporcionar herramientas especializadas;
- включать метаданные, готовые для Governance (espacio de nombres, nombre, semver), а также рекомендации
  по миграции и окна совместимости; y
- Proходить детерминированную diff-suite до ревью совета.

Asegúrese de que no haya ningún problema con el funcionamiento del aparato.## Снимок чартеров реестра

Antes de que se realice una reserva previa de la carta,
который обеспечивает `sorafs_manifest::chunker_registry::ensure_charter_compliance()`:

- Perfil de identificación: положительные целые числа, монотонно возрастающие без пропусков.
- Mango canónico (`namespace.name@semver`) que se utiliza en alias y
- Ni odin alias не должен конфликтовать с другим каноническим handle и повторяться.
- Alias ​​должны быть непустыми и без пробелов по краям.

Полезные CLI помощники:

```bash
# JSON список всех зарегистрированных дескрипторов (ids, handles, aliases, multihash)
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles

# Эмитить метаданные для кандидата на профиль по умолчанию (канонический handle + aliases)
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
  --promote-profile=sorafs.sf1@1.0.0 --json-out=-
```

Estos comandos están relacionados con la programación de la carta y los datos canónicos.
метаданные для обсуждений la gobernanza.

## Требуемые метаданные| Polo | Descripción | Primer (`sorafs.sf1@1.0.0`) |
|------|----------|------------------------------|
| `namespace` | Логическая группировка связанных профилей. | `sorafs` |
| `name` | Читаемая человеком метка. | `sf1` |
| `semver` | Строка семантической версии для набора параметров. | `1.0.0` |
| `profile_id` | Identificador de pantalla monocromático, según el perfil actual. Guarde el ID de usuario, no modifique el número de identificación. | `1` |
| `profile.min_size` | Cantidad mínima de bytes. | `65536` |
| `profile.target_size` | Целевая длина чанка в bytes. | `262144` |
| `profile.max_size` | Número máximo de bytes. | `524288` |
| `profile.break_mask` | Máscara adaptativa para rodar hash (hexadecimal). | `0x0000ffff` |
| `profile.polynomial` | Константа полинома del engranaje (hex). | `0x3da3358b4dc173` |
| `gear_seed` | Seed для вычисления 64 KiB gear таблицы. | `sorafs-v1-gear` |
| `chunk_multihash.code` | Código multihash para digerir el código. | `0x1f` (BLAKE3-256) |
| `chunk_multihash.digest` | Resumen de accesorios de paquetes de канонического. | `13fa...c482` |
| `fixtures_root` | Catálogo completo de accesorios regenerativos. | `fixtures/sorafs_chunker/sorafs.sf1@1.0.0/` |
| `por_seed` | Semilla для детерминированной PoR выборки (`splitmix64`). | `0xfeedbeefcafebabe` (principal) |Los metadanos que aparecen en los documentos previos, las etiquetas y las fuentes registradas
accesorios, чтобы реестр, herramientas CLI y автоматизация gobernancia могли подтвердить значения без
ручных сверок. En este caso, seleccione el almacén de fragmentos y las CLI de manifiesto en `--json-out=-`,
чтобы стримить вычисленные метаданные в заметки ревью.

### Точки взаимодействия CLI y реестра

- `sorafs_manifest_chunk_store --profile=<handle>` — повторно запускает метаданные чанка,
  resumen manifiesto y PoR proporcionan parámetros predeterminados.
- `sorafs_manifest_chunk_store --json-out=-` — стримит отчет chunk-store en stdout para для
  автоматизированных сравнений.
- `sorafs_manifest_stub --chunker-profile=<handle>` — подтверждает, что manifests и CAR
  планы встраивают канонический identificador y alias.
- `sorafs_manifest_stub --plan=-` — подает предыдущие `chunk_fetch_specs` для проверки
  compensaciones/resúmenes после изменения.

Запишите вывод команд (resúmenes, raíces PoR, hashes de manifiesto) en предложении, чтобы ревьюеры могли
воспроизвести их буквально.

## Чеклист детерминизма и валидации1. **Regenerar accesorios**
   ```bash
   cargo run --locked -p sorafs_chunker --bin export_vectors \
     --signature-out=fixtures/sorafs_chunker/manifest_signatures.json
   ```
2. **Запустить suite паритета** — `cargo test -p sorafs_chunker` y arnés de diferenciación entre idiomas
   (`crates/sorafs_chunker/tests/vectors.rs`) должны быть зелеными с новыми luminarias.
3. **Переиграть fuzz/contrapresión corpus** — выполните `cargo fuzz list` y arnés de transmisión
   (`fuzz/sorafs_chunker`) sobre activos regulares.
4. **Проверить Testigos de prueba de recuperabilidad** — запустите
   `sorafs_manifest_chunk_store --por-sample=<n>` с предлагаемым профилем и подтвердите,
   что raíces совпадают с accesorio manifiesto.
5. **Funcionamiento en seco de CI** — выполните `ci/check_sorafs_fixtures.sh` локально; script должен
   пройти с новыми accesorios y существующим `manifest_signatures.json`.
6. **Poder de ejecución cruzada** — убедитесь, что Go/TS enlaces потребляют регенерированный
   JSON y archivos de textos y resúmenes idénticos.

Puede documentar comandos y resúmenes populares en los sitios anteriores, y puede que Tooling WG pueda exportarlos sin problemas.

### Подтверждение Manifiesto / PoR

После регенерации accesorios запустите полный manifest pipeline, чтобы убедиться, что
CAR метаданные и PoR pruebas остаются согласованными:

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

Замените входной файл любым представительным корпусом, используемым в ваших accesorios
(por ejemplo, детерминированным потоком 1 GiB), y приложите полученные digests к предложению.

## Шаблон предложенияPredaciones de Norito, notas `ChunkerProfileProposalV1` y ficciones en
`docs/source/sorafs/proposals/`. JSON шаблон ниже показывает ожидаемую форму
(подставьте свои значения по мере необходимости):


La versión anterior de Markdown (`determinism_report`), versión ficticia
команд, digiere чанков и любые отклонения, обнаруженные при валидации.

## Flujo de trabajo de gobernanza

1. **Отправить PR с предложением + accesorios.** Включите сгенерированные activos, Norito
   предложение и обновления `chunker_registry_data.rs`.
2. **Ревью Tooling WG.** Ревьюеры повторно запускают чеклист валидации и подтверждают,
   что предложение соответствует правилам реестра (без повторного использования id,
   детерминизм достигнут).
3. **Конверт совета.** После одобрения члены совета подписывают digest предложения
   (`blake3("sorafs-chunker-profile-v1" || canonical_bytes)`) и добавляют подписи
   в конверт профиля, хранящийся вместе с accesorios.
4. **Registro de archivos.** Combina archivos de archivos, documentos y accesorios. По умолчанию CLI
   остается на предыдущем профиле, пока gobernancia не объявит миграцию готовой.
5. **Отслеживание депрекации.** Después de que las migraciones eliminen el registro, отметив замененные

## Советы по авторингу- Prepárese para grandes cantidades de alimentos con fragmentación.
- No utilice códigos multihash sin coordinaciones con el manifiesto y la puerta de enlace de los usuarios; добавляйте
  заметку о совместимости.
- Haga clic en las tablas de engranajes de semillas, no globalmente únicas para la auditoría.
- Сохраняйте артефакты бенчмаркинга (por ejemplo, rendimiento rápido) en
  `docs/source/sorafs/reports/` para un niño.

Операционные ожидания во время implementación см. в libro de migración
(`docs/source/sorafs/migration_ledger.md`). Правила runtime соответствия см. в
`docs/source/sorafs/chunker_conformance.md`.