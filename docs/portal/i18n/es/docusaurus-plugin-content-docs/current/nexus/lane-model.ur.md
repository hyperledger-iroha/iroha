---
lang: es
direction: ltr
source: docs/portal/docs/nexus/lane-model.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: modelo-carril-nexus
título: modelo de carril Nexus
descripción: Sora Nexus کے لئے carriles کی منطقی taxonomía, configuración geométrica, اور fusión de estado mundial کے اصول۔
---

# Nexus modelo de carril y partición WSV

> **Estado:** Entregable NX-1: taxonomía de carriles, geometría de configuración, diseño de almacenamiento نفاذ کے لئے تیار ہیں۔  
> **Propietarios:** Nexus Grupo de Trabajo Principal, Grupo de Trabajo de Gobernanza  
> **Referencia de la hoja de ruta:** `roadmap.md` میں NX-1

یہ پورٹل صفحہ canonical `docs/source/nexus_lanes.md` breve کی عکاسی کرتا ہے تاکہ Sora Nexus آپریٹرز، Propietarios de SDK اور revisores mono-repo tree میں جائے بغیر guía de carril پڑھ سکیں۔ ہدفی arquitectura estado mundial کی determinismo برقرار رکھتا ہے جبکہ انفرادی espacios de datos (carriles) کو públicos یا conjuntos de validadores privados کے ساتھ cargas de trabajo aisladas چلانے دیتا ہے۔

## Conceptos- **Carril:** Libro mayor Nexus کا منطقی shard, اپنے conjunto de validadores اور ejecución pendiente کے ساتھ۔ اسے ایک مستحکم `LaneId` سے شناخت کیا جاتا ہے۔
- **Espacio de datos:** segmento de gobernanza, carriles, cumplimiento, enrutamiento, políticas de liquidación, políticas de liquidación
- **Lane Manifest:** metadatos controlados por la gobernanza, validadores, política DA, token de gas, reglas de liquidación, permisos de enrutamiento
- **Compromiso global:** ایک prueba جو carril جاری کرتی ہے، نئے raíces estatales, datos de liquidación, اور transferencias opcionales entre carriles کا خلاصہ دیتی ہے۔ Compromisos globales de anillo NPoS کو ترتیب دیتا ہے۔

## Taxonomía de carriles

Tipos de carriles اپنی visibilidad, superficie de gobernanza اور ganchos de asentamiento کو canonical طور پر بیان کرتے ہیں۔ geometría de configuración (`LaneConfig`) atributos, captura, nodos, SDK, herramientas, lógica personalizada, diseño, configuración| Tipo de carril | Visibilidad | Membresía del validador | Exposición al WSV | Gobernanza por defecto | Política de liquidación | Uso típico |
|-----------|------------|----------------------|--------------|--------------------|-------------------|-------------|
| `default_public` | público | Sin permiso (participación global) | Réplica de estado completo | SORA Parlamento | `xor_global` | Libro mayor público de referencia |
| `public_custom` | público | Sin permiso o controlado por participación | Réplica de estado completo | Módulo ponderado por participación | `xor_lane_weighted` | Aplicaciones públicas de alto rendimiento |
| `private_permissioned` | restringido | Conjunto de validadores fijos (aprobado por la gobernanza) | Compromisos y pruebas | Consejo federado | `xor_hosted_custody` | CBDC, cargas de trabajo del consorcio |
| `hybrid_confidential` | restringido | Membresía mixta; envuelve pruebas ZK | Compromisos + divulgación selectiva | Módulo de dinero programable | `xor_dual_fund` | Dinero programable que preserva la privacidad |

تمام tipos de carriles کو درج ذیل declaran کرنا ہوگا:

- Alias de espacio de datos - انسان کے لئے پڑھنے کے قابل agrupación جو políticas de cumplimiento کو enlazar کرتی ہے۔
- Identificador de gobernanza - Identificador ایسا جو `Nexus.governance.modules` کے ذریعے resolver ہوتا ہے۔
- Identificador de liquidación - Identificador de liquidación, enrutador de liquidación, buffers XOR, débito, کرنے کے لئے استعمال کرتا ہے۔
- Metadatos de telemetría opcionales (descripción, contacto, dominio comercial) جو `/status` اور paneles کے ذریعے ظاہر ہوتے ہیں۔## Geometría de configuración de carril (`LaneConfig`)

`LaneConfig` Catálogo de carriles validado سے geometría de tiempo de ejecución derivada ہے۔ یہ manifiestos de gobernanza کو reemplazan نہیں کرتا؛ اس کے بجائے ہر carril configurado کے لئے identificadores de almacenamiento deterministas اور sugerencias de telemetría فراہم کرتا ہے۔

