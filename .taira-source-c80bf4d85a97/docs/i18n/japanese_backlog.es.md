---
lang: es
direction: ltr
source: docs/i18n/japanese_backlog.md
status: complete
translator: manual
generator: scripts/sync_docs_i18n.py
source_hash: 569668c78c5fe322f602bfeb31944cde7c1f10c4e78cfae66b31cdfcca845885
source_last_modified: "2025-11-06T15:33:45.173464+00:00"
translation_last_reviewed: 2025-11-14
---

# Backlog de traducción de documentación al japonés

Este archivo rastrea la documentación japonesa que todavía está marcada con
`status: needs-translation`, agrupada por categoría para poder planificar los próximos
lotes. Los archivos que coinciden con `CHANGELOG.*`, `status.*` y `roadmap.*` se tratan
como temporales y se excluyen del backlog. Ejecute
`python3 scripts/sync_docs_i18n.py --dry-run` para refrescar la lista antes de iniciar
un nuevo lote.

## Visión general

- Archivos pendientes: 0 (actualizado 2025-10-26)
- Exclusiones de traducción: seguir omitiendo `CHANGELOG.*`, `status.*` y `roadmap.*`.
- No hay ningún backlog abierto actualmente. Cuando se añadan nuevos documentos en inglés,
  ejecute `scripts/sync_docs_i18n.py --dry-run` para detectar el delta y poner en cola el
  siguiente lote.

### Traducciones japonesas completadas recientemente

La lista siguiente enumera los archivos japoneses que ya se han traducido, para ayudar a
planificar futuros lotes y evitar trabajo duplicado.

## Sugerencias de lote

> Con cero elementos pendientes, conservamos el plan de lotes como material de referencia
> para la próxima oleada de documentos.

### Lote A – Documentación operativa ZK / Torii
- Objetivo: mantener actualizados en japonés los runbooks de adjunción y verificación ZK.
- Alcance: cualquier `docs/source/zk/*.ja.md` pendiente (tanto `lifecycle` como
  `prover_runbook` ya están completos).
- Entregable: cobertura completa de las operaciones ZK más un glosario añadido a los
  runbooks.

### Lote B – Diseño núcleo de IVM / Kotodama / Norito
- Objetivo: mejorar la descubribilidad para desarrolladores traduciendo la documentación
  de diseño del VM y del códec.
- Alcance: `docs/source/ivm_*.ja.md`, `docs/source/kotodama_*.ja.md`,
  `docs/source/norito_*.ja.md`.
- Entregable: resúmenes concisos e índices de palabras clave para cada documento.

### Lote C – Sumeragi / operaciones de red
- Objetivo: proporcionar runbooks operativos para consenso, pipeline y capa de red.
- Alcance: `docs/source/sumeragi*.ja.md`, `docs/source/governance_api.ja.md`,
  `docs/source/pipeline.ja.md`, `docs/source/p2p.ja.md`,
  `docs/source/state_tiering.ja.md`.
- Entregable: la primera edición en japonés del manual de operador.

### Lote D – Herramientas / ejemplos / referencias
- Objetivo: ofrecer a los desarrolladores material de referencia y ejemplos localizados.
- Alcance: los `docs/source/query_*.ja.md` pendientes (y cualquier referencia nueva).
- Entregable: garantizar que las referencias clave de CLI y API se mantengan alineadas con
  la fuente en inglés.

## Recordatorios de flujo de trabajo de traducción

1. Elija los archivos de destino y trabaje en una rama dedicada.
2. Tras completar la traducción, establezca `status: complete` en el front matter y añada
   `translator` si procede.
3. Ejecute `python3 scripts/sync_docs_i18n.py --dry-run` para verificar que no quedan
   stubs.
4. Alinee la terminología con la documentación japonesa existente, como `README.ja.md`.

## Notas

- Vigile el formato Markdown (sangrado, tablas, bloques de código) en lugar de ejecutar
  `cargo fmt --all`.
- Cuando cambie el documento fuente, actualice la edición japonesa en el mismo PR cuando
  sea posible.
- Siga las convenciones terminológicas compartidas en la documentación publicada; programe
  revisiones si el texto requiere consenso.
- Ejecute periódicamente `python3 scripts/sync_docs_i18n.py --dry-run` y actualice este
  backlog si aparecen nuevas diferencias.
