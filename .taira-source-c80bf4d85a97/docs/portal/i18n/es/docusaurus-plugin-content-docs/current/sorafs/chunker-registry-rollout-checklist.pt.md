---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/chunker-registry-rollout-checklist.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: lista de verificación de implementación de registro fragmentado
título: Lista de verificación de implementación del registro de fragmentador de SoraFS
sidebar_label: Lista de verificación de implementación de fragmentador
descripción: Plano de implementación paso a paso para actualizar el registro de fragmentador.
---

:::nota Fuente canónica
Reflejo `docs/source/sorafs/chunker_registry_rollout_checklist.md`. Mantenha ambas como copias sincronizadas.
:::

# Lista de verificación de implementación del registro de SoraFS

Esta lista de verificación captura los pasos necesarios para promover un nuevo perfil de chunker
ou paquete de admisión de proveedor da revisióno para producción después de que o charter
de gobernanca para ratificado.

> **Escopo:** Aplica-se a todas las versiones que modificam
> `sorafs_manifest::chunker_registry`, sobres de admisión de proveedores, o paquetes
> de accesorios canónicos (`fixtures/sorafs_chunker/*`).

## 1. Prevuelo de Validacao

1. Regenere los partidos y verifique el determinismo:
   ```bash
   cargo run --locked -p sorafs_chunker --bin export_vectors
   cargo test -p sorafs_chunker --offline vectors
   ci/check_sorafs_fixtures.sh
   ```
2. Confirme que os hashes de determinismo em
   `docs/source/sorafs/reports/sf1_determinism.md` (ou o relatorio de perfil
   relevante) batem com os artefatos regenerados.
3. Garantía que `sorafs_manifest::chunker_registry` compila con
   `ensure_charter_compliance()` ejecutando:
   ```bash
   cargo test -p sorafs_manifest --lib chunker_registry::tests::ensure_charter_compliance
   ```
4. Actualizar el expediente de propuesta:
   - `docs/source/sorafs/proposals/<profile>.json`
   - Entrada de atas do consejo en `docs/source/sorafs/council_minutes_*.md`
   - Relatorio de determinismo

## 2. Aprobación de gobernanza1. Presentar el informe del Grupo de Trabajo sobre Herramientas y el resumen de la propuesta
   Panel de Infraestructura del Parlamento de Sora.
2. Registre detalles de aprobación em
   `docs/source/sorafs/council_minutes_YYYY-MM-DD.md`.
3. Publico o sobre assinado pelo Parlamento junto con los partidos:
   `fixtures/sorafs_chunker/manifest_signatures.json`.
4. Verifique que el sobre esté disponible mediante el ayudante de gobernanza para buscar:
   ```bash
   cargo xtask sorafs-fetch-fixture \
     --signatures <url-or-path-to-manifest_signatures.json> \
     --out fixtures/sorafs_chunker
   ```

## 3. Implementar la puesta en escena

Consulte el [playbook de manifest em staging](./staging-manifest-playbook) para um
paso a paso detallado.

1. Implante Torii com descubrimiento `torii.sorafs` habilitado e cumplimiento de admisión
   ligado (`enforce_admission = true`).
2. Envie os sobres de admisión de proveedores aprobados para o diretorio de registro
   de puesta en escena referenciado por `torii.sorafs.discovery.admission.envelopes_dir`.
3. Verifique que los anuncios del proveedor se propaguen a través de una API de descubrimiento:
   ```bash
   curl -sS http://<torii-host>/v1/sorafs/providers | jq .
   ```
4. Ejercite los puntos finales de manifiesto/plan con encabezados de gobierno:
   ```bash
   sorafs-fetch --plan fixtures/chunk_fetch_specs.json \
     --gateway-provider "...staging config..." \
     --gateway-manifest-id <manifest-hex> \
     --gateway-chunker-handle sorafs.sf1@1.0.0
   ```
5. Confirme que paneles de telemetría (`torii_sorafs_*`) y registros de alerta
   Reportam o novo perfil sin errores.

## 4. Lanzamiento y producción1. Repita los pasos de staging nos nodes Torii de producao.
2. Anuncie a janela de ativacao (data/hora, período de gracia, plano de reversión) nos
   canales de operadores y SDK.
3. Fusionar el PR de lanzamiento del contendo:
   - Accesorios y envolvente atualizados
   - Mudancas na documentacao (referencias ao charter, relatorio de determinismo)
   - Actualizar la hoja de ruta/estado
4. Tagueie a release e archiva os artefatos assinados para provenance.

## 5. Pos-implementación de Auditoria

1. Capture métricas finales (recuentos de descubrimiento, taxones de éxito de recuperación, histogramas
   de error) 24h apos o rollout.
2. Atualize `status.md` com um resumo curto e link para o relatorio de determinismo.
3. Registre tarefas de acompanhamento (ej., orientación adicional para autoría
   de perfis) en `roadmap.md`.