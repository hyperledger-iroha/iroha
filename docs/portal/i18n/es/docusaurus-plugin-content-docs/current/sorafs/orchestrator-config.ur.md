---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/orchestrator-config.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: configuración del orquestador
título: SoraFS آرکسٹریٹر کنفیگریشن
sidebar_label: آرکسٹریٹر کنفیگریشن
descripción: ملٹی سورس fetch آرکسٹریٹر کو کنفیگر کریں، ناکامیوں کی تشریح کریں، اور ٹیلیمیٹری آؤٹ پٹ ڈی بگ کریں۔
---

:::nota مستند ماخذ
یہ صفحہ `docs/source/sorafs/developer/orchestrator.md` کی عکاسی کرتا ہے۔ جب تک پرانی ڈاکیومنٹیشن ریٹائر نہ ہو جائے، دونوں نقول کو ہم آہنگ رکھیں۔
:::

# ملٹی سورس buscar آرکسٹریٹر گائیڈ

SoraFS کا ملٹی سورس buscar آرکسٹریٹر گورننس سپورٹڈ anuncios میں شائع شدہ
پرووائیڈرز سیٹ سے ڈٹرمنسٹک اور متوازی ڈاؤن لوڈز چلاتا ہے۔ یہ گائیڈ وضاحت کرتی
ہے کہ آرکسٹریٹر کیسے کنفیگر کیا جائے، رول آؤٹس کے دوران کون سی ناکامی سگنلز
متوقع ہیں، اور کون سے ٹیلیمیٹری اسٹریمز صحت کے اشارے ظاہر کرتے ہیں۔

## 1. کنفیگریشن اوور ویو

آرکسٹریٹر تین سورسز کی کنفیگریشن کو مرج کرتا ہے:

| سورس | مقصد | نوٹس |
|------|------|------|
| `OrchestratorConfig.scoreboard` | Pesos de pesas کے لیے Marcador JSON محفوظ کرتا ہے۔ | `crates/sorafs_car::scoreboard::ScoreboardConfig` پر مبنی۔ |
| `OrchestratorConfig.fetch` | رن ٹائم حدود لگاتا ہے (reintentar presupuestos, límites de concurrencia, alternancias de verificación)۔ | `crates/sorafs_car::multi_fetch` میں `FetchOptions` سے میپ ہوتا ہے۔ |
| CLI / SDK پیرامیٹرز | pares کی تعداد محدود کرتے ہیں، regiones de telemetría لگاتے ہیں، اور denegar/impulsar پالیسیز ظاہر کرتے ہیں۔ | `sorafs_cli fetch` یہ banderas براہ راست دیتا ہے؛ SDKs انہیں `OrchestratorConfig` کے ذریعے پاس کرتے ہیں۔ |`crates/sorafs_orchestrator::bindings` Asistentes JSON completos en formato Norito
JSON میں serializar کرتے ہیں، جس سے اسے Enlaces SDK اور آٹومیشن میں پورٹیبل
بنایا جا سکے۔

### 1.1 Versión JSON کنفیگریشن

```json
{
  "scoreboard": {
    "latency_cap_ms": 6000,
    "weight_scale": 12000,
    "telemetry_grace_secs": 900,
    "persist_path": "/var/lib/sorafs/scoreboards/latest.json"
  },
  "fetch": {
    "verify_lengths": true,
    "verify_digests": true,
    "retry_budget": 4,
    "provider_failure_threshold": 3,
    "global_parallel_limit": 8
  },
  "telemetry_region": "iad-prod",
  "max_providers": 6,
  "transport_policy": "soranet_first"
}
```

فائل کو معمول کے `iroha_config` capas (`defaults/`, usuario, real) کے ذریعے
محفوظ کریں تاکہ ڈٹرمنسٹک implementaciones تمام نوڈز پر ایک جیسے límites استعمال کریں۔
Lanzamiento de SNNet-5a سے ہم آہنگ reserva directa solo پروفائل کے لیے
`docs/examples/sorafs_direct_mode_policy.json`
`docs/source/sorafs/direct_mode_pack.md` دیکھیں۔

### 1.2 Anulaciones de cumplimiento

SNNet-9 آرکسٹریٹر میں گورننس ڈرائیون کمپلائنس شامل کرتا ہے۔ Norito JSON
کنفیگریشن میں ایک نیا `compliance` آبجیکٹ carve-outs محفوظ کرتا ہے جو fetch
پائپ لائن کو solo directo موڈ پر مجبور کرتے ہیں:

