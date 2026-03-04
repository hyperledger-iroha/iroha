---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/chunker-conformance.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: conformidad con fragmentos
título: Guía de conformidad del fragmentador de SoraFS
sidebar_label: Conformidad de fragmentador
descripción: Requisitos y flujos para preservar el perfil determinístico de fragmentador SF1 en accesorios y SDK.
---

:::nota Fuente canónica
Esta página espelha `docs/source/sorafs/chunker_conformance.md`. Mantenha ambas como copias sincronizadas.
:::

Esta guía codificada de los requisitos que toda implementación debe seguir para permanecer
Comparativo con el perfil determinístico de fragmentador de SoraFS (SF1). Ele también
documenta o fluxo de regeneracao, a politica de assinaturas e os passos de verificacao para que
Los consumidores de dispositivos y SDK están sincronizados.

## Perfil canónico

- Mango del perfil: `sorafs.sf1@1.0.0`
- Semilla de entrada (hex): `0000000000dec0ded`
- Tamanho alvo: 262144 bytes (256 KiB)
- Tamaño mínimo: 65536 bytes (64 KiB)
- Tamaño máximo: 524288 bytes (512 KiB)
- Polinomio de rodadura: `0x3DA3358B4DC173`
- Semilla da tabla engranaje: `sorafs-v1-gear`
- Máscara de rotura: `0x0000FFFF`

Implementación de referencia: `sorafs_chunker::chunk_bytes_with_digests_profile`.
Cualquier aceleración SIMD debe producir límites y digestiones idénticos.

## Paquete de accesorios

`cargo run --locked -p sorafs_chunker --bin export_vectors` regenerar como
accesorios y emite los siguientes archivos en `fixtures/sorafs_chunker/`:- `sf1_profile_v1.{json,rs,ts,go}` - límites canónicos de fragmentos para consumidores
  Rust, TypeScript y Go. Cada archivo anuncia o handle canonico como a primeira
  entrada em `profile_aliases`, seguido de quaisquer alias alternativos (ej.,
  `sorafs.sf1@1.0.0`, depósito `sorafs.sf1@1.0.0`). A ordem e imposta por
  `ensure_charter_compliance` y NAO DEVE están alterados.
- `manifest_blake3.json` - manifiesto verificado por BLAKE3 cobrindo cada archivo de accesorios.
- `manifest_signatures.json` - assinaturas do conselho (Ed25519) sobre o digest do manifest.
- `sf1_profile_v1_backpressure.json` y cuerpos brutos dentro de `fuzz/` -
  Escenarios determinísticos de streaming usados por testes de back-pression do chunker.

### Política de asesinatos

La regeneración de accesorios **deve** incluye una assinatura válida del consejo. O gerador
rejeita dijo sem assinatura a menos que `--allow-unsigned` seja passado explícitamente (destinado
apenas para experimentación local). Los sobres de assinatura sao de solo anexar e
Sao deduplicados por signatario.

Para agregar una assinatura del consejo:

```bash
cargo run --locked -p sorafs_chunker --bin export_vectors \
  --signing-key=<ed25519-private-key-hex> \
  --signature-out=fixtures/sorafs_chunker/manifest_signatures.json
```

## Verificación

O helper de CI `ci/check_sorafs_fixtures.sh` reexecuta o gerador com
`--locked`. Si los accesorios divergirem o assinaturas faltarem, o el trabajo falla. uso
este script em flujos de trabajo noturnos y antes de enviar mudancas de accesorios.

Manual de pasos de verificación:

1. Ejecute `cargo test -p sorafs_chunker`.
2. Ejecute `ci/check_sorafs_fixtures.sh` localmente.
3. Confirme que `git status -- fixtures/sorafs_chunker` está limpio.## Libro de jugadas de actualización

Para proporcionar un nuevo perfil de fragmentador o actualizar SF1:

Veja también: [`docs/source/sorafs/chunker_profile_authoring.md`](./chunker-profile-authoring.md) para
Requisitos de metadados, plantillas de propuestas y listas de verificación de validación.

1. Redija um `ChunkProfileUpgradeProposalV1` (veja RFC SF-1) con nuevos parámetros.
2. Regenere los dispositivos a través de `export_vectors` y registre el nuevo resumen del manifiesto.
3. Assine o manifest com o quorum do conselho exigido. Todas las assinaturas deben ser
   anexadas a `manifest_signatures.json`.
4. Actualizar como accesorios de SDK actualizados (Rust/Go/TS) y garantizar la paridade cross-runtime.
5. Regenere corpora fuzz se os parametros mudarem.
6. Realice esta guía con el nuevo manejo de perfil, semillas y digestión.
7. Envie a mudanca junto com testes atualizados e atualizacoes do roadmap.

Mudancas que afetem limites de chunk ou digests sem seguir este proceso
sao invalidas e nao devem ser mergeadas.