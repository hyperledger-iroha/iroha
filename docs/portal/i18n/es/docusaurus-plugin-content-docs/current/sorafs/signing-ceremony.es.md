---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/signing-ceremony.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: ceremonia de firma
título: Sustitución de la ceremonia de firma
descripción: Como el Parlamento de Sora aprueba y distribuye accesorios del chunker de SoraFS (SF-1b).
sidebar_label: Ceremonia de firma
---

> Hoja de ruta: **SF-1b — aprobaciones de fixtures del Parlamento de Sora.**
> El flujo del Parlamento reemplaza la antigua “ceremonia de firma del consejo” fuera de línea.

El ritual manual de firmas usado para los accesorios del chunker de SoraFS queda retirado. todas
las aprobaciones ahora pasan por el **Parlamento de Sora**, la DAO basada en sorteo que gobierna
Nexus. Los miembros del Parlamento fianzan XOR para obtener ciudadania, rotan entre paneles y
emiten votos on-chain que aprueban, rechazan o revierten lanzamientos de accesorios. Esta guia explica
el proceso y las herramientas para desarrolladores.

## Resumen del Parlamento

- **Ciudadania** — Los operadores fianzan el XOR requerido para inscribirse como ciudadanos y
  volverse elegibles para sorteo.
- **Paneles** — Las responsabilidades se dividen en paneles rotativos (Infraestructura,
  Moderación, Tesorería,…). El Panel de Infraestructura es el dueno de las aprobaciones de
  accesorios de SoraFS.
- **Sorteo y rotación** — Los asientos de panel se reasignan con la cadencia especificada en
  la constitucion del Parlamento para que ningun grupo monopolice las aprobaciones.

## Flujo de aprobación de accesorios1. **Envio de propuesta**
   - El WG de Tooling sube el paquete candidato `manifest_blake3.json` mas la diferencia del accesorio
     al registro on-chain vía `sorafs.fixtureProposal`.
   - La propuesta registra el resumen BLAKE3, la versión semántica y las notas de cambio.
2. **Revisión y votación**
   - El Panel de Infraestructura recibe la asignación a través de la cola de tareas del
     Parlamento.
   - Los miembros del panel inspeccionan artefactos de CI, corren pruebas de paridad y emiten
     votos ponderados en cadena.
3. **Finalización**
   - Una vez alcanzado el quórum, el tiempo de ejecución emite un evento de aprobación que incluye el
     digest canonico del manifest y el compromiso Merkle del payload del fixture.
   - El evento se refleja en el registro de SoraFS para que los clientes puedan obtener el
     manifiesto mas reciente aprobado por el Parlamento.
4. **Distribución**
   - Los helpers de CLI (`cargo xtask sorafs-fetch-fixture`) traen el manifiesto aprobado desde
     NexusRPC. Las constantes JSON/TS/Go del repositorio se mantienen sincronizadas al
     reejecutar `export_vectors` y validar el digest contra el registro on-chain.

## Flujo de trabajo para desarrolladores

- Regenera accesorios con:

```bash
cargo run -p sorafs_chunker --bin export_vectors
```- Usa el helper de fetch del Parlamento para descargar el sobre aprobado, verificar
  firmas y refrescar accesorios locales. Apunta `--signatures` al sobre publicado por el
  Parlamento; el helper resuelve el manifiesto acompañante, recalcula el digest BLAKE3 e
  impone el perfil canónico `sorafs.sf1@1.0.0`.

```bash
cargo xtask sorafs-fetch-fixture \
  --signatures https://nexus.example/api/sorafs/manifest_signatures.json \
  --out fixtures/sorafs_chunker
```

Pasa `--manifest` si el manifiesto vive en otra URL. Los sobres sin firma se rechazan
salvo que se configure `--allow-unsigned` para que smoke ejecute locales.

- Al validar un manifiesto a través de una puerta de enlace de puesta en escena, apunta a Torii en vez de cargas útiles
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
  `ci/check_sorafs_fixtures.sh` compara el estado del repositorio contra el último compromiso
  on-chain y falla cuando divergen.

## Notas de gobernanza

- La constitución del Parlamento gobierna quórum, rotación y escalamiento; no se requiere
  configuración a nivel de caja.
- Los rollbacks de emergencia se manejan a través del panel de moderación del Parlamento.
  El Panel de Infraestructura presenta una propuesta de revertir que referencia el digest
  previo del manifiesto, reemplazando la liberación una vez aprobada.
- Las aprobaciones históricas permanecen disponibles en el registro de SoraFS para
  reproducir forense.

## Preguntas frecuentes- **¿A donde se fue `signer.json`?**  
  Se elimina. Toda la atribucion de firmas vive on-chain; `manifest_signatures.json`
  en el repositorio es solo un accesorio de desarrollador que debe coincidir con el último
  evento de aprobación.

- **¿Seguimos requiriendo firmas Ed25519 locales?**  
  No. Las aprobaciones del Parlamento se almacenan como artefactos en cadena. Los partidos
  Existen locales para reproducibilidad, pero se validan contra el digest del Parlamento.

- **¿Como monitorean las aprobaciones los equipos?**  
  Suscribanse al evento `ParliamentFixtureApproved` o consulten el registro vía Nexus RPC
  para recuperar el digest actual del manifiesto y el roll call del panel.