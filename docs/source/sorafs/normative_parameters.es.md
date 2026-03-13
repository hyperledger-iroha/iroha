---
lang: es
direction: ltr
source: docs/source/sorafs/normative_parameters.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f06bd60e4bdf20ba62698b20cae20cd7d77829be86f934f0d2b5f01b1400e4da
source_last_modified: "2025-11-22T12:56:17.003293+00:00"
translation_last_reviewed: "2026-01-30"
---

# Snapshot de parametros normativos SoraFS (Q4 2026)

Esta nota registra los parametros que cada despliegue SoraFS debe hacer cumplir
 durante los rollouts SF-6/SF-8. Expande la entrada corta **Roadmap -> SoraFS
Normative Parameters Snapshot** en una referencia independiente que empareja
cada guardrail con pasos de verificacion concretos. Mantenla junto a la
referencia de wire-format (`docs/source/sorafs_proto_plan.md`) para que
operadores, autores de SDK y auditores compartan una unica fuente de verdad.

## Referencia rapida

| Dominio | Parametro | Valor / Comportamiento | Evidencia y tooling |
|---------|-----------|------------------------|---------------------|
| Chunking y proofs | Perfil CDC | Target 256 KiB (64 KiB min, 512 KiB max) con digests BLAKE3, arbol PoR de dos niveles (64 KiB / 4 KiB) por chunk. | `cargo run -p sorafs_chunker --bin export_vectors` y `ci/check_sorafs_fixtures.sh` (ver `docs/source/sorafs/chunker_conformance.md`). `sorafs_car::ChunkStore` emite el arbol PoR (`sorafs_car/src/lib.rs`). |
| Provider adverts | Refresh/expiry | Providers refrescan adverts cada 12 h; TTL es 24 h. Payloads firmados `ProviderAdvertBodyV1` con metadata determinista de nonce/QoS. | Pipelines de publicacion de advert y fixtures de admision hacen cumplir cadencia/limites TLV. Dashboards: `torii_sorafs_admission_total`, `torii_sorafs_provider_range_capability_total` (ver `docs/source/sorafs/provider_advert_multisource.md`). |
| Cadencia de proofs | Ventanas PoR/PDP | Epochs de 1 h, <=32 samples, ventana de probe de 10 m + gracia de 2 m. Sora-PDP corre en todas las replicas hot; Sora-PoTR clasifica deadlines de 90 s (hot) / 5 m (warm). | `sorafs_cli proof verify` (PoR) y harnesses PDP; dashboards `dashboards/grafana/sorafs_capacity_health.json` (`torii_da_pdp_bonus_micro_total`, `torii_da_potr_bonus_micro_total`). Fetches HTTP deben llevar headers `Sora-PDP` / `Sora-PoTR` (`docs/source/soradns_gateway_content_binding.md`). |
| Ordenes de replicacion | SLA de aceptacion | Ordenes expiran si no se aceptan en 5 min, requieren `precommit` + QA PoR `PASS` antes de completion, y cada artefacto aterriza en el DAG GovernanceLog. | `torii_sorafs_registry_orders_total`, `torii_sorafs_replication_sla_total`, y fixtures DAG en `docs/source/sorafs_governance_dag_plan.md`. Regenerar fixtures via `cargo run -p sorafs_manifest --bin generate_replication_order_fixture`. |
| Pricing y auto-renew | Moneda y ladder de degradacion | Cargos denominados en **XOR** con USD TWAP visible en UX; auto-renew habilitado por defecto con caps de gasto por cuenta. Ladder: Servicio completo -> Solo lectura (30 d) -> Replicas reducidas (30 d) -> Archivo frio (60 d) -> Tombstone. | Metricas `sorafs.node.deal_*` (deal engine), dashboards `dashboards/grafana/sorafs_capacity_health.json`, docs `docs/source/sorafs/deal_engine.md` y `docs/source/sorafs/storage_capacity_marketplace.md`. |
| Transporte | Modo default | Retrieval read-only usa SoraNet anonymity (entry/middle/exit). Modo directo requiere overrides explicitos en CLI/SDK. Handshake usa X25519 + Kyber hibrido (SNNet-16) con rollout PQ para identidades/tickets de relays. | Overrides de policy `sorafs_cli fetch` / `sorafs_fetch`, `crates/iroha_cli/src/commands/streaming.rs`, `crates/soranet_pq`, y el plan de rollout PQ (`docs/source/soranet/pq_rollout_plan.md`). Dashboards flag direct-mode downgrades via `torii_stream_transport_policy_total`. |

## 1. Chunking y layout de proofs

- Perfil canonico: `sorafs.sf1@1.0.0` (ver `docs/source/sorafs/chunker_conformance.md`).
- Los arboles PoR hashean segmentos de 64 KiB a hojas de 4 KiB para que lecturas
  parciales mantengan cobertura power-of-two incluso cuando providers streamean
  ventanas mas pequenas.
- Workflow de verificacion:

  ```bash
  cargo run --locked -p sorafs_chunker --bin export_vectors
  ci/check_sorafs_fixtures.sh
  cargo test -p sorafs_car --lib chunk_store::tests::por_tree_smoke
  ```

  El chunk store emite folders deterministas `chunk_{index}.bin` mas witnesses
  por hoja reutilizados por `da_reconstruct.rs` y la API de proof streaming del
  gateway.

## 2. Ciclo de vida de Provider Adverts

- Los adverts se regeneran cada 12 horas, expiran despues de 24 horas y deben
  re-firmarse antes de que Torii los sirva via `/v2/sorafs/providers`.
- `ProviderAdvertBodyV1` incluye QoS, stake y transport hints; governance
  compara nonces deterministas para rechazar payloads replayados.
