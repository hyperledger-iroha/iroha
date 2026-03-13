---
lang: es
direction: ltr
source: docs/portal/docs/da/replication-policy.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::nota مستند ماخذ
ریٹائر ہونے تک دونوں ورژنز کو sincronización رکھیں۔
:::

# Política de replicación de disponibilidad de datos (DA-4)

_حالت: En progreso - Propietarios: Core Protocol WG / Equipo de almacenamiento / SRE_

Canalización de ingesta de DA اب `roadmap.md` (workstream DA-4) میں بیان کردہ ہر blob class
کے لئے objetivos de retención deterministas نافذ کرتا ہے۔ Torii ایسے retención
sobres کو persist کرنے سے انکار کرتا ہے جو llamante فراہم کرے لیکن configurado
política سے coincidencia نہ کریں، تاکہ ہر validador/nodo de almacenamiento مطلوبہ épocas اور
las réplicas pueden retener la intención del remitente پر انحصار کے۔

## Política predeterminada

| Clase de burbuja | Retención en caliente | Retención de frío | Réplicas requeridas | Clase de almacenamiento | Etiqueta de gobernanza |
|------------|---------------|----------------|-------------------|----------------|----------------|
| `taikai_segment` | 24 horas | 14 días | 5 | `hot` | `da.taikai.live` |
| `nexus_lane_sidecar` | 6 horas | 7 días | 4 | `warm` | `da.sidecar` |
| `governance_artifact` | 12 horas | 180 días | 3 | `cold` | `da.governance` |
| _Predeterminado (todas las demás clases)_ | 6 horas | 30 días | 3 | `warm` | `da.default` |

یہ اقدار `torii.da_ingest.replication_policy` میں incrustado ہیں اور تمام
Envíos `/v2/da/ingest` پر لاگو ہوتی ہیں۔ Perfil de retención forzada Torii
کے ساتھ manifiesta دوبارہ لکھتا ہے اور جب los valores de las personas que llaman no coinciden فراہم کرتے ہیں
تو advertencia دیتا ہے تاکہ operadores SDK obsoletos پکڑ سکیں۔### Clases de disponibilidad de Taikai

Manifiestos de enrutamiento Taikai (`taikai.trm`) ایک `availability_class` (`hot`, `warm`,
یا `cold`) declarar کرتے ہیں۔ Torii fragmentación y política de coincidencia نافذ کرتا
ہے تاکہ operadores edición de tabla global کئے بغیر stream کے لحاظ سے recuentos de réplicas
escala کر سکیں۔ Valores predeterminados:

| Clase de disponibilidad | Retención en caliente | Retención de frío | Réplicas requeridas | Clase de almacenamiento | Etiqueta de gobernanza |
|--------------------|---------------|----------------|-------------------|----------------|----------------|
| `hot` | 24 horas | 14 días | 5 | `hot` | `da.taikai.live` |
| `warm` | 6 horas | 30 días | 4 | `warm` | `da.taikai.warm` |
| `cold` | 1 hora | 180 días | 3 | `cold` | `da.taikai.archive` |

Pistas faltantes predeterminadas طور پر `hot` رکھتے ہیں تاکہ transmisiones en vivo سب سے مضبوط
política retener کریں۔ اگر آپ کا red مختلف objetivos استعمال کرتا ہے تو
`torii.da_ingest.replication_policy.taikai_availability` کے ذریعے valores predeterminados
anular کریں۔

## Configuración

یہ política `torii.da_ingest.replication_policy` کے تحت رہتی ہے اور ایک *predeterminado*
plantilla کے ساتھ anulaciones por clase کا matriz فراہم کرتی ہے۔ Identificadores de clase
no distingue entre mayúsculas y minúsculas ہیں اور `taikai_segment`, `nexus_lane_sidecar`,
`governance_artifact`, یا `custom:<u16>` (extensiones aprobadas por la gobernanza) قبول
کرتے ہیں۔ Clases de almacenamiento `hot`, `warm`, یا `cold` قبول کرتی ہیں۔

```toml
[torii.da_ingest.replication_policy.default_retention]
hot_retention_secs = 21600          # 6 h
cold_retention_secs = 2592000       # 30 d
required_replicas = 3
storage_class = "warm"
governance_tag = "da.default"

[[torii.da_ingest.replication_policy.overrides]]
class = "taikai_segment"
[torii.da_ingest.replication_policy.overrides.retention]
hot_retention_secs = 86400          # 24 h
cold_retention_secs = 1209600       # 14 d
required_replicas = 5
storage_class = "hot"
governance_tag = "da.taikai.live"
```Valores predeterminados کے ساتھ چلنے کے لئے block کو ویسا ہی رہنے دیں۔ کسی clase کو apretar
کرنے کے لئے متعلقہ anular la actualización کریں؛ نئی clases کے línea base کو بدلنے کے لئے
`default_retention` editar کریں۔

