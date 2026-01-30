---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/signing-ceremony.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
id: signing-ceremony
title: Sustitucion de la ceremonia de firma
description: Como el Parlamento de Sora aprueba y distribuye fixtures del chunker de SoraFS (SF-1b).
sidebar_label: Ceremonia de firma
---

> Roadmap: **SF-1b — aprobaciones de fixtures del Parlamento de Sora.**
> El flujo del Parlamento reemplaza la antigua “ceremonia de firma del consejo” offline.

El ritual manual de firmas usado para los fixtures del chunker de SoraFS queda retirado. Todas
las aprobaciones ahora pasan por el **Parlamento de Sora**, la DAO basada en sorteo que gobierna
Nexus. Los miembros del Parlamento fianzan XOR para obtener ciudadania, rotan entre paneles y
emiten votos on-chain que aprueban, rechazan o revierten releases de fixtures. Esta guia explica
el proceso y el tooling para developers.

## Resumen del Parlamento

- **Ciudadania** — Los operadores fianzan el XOR requerido para inscribirse como ciudadanos y
  volverse elegibles para sorteo.
- **Paneles** — Las responsabilidades se dividen en paneles rotativos (Infraestructura,
  Moderacion, Tesoreria, …). El Panel de Infraestructura es el dueno de las aprobaciones de
  fixtures de SoraFS.
- **Sorteo y rotacion** — Los asientos de panel se reasignan con la cadencia especificada en
  la constitucion del Parlamento para que ningun grupo monopolice las aprobaciones.

## Flujo de aprobacion de fixtures

1. **Envio de propuesta**
   - El WG de Tooling sube el bundle candidato `manifest_blake3.json` mas el diff del fixture
     al registro on-chain via `sorafs.fixtureProposal`.
   - La propuesta registra el digest BLAKE3, la version semantica y las notas de cambio.
2. **Revision y votacion**
   - El Panel de Infraestructura recibe la asignacion a traves de la cola de tareas del
     Parlamento.
   - Los miembros del panel inspeccionan artefactos de CI, corren tests de paridad y emiten
     votos ponderados on-chain.
3. **Finalizacion**
   - Una vez alcanzado el quorum, el runtime emite un evento de aprobacion que incluye el
     digest canonico del manifest y el compromiso Merkle del payload del fixture.
   - El evento se refleja en el registry de SoraFS para que los clientes puedan obtener el
     manifest mas reciente aprobado por el Parlamento.
4. **Distribucion**
   - Los helpers de CLI (`cargo xtask sorafs-fetch-fixture`) traen el manifest aprobado desde
     Nexus RPC. Las constantes JSON/TS/Go del repositorio se mantienen sincronizadas al
     re-ejecutar `export_vectors` y validar el digest contra el registro on-chain.

## Flujo de trabajo para developers

- Regenera fixtures con:

```bash
cargo run -p sorafs_chunker --bin export_vectors
```

- Usa el helper de fetch del Parlamento para descargar el envelope aprobado, verificar
  firmas y refrescar fixtures locales. Apunta `--signatures` al envelope publicado por el
  Parlamento; el helper resuelve el manifest acompanante, recomputa el digest BLAKE3 e
  impone el perfil canonico `sorafs.sf1@1.0.0`.

```bash
cargo xtask sorafs-fetch-fixture \
  --signatures https://nexus.example/api/sorafs/manifest_signatures.json \
  --out fixtures/sorafs_chunker
```

Pasa `--manifest` si el manifest vive en otra URL. Los envelopes sin firma se rechazan
salvo que se configure `--allow-unsigned` para smoke runs locales.

- Al validar un manifest a traves de un gateway de staging, apunta a Torii en vez de payloads
  locales:

```bash
sorafs-fetch \
  --plan=fixtures/chunk_fetch_specs.json \
  --gateway-provider=name=staging,provider-id=<hex>,base-url=https://gw-stage.example/,stream-token=<base64> \
  --gateway-manifest-id=<manifest_id_hex> \
  --gateway-chunker-handle=sorafs.sf1@1.0.0 \
  --json-out=reports/staging_gateway.json
```

- El CI local ya no requiere un roster `signer.json`.
  `ci/check_sorafs_fixtures.sh` compara el estado del repo contra el ultimo compromiso
  on-chain y falla cuando divergen.

## Notas de gobernanza

- La constitucion del Parlamento gobierna quorum, rotacion y escalamiento; no se requiere
  configuracion a nivel de crate.
- Los rollbacks de emergencia se manejan a traves del panel de moderacion del Parlamento.
  El Panel de Infraestructura presenta una propuesta de revert que referencia el digest
  previo del manifest, reemplazando la release una vez aprobada.
- Las aprobaciones historicas permanecen disponibles en el registry de SoraFS para
  replay forense.

## FAQ

- **A donde se fue `signer.json`?**  
  Se elimino. Toda la atribucion de firmas vive on-chain; `manifest_signatures.json`
  en el repositorio es solo un fixture de developer que debe coincidir con el ultimo
  evento de aprobacion.

- **Seguimos requiriendo firmas Ed25519 locales?**  
  No. Las aprobaciones del Parlamento se almacenan como artefactos on-chain. Los fixtures
  locales existen para reproducibilidad, pero se validan contra el digest del Parlamento.

- **Como monitorean las aprobaciones los equipos?**  
  Suscribanse al evento `ParliamentFixtureApproved` o consulten el registry via Nexus RPC
  para recuperar el digest actual del manifest y el roll call del panel.