```json
"compliance": {
  "operator_jurisdictions": ["US", "JP"],
  "jurisdiction_opt_outs": ["US"],
  "blinded_cid_opt_outs": [
    "C6B434E5F23ABD318F01FEDB834B34BD16B46E0CC44CD70536233A632DFA3828"
  ],
  "audit_contacts": ["mailto:compliance@example.org"]
}
```- `operator_jurisdictions` ISO‑3166 alpha‑2 کوڈز ڈکلیئر کرتا ہے جہاں یہ
  آرکسٹریٹر انسٹینس آپریٹ کرتا ہے۔ analizando کے دوران کوڈز mayúsculas میں normalizar
  ہوتے ہیں۔
- `jurisdiction_opt_outs` گورننس رجسٹر کو espejo کرتا ہے۔ جب کوئی آپریٹر
  jurisdicción فہرست میں ہو، آرکسٹریٹر `transport_policy=direct-only` نافذ
  کرتا ہے اور motivo de reserva `compliance_jurisdiction_opt_out` emite کرتا ہے۔
- Resúmenes de manifiesto `blinded_cid_opt_outs` (CID ciegos, hexadecimal en mayúsculas) لسٹ کرتا
  ہے۔ coincidencia de cargas útiles programación directa únicamente مجبور کرتی ہیں اور ٹیلیمیٹری میں
  `compliance_blinded_cid_opt_out` respaldo دکھاتی ہیں۔
- `audit_contacts` ان URI کو ریکارڈ کرتا ہے جنہیں گورننس آپریٹرز کے GAR
  libros de jugadas میں شائع کرنے کی توقع کرتی ہے۔
- `attestations` کمپلائنس کی سائنڈ پیکٹس محفوظ کرتا ہے۔ ہر entrada ایک opcional
  `jurisdiction` (ISO-3166 alfa-2), `document_uri`, `digest_hex` de 64 caracteres,
  marca de tiempo de emisión `issued_at_ms`, y opcional `expires_at_ms` بیان کرتی ہے۔
  یہ artefactos آرکسٹریٹر کی lista de verificación de auditoría میں جاتے ہیں تاکہ herramientas de gobernanza
  anula کو سائنڈ دستاویزات سے جوڑ سکے۔

کمپلائنس بلاک کو معمول کے کنفیگریشن capas کے ذریعے دیں تاکہ آپریٹرز کو
ڈٹرمنسٹک anula ملیں۔ آرکسٹریٹر کمپلائنس کو sugerencias del modo de escritura کے _بعد_
لاگو کرتا ہے: اگر SDK `upload-pq-only` بھی مانگے تو jurisdicción یا manifiesto
exclusión voluntaria پھر بھی transporte directo پر لے جاتے ہیں اور جب کوئی compatible
proveedor نہ ہو تو فوری فیل کرتے ہیں۔Catálogos de exclusión voluntaria de Canonical `governance/compliance/soranet_opt_outs.json` میں
موجود ہیں؛ Lanzamientos etiquetados del Consejo de Gobernanza کے ذریعے اپڈیٹس شائع کرتی ہے۔
Atestaciones سمیت مکمل مثال `docs/examples/sorafs_compliance_policy.json` میں
دستیاب ہے، اور آپریشنل پروسیس
[Guía de cumplimiento de GAR](../../../source/soranet/gar_compliance_playbook.md)
میں درج ہے۔

### 1.3 CLI y mandos SDK| Bandera / Campo | اثر |
|--------------|-----|
| `--max-peers` / `OrchestratorConfig::with_max_providers` | اس بات کی حد مقرر کرتا ہے کہ کتنے پرووائیڈر marcador فلٹر سے گزر سکیں۔ `None` پر سیٹ کریں تاکہ تمام elegible پرووائیڈرز استعمال ہوں۔ |
| `--retry-budget` / `FetchOptions::per_chunk_retry_limit` | Reintento de fragmentos حد مقرر کرتا ہے۔ حد سے تجاوز `MultiSourceError::ExhaustedRetries` اٹھاتا ہے۔ |
| `--telemetry-json` | instantáneas de latencia/fallo کو generador de marcadores میں inyectar کرتا ہے۔ `telemetry_grace_secs` سے پرانی ٹیلیمیٹری پرووائیڈرز کو no elegible کر دیتی ہے۔ |
| `--scoreboard-out` | حساب شدہ marcador (elegible + no elegible) کو پوسٹ رن معائنے کے لیے محفوظ کرتا ہے۔ |
| `--scoreboard-now` | anulación de marca de tiempo del marcador (segundos de Unix) کرتا ہے تاکہ capturas de dispositivos deterministas رہیں۔ |
| `--deny-provider` / gancho de política de puntuación | anuncios ڈیلیٹ کیے بغیر پرووائیڈرز کو determinista طور پر excluir کرتا ہے۔ فوری بلیک لسٹنگ کے لیے مفید۔ |
| `--boost-provider=name:delta` | گورننس pesos برقرار رکھتے ہوئے پرووائیڈر کے créditos ponderados de todos contra todos ایڈجسٹ کرتا ہے۔ |
| `--telemetry-region` / `OrchestratorConfig::with_telemetry_region` | میٹرکس اور registros estructurados کو لیبل دیتا ہے تاکہ ڈیش بورڈز جغرافیہ یا rollout wave کے لحاظ سے pivot کر سکیں۔ |
| `--transport-policy` / `OrchestratorConfig::with_transport_policy` | اب ڈیفالٹ `soranet-first` ہے کیونکہ ملٹی سورس آرکسٹریٹر baseline ہے۔ degradar la directiva de cumplimiento کے لیے `direct-only` استعمال کریں، اور PQ-only pilots کے لیے `soranet-strict` محفوظ رکھیں؛ anulaciones de cumplimiento پھر بھی سخت حد ہیں۔ |SoraNet-first اب envío predeterminado ہے، اور reversiones کو متعلقہ Bloqueador SNNet کا
حوالہ دینا چاہیے۔ SNNet-4/5/5a/5b/6a/7/8/12/13 کے گریجویٹ ہونے کے بعد گورننس
postura requerida کو مزید سخت کرے گی (`soranet-strict` کی طرف)؛ تب تک صرف
anulaciones impulsadas por incidentes کو `direct-only` ترجیح دینی چاہیے، اور انہیں implementación
iniciar sesión میں ریکارڈ کرنا لازمی ہے۔