- Operadores envian nuevos adverts posteando el payload Norito firmado emitido
  por su pipeline de provider:

  ```bash
  curl -sS -X POST --data-binary @provider_advert.to \
    http://<torii-host>/v2/sorafs/provider/advert
  ```

- Dashboards a vigilar: `torii_sorafs_admission_total` (labels de resultado),
  ratios de warning de `docs/source/sorafs/provider_advert_rollout.md` y los
  gauges de range capability mencionados arriba. Alertas disparan si la tasa de
  warning >5% o rechazos >0.

## 3. Ventanas de proof y headers

- Probes PoR: el orquestador agenda <=32 samples por epoch con ventana de 10
  minutos y gracia de 2 minutos. PDP corre cada hora en replicas hot sin
  importar volumen de fetch; replicas warm/cold siguen la cadencia PDP definida
  en `docs/source/sorafs_proof_streaming_plan.md`.
- PoTR: clasifica deadlines de 90 segundos para replicas hot y 5 minutos para
  replicas warm. Los receipts son payloads `PotrReceiptV1` firmados.
- Clientes HTTP DEBEN enviar headers `Sora-PDP` y (cuando aplica) `Sora-PoTR-*`
  al recuperar via gateways; governance rechaza capturas sin ellos
  (`docs/source/soradns_gateway_content_binding.md`).
- Hooks de verificacion:

  ```bash
  sorafs_cli proof verify \
    --manifest artifacts/sorafs/sample.manifest.to \
    --car artifacts/sorafs/sample.car \
    --summary-out artifacts/sorafs/sample.pdp.json

  python3 scripts/sorafs_proof_probe.py --manifest <cid> --samples 32
  ```

## 4. Ciclo de vida de ordenes de replicacion

- Las ordenes emitidas expiran si los providers no reconocen (`precommit`) en
  5 minutos. Torii incrementa `torii_sorafs_registry_orders_total{status="expired"}`
  y dispara la alerta `SorafsReplicationExpiredOrders` definida en
  `docs/source/sorafs/runbooks/pin_registry_ops.md`.
- La completitud requiere:
  1. Ack `precommit` del provider,
  2. QA PoR `PASS` publicado en el DAG de governance, y
  3. incremento de `torii_sorafs_replication_sla_total{outcome="met"}` dentro del
     mismo epoch.
- Artefactos: entradas `GovernanceLogNodeV1` (ver
  `docs/source/sorafs_governance_dag_plan.md`) referencian digest del manifiesto,
  provider id, replication order id y resumen PoR para que operadores repliquen
  cada etapa.
- Refresh de fixtures:

  ```bash
  cargo run -p sorafs_manifest --bin generate_replication_order_fixture \
    -- --manifest fixtures/sorafs_manifest/ci_sample/manifest.json
  ```

## 5. Pricing, auto-renew y ladder de degradacion

- Los deals denominan cargos de storage/egress en **XOR** y exponen metadata de
  tracking USD para que UX muestre equivalentes fiat (`docs/source/sorafs/deal_engine.md`).
- Auto-renew esta **habilitado por defecto**. Clientes configuran caps via
  payloads Norito (`DealTermsV1.account_cap`, `DealTermsV1.egress_cap`) y el
  CLI/SDK los expone via `sorafs_cli deal terms` / `iroha app sorafs deal terms`.
- Ladder de degradacion:
  1. **Servicio completo** — autopayments saludables.
  2. **Solo lectura** (30 dias) — ordenes de replicacion se congelan; clientes
     pueden hacer fetch pero no mutar.
  3. **Replicas reducidas** (30 dias) — governance reasigna al minimo de replicas
     que mantiene proofs posibles.
  4. **Archivo frio** (60 dias) — archivado en storage de bajo costo; fetches
     requieren aprobacion del operador.
  5. **Tombstone** — metadata retenida para auditoria; datos removidos.
- Monitorear `sorafs.node.deal_*`, `torii_sorafs_capacity_*` y los paneles de
  rent/bonus en `dashboards/grafana/sorafs_capacity_health.json`. Alertar si
  `deal_outstanding_nano` crece mientras auto-renew sigue habilitado.

## 6. Defaults de transporte SoraNet

- `sorafs_cli fetch` y SDKs default a `transport_policy=soranet-first` con
  circuitos de tres saltos (entry/middle/exit). Bajar a `direct-only` o
  `max_peers=1` requiere override explicito registrado en artefactos de
  adopcion (ver `docs/source/sorafs_orchestrator_rollout.md`).
- Los relays de gateway deben honrar politicas GAR para trafico anonimizado.
  Mapping determinista de hosts + scaffolding GAR se generan via
  `cargo xtask soradns-hosts`, `cargo xtask soradns-gar-template` y
  `cargo xtask soradns-binding-template`.
- Handshake: X25519 + Kyber (ML-KEM-768) hibrido segun `crates/iroha_crypto` y
  `crates/soranet_pq`. El plan de rollout PQ en `docs/source/soranet/pq_rollout_plan.md`
  rastrea gates de adopcion de ticket/identidad. Helpers CLI (`iroha app streaming hpke
  fingerprint`) exponen fingerprints usados en telemetria.
- Telemetria: `torii_stream_transport_policy_total` y
  `torii_stream_anonymity_stage_total` respaldan dashboards SNNet. Alertas
  disparan cuando uso direct-mode excede el presupuesto de incidente especificado
  en `docs/source/ops/soranet_transport_rollback.md`.

---

**Maintenance:** actualizar este snapshot cuando cambien parametros de chunking,
proof o transporte. Incluir el hash de commit, UID de dashboard o comando CLI
necesario para reproducir cada valor para que governance pueda vincular
revisiones de roadmap a un artefacto estatico. Espejar ediciones significativas
al copy del portal si se agrega uno.