```text
LaneConfigEntry {
    lane_id: LaneId,           // stable identifier
    alias: String,             // human-readable alias
    slug: String,              // sanitised alias for file/metric keys
    kura_segment: String,      // Kura segment directory: lane_{id:03}_{slug}
    merge_segment: String,     // Merge-ledger segment: lane_{id:03}_merge
    key_prefix: [u8; 4],       // Big-endian LaneId prefix for WSV key spaces
    shard_id: ShardId,         // WSV/Kura shard binding (defaults to lane_id)
    visibility: LaneVisibility,// public vs restricted lanes
    storage_profile: LaneStorageProfile,
    proof_scheme: DaProofScheme,// DA proof policy (merkle_sha256 default)
}
```

- `LaneConfig::from_catalog` geometría کو دوبارہ cálculo کرتا ہے جب carga de configuración ہو (`State::set_nexus`).
- Alias ​​کو babosas minúsculas میں desinfectar کیا جاتا ہے؛ مسلسل caracteres no alfanuméricos `_` میں colapso ہوتے ہیں۔ اگر alias babosa vacía دے تو ہم `lane{id}` respaldo کرتے ہیں۔
- `shard_id` clave de metadatos del catálogo `da_shard_id` سے derivar ہوتا ہے (predeterminado `lane_id`) اور diario de cursor de fragmento persistente کو unidad کرتا ہے تاکہ reinicios/reharding میں reproducción DA determinista رہے۔
- Prefijos clave یہ یقینی بناتے ہیں کہ WSV rangos de claves por carril کو جدا رکھے، چاہے backend compartido ہو۔
- Los nombres del segmento Kura alojan کے درمیان determinista ہوتے ہیں؛ auditores بغیر herramientas a medida کے directorios de segmentos اور verificación cruzada de manifiestos کر سکتے ہیں۔
- Fusionar segmentos (`lane_{id:03}_merge`) اس lane کے últimas raíces de sugerencias de fusión اور compromisos estatales globales محفوظ کرتے ہیں۔

## Partición del estado mundial- Espacios de estado por carril de estado mundial lógico Nexus کا unión ہے۔ Persisten los carriles públicos en estado completo کرتی ہیں؛ carriles privados/confidenciales Merkle/raíces de compromiso کو fusionar libro mayor میں export کرتی ہیں۔
- Almacenamiento MV ہر clave کو `LaneConfigEntry::key_prefix` کے Prefijo de 4 bytes سے prefijo کرتا ہے، جس سے `[00 00 00 01] ++ PackedKey` جیسے teclas بنتے ہیں۔
- Entradas de tablas compartidas (cuentas, activos, activadores, registros de gobierno) کو prefijo de carril کے حساب سے grupo کرتی ہیں، جس سے escaneos de rango determinista رہتے ہیں۔
- Metadatos del libro mayor de fusión, diseño, espejo,: carril `lane_{id:03}_merge`, raíces de sugerencia de fusión, raíces de estado global reducidas, retención dirigida. desalojo ممکن ہوتی ہے۔
- Índices entre carriles (alias de cuentas, registros de activos, manifiestos de gobierno) Prefijos de carriles explícitos almacenan entradas de operadores کرتے ہیں تاکہ جلد concilian کر سکیں۔
- **Política de retención** - carriles públicos مکمل cuerpos de bloqueo رکھتی ہیں؛ carriles exclusivos puntos de control کے بعد پرانے cuerpos compactos کر سکتی ہیں کیونکہ compromisos autorizados ہیں۔ Carriles confidenciales diarios de texto cifrado کو segmentos dedicados میں رکھتی ہیں تاکہ دوسرے bloque de cargas de trabajo نہ ہوں۔
- **Herramientas** - utilidades de mantenimiento (`kagami`, comandos de administración CLI) Las métricas exponen las etiquetas Prometheus بناتے یا Archivo de segmentos de Kura کرتے y slugged namespace refer کرنا چاہیے۔

## Enrutamiento y API- Puntos finales Torii REST/gRPC opcionales `lane_id` قبول کرتے ہیں؛ عدم موجودگی `lane_default` کو ظاہر کرتی ہے۔
- Selectores de carril SDK فراہم کرتے ہیں اور alias fáciles de usar کو catálogo de carril کے ذریعے `LaneId` سے mapa کرتے ہیں۔
- Reglas de enrutamiento catálogo validado پر operar کرتے ہیں اور carril اور espacio de datos دونوں منتخب کر سکتے ہیں۔ `LaneConfig` paneles de control y registros کے لئے alias compatibles con telemetría فراہم کرتا ہے۔