Estas son las banderas `sorafs_cli fetch` y las orientadas al desarrollador `sorafs_fetch`.
میں `--` اسٹائل sintaxis قبول کرتے ہیں۔ SDK یہی آپشنز compiladores mecanografiados کے ذریعے
ایکسپوز کرتے ہیں۔

### 1.4 Caché de guardia مینجمنٹ

CLI اب SoraNet guard selector کو cableado کرتی ہے تاکہ آپریٹرز SNNet-5 کے مکمل
despliegue de transporte سے پہلے relés de entrada کو determinista انداز میں pin کر سکیں۔
تین نئے banderas اس ورک فلو کو کنٹرول کرتے ہیں:

| Bandera | مقصد |
|------|------|
| `--guard-directory <PATH>` | ایک JSON فائل کی طرف اشارہ کرتا ہے جو تازہ ترین consenso de retransmisión بیان کرتی ہے (نیچے subconjunto دکھایا گیا ہے)۔ directorio پاس کرنے سے buscar سے پہلے guardar caché actualizar ہوتی ہے۔ |
| `--guard-cache <PATH>` | Norito codificado `GuardSet` محفوظ کرتا ہے۔ بعد کے رنز بغیر نئے directorio کے بھی reutilización de caché کرتے ہیں۔ |
| `--guard-target <COUNT>` / `--guard-retention-days <DAYS>` | guardias de entrada کی تعداد (ڈیفالٹ 3) اور ventana de retención (ڈیفالٹ 30 دن) کے لیے anulaciones opcionales۔ |
| `--guard-cache-key <HEX>` | clave opcional de 32 bytes, Blake3 MAC, guardar caché, etiqueta, reutilizar, verificar, verificar |Cargas útiles del directorio de guardia ایک esquema compacto استعمال کرتی ہیں:

Bandera `--guard-directory` y carga útil `GuardDirectorySnapshotV2` codificada con Norito
esperar کرتا ہے۔ instantánea binaria میں شامل ہے:

- `version` — esquema ورژن (فی الحال `2`).
- `directory_hash`, `published_at_unix`, `valid_after_unix`, `valid_until_unix` —
  metadatos de consenso جو ہر certificado integrado سے coincidencia کرنا ضروری ہے۔
- `validation_phase` — puerta de política de certificado (`1` = firma única Ed25519
  permitir, `2` = se prefieren firmas duales, `3` = se requieren firmas duales).
- `issuers`: emisores de gobernanza de `fingerprint`, `ed25519_public`, `mldsa65_public`.
  huellas dactilares اس طرح بنائے جاتے ہیں:
  `BLAKE3("soranet.src.v2.issuer" || ed25519 || u32(len(ml-dsa)) || ml-dsa)`.
- `relays` — Paquetes SRCv2 کی فہرست (salida `RelayCertificateBundleV2::to_cbor()`).
  ہر descriptor de retransmisión de paquete, indicadores de capacidad, política ML-KEM, اور Ed25519/ML-DSA-65
  firmas duales رکھتا ہے۔

