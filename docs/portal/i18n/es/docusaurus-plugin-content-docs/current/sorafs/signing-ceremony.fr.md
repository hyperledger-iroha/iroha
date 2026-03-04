---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/signing-ceremony.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: ceremonia de firma
título: Reemplazo de la ceremonia de firma
descripción: Comment le Parlement Sora aprueba y distribuye los accesorios del fragmentador SoraFS (SF-1b).
sidebar_label: Ceremonia de firma
---

> Hoja de ruta: **SF-1b — Aprobaciones de los partidos del Parlamento Sora.**
> El flujo de trabajo del Parlamento sustituye a la antigua «ceremonia de firma del consejo» fuera de línea.

El ritual manual de firma de los accesorios del chunker SoraFS está retirado. Todos los
aprobaciones passent desormais par le **Parlement Sora**, la DAO basee sur le tirage
au sort qui gouverne Nexus. Los miembros del Parlamento bloquearon el XOR para obtener la
citoyennete, torneo entre paneles y votación en cadena para aprobar, rechazar o
revenir sur des releases de fixtures. Esta guía explica el proceso y las herramientas.
para los desarrolladores.

## Vista del conjunto del Parlamento- **Citoyennete** — Les operatorurs immobilisent le XOR requis pour s'inscrire comme
  citoyens et devenir elegibles au tirage au sort.
- **Paneles** — Las responsabilidades son repartidas entre los paneles rotativos
  (Infraestructuras, Moderación, Tesorería, ...). Detiente de infraestructura de Le Panel
  les aprobaciones de accesorios SoraFS.
- **Tirage au sort et rotación** — Les sieges de panel sont reattribues selon la
  cadencia especificada en la constitución del Parlamento afin qu'aucun groupe ne
  monopolizar las aprobaciones.

## Flujo de aprobación de accesorios1. **Sumisión de propuesta**
   - Le Tooling WG televerse le bundle candidat `manifest_blake3.json` et le diff
     de dispositivo en el registro en cadena a través de `sorafs.fixtureProposal`.
   - La propuesta registra el resumen BLAKE3, la versión semántica y las notas.
     de cambio.
2. **Revisar y votar**
   - Le Panel Infrastructure recupera la afectación a través del expediente de taches du Parlement.
   - Los miembros inspeccionan los artefactos CI, ejecutan las pruebas de paridad y
     emettent des votes ponderes on-chain.
3. **Finalización**
   - Una vez que se haya cumplido el quórum, el tiempo de ejecución generará un evento de aprobación incluido.
     El resumen canónico del manifiesto y el compromiso Merkle de la carga útil del accesorio.
   - El evento está duplicado en el registro SoraFS para que los clientes puedan
     recuperar el último manifiesto aprobado por el Parlamento.
4. **Distribución**
   - Los ayudantes CLI (`cargo xtask sorafs-fetch-fixture`) recuperan el manifiesto
     aprobar a través de Nexus RPC. Las constantes JSON/TS/Go del depósito permanecen sincronizadas
     en relancant `export_vectors` y en validant le digest par rapport a l'engistrement
     en cadena.

## Desarrollador de flujo de trabajo

- Regenerar los accesorios con:

```bash
cargo run -p sorafs_chunker --bin export_vectors
```- Utilice el asistente de búsqueda del Parlamento para telecargar el sobre aprobado,
  verifique las firmas y rafraichir los accesorios locales. Puntero `--signatures`
  vers l'enveloppe publiee par le Parlement; le helper resout le manifest associe,
  Vuelva a calcular el resumen BLAKE3 e imponga el perfil canónico `sorafs.sf1@1.0.0`.

```bash
cargo xtask sorafs-fetch-fixture \
  --signatures https://nexus.example/api/sorafs/manifest_signatures.json \
  --out fixtures/sorafs_chunker
```

Pasador `--manifest` si el manifiesto se encuentra en otra URL. Los sobres no
Los firmantes son rechazados sauf si `--allow-unsigned` está activo para las carreras de humo locaux.

- Para validar un manifiesto a través de una puerta de enlace de preparación, cibler Torii plutot que des
  cargas útiles ubicadas:

```bash
sorafs-fetch \
  --plan=fixtures/chunk_fetch_specs.json \
  --gateway-provider=name=staging,provider-id=<hex>,base-url=https://gw-stage.example/,stream-token=<base64> \
  --gateway-manifest-id=<manifest_id_hex> \
  --gateway-chunker-handle=sorafs.sf1@1.0.0 \
  --json-out=reports/staging_gateway.json
```

- El CI local no exige más una lista `signer.json`.
  `ci/check_sorafs_fixtures.sh` compare l'etat du repo avec le dernier engagement
  on-chain y echoue lorsqu'ils divergentes.

## Notas de gobierno

- La Constitución del Parlamento rige el quórum, la rotación y la escalada;
  Ninguna configuración en el nivel de la caja es necesaria.
- Las reversiones de urgencia se gestionan a través del panel de moderación del Parlamento. le
  Panel Infraestructura depone una propuesta de revertir qui referencia le resumen
  precedente del manifiesto, et la liberación est remplacee une fois approuvee.
- Las aprobaciones históricas están disponibles en el registro SoraFS para
  una repetición forense.

## Preguntas frecuentes- **¿Está pasado `signer.json`?**  
  Il a ete supprime. Toda la atribución de firma en cadena; `manifest_signatures.json`
  dans le depot n'est qu'un accesorio desarrollador que corresponde al último
  Evento de aprobación.

- **¿Faut-il encore des firmas locales Ed25519?**  
  No. Las aprobaciones del Parlamento son existencias como artefactos en cadena. Los partidos
  Los locales existentes para la reproducibilidad son válidos con respecto al resumen del Parlamento.

- **Comentar les equipes surveillent-elles les approbations ?**  
  Abra el evento `ParliamentFixtureApproved` o interrogue el registro a través de
  Nexus RPC para obtener el resumen actual del manifiesto y la lista de miembros del panel.