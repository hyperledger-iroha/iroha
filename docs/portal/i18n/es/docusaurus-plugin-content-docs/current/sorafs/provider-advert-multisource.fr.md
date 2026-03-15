---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/provider-advert-multisource.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Anuncios de proveedores de múltiples fuentes y planificación

Esta página resume la especificación canónica en
[`docs/source/sorafs/provider_advert_multisource.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_multisource.md).
Utilice este documento para los esquemas Norito palabra por palabra y los registros de cambios; la copia del portal
conservar los consignatarios operadores, las notas SDK y las referencias de telemetría cerca del resto
des runbooks SoraFS.

## Agregado al esquema Norito

### Capacitación de playa (`CapabilityType::ChunkRangeFetch`)
- `max_chunk_span` – plus grande plage contigue (octetos) por requete, `>= 1`.
- `min_granularity` – resolución de búsqueda, `1 <= valeur <= max_chunk_span`.
- `supports_sparse_offsets`: permite compensaciones no contiguas en una sola solicitud.
- `requires_alignment`: si es cierto, las compensaciones deben alinearse en `min_granularity`.
- `supports_merkle_proof` – indique el premio a cargo de los temas PoR.

`ProviderCapabilityRangeV1::to_bytes` / `from_bytes` aplica una codificación canónica
Para que las cargas útiles de chismes permanezcan determinadas.

### `StreamBudgetV1`
- Campeones: `max_in_flight`, `max_bytes_per_sec`, `burst_bytes` opcional.
- Reglas de validación (`StreamBudgetV1::validate`):
  - `max_in_flight >= 1`, `max_bytes_per_sec > 0`.
  - `burst_bytes`, cuando esté presente, debe ser `> 0` y `<= max_bytes_per_sec`.### `TransportHintV1`
- Campeones: `protocol: TransportProtocol`, `priority: u8` (fenetre 0-15 appliquee par
  `TransportHintV1::validate`).
- Protocolos continuos: `torii_http_range`, `quic_stream`, `soranet_relay`,
  `vendor_reserved`.
- Les entrees de protocole dupliquées par fournisseur sont rejetees.

### Adjunto a `ProviderAdvertBodyV1`
- `stream_budget` opcional: `Option<StreamBudgetV1>`.
- `transport_hints` opcional: `Option<Vec<TransportHintV1>>`.
- Les deux champs transitent desormais via `ProviderAdmissionProposalV1`, les enveloppes
  de gobierno, los accesorios CLI y la telemetría JSON.

## Validación y enlace con la gobernanza

`ProviderAdvertBodyV1::validate` y `ProviderAdmissionProposalV1::validate`
Rejettent les metadonnees mal formees:

- Les capacites de plage doivent se decoder et respecter les limites de plage/granularite.
- Los presupuestos de flujo / sugerencias de transporte exigen un TLV `CapabilityType::ChunkRangeFetch`
  corresponsal y una lista de sugerencias no vide.
- Les protocolos de transport dupliques et les priorites invalides generent des erreurs
  de validación antes de la difusión de anuncios.
- Los sobres de admisión comparan propuestas/anuncios para las metadonnees de plage vía
  `compare_core_fields` afin que les payloads de gossip non concordants soient rechazados tot.

La cobertura de regresión se trouve dans
`crates/sorafs_manifest/src/{provider_advert,provider_admission}.rs`.

## Herramientas y accesorios- Las cargas útiles de anuncios de proveedores deben incluir los metadones `range_capability`,
  `stream_budget` y `transport_hints`. Validez via les reponses `/v1/sorafs/providers` et les
  accesorios de admisión; Los currículums JSON deben incluir la capacidad de análisis y el presupuesto de flujo.
  y los cuadros de sugerencias para la ingestión telemétrica.
- `cargo xtask sorafs-admission-fixtures` expone los presupuestos de flujo y sugerencias de transporte en
  Estos artefactos JSON se ajustan a los paneles de control después de la adopción de la función.
- Los accesorios bajo `fixtures/sorafs_manifest/provider_admission/` incluyen desormais:
  - des anuncios canónicos de múltiples fuentes,
  - `multi_fetch_plan.json` para que las suites SDK puedan reanudar un plan de recuperación
    determinista de múltiples pares.

## Integración con el orquestador y Torii- Torii `/v1/sorafs/providers` envíe los metadones de capacite de plage parsees con
  `stream_budget` y `transport_hints`. Los anuncios de degradación se debilitan cuando
  los proveedores omettent la nouvelle metadonnee, et les endpoints de plage du gateway
  aplique los memes contraintes para los clientes directos.
- L'orchestrateur multi-source (`sorafs_car::multi_fetch`) applique desormais les limites de
  plage, l'alignement des capacites et les stream Budgets lors de l'affectation du travail.
  Las pruebas unitarias cubren los escenarios de chunk trop grand, de seek disperses y de
  estrangulamiento.
- `sorafs_car::multi_fetch` señales difusas de degradación (echecs d'alignement,
  solicitudes estranguladas) afin que les operatorurs puissent tracer pourquoi sures fournisseurs
  Ont ete ignora la planificación.

## Referencia de telemetría

La instrumentación de búsqueda de playa de Torii alimenta el tablero Grafana
**SoraFS Observabilidad de recuperación** (`dashboards/grafana/sorafs_fetch_observability.json`) y
les regles d'alerte associees (`dashboards/alerts/sorafs_fetch_rules.yml`).| Métrica | Tipo | Etiquetas | Descripción |
|----------|------|------------|-------------|
| `torii_sorafs_provider_range_capability_total` | Calibre | `feature` (`providers`, `supports_sparse_offsets`, `requires_alignment`, `supports_merkle_proof`, `stream_budget`, `transport_hints`) | Fournisseurs annonçant des fonctionnalites de capacite de plage. |
| `torii_sorafs_range_fetch_throttle_events_total` | Mostrador | `reason` (`quota`, `concurrency`, `byte_rate`) | Tentativas de buscar novias en la playa por política. |
| `torii_sorafs_range_fetch_concurrency_current` | Calibre | — | Streams actifs gardes consommant le presupuesto de concurrencia partage. |

Ejemplos de PromQL:

```promql
sum(rate(torii_sorafs_range_fetch_throttle_events_total[5m])) by (reason)
max(torii_sorafs_range_fetch_concurrency_current)
torii_sorafs_provider_range_capability_total
```

Utilice el ordenador de aceleración para confirmar la aplicación de cuotas antes de activarla
les valeurs par defaut de l'orchestrateur multi-source, et alertez quand la concurrence se
acercamiento a los máximos de presupuesto de corrientes para su flota.