Paquete CLI کو claves de emisor declaradas کے خلاف verificar کرتی ہے اور پھر directorio کو
instantáneas لازمی ہیں۔`--guard-directory` کے ساتھ CLI چلائیں تاکہ تازہ ترین consenso y caché de caché
کے ساتھ fusionar کیا جا سکے۔ selector de guardias fijadas کو برقرار رکھتا ہے جو ابھی
ventana de retención کے اندر اور directorio میں elegible ہیں؛ نئے relés caducados
entradas کو reemplazan کرتے ہیں۔ کامیاب buscar کے بعد caché actualizado `--guard-cache`
کے camino پر لکھ دی جاتی ہے، جس سے اگلی سیشنز determinista رہتی ہیں۔ SDK یہی
comportamiento `GuardSelector::select(&RelayDirectory, existing_guard_set, now_unix_secs)`
کو کال کر کے اور نتیجہ `GuardSet` کو `SorafsGatewayFetchOptions` میں پاس کر کے
reproducir کر سکتی ہیں۔

Selector `ml_kem_public_hex` کو Protectores con capacidad PQ کو ترجیح دینے دیتا ہے جب
Lanzamiento de SNNet-5 جاری ہو۔ alterna etapa (`anon-guard-pq`, `anon-majority-pq`,
`anon-strict-pq`) اب relés clásicos کو خودکار طور پر degradar کرتے ہیں: جب PQ
guard دستیاب ہو تو selector اضافی pines clásicos ہٹا دیتا ہے تاکہ اگلی سیشنز
apretones de manos híbridos کو ترجیح دیں۔ Resúmenes de CLI/SDK نتیجہ خیز mix کو
`anonymity_status`/`anonymity_reason`, `anonymity_effective_policy`,
`anonymity_pq_selected`, `anonymity_classical_selected`, `anonymity_pq_ratio`,
`anonymity_classical_ratio` اور متعلقہ candidato/déficit/delta de oferta فیلڈز کے
ذریعے ظاہر کرتے ہیں، جس سے apagones اور retrocesos clásicos واضح ہو جاتے ہیں۔Directorios de guardia اب `certificate_base64` کے ذریعے مکمل SRCv2 paquete incrustado کر
سکتی ہیں۔ آرکسٹریٹر ہر paquete کو decodificar کرتا ہے، Ed25519/ML-DSA firmas کو
volver a validar el certificado analizado y proteger la caché
ہے۔ جب certificado موجود ہو تو وہ claves PQ, preferencias de protocolo de enlace, اور ponderación
کا fuente canónica بن جاتا ہے؛ los certificados caducados descartan کر دیے جاتے ہیں اور
gestión کے ذریعے propagar ہوتے ہیں اور `telemetry::sorafs.guard` اور
`telemetry::sorafs.circuit` کے ذریعے superficie ہوتے ہیں، جو ventana de validez،
suites de protocolo de enlace, firmas duales کے مشاہدے کو ریکارڈ کرتے ہیں۔

Ayudantes de CLI, instantáneas y editores, sincronización de archivos:

```bash
sorafs_cli guard-directory fetch \
  --url https://directory.soranet.dev/mainnet_snapshot.norito \
  --output ./state/guard_directory.norito \
  --expected-directory-hash <directory-hash-hex>

sorafs_cli guard-directory verify \
  --path ./state/guard_directory.norito \
  --expected-directory-hash <directory-hash-hex>
```

`fetch` Instantánea SRCv2 کو ڈاؤن لوڈ اور verificar کر کے ڈسک پر لکھتا ہے، جبکہ `verify`
دوسری ٹیموں سے آئے artefactos کے لیے canalización de validación دوبارہ چلاتا ہے اور
Salida del selector de protección CLI/SDK سے coincidencia کرنے y resumen JSON emitir کرتا ہے۔

### 1.5 Administrador del ciclo de vida del circuito

جب directorio de retransmisión اور guard cache دونوں فراہم ہوں تو آرکسٹریٹر ciclo de vida del circuito
administrador کو فعال کرتا ہے تاکہ ہر buscar سے پہلے circuitos SoraNet کو preconstrucción اور
renovar کیا جائے۔ کنفیگریشن `OrchestratorConfig`
(`crates/sorafs_orchestrator/src/lib.rs:305`) میں دو نئے فیلڈز کے ذریعے ہوتی ہے:- `relay_directory`: instantánea del directorio SNNet-3 con saltos intermedios/de salida
  determinista طریقے سے منتخب ہوں۔
- `circuit_manager`: Opcional کنفیگریشن (ڈیفالٹ طور پر habilitado) y circuito TTL
  کو کنٹرول کرتی ہے۔

Norito JSON y `circuit_manager` están escritos en:

```json
"circuit_manager": {
  "enabled": true,
  "circuit_ttl_secs": 900
}
```

