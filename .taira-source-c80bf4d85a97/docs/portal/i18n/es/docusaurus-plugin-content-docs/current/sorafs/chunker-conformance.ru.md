---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/chunker-conformance.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: conformidad con fragmentos
título: Руководство по соответствию fragmentador SoraFS
sidebar_label: fragmentador de contenido
descripción: Tres modos de trabajo y flujo de trabajo para el perfil de configuración del fragmentador SF1 en dispositivos y SDK.
---

:::nota Канонический источник
:::

Este documento tiene tres archivos adjuntos, cómo realizar una copia de seguridad, cuáles son
Utilice el fragmentador de perfil predeterminado SoraFS (SF1). Он также
документирует flujo de trabajo regeneraciones, políticas de seguimiento y verificación, чтобы
Los accesorios instalados en los SDK están sincronizados.

## Perfil canónico

- Semilla Входной (hex): `0000000000dec0ded`
- Memoria RAM: 262144 bytes (256 KiB)
- Memoria mínima: 65536 bytes (64 KiB)
- Tamaño máximo: 524288 bytes (512 KiB)
- Polinomio rodante: `0x3DA3358B4DC173`
- Engranaje de mesas de semillas: `sorafs-v1-gear`
- Máscara de rotura: `0x0000FFFF`

Versión estándar: `sorafs_chunker::chunk_bytes_with_digests_profile`.
Любое SIMD-ускорение должно выдавать идентичные границы и digests.

## Calendario de Набор

`cargo run --locked -p sorafs_chunker --bin export_vectors` regenerador
accesorios y archivos de archivos en `fixtures/sorafs_chunker/`:- `sf1_profile_v1.{json,rs,ts,go}` — канонические границы чанков для
  Los usuarios de Rust, TypeScript y Go. Каждый файл объявляет канонический mango как
  `sorafs.sf1@1.0.0`, como `sorafs.sf1@1.0.0`). Порядок фиксируется
  `ensure_charter_compliance` и НЕ ДОЛЖЕН изменяться.
- `manifest_blake3.json` — BLAKE3-верифицированный manifiesto, покрывающий каждый файл accesorios.
- `manifest_signatures.json` — подписи совета (Ed25519) поверх digest манифеста.
- `sf1_profile_v1_backpressure.json` y corpus en `fuzz/` —
  Escenarios de transmisión predeterminados, utilizados en pruebas de fragmentación de contrapresión.

### Política de privacidad

Регенерация accesorios **должна** включать валидную подпись совета. Generador
отклоняет неподписанный вывод, если явно не передан `--allow-unsigned` (предназначен
только для локальных экспериментов). Convertir sólo en anexos y
дедуплицируются по подписанту.

Чтобы добавить подпись совета:

```bash
cargo run --locked -p sorafs_chunker --bin export_vectors \
  --signing-key=<ed25519-private-key-hex> \
  --signature-out=fixtures/sorafs_chunker/manifest_signatures.json
```

## Verificación

CI helper `ci/check_sorafs_fixtures.sh` generador de energía eléctrica
`--locked`. Если accesorios расходятся или отсутствуют подписи, trabajo падает. Используйте
Esto se escribe en flujos de trabajo nocturnos y en accesorios seleccionados.

Шаги ручной проверки:

1. Introduzca `cargo test -p sorafs_chunker`.
2. Utilice `ci/check_sorafs_fixtures.sh` localmente.
3. Убедитесь, что `git status -- fixtures/sorafs_chunker` чист.

## Плейбук обновления

Por ejemplo, el nuevo perfil de fragmentación y las novedades SF1:См. Tema: [`docs/source/sorafs/chunker_profile_authoring.md`](./chunker-profile-authoring.md) para
требований к метаданным, шаблонов предложений и чеклистов валидации.

1. Introduzca `ChunkProfileUpgradeProposalV1` (como RFC SF-1) en nuevos parámetros.
2. Regenere los accesorios con `export_vectors` y actualice el nuevo resumen del manifiesto.
3. Подпишите манифест требуемым кворумом совета. Все подписи должны быть
   добавлены в `manifest_signatures.json`.
4. Desactive los dispositivos SDK (Rust/Go/TS) y elimine la partición en tiempo de ejecución.
5. Regenere los corpus fuzz según los parámetros de configuración.
6. Обновите это руководство новым manejar perfiles, semillas y digerir.
7. Mejore la configuración con pruebas y hojas de ruta actualizadas.

Изменения, которые затрагивают границы чанков или digests без соблюдения этого процесса,
недействительны и не должны быть смержены.