## Liquidación y tarifas

- Conjunto de validadores globales de ہر carriles کو tarifas XOR ادا کرتی ہے۔ Tokens de gas nativos de carriles جمع کر سکتی ہیں مگر compromisos کے ساتھ XOR equivalentes de depósito en garantía کرنا لازم ہے۔
- Pruebas de liquidación میں monto, metadatos de conversión, اور prueba de depósito en garantía شامل ہوتے ہیں (مثلا bóveda de tarifas globales کو transferencia) ۔
- Amortiguadores del enrutador de liquidación unificado (NX-3) کو انہی prefijos de carril کے ساتھ débito کرتا ہے، لہذا geometría de almacenamiento de telemetría de liquidación کے ساتھ alinear ہوتی ہے۔

## Gobernanza

- Módulo de gobernanza de carriles کو catálogo کے ذریعے declarar کرتی ہیں۔ `LaneConfigEntry` اصل alias اور slug ساتھ رکھتا ہے تاکہ telemetría اور audit trails legibles رہیں۔
- Manifiestos de carril firmados del registro Nexus تقسیم کرتا ہے جن میں `LaneId`, enlace de espacio de datos, identificador de gobernanza, identificador de liquidación اور metadatos شامل ہوتے ہیں۔
- Políticas de gobernanza de ganchos de actualización de tiempo de ejecución (`gov_upgrade_id` predeterminado) نافذ کرتے رہتے ہیں اور puente de telemetría (eventos `nexus.config.diff`) کے ذریعے registro de diferencias کرتے ہیں۔## Telemetría y estado

- `/status` alias de carril, enlaces de espacio de datos, identificadores de gobernanza, perfiles de liquidación, exposición, catálogo, `LaneConfig`, derivación,
- Métricas del programador (`nexus_scheduler_lane_teu_*`) alias de carril/slugs دکھاتے ہیں تاکہ backlog de operadores اور TEU presión کو جلد mapa کر سکیں۔
- Entradas de carril derivadas `nexus_lane_configured_total` کی تعداد شمار کرتا ہے اور configuración تبدیل ہونے پر دوبارہ Compute ہوتا ہے۔ Geometría del carril de telemetría بدلنے پر las diferencias con signo emiten کرتی ہے۔
- El backlog del espacio de datos mide los metadatos de alias/descripción

## Configuración y tipos Norito

- `LaneCatalog`, `LaneConfig`, y `DataSpaceCatalog` `iroha_data_model::nexus` Incluye múltiples manifiestos y SDK y estructuras de formato Norito فراہم کرتے ہیں۔
- `LaneConfig` `iroha_config::parameters::actual::Nexus` میں رہتا ہے اور catálogo سے خودکار طور پر derivar ہوتا ہے؛ اسے Codificación Norito کی ضرورت نہیں کیونکہ یہ ayudante de tiempo de ejecución interno ہے۔
- Línea declarativa de configuración orientada al usuario (`iroha_config::parameters::user::Nexus`) y descriptores de espacio de datos کو قبول کرتی رہتی ہے؛ analizar اب geometría derivar کرتا ہے اور alias no válidos یا duplicar ID de carril کو rechazar کرتا ہے۔

## Trabajo excepcional- actualizaciones del enrutador de liquidación (NX-3) کو نئی geometría کے ساتھ integrar کریں تاکہ XOR buffer débitos اور recibos carril slug کے مطابق etiqueta ہوں۔
- Herramientas de administración کو extender کریں تاکہ lista de familias de columnas ہوں، carriles retirados compacto ہوں، اور slugged namespace کے ساتھ registros de bloqueo por carril inspeccionar ہوں۔
- Algoritmo de fusión (orden, poda, detección de conflictos) finaliza la repetición entre carriles y los accesorios de regresión
- Listas blancas/listas negras اور políticas de dinero programables کے لئے ganchos de cumplimiento شامل کریں (NX-12 میں ٹریک)۔

---

*یہ صفحہ NX-2 سے NX-18 کے اترنے کے ساتھ Seguimientos de NX-1 کو ٹریک کرتا رہے گا۔ براہ کرم کھلے سوالات `roadmap.md` یا rastreador de gobernanza میں سامنے لائیں تاکہ پورٹل documentos canónicos کے ساتھ alineados رہے۔*