Datos del directorio SDKs کو
`SorafsGatewayFetchOptions::relay_directory`
(`crates/iroha/src/client.rs:320`) کے ذریعے adelante کرتی ہیں، اور CLI اسے اس وقت
خودکار طور پر cableado کرتی ہے جب `--guard-directory` دیا جائے
(`crates/iroha_cli/src/commands/sorafs.rs:365`).

Administrador de circuitos کو اس وقت renovar کرتا ہے جب proteger metadatos (punto final, clave PQ یا
marca de tiempo fijada) بدل جائے یا TTL ختم ہو۔ ہر buscar سے پہلے invocar ہونے والا
ayudante `refresh_circuits` (`crates/sorafs_orchestrator/src/lib.rs:1346`) `CircuitEvent`
لاگز emite کرتا ہے تاکہ آپریٹرز ciclo de vida فیصلوں کو ٹریس کر سکیں۔ prueba de remojo
`circuit_manager_latency_soak_remains_stable_across_rotations`
(`crates/sorafs_orchestrator/src/soranet.rs:1479`) تین rotaciones de guardia میں مستحکم
latencia دکھاتا ہے؛ رپورٹ `docs/source/soranet/reports/circuit_stability.md:1` میں
دیکھیں۔

### 1.6 Modo proxy QUIC

آرکسٹریٹر ایک لوکل QUIC proxy بھی چلا سکتا ہے تاکہ extensiones del navegador اور SDK
adaptadores کو certificados یا guardar claves de caché administrar نہ کرنا پڑیں۔ bucle invertido de proxy
ایڈریس پر bind ہوتا ہے، Conexiones QUIC ختم کرتا ہے، اور کلائنٹ کو Norito
manifiesto واپس کرتا ہے جو certificado اور clave de caché de protección opcional بیان کرتا ہے۔
proxy کے emite کردہ eventos de transporte کو `sorafs_orchestrator_transport_events_total`
میں شمار کیا جاتا ہے۔

Nombre del archivo JSON `local_proxy` para habilitar el proxy:

```json
"local_proxy": {
  "bind_addr": "127.0.0.1:9443",
  "telemetry_label": "dev-proxy",
  "guard_cache_key_hex": "0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF",
  "emit_browser_manifest": true,
  "proxy_mode": "bridge",
  "prewarm_circuits": true,
  "max_streams_per_circuit": 64,
  "circuit_ttl_hint_secs": 300,
  "norito_bridge": {
    "spool_dir": "./storage/streaming/soranet_routes",
    "extension": "norito"
  },
  "kaigi_bridge": {
    "spool_dir": "./storage/streaming/soranet_routes",
    "extension": "norito",
    "room_policy": "public"
  }
}
```- `bind_addr` کنٹرول کرتا ہے کہ proxy کہاں escuchar کرے (پورٹ `0` سے efímero
  پورٹ مانگیں)۔
- `telemetry_label` میٹرکس میں propagar ہوتا ہے تاکہ paneles proxy اور
  buscar sesiones میں فرق کر سکیں۔
- `guard_cache_key_hex` (اختیاری) proxy کو وہی caché de protección con clave دکھانے دیتا ہے
  جو CLI/SDK استعمال کرتے ہیں، تاکہ extensiones del navegador sincronización رہیں۔
- `emit_browser_manifest` اس بات کو alternar کرتا ہے کہ apretón de manos ایسی manifiesto
  واپس کرے جسے extensiones ذخیرہ/verificar کر سکیں۔
- `proxy_mode` طے کرتا ہے کہ proxy مقامی طور پر bridge کرے (`bridge`) یا صرف
  Los metadatos emiten SDK y circuitos SoraNet (`metadata-only`).
  ڈیفالٹ `bridge` ہے؛ جب صرف manifiesto دینا ہو تو `metadata-only` سیٹ کریں۔
- `prewarm_circuits`, `max_streams_per_circuit`, `circuit_ttl_hint_secs` Acero
  sugerencias دیتے ہیں تاکہ براؤزر flujos paralelos بجٹ کر سکے اور proxy کے circuito
  reutilizar رویے کو سمجھ سکے۔
- `car_bridge` (اختیاری) مقامی Caché de archivo CAR کی طرف اشارہ کرتا ہے۔ `extension`
  اس sufijo کو کنٹرول کرتا ہے جو `*.car` غائب ہونے پر لگایا جاتا ہے؛ `allow_zst = true`
  سے `*.car.zst` براہ راست servir ہو سکتا ہے۔
- `kaigi_bridge` (اختیاری) Kaigi rutas کو proxy پر exponer کرتا ہے۔ `room_policy`
  بتاتا ہے کہ puente `public` ہے یا `authenticated` تاکہ clientes del navegador درست
  Etiquetas GAR چن سکیں۔
