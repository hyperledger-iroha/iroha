---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/provider-advert-multisource.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# ملٹی سورس پرووائیڈر anuncios اور شیڈولنگ

یہ صفحہ درج ذیل دستاویز میں موجود especificaciones canónicas کو خلاصہ کرتا ہے:
[`docs/source/sorafs/provider_advert_multisource.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_multisource.md).
Esquemas Norito y registros de cambios portal کی کاپی
Incluye SDK y runbooks SoraFS. رکھتی ہے۔

## Norito esquema میں اضافے

### Capacidad de alcance (`CapabilityType::ChunkRangeFetch`)
- `max_chunk_span` – فی درخواست سب سے بڑا مسلسل span (bytes), `>= 1`.
- `min_granularity` – buscar ریزولوشن، `1 <= قدر <= max_chunk_span`.
- `supports_sparse_offsets` – ایک درخواست میں غیر مسلسل compensaciones کی اجازت دیتا ہے۔
- `requires_alignment` – اگر true ہو تو compensaciones کو `min_granularity` کے مطابق align ہونا لازم ہے۔
- `supports_merkle_proof` – Testigo PoR کی سپورٹ ظاہر کرتا ہے۔

`ProviderCapabilityRangeV1::to_bytes` / `from_bytes` codificación canónica نافذ کرتے ہیں
تاکہ cargas útiles de chismes deterministas رہیں۔

### `StreamBudgetV1`
- Números: `max_in_flight`, `max_bytes_per_sec`, اختیاری `burst_bytes`.
- Reglas de validación (`StreamBudgetV1::validate`):
  - `max_in_flight >= 1`, `max_bytes_per_sec > 0`.
  - `burst_bytes` موجود ہو تو `> 0` اور `<= max_bytes_per_sec` ہونا چاہیے۔

### `TransportHintV1`
- Número: `protocol: TransportProtocol`, `priority: u8` (0-15 pulgadas
  `TransportHintV1::validate` نافذ کرتا ہے).
- Protocolos múltiples: `torii_http_range`, `quic_stream`, `soranet_relay`,
  `vendor_reserved`.
- فی proveedor ڈپلیکیٹ entradas de protocolo مسترد کی جاتی ہیں۔### `ProviderAdvertBodyV1` میں اضافے
- اختیاری `stream_budget: Option<StreamBudgetV1>`.
- اختیاری `transport_hints: Option<Vec<TransportHintV1>>`.
- دونوں فیلڈز اب `ProviderAdmissionProposalV1`, sobres de gobierno, accesorios CLI, اور telemétrico JSON میں بہاؤ رکھتے ہیں۔

## Validación y vinculación de gobernanza

`ProviderAdvertBodyV1::validate` y `ProviderAdmissionProposalV1::validate`
خراب metadatos کو مسترد کرتے ہیں:

- Capacidades de alcance کو decodificar ہونا چاہیے اور límites de amplitud/granularidad پوری کرنی چاہئیں۔
- Presupuestos de flujo/sugerencias de transporte کے لیے `CapabilityType::ChunkRangeFetch` TLV اور lista de sugerencias no vacías لازم ہے۔
- ڈپلیکیٹ protocolos de transporte اور غیر درست prioridades chismes سے پہلے errores de validación پیدا کرتے ہیں۔
- Sobres de admisión `compare_core_fields` کے ذریعے propuesta/anuncios کے metadatos de rango کو comparar کرتے ہیں تاکہ cargas útiles de chismes no coincidentes جلدی مسترد ہوں۔

Cobertura de regresión یہاں موجود ہے:
`crates/sorafs_manifest/src/{provider_advert,provider_admission}.rs`.

## Herramientas y accesorios- Cargas útiles de anuncios del proveedor میں `range_capability`, `stream_budget`, اور `transport_hints` شامل ہونا لازم ہے۔
  `/v1/sorafs/providers` respuestas اور accesorios de admisión کے ذریعے validar کریں؛ Resúmenes JSON, capacidad analizada, presupuesto de flujo, matrices de sugerencias, ingesta de telemetría y datos
- `cargo xtask sorafs-admission-fixtures` Artefactos JSON میں presupuestos de flujo اور sugerencias de transporte دکھاتا ہے تاکہ paneles de control seguimiento de adopción de funciones کر سکیں۔
- `fixtures/sorafs_manifest/provider_admission/` کے تحت accesorios اب شامل کرتے ہیں:
  - anuncios canónicos de múltiples fuentes,
  - `multi_fetch_plan.json` تاکہ SDK suites determinista plan de recuperación multi-peer reproducción کر سکیں۔

## Orquestador اور Torii انضمام

- Torii `/v1/sorafs/providers` metadatos de capacidad de rango analizados کے ساتھ `stream_budget` اور `transport_hints` واپس کرتا ہے۔
  proveedores جب نئی metadatos چھوڑ دیں تو advertencias de degradación چلتے ہیں، اور puntos finales del rango de puerta de enlace براہ راست clientes کے لیے یہی restricciones
- Orquestador de fuentes múltiples (`sorafs_car::multi_fetch`) اب límites de rango, alineación de capacidad, اور presupuestos de flujo کو asignación de trabajo کے دوران hacer cumplir کرتا ہے۔ Pruebas unitarias con fragmentos demasiado grandes, búsqueda dispersa y escenarios de limitación.
- `sorafs_car::multi_fetch` señales de degradación (fallas de alineación, solicitudes limitadas) flujo کرتا ہے تاکہ operadores دیکھ سکیں کہ پلاننگ کے دوران مخصوص proveedores کیوں skip ہوئے۔

## Referencia de telemetríaTorii کی instrumentación de búsqueda de rango **SoraFS Observabilidad de búsqueda** Panel de control Grafana
(`dashboards/grafana/sorafs_fetch_observability.json`) اور متعلقہ reglas de alerta
(`dashboards/alerts/sorafs_fetch_rules.yml`) کو feed کرتی ہے۔

| Métrica | Tipo | Etiquetas | Descripción |
|--------|------|--------|-------------|
| `torii_sorafs_provider_range_capability_total` | Calibre | `feature` (`providers`, `supports_sparse_offsets`, `requires_alignment`, `supports_merkle_proof`, `stream_budget`, `transport_hints`) | Las funciones de capacidad de alcance anuncian proveedores کرنے والے۔ |
| `torii_sorafs_range_fetch_throttle_events_total` | Mostrador | `reason` (`quota`, `concurrency`, `byte_rate`) | Política کے مطابق intentos de recuperación de rango limitado ۔ |
| `torii_sorafs_range_fetch_concurrency_current` | Calibre | — | Presupuesto de simultaneidad compartido استعمال کرنے والی transmisiones protegidas activas۔ |

Ejemplos de fragmentos de PromQL:

```promql
sum(rate(torii_sorafs_range_fetch_throttle_events_total[5m])) by (reason)
max(torii_sorafs_range_fetch_concurrency_current)
torii_sorafs_provider_range_capability_total
```

Aplicación de cuotas کی تصدیق کے لیے contador de limitación استعمال کریں اس سے پہلے کہ valores predeterminados del orquestador de múltiples fuentes فعال کریں، اور جب concurrencia آپ کے flota کے presupuesto máximo de transmisión کے قریب ہو تو alerta کریں۔