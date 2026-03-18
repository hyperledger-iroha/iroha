---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/gateway-dns-runbook.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Runbook de lanzamiento de puerta de enlace y DNS SoraFS

Esta copia del portal refleja el runbook canónico en el
[`docs/source/sorafs_gateway_dns_design_runbook.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_gateway_dns_design_runbook.md).
Capture las numerosas operaciones del flujo de trabajo DNS descentralizado y Gateway
afin que los responsables de investigación, operaciones y documentación puedan repetir la pila
d'automatisation avant le lancement 2025-03.

## Portée et livrables

- Coloque los botones DNS (SF-4) y gateway (SF-5) al repetir la derivación determinada
  des hôtes, les releases du répertoire de resolutors, l’automatisation TLS/GAR et la
  captura de preuves.
- Garder les entrées du kickoff (agenda, invitación, seguimiento de presencia, instantánea de
  télémétrie GAR) sincronizadas con las últimas afectaciones de los propietarios.
- Produire un paquete de artefactos auditables para los revisores de gobierno: notas
  de liberación del repertorio de resolutores, registros de sondas de entrada, salida del arnés de
  conformidad y síntesis Docs/DevRel.

## Roles y responsabilidades| Flujo de trabajo | Responsabilidades | Requisitos de artefactos |
|------------|------------------|------------------|
| Redes TL (pila DNS) | Mantenga el plan determinado de los hogares, ejecute las liberaciones del repertorio RAD y publique las entradas de télémétrie de los resolutores. | `artifacts/soradns_directory/<ts>/`, diferencias de `docs/source/soradns/deterministic_hosts.md`, metadonnées RAD. |
| Líder de automatización de operaciones (puerta de enlace) | Ejecute los taladros de automatización TLS/ECH/GAR, lancer `sorafs-gateway-probe`, coloque cada día los ganchos PagerDuty. | `artifacts/sorafs_gateway_probe/<ts>/`, JSON de sonda, platos principales `ops/drill-log.md`. |
| Grupo de trabajo sobre herramientas y gremio de control de calidad | Lancer `ci/check_sorafs_gateway_conformance.sh`, curador de accesorios, archivador de paquetes con autocertificación Norito. | `artifacts/sorafs_gateway_conformance/<ts>/`, `artifacts/sorafs_gateway_attest/<ts>/`. |
| Documentos/DevRel | Capture los minutos, ponga al día la lectura previa del diseño + anexos y publique la síntesis de evidencia en este portal. | Archivos `docs/source/sorafs_gateway_dns_design_*.md` del día y notas de implementación. |

## Entradas y prérequis- Especificación de huéspedes determinados (`docs/source/soradns/deterministic_hosts.md`) y
  andamio de atestación de resolución (`docs/source/soradns/resolver_attestation_directory.md`).
- Portal de artefactos: manuel operador, ayudantes de automatización TLS/ECH,
  Guía en modo directo, flujo de trabajo autocertificado sous `docs/source/sorafs_gateway_*`.
- Herramientas: `cargo xtask soradns-directory-release`,
  `cargo xtask sorafs-gateway-probe`, `scripts/telemetry/run_soradns_transparency_tail.sh`,
  `scripts/sorafs_gateway_self_cert.sh`, y ayudantes CI
  (`ci/check_sorafs_gateway_conformance.sh`, `ci/check_sorafs_gateway_probe.sh`).
- Secretos: clave de liberación GAR, credenciales ACME DNS/TLS, clave de enrutamiento PagerDuty,
  autenticación de token Torii para recuperar los solucionadores.

## Lista de verificación anterior al vol

1. Confirmar los participantes y la agenda actualizada
   `docs/source/sorafs_gateway_dns_design_attendance.md` y difusor de agenda
   corriente (`docs/source/sorafs_gateway_dns_design_agenda.md`).
2. Préparer les racines d’artefacts telles que
   `artifacts/sorafs_gateway_dns/<YYYYMMDD>/` y
   `artifacts/soradns_directory/<YYYYMMDD>/`.
3. Rafraîchir les fixtures (manifiestos GAR, preuves RAD, paquetes de conformidad gateway) et
   Asegúrese de que el estado `git submodule` corresponda a la última etiqueta de ensayo.
4. Verificador de secretos (clave de lanzamiento Ed25519, archivo de cuenta ACME, token PagerDuty)
   et leur correspondance avec les checksums du vault.
5. Haga una prueba de humo de los cibles de télémétrie (punto final Pushgateway, placa GAR Grafana)
   delante del taladro.

## Étapes de ensayo de automatización

### Carta de huéspedes determinada y liberación del repertorio RAD1. Ejecutar el ayudante de derivación determinante de los hogares en el conjunto de manifiestos.
   Proponemos y confirmamos la ausencia de deriva por relación.
   `docs/source/soradns/deterministic_hosts.md`.
2. Generar un paquete de repertorio de resolutores:

```bash
cargo xtask soradns-directory-release \
  --rad-dir artifacts/soradns/rad_candidates \
  --output-root artifacts/soradns_directory \
  --release-key-path secrets/soradns/release.key \
  --car-cid bafybeigdyrdnsmanifest... \
  --note "dns-kickoff-20250303"
```

3. Registrar el ID del repertorio, el SHA-256 y los caminos de salida impresos en
   `docs/source/sorafs_gateway_dns_design_gar_telemetry.md` y en los minutos del saque inicial.

### Captura de televisión DNS

- Guarde los registros de transparencia de los resolutores durante ≥10 minutos con
  `scripts/telemetry/run_soradns_transparency_tail.sh --mode staging`.
- Exportador de medidas Pushgateway y archivador de instantáneas NDJSON en el côté du
  Repertorio de ID de ejecución.

### Ejercicios de automatización de la puerta de enlace

1. Ejecute la sonda TLS/ECH:

```bash
cargo xtask sorafs-gateway-probe \
  --config configs/sorafs_gateway/probe.staging.toml \
  --output artifacts/sorafs_gateway_probe/<run-id>.json
```

2. Lanzar el arnés de conformidad (`ci/check_sorafs_gateway_conformance.sh`) y
   l'helper self-cert (`scripts/sorafs_gateway_self_cert.sh`) para rafraîchir le paquete
   d'attestations Norito.
3. Capture los eventos PagerDuty/Webhook para comprobar la cadena de automatización
   función de combate en combate.

### Embalaje de preuves

- Mettre à jour `ops/drill-log.md` con marcas de tiempo, participantes y hashes de sonda.
- Almacenar los artefactos en los repertorios de run ID y publicar una síntesis ejecutiva.
  en los minutos Docs/DevRel.
- Lier le bundle de preuves dans le ticket de gouvernance avant la revue de kickoff.

## Animación de sesión y remise des preuves- **Cronología del moderador:**
  - T-24 h — Gestión del programa poste le rappel + agenda instantánea/presencia en `#nexus-steering`.
  - T-2 h — Networking TL graba la instantánea télémétrie GAR y consigna los deltas en `docs/source/sorafs_gateway_dns_design_gar_telemetry.md`.
  - T-15 m — Ops Automation verifica la preparación de sondas y escribe el ID de ejecución activo en `artifacts/sorafs_gateway_dns/current`.
  - Colgante de llamada: el moderador parte este runbook y asigna un escriba en directo; Docs/DevRel captura las acciones en línea.
- **Plantilla de actas:** Copiez le squelette de
  `docs/source/sorafs_gateway_dns_design_minutes.md` (también espejo en el paquete
  du portail) y confirma una instancia replicada por sesión. Incluir la lista de
  participantes, decisiones, acciones, hashes de preuves et risques ouverts.
- **Upload des preuves :** Zipper le répertoire `runbook_bundle/` du rehearsal,
  Únase al PDF de los minutos, registre los hashes SHA-256 en los minutos +
  agenda, luego pingue el alias des reviewers de gouvernance une fois les uploads
  disponibles en `s3://sora-governance/sorafs/gateway_dns/<date>/`.

## Instantánea de los anteriores (inicio en marzo de 2025)

Les derniers artefactos ensayo/live référencés dans la roadmap et les minutes
sont stockés dans le cubeta `s3://sora-governance/sorafs/gateway_dns/`. Los hashes
ci-dessous reflètent le manifest canonique (`artifacts/sorafs_gateway_dns/<run-id>/runbook_bundle/evidence_manifest_*.json`).- **Ejecución en seco — 2025-03-02 (`artifacts/sorafs_gateway_dns/20250302/`)**
  - Paquete tarball: `b13571d2822c51f771d0e471f4f66d088a78ed6c1a5adb0d4b020b04dd9a5ae0`
  - PDF de actas : `cac89ee3e6e4fa0adb9694941c7c42ffddb513f949cf1b0c9f375e14507f4f18`
- **Atelier en vivo — 2025-03-03 (`artifacts/sorafs_gateway_dns/20250303/runbook_bundle/`)**
  - `bc83e6a014c2d223433f04ddc3c588bfeff33ee5cdcb15aad6527efeba582a1c  minutes_20250303.md`
  - `030a98fb3e3a52dbb0fcf25a6ea4365b11d9487707bb6700cb632710f7c082e4  gar_snapshot_20250303.json`
  - `5ac17e684976d6862628672627f229f7719da74235aa0a5f0ce994dad34cb3c4  sorafs_gateway_dns_design_metrics_20250303.prom`
  - `5c6163d0ae9032c2d52ca2ecca4037dfaddcc503eb56239b53c5e9c4000997cf  probe_20250303.json`
  - `87f6341896bfb830966a4a5d0fc9158fabcc135ba16ef0d53882e558de77ba49  probe_20250303_webhook.jsonl`
  - `9b968b0bf4ca654d466ec2be5291936f1441908354e9d2da4d0a52f1568bbe03  probe.staging.toml`
  - _(Subir attente: `gateway_dns_minutes_20250303.pdf` — Docs/DevRel agregará el SHA-256 al PDF que se reproducirá en el paquete.)_

## Conexión de material

- [Pasarela de operaciones del libro de estrategias](./operations-playbook.md)
- [Plan de observabilidad SoraFS](./observability-plan.md)
- [Rastreador DNS descentralizado y puerta de enlace](https://github.com/hyperledger-iroha/iroha/blob/master/roadmap.md#core-workstreams)