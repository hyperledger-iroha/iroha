---
lang: es
direction: ltr
source: docs/source/sorafs/signing_ceremony.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8b950fba8f334478f58ba3176a10640b656610d59cc86ba44de535c0128b871e
source_last_modified: "2025-11-02T04:40:40.197754+00:00"
translation_last_reviewed: "2026-01-30"
---

# Aprobacion de fixtures por el Sora Parliament (SF-1b)

La "ceremonia de firmado del council" manual fue retirada. Los fixtures de
chunker SoraFS ahora se ratifican exclusivamente a traves del **Sora
Parliament**, el DAO multi-cuerpo basado en sortition que gobierna la red Nexus.
Los miembros del Parliament bloquean XOR para obtener estatus de ciudadano Sora,
se asignan aleatoriamente a paneles especializados y emiten votos on-chain que
aprueban, rechazan o revierten releases de fixtures de chunker.

Este documento describe el proceso offline y como los developers interactuan con
el.

## Overview del Parliament

- **Ciudadania**: los participantes bloquean el bond de XOR requerido para
  enrolarse como ciudadanos Sora y ser elegibles para sortition.
- **Paneles**: el Parliament divide responsabilidades entre paneles rotativos
  (Infrastructure, Moderation, Treasury, etc.). El Infrastructure Panel ahora
  maneja las aprobaciones de fixtures SoraFS.
- **Sortition y rotacion**: los asientos de panel se re-sortean con la cadencia
  definida en la constitucion del Parliament para que ningun grupo monopolice
  aprobaciones.

## Flujo de aprobacion de fixtures

1. **Envio de propuesta**
   - Tooling WG sube el bundle candidato `manifest_blake3.json` y diff de fixtures
     al registry on-chain SoraFS (`sorafs.fixtureProposal` call).
   - La propuesta almacena el digest BLAKE3, metadata de version y notas de cambio.
2. **Revision y votacion**
   - El Infrastructure Panel recibe la asignacion de propuesta via la task queue
     del Parliament.
   - Miembros del panel inspeccionan artefactos de CI, corren tests de paridad y
     emiten votos ponderados on-chain.
3. **Finalizacion**
   - Una vez alcanzado el threshold on-chain, el runtime Nexus emite un evento
     de aprobacion con el digest canonico del manifiesto y el compromiso Merkle
     al payload de fixtures.
   - El evento se espeja en el registry SoraFS para que los clientes puedan
     obtener el manifiesto aprobado mas reciente.
4. **Distribucion**
   - Helpers de CLI (`cargo xtask sorafs-fetch-fixture`) descargan el manifiesto
     aprobado directo desde el endpoint Nexus RPC. El repositorio local mantiene
     en sync los constants JSON/TS/Go de fixtures re-ejecutando `export_vectors`
     y validando el digest contra el registro on-chain.

## Workflow de developers

- Regenerar fixtures con `cargo run -p sorafs_chunker --bin export_vectors`.
- Usar el helper de fetch del Parliament desde `xtask` para descargar el sobre
  aprobado, verificar firmas del council y refrescar fixtures locales. Apuntar
  `--signatures` al sobre publicado por el Parliament; el helper resuelve el
  manifiesto adjunto, recomputa el digest BLAKE3 y hace cumplir el perfil de
  chunk canonico `sorafs.sf1@1.0.0`.

  ```
  cargo xtask sorafs-fetch-fixture \
    --signatures https://nexus.example/api/sorafs/manifest_signatures.json \
    --out fixtures/sorafs_chunker
  ```

  Pasar `--manifest` si el manifiesto vive en una URL o ruta distinta. El
  comando rechaza sobres sin firma salvo que se provea `--allow-unsigned` para
  smoke runs locales.
  Al validar el manifiesto publicado contra un gateway de staging, apuntar
  `sorafs-fetch` al host Torii en lugar de payloads locales:

  ```
  sorafs-fetch \
    --plan=fixtures/chunk_fetch_specs.json \
    --gateway-provider=name=staging,provider-id=<hex>,base-url=https://gw-stage.example/,stream-token=<base64> \
    --gateway-manifest-id=<manifest_id_hex> \
    --gateway-chunker-handle=sorafs.sf1@1.0.0 \
    --json-out=reports/staging_gateway.json
  ```

- CI local ya no requiere un roster `signer.json`. En su lugar,
  `ci/check_sorafs_fixtures.sh` compara el estado del repo con el compromiso
  on-chain mas reciente y falla si divergen.

## Paquete de revision de Data Availability

Cuando los paneles de Infrastructure o Moderation evaluan manifiestos de Data
Availability (DA-10), deben adjuntar un paquete de governance para que el
Parliament pueda trazar el voto al artefacto Norito firmado.【docs/source/governance_playbook.md:24】

1. Obtener el snapshot de politica de retencion (`torii.da_ingest.replication_policy`) y
   confirmar que el `RetentionPolicy` del manifiesto coincide con la clase
   aplicada (ver `docs/source/da/replication_policy.md`). Guardar el output de
   CLI o el artefacto de CI.
2. Registrar el digest del manifiesto, governance tag, clase de blob y valores
   de retencion dentro de `docs/examples/da_manifest_review_template.md`. Adjuntar
   la plantilla completada, el payload Norito firmado (`.to`) y referencias a
   tickets de subsidio/takedown al docket del Parliament.
3. Incluir contexto de moderacion/compliance cuando la request se origine en un
   takedown o apelacion para que el Governance DAG enlace el hash del manifiesto
   con el registro de escalamiento (`docs/source/sorafs_gateway_compliance_plan.md`,
   `docs/source/sorafs_moderation_panel_plan.md`).
4. Durante rollbacks, referenciar el paquete previo y hacer delta de los campos
   de retencion o governance para que auditores confirmen que no hubo drift
   silencioso de parametros.

## Notas de governance

- La constitucion del Parliament gobierna quorum, rotacion y escalamiento; no se
  requiere configuracion a nivel crate.
- Rollbacks de emergencia se disparan via el panel "moderation" del Parliament.
  El panel de infrastructure presenta una propuesta de revert que referencia el
  digest del manifiesto previo, que reemplaza el release una vez aprobado.
- Aprobaciones historicas permanecen disponibles en el registry SoraFS para
  replay forense.

## FAQ

- **¿A donde fue `signer.json`?**
  Fue removido. Toda la atribucion de firmantes se maneja por el contrato del
  Sora Parliament; los tokens de hardware individuales ya no se trackean en el
  repo.

- **¿Seguimos requiriendo firmas Ed25519 locales?**
  El runtime del Parliament almacena aprobaciones como artefactos on-chain.
  `manifest_signatures.json` permanece en el repositorio solo como fixtures de
  developer y debe coincidir con el digest on-chain registrado en el ultimo
  evento de aprobacion.

- **¿Como monitorean las aprobaciones los equipos?**
  Suscribirse al evento `ParliamentFixtureApproved` o consultar el registry via
  Nexus RPC para recuperar el digest del manifiesto actual y el roll call del
  panel.