Clases de disponibilidad de Taikai کو الگ سے anular کیا جا سکتا ہے vía
`torii.da_ingest.replication_policy.taikai_availability`:

```toml
[[torii.da_ingest.replication_policy.taikai_availability]]
availability_class = "cold"
[torii.da_ingest.replication_policy.taikai_availability.retention]
hot_retention_secs = 3600          # 1 h
cold_retention_secs = 15552000     # 180 d
required_replicas = 3
storage_class = "cold"
governance_tag = "da.taikai.archive"
```

## Semántica de aplicación

- Torii `RetentionPolicy` proporcionado por el usuario Perfil aplicado سے reemplaza کرتا ہے
  fragmentación یا emisión manifiesta سے پہلے۔
- Los manifiestos prediseñados y el perfil de retención de discrepancia declaran کریں، `400 schema mismatch`
  کے ساتھ rechazar ہوتے ہیں تاکہ contratos de clientes obsoletos کو debilitar نہ کر سکیں۔
- Anulación del registro de eventos ہوتا ہے (`blob_class`, política enviada versus esperada)
  تاکہ implementación کے دوران personas que llaman no conformes سامنے آئیں۔

Puerta actualizada کے لئے [Plan de ingesta de disponibilidad de datos](ingest-plan.md)
(Lista de verificación de validación) دیکھیں جو cumplimiento de retención کو cubierta کرتا ہے۔

## Flujo de trabajo de nueva replicación (seguimiento de DA-4)

Aplicación de la retención صرف پہلا قدم ہے۔ Operadores کو یہ بھی ثابت کرنا ہوگا کہ en vivo
manifiestos y órdenes de replicación configuradas política کے مطابق رہیں تاکہ SoraFS
Blobs no conformes کو خودکار طور پر volver a replicar کر سکے۔1. **Deriva پر نظر رکھیں۔** Torii
   `overriding DA retention policy to match configured network baseline` emite
   کرتا ہے جب valores de retención obsoletos de la persona que llama بھیجتا ہے۔ اس registro کو
   `torii_sorafs_replication_*` telemetría کے ساتھ جوڑ کر réplica de déficits یا
   redistribuciones retrasadas کو پکڑیں۔
2. **Intento de réplicas en vivo کا diff۔** نیا ayudante de auditoría استعمال کریں:

   ```bash
   cargo xtask da-replication-audit \
     --config configs/iroha/torii.toml \
     --manifest spool/da/manifests/*.json \
     --replication-order artifacts/da/orders/*.norito \
     --json-out artifacts/da/replication_audit.json
   ```

   یہ comando `torii.da_ingest.replication_policy` کو config سے لوڈ کرتا ہے، ہر
   manifiesto (JSON یا Norito) decodificar کرتا ہے، اور اختیاری طور پر `ReplicationOrderV1`
   cargas útiles کو resumen de manifiesto سے coincidencia کرتا ہے۔ خلاصہ دو bandera de condiciones کرتا ہے:

   - `policy_mismatch` - Política aplicada del perfil de retención de manifiesto سے مختلف ہے
     (یہ Torii error de configuración کے بغیر نہیں ہونا چاہئے).
   - `replica_shortfall` - orden de replicación en vivo `RetentionPolicy.required_replicas`
     سے کم réplicas طلب کرتا ہے یا objetivo سے کم asignaciones دیتا ہے۔Estado de salida distinto de cero فعال déficit کی نشاندہی کرتا ہے تاکہ CI/de guardia
   automatización فوراً página کر سکے۔ JSON رپورٹ کو
   Paquete `docs/examples/da_manifest_review_template.md` کے ساتھ adjuntar کریں
   تاکہ El Parlamento vota کے لئے دستیاب ہو۔
3. **Activador de nueva replicación کریں۔** جب auditoría déficit رپورٹ کرے، ایک نیا
   `ReplicationOrderV1` جاری کریں a través de herramientas de gobernanza جو
   [Mercado de capacidad de almacenamiento SoraFS](../sorafs/storage-capacity-marketplace.md)
   میں بیان ہے، اور auditoría دوبارہ چلائیں جب تک conjunto de réplicas convergen نہ کرے۔
   Anulaciones de emergencia کے لئے Salida CLI کو `iroha app da prove-availability` کے
   ساتھ جوڑیں تاکہ SRE وہی resumen اور PDP evidencia referir کر سکیں۔

Cobertura de regresión `integration_tests/tests/da/replication_policy.rs` میں ہے؛
suite `/v2/da/ingest` کو política de retención no coincidente بھیجتی ہے اور verificar کرتی ہے
کہ intención de llamada manifiesta recuperada کے بجائے exposición de perfil forzada کرے۔