- `sorafs_cli fetch` `--local-proxy-mode=bridge|metadata-only` Más
  `--local-proxy-norito-spool=PATH` anula فراہم کرتا ہے، جس سے JSON پالیسیبدلے بغیر modo de tiempo de ejecución یا spool تبدیل ہو سکتے ہیں۔
- `downgrade_remediation` (اختیاری) خودکار gancho de degradación کو کنفیگر کرتا ہے۔
  جب habilitado ہو تو آرکسٹریٹر telemetría de retransmisión میں ráfagas de degradación دیکھتا ہے اور
  `window_secs` o `threshold` o `target_mode` o `target_mode`
  (ڈیفالٹ `metadata-only`) پر مجبور کرتا ہے۔ جب degrada رک جائیں تو proxy
  `cooldown_secs` کے بعد `resume_mode` پر واپس آتا ہے۔ `modes` matriz específica
  roles de relevo تک محدود کرنے کے لیے استعمال کریں (ڈیفالٹ relevos de entrada)۔

جب modo puente proxy میں چلتا ہے تو دو servicios de aplicaciones فراہم کرتا ہے:

- **`norito`** — کلائنٹ کا objetivo de flujo `norito_bridge.spool_dir` کے نسبت resolver
  ہوتا ہے۔ objetivos کو desinfectar کیا جاتا ہے (recorrido اور rutas absolutas نہیں)
  اور اگر فائل میں extensión نہیں تو sufijo configurado لگایا جاتا ہے، پھر carga útil
  براؤزر کو stream کیا جاتا ہے۔
- **`car`** — objetivos de transmisión `car_bridge.cache_dir` کے اندر resolver ہوتے ہیں،
  la extensión predeterminada hereda کرتے ہیں، اور cargas útiles comprimidas کو رد کرتے ہیں جب تک
  `allow_zst` فعال نہ ہو۔ کامیاب puentes `STREAM_ACK_OK` کے ساتھ جواب دیتے ہیں
  اور پھر bytes de archivo بھیجتے ہیں تاکہ کلائنٹس verificación کو canalización کر سکیں۔دونوں صورتوں میں proxy cache-tag HMAC مہیا کرتا ہے (جب guard cache key handshake
کے دوران موجود ہو) اور `norito_*` / `car_*` códigos de motivo de telemetría ریکارڈ کرتا ہے
تاکہ paneles de control کامیابی، archivos faltantes, اور fallas de desinfección
سمجھ سکیں۔

`Orchestrator::local_proxy().await` چلتے ہوئے manejar کو exponer کرتا ہے تاکہ
کالرز certificado PEM پڑھ سکیں، manifiesto del navegador حاصل کر سکیں، یا ایپلیکیشن
ختم ہونے پر apagado elegante درخواست کر سکیں۔

جب habilitado ہو تو proxy اب **manifiesto v2** ریکارڈز سرور کرتا ہے۔ Certificado de موجودہ
La clave de caché de guardia es la siguiente:

- `alpn` (`"sorafs-proxy/1"`) y `capabilities` matriz تاکہ کلائنٹس flujo
  پروٹوکول کی تصدیق کر سکیں۔
- ہر apretón de manos کے لیے `session_id` اور `cache_tagging` bloque تاکہ por sesión
  afinidades de guardia y etiquetas HMAC مشتق کیے جا سکیں۔
- circuito y sugerencias de selección de guardia (`circuit`, `guard_selection`, `route_hints`)
  تاکہ flujos de integraciones del navegador کھلنے سے پہلے UI más rica دکھا سکیں۔
- `telemetry_v2` مقامی instrumentación کے لیے muestreo/perillas de privacidad کے ساتھ۔
- ہر `STREAM_ACK_OK` میں `cache_tag_hex` شامل ہوتا ہے۔ کلائنٹس اسے
  `x-sorafs-cache-tag` encabezado میں mirror کرتے ہیں تاکہ selecciones de protección en caché
  resto میں cifrado رہیں۔

Subconjunto v1 پر انحصار جاری رکھ سکتے ہیں۔

## 2. Semántica del fracaso

Capacidad de verificación de presupuesto آرکسٹریٹر ایک بائٹ منتقل ہونے سے پہلے سخت
ہے۔ ناکامیاں تین زمروں میں آتی ہیں:1. **Fallos de elegibilidad (antes del vuelo).** La capacidad de alcance نہ رکھنے والے، expiró
   anuncios, telemetría obsoleta y proveedores, artefactos de marcador, registros
   جاتا ہے اور programación سے نکال دیا جاتا ہے۔ Resúmenes CLI `ineligible_providers`
   matriz میں razones بھر دیتی ہیں تاکہ operadores registros raspado کیے بغیر gobernanza
   deriva دیکھ سکیں۔
2. **Agotamiento del tiempo de ejecución.** ہر proveedor مسلسل fallas کو seguimiento کرتا ہے۔ جب
   `provider_failure_threshold` پہنچ جائے تو proveedor کو سیشن کے باقی حصے کے لیے
   `disabled` کر دیا جاتا ہے۔ Proveedores de proveedores `disabled` ہو جائیں تو آرکسٹریٹر
   `MultiSourceError::NoHealthyProviders { last_error, chunk_index }` واپس کرتا ہے۔
3. **Abortos deterministas.** سخت حدود errores estructurados کے طور پر ظاہر ہوتی ہیں:
   - `MultiSourceError::NoCompatibleProviders` — manifiesto ایسی fragmento de tramo یا
     alineación مانگتا ہے جسے باقی proveedores honor نہیں کر سکتے۔
   - `MultiSourceError::ExhaustedRetries` — presupuesto de reintento por fragmento ختم ہو گیا۔
   - `MultiSourceError::ObserverFailed` — observadores posteriores (ganchos de transmisión)
     نے trozo verificado رد کر دیا۔

ہر error que ofende el índice de fragmentos اور جہاں ممکن ہو motivo final de falla del proveedor
کے ساتھ آتا ہے۔ انہیں bloqueadores de liberación سمجھیں - انہی entradas کے ساتھ reintentos
تب تک falla دہراتی رہیں گے جب تک anuncio, telemetría یا salud del proveedor نہ بدلے۔

### 2.1 Persistencia del marcador

جب `persist_path` کنفیگر ہو تو آرکسٹریٹر ہر رن کے بعد marcador final لکھتا ہے۔
Archivo JSON:- `eligibility` (`eligible` یا `ineligible::<reason>`).
- `weight` (اس رن کے لیے peso normalizado asignado).
- Metadatos `provider` (identificador, puntos finales, presupuesto de concurrencia).

Instantáneas del marcador کو artefactos de publicación کے ساتھ archivo کریں تاکہ lista negra
اور implementación فیصلے رہیں۔ auditable

## 3. Telemetría y depuración

### 3.1 Métricas Prometheus

آرکسٹریٹر `iroha_telemetry` کے ذریعے درج ذیل میٹرکس emite کرتا ہے:| Métrica | Etiquetas | Descripción |
|--------|--------|-------------|
| `sorafs_orchestrator_active_fetches` | `manifest_id`, `region` | recuperaciones orquestadas en vuelo کا calibre۔ |
| `sorafs_orchestrator_fetch_duration_ms` | `manifest_id`, `region` | latencia de recuperación de extremo a extremo ریکارڈ کرنے والا histograma۔ |
| `sorafs_orchestrator_fetch_failures_total` | `manifest_id`, `region`, `reason` | fallas de terminal کا contador (reintentos agotados, sin proveedores, falla del observador) ۔ |
| `sorafs_orchestrator_retries_total` | `manifest_id`, `provider`, `reason` | فی-intentos de reintento del proveedor کا contador۔ |
| `sorafs_orchestrator_provider_failures_total` | `manifest_id`, `provider`, `reason` | Fallos del proveedor a nivel de sesión کا contador جو desactivación تک لے جائیں۔ |
| `sorafs_orchestrator_policy_events_total` | `region`, `stage`, `outcome`, `reason` | política de anonimato فیصلوں کی etapa de conteo (cumplir vs apagón) اور motivo de reserva کے مطابق۔ |
| `sorafs_orchestrator_pq_ratio` | `region`, `stage` | منتخب SoraNet establece میں PQ relé compartido کا histograma۔ |
| `sorafs_orchestrator_pq_candidate_ratio` | `region`, `stage` | instantánea del marcador میں Relaciones de suministro del relé PQ کا histograma۔ |
| `sorafs_orchestrator_pq_deficit_ratio` | `region`, `stage` | déficit de políticas (objetivo اور participación real de PQ کے فرق) کا histograma۔ |
| `sorafs_orchestrator_classical_ratio` | `region`, `stage` | ہر سیشن میں relé clásico compartir کا histograma۔ |
| `sorafs_orchestrator_classical_selected` | `region`, `stage` | فی سیشن منتخب relés clásicos کی تعداد کا histograma۔ |Perillas de producción آن کرنے سے پہلے métricas کو paneles de preparación میں شامل کریں۔
تجویز کردہ diseño Plan de observabilidad SF-6 کو sigue کرتا ہے:

1. **Recuperaciones activas** — Completaciones de indicadores کے بغیر بڑھے تو الرٹ۔
2. **Proporción de reintentos**: contadores `retry`, líneas de base, advertencias
3. **Fallas del proveedor** — 15 meses de proveedor کے `session_failure > 0` پر
   alertas de buscapersonas۔

