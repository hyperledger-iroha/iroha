---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/gateway-dns-runbook.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Runbook de inicio de Gateway y DNS de SoraFS

Esta copia del portal espelha o runbook canonico em
[`docs/source/sorafs_gateway_dns_design_runbook.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_gateway_dns_design_runbook.md).
La captura de las barandillas operativas del flujo de trabajo de DNS descentralizado y Gateway
para que líderes de networking, operaciones y documentacao possam ensaiar a pilha de
automatizacao antes del inicio del 2025-03.

## Escopo y entregaveis

- Conectar los marcos de DNS (SF-4) y gateway (SF-5) ensaiando derivacao
  determinística de hosts, liberaciones de directorio de resolutores, automatización TLS/GAR
  e captura de evidencias.
- Manter os insumos do kickoff (agenda, convite, tracker de presenca, snapshot de
  telemetria GAR) sincronizados com as ultimas atribuicoes de propietarios.
- Produzir um bundle de artefatos auditavel para revisores degobernanza: notas de
  liberación del directorio de resolutores, registros de sonda de la puerta de enlace, dijo el arnés de
  conformidad y resumen de Docs/DevRel.

## Papeles y responsabilidades| Flujo de trabajo | Responsabilidades | Artefatos requeridos |
|------------|-------------------|----------------------|
| Redes TL (pila DNS) | Manter o plano determinístico de hosts, ejecutar lanzamientos de directorio RAD, publicar entradas de telemetría de resolutores. | `artifacts/soradns_directory/<ts>/`, diferencias de `docs/source/soradns/deterministic_hosts.md`, metadatos RAD. |
| Líder de automatización de operaciones (puerta de enlace) | Ejecutar taladros de automatización TLS/ECH/GAR, varillar `sorafs-gateway-probe`, actualizar ganchos de PagerDuty. | `artifacts/sorafs_gateway_probe/<ts>/`, sonda JSON, entradas `ops/drill-log.md`. |
| Grupo de trabajo sobre herramientas y gremio de control de calidad | Rodar `ci/check_sorafs_gateway_conformance.sh`, curar accesorios, archivar paquetes de autocertificación Norito. | `artifacts/sorafs_gateway_conformance/<ts>/`, `artifacts/sorafs_gateway_attest/<ts>/`. |
| Documentos/DevRel | Capturar minutos, actualizar o preleer de diseño + apéndices, y publicar o resumen de evidencias en este portal. | Arquivos `docs/source/sorafs_gateway_dns_design_*.md` atualizados y notas de rollout. |

## Entradas y requisitos previos- Específicacao de hosts deterministas (`docs/source/soradns/deterministic_hosts.md`) e
  o andamio de atestacao de resolutores (`docs/source/soradns/resolver_attestation_directory.md`).
- Artefatos de gateway: manual del operador, ayudantes de automatización TLS/ECH,
  Guía de modo directo y flujo de trabajo de autocertificación en `docs/source/sorafs_gateway_*`.
- Útiles: `cargo xtask soradns-directory-release`,
  `cargo xtask sorafs-gateway-probe`, `scripts/telemetry/run_soradns_transparency_tail.sh`,
  `scripts/sorafs_gateway_self_cert.sh`, y ayudantes de CI
  (`ci/check_sorafs_gateway_conformance.sh`, `ci/check_sorafs_gateway_probe.sh`).
- Segredos: chave de liberación GAR, credenciales ACME DNS/TLS, clave de enrutamiento de PagerDuty,
  token de autenticación de Torii para recuperar los resolutores.

## Lista de verificación previa al vuelo

1. Confirme participantes y agenda actualizando
   `docs/source/sorafs_gateway_dns_design_attendance.md` y circulando una agenda
   actual (`docs/source/sorafs_gateway_dns_design_agenda.md`).
2. Prepare raizes de artefatos como
   `artifacts/sorafs_gateway_dns/<YYYYMMDD>/` y
   `artifacts/soradns_directory/<YYYYMMDD>/`.
3. Actualizar accesorios (manifiestos GAR, pruebas RAD, paquetes de conformidade do gateway) e
   garanta que o estado de `git submodule` esteja alinhado ao ultimo tag de ensaio.
4. Verifique secretos (chave de release Ed25519, archivo de contacto ACME, token do PagerDuty)
   e se batem com checksums do vault.
5. Faça smoke-test nos objetivos de telemetría (punto final Pushgateway, placa GAR Grafana)
   antes de perforar.

## Etapa de ensayo de automatización

### Mapa determinístico de hosts y lanzamiento del directorio RAD1. Rode o helper de derivacao deterministica de hosts contra o set de manifests
   Proposto e confirme que nao ha drift em relacao a
   `docs/source/soradns/deterministic_hosts.md`.
2. Gere um paquete de directorio de resolutores:

```bash
cargo xtask soradns-directory-release \
  --rad-dir artifacts/soradns/rad_candidates \
  --output-root artifacts/soradns_directory \
  --release-key-path secrets/soradns/release.key \
  --car-cid bafybeigdyrdnsmanifest... \
  --note "dns-kickoff-20250303"
```

3. Registre o ID do diretorio, o SHA-256 y os caminhos de saya impressos dentro de
   `docs/source/sorafs_gateway_dns_design_gar_telemetry.md` y nas minutos del saque inicial.

### Captura de telemetría DNS

- Faça tail dos logs de transparencia de resolutores por ≥10 minutos usando
  `scripts/telemetry/run_soradns_transparency_tail.sh --mode staging`.
- Exportar métricas de Pushgateway y archivar instantáneas NDJSON al lado de
  directorio ejecutar ID.

### Taladros de automatización del gateway

1. Ejecute o sonda TLS/ECH:

```bash
cargo xtask sorafs-gateway-probe \
  --config configs/sorafs_gateway/probe.staging.toml \
  --output artifacts/sorafs_gateway_probe/<run-id>.json
```

2. Montado o arnés de conformidad (`ci/check_sorafs_gateway_conformance.sh`) e
   o ayudante de autocertificación (`scripts/sorafs_gateway_self_cert.sh`) para actualizar
   o paquete de atestacoes Norito.
3. Capture eventos de PagerDuty/Webhook para probar el camino de automatización
   funciona de ponta a ponta.

### Empacotamiento de evidencias

- Actualizar `ops/drill-log.md` con marcas de tiempo, participantes y hashes de sondas.
- Armazene artefatos nos diretorios de run ID e publique um resumo executivo
  nas minutos do Docs/DevRel.
- Enlace el paquete de evidencias sin boleto de gobierno antes de la revisión del saque inicial.

## Facilitacao de sessao e hand-off de evidencias- **Linha do tempo do moderador:**
  - T-24 h — Gestión de Programas posta o lembrete + snapshot de agenda/presenca em `#nexus-steering`.
  - T-2 h — Networking TL actualiza o instantánea de telemetría GAR y registra los deltas en `docs/source/sorafs_gateway_dns_design_gar_telemetry.md`.
  - T-15 m — Ops Automation verifica a prontidao de probes y escreve o run ID ativo em `artifacts/sorafs_gateway_dns/current`.
  - Durante una chamada — O moderador compartilha este runbook e designa um escriba ao vivo; Docs/DevRel capturan elementos de acao en línea.
- **Plantilla de minutos:** Copia o esqueleto de
  `docs/source/sorafs_gateway_dns_design_minutes.md` (también espelhado sin paquete
  do portal) e comité uma instancia preenchida por sesión. Incluye lista de
  participantes, decisiones, elementos de acao, hashes de evidencias y riesgos pendientes.
- **Subir evidencias:** Zip o diretorio `runbook_bundle/` do ensaio,
  anexe o PDF de minutos renderizado, registre hashes SHA-256 nas minutos +
  agenda, e depois avise o alias de revisores de gobierno cuando las cargas
  Chegarem em `s3://sora-governance/sorafs/gateway_dns/<date>/`.

## Instantánea de evidencias (kickoff de marco 2025)

Os ultimos artefatos de ensaio/live referenciados no roadmap e nas minutos
ficam sin cubo `s3://sora-governance/sorafs/gateway_dns/`. Os hashes abaixo
espelham o manifiesto canonico (`artifacts/sorafs_gateway_dns/<run-id>/runbook_bundle/evidence_manifest_*.json`).- **Ejecución en seco — 2025-03-02 (`artifacts/sorafs_gateway_dns/20250302/`)**
  - Tarball hacer paquete: `b13571d2822c51f771d0e471f4f66d088a78ed6c1a5adb0d4b020b04dd9a5ae0`
  - PDF en minutos: `cac89ee3e6e4fa0adb9694941c7c42ffddb513f949cf1b0c9f375e14507f4f18`
- **Taller ao vivo — 2025-03-03 (`artifacts/sorafs_gateway_dns/20250303/runbook_bundle/`)**
  - `bc83e6a014c2d223433f04ddc3c588bfeff33ee5cdcb15aad6527efeba582a1c  minutes_20250303.md`
  - `030a98fb3e3a52dbb0fcf25a6ea4365b11d9487707bb6700cb632710f7c082e4  gar_snapshot_20250303.json`
  - `5ac17e684976d6862628672627f229f7719da74235aa0a5f0ce994dad34cb3c4  sorafs_gateway_dns_design_metrics_20250303.prom`
  - `5c6163d0ae9032c2d52ca2ecca4037dfaddcc503eb56239b53c5e9c4000997cf  probe_20250303.json`
  - `87f6341896bfb830966a4a5d0fc9158fabcc135ba16ef0d53882e558de77ba49  probe_20250303_webhook.jsonl`
  - `9b968b0bf4ca654d466ec2be5291936f1441908354e9d2da4d0a52f1568bbe03  probe.staging.toml`
  - _(Subir pendiente: `gateway_dns_minutes_20250303.pdf` — Docs/DevRel adjunta el SHA-256 cuando el PDF renderizado se compra en el paquete.)_

## Material relacionado

- [Guía de operaciones de puerta de enlace](./operations-playbook.md)
- [Plano de observabilidad de SoraFS](./observability-plan.md)
- [Rastreador de DNS descentralizado y puerta de enlace](https://github.com/hyperledger-iroha/iroha/blob/master/roadmap.md#core-workstreams)