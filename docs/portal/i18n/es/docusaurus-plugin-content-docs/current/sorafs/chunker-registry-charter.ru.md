---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/chunker-registry-charter.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: carta-registro-fragmento
título: Хартия реестра fragmentador SoraFS
sidebar_label: fragmentador del restaurante Hartia
descripción: Хартия управления для подачи и утверждения профилей fragmenter.
---

:::nota Канонический источник
Esta página está escrita `docs/source/sorafs/chunker_registry_charter.md`. Deje copias sincronizadas, ya que la estrella de Sphinx no se preocupará por sus especificaciones.
:::

# Хартия управления реестром fragmentador SoraFS

> **Ратифицировано:** 2025-10-29 Panel de Infraestructura del Parlamento de Sora (см.
> `docs/source/sorafs/council_minutes_2025-10-29.md`). Любые поправки требуют
> формального голосования по gobernancia; команды внедрения должны считать этот документ
> нормативным, пока не будет утверждена новая хартия.

Esta parte está determinada por procesos y funciones de evolución del fragmentador SoraFS.
Она дополняет [Руководство по авторингу профилей fragmenter](./chunker-profile-authoring.md), описывая, как новые
профили предлагаются, рассматриваются, ратифицируются и в итоге выводятся из обращения.

## Область

Hartia presenta una descripción detallada en `sorafs_manifest::chunker_registry` y
к любому herramientas, который потребляет реестр (CLI de manifiesto, CLI de anuncio de proveedor,
SDK). Она фиксирует инварианты alias и handle, проверяемые
`chunker_registry::ensure_charter_compliance()`:

- Perfil de identificación: положительные целые числа, монотонно возрастающие.
- Mango canónico `namespace.name@semver` **должен** быть первой записью
- Строки alias обрезаны, уникальны и не конфликтуют с каноническими handle других записей.## Роли

- **Автор(ы)** – готовят предложение, regenerar accesorios y собирают
  доказательства детерминизма.
- **Grupo de Trabajo sobre Herramientas (TWG)** – валидирует предложение по опубликованным
  чеклистам и убеждается, что инварианты рееестра соблюдены.
- **Consejo de Gobernanza (GC)** – рассматривает отчет TWG, подписывает конверт предложения
  и утверждает сроки публикации/депрекации.
- **Equipo de almacenamiento** – puede realizar registros y publicaciones
  обновления документации.

## Жизненный цикл

1. **Подача предложения**
   - Автор запускает чеклист валидации из руководства по авторингу и создает
     JSON `ChunkerProfileProposalV1`
     `docs/source/sorafs/proposals/`.
   - Включает вывод CLI из:
     ```bash
     cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles
     cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
       --promote-profile=<handle> --json-out=-
     cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- \
       --chunker-profile=<handle> --json-out=-
     ```
   - Отправляет PR, содержащий accesorios, предложение, отчет о детерминизме и
     обновления реестра.

2. **Útil de herramientas (TWG)**
   - Повторяет чеклист валидации (accesorios, fuzz, manifiesto de tubería/PoR).
   - Запускает `cargo test -p sorafs_car --chunker-registry` и убеждается, что
     `ensure_charter_compliance()` Proходит с новой записью.
   - Proveedor de datos CLI (`--list-profiles`, `--promote-profile`, streaming
     `--json-out=-`) отражает обновленные alias и handle.
   - Готовит краткий отчет с выводами and статусом pasa/falla.3. **Одобрение совета (GC)**
   - Рассматривает отчет TWG and метаданные предложения.
   - Подписывает digest предложения (`blake3("sorafs-chunker-profile-v1" || bytes)`)
     Y добавляет подписи в конверт совета, который хранится рядом с accesorios.
   - Фиксирует результат голосования в gobernabilidad protocolo.

4. **Публикация**
   - Мержит PR, обновляя:
     -`sorafs_manifest::chunker_registry_data`.
     - Documentación (`chunker_registry.md`, руководства по авторингу/соответствию).
     - Accesorios y accesorios de determinación.
   - SDK mejorado de operadores y comandos para un nuevo perfil y plan de implementación.

5. **Депрекация / Закат**
   - Antes de usar el perfil de usuario, consulte las siguientes publicaciones
     (períodos antiguos) y actualización del plan.
     в реестре и обновить el libro mayor de migración.

6. **Imagen exclusiva**
   - Удаление или hotfix требуют голосования совета с большинством.
   - TWG para documentar información sobre riesgos y notificar incidentes diarios.

## Ожидания от herramientas

- `sorafs_manifest_chunk_store` e `sorafs_manifest_stub` previos:
  - `--list-profiles` para la inspección del registro.
  - `--promote-profile=<handle>` para generaciones de bloques de metadanos,
    используемого при продвижении профиля.
  - `--json-out=-` para conectar la salida estándar a la salida estándar, según lo previsto
    логи ревью.
- `ensure_charter_compliance()` вызывается при запуске релевантных бинарников
  (`manifest_chunk_store`, `provider_advert_stub`). CI тесты должны падать, если
  новые записи нарушают хартию.## Documentación

- Haga clic en el botón de configuración en `docs/source/sorafs/reports/`.
- Протоколы совета с решениями по chunker находятся в
  `docs/source/sorafs/migration_ledger.md`.
- Desconecte `roadmap.md` e `status.md` después del registro de grupo.

## Ссылки

- Руководство по авторингу: [Руководство по авторингу профилей fragmenter](./chunker-profile-authoring.md)
- Чеклист соответствия: `docs/source/sorafs/chunker_conformance.md`
- Справочник реестра: [Реестр профилей fragmentador](./chunker-registry.md)