### 3.2 Destinos de registro estructurados

آرکسٹریٹر objetivos deterministas پر eventos estructurados شائع کرتا ہے:

- `telemetry::sorafs.fetch.lifecycle` — `start` y `complete` marcadores de ciclo de vida
  کے ساتھ recuentos de fragmentos, reintentos, duración total ۔
- `telemetry::sorafs.fetch.retry`: eventos de reintento (`provider`, `reason`, `attempts`).
- `telemetry::sorafs.fetch.provider_failure` — errores repetidos کی وجہ سے deshabilitados
  proveedores۔
- `telemetry::sorafs.fetch.error` — `reason` اور metadatos de proveedor opcional کے
  ساتھ fallas terminales۔

ان transmisiones کو موجودہ Norito canalización de registros میں reenviar کریں تاکہ respuesta a incidentes
کو ایک ہی fuente de verdad ملے۔ eventos del ciclo de vida PQ/mezcla clásica کو
`anonymity_effective_policy`, `anonymity_pq_ratio`, `anonymity_classical_ratio`
اور متعلقہ contadores کے ذریعے ظاہر کرتے ہیں، جس سے paneles y métricas raspado
کیے بغیر alambre کرنا آسان ہوتا ہے۔ Lanzamientos de GA کے دوران eventos de reintento/ciclo de vida
کے لیے nivel de registro `info` رکھیں اور errores de terminal کے لیے `warn` استعمال کریں۔

### 3.3 Resúmenes JSON`sorafs_cli fetch` es un resumen estructurado del SDK de Rust que incluye:

- `provider_reports` con recuentos de éxito/fracaso y deshabilitación del proveedor ہونے کی حالت۔
- `chunk_receipts` جو دکھاتے ہیں کہ کس proveedor نے کون سا trozo پورا کیا۔
- Matrices `retry_stats` y `ineligible_providers`.

مشکوک proveedores کو depuración کرتے وقت archivo de resumen کریں — recibos اوپر
دیے گئے metadatos de registro سے براہ راست جڑتے ہیں۔

## 4. Lista de verificación operativa

1. **CI میں کنفیگریشن stage کریں۔** `sorafs_fetch` کو configuración de destino کے ساتھ چلائیں،
   vista de elegibilidad کے لیے `--scoreboard-out` دیں، اور پچھلے lanzamiento سے diff کریں۔
   کوئی غیر متوقع promoción de proveedor no elegible روک دیتا ہے۔
2. **Telemetría ویلیڈیٹ کریں۔** las recuperaciones de múltiples fuentes habilitan کرنے سے پہلے یقینی
   Implementación de métricas `sorafs.fetch.*` y exportación de registros estructurados
   ہے۔ métricas کی عدم موجودگی عموماً اس بات کا اشارہ ہے کہ orquestador fachada
   invocar نہیں ہوئی۔
3. **Anula دستاویزی بنائیں۔** ہنگامی `--deny-provider` یا `--boost-provider`
   Archivo de registro de cambios y confirmación de JSON (invocación de CLI) reversiones کو
   anular revertir کرنا اور نیا captura de instantáneas del marcador کرنا چاہیے۔
4. **Pruebas de humo دوبارہ چلائیں۔** reintentar presupuestos یا límites de proveedores بدلنے کے بعد
   accesorio canónico (`fixtures/sorafs_manifest/ci_sample/`) پر دوبارہ buscar کریں
   اور verificar کریں کہ recibos de fragmentos deterministas رہیں۔اوپر دیے گئے مراحل آرکسٹریٹر کے رویے کو implementaciones por etapas میں reproducible رکھتے
ہیں اور respuesta a incidentes کے لیے ضروری telemetría فراہم کرتے ہیں۔

### 4.1 Anulaciones de políticas

Etapa de transporte/anonimato de operadores کو configuración base تبدیل کیے بغیر pin
کر سکتے ہیں، بس `policy_override.transport_policy` اور
`policy_override.anonymity_policy` کو اپنے `orchestrator` JSON میں سیٹ کریں (یا
`sorafs_cli fetch` y `--transport-policy-override=` / `--anonymity-policy-override=`
پاس کریں)۔ جب anular موجود ہو تو آرکسٹریٹر respaldo de apagón habitual کو skip
کرتا ہے: اگر مطلوبہ PQ tier فراہم نہ ہو تو fetch `no providers` کے ساتھ فیل
ہوتا ہے بجائے خاموشی سے degradación ہونے کے۔ comportamiento predeterminado پر واپسی اتنی ہی
آسان ہے جتنی anular campos کو صاف کرنا۔