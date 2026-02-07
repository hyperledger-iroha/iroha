---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/orchestrator-tuning.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: afinación del orquestador
título: آرکسٹریٹر رول آؤٹ اور ٹیوننگ
sidebar_label: آرکسٹریٹر ٹیوننگ
descripción: ملٹی سورس آرکسٹریٹر کو GA تک لے جانے کے لیے عملی ڈیفالٹس، ٹیوننگ رہنمائی، اور آڈٹ چیک پوائنٹس۔
---

:::nota مستند ماخذ
یہ صفحہ `docs/source/sorafs/developer/orchestrator_tuning.md` کی عکاسی کرتا ہے۔ جب تک پرانی ڈاکیومنٹیشن مکمل طور پر ریٹائر نہ ہو جائے، دونوں نقول کو ہم آہنگ رکھیں۔
:::

# آرکسٹریٹر رول آؤٹ اور ٹیوننگ گائیڈ

یہ گائیڈ [کنفیگریشن ریفرنس](orchestrator-config.md) اور
[ملٹی سورس رول آؤٹ رن بک](multi-source-rollout.md) پر مبنی ہے۔ یہ وضاحت کرتی ہے
کہ ہر رول آؤٹ مرحلے میں آرکسٹریٹر کو کیسے ٹیون کیا جائے، marcador کے
آرٹیفیکٹس کو کیسے سمجھا جائے، اور ٹریفک بڑھانے سے پہلے کون سے ٹیلیمیٹری سگنلز
لازمی ہیں۔ ان سفارشات کو CLI, SDK اور آٹومیشن میں یکساں طور پر نافذ کریں تاکہ
ہر نوڈ ایک ہی ڈٹرمنسٹک buscar پالیسی پر عمل کرے۔

## 1. بنیادی پیرامیٹر سیٹس

ایک مشترکہ کنفیگریشن ٹیمپلیٹ سے آغاز کریں اور رول آؤٹ کے ساتھ چند منتخب perillas
کو ایڈجسٹ کریں۔ نیچے جدول عام مراحل کے لیے تجویز کردہ اقدار دکھاتا ہے؛
Este es el nombre de `OrchestratorConfig::default()` y `FetchOptions::default()`.
کے ڈیفالٹس پر واپس جاتی ہیں۔| مرحلہ | `max_providers` | `fetch.per_chunk_retry_limit` | `fetch.provider_failure_threshold` | `scoreboard.latency_cap_ms` | `scoreboard.telemetry_grace_secs` | نوٹس |
|-------|-----------------|-------------------------------|------------------------------------|--------------------------------|------------------------------|-------|
| **Laboratorio/CI** | `3` | `2` | `2` | `2500` | `300` | سخت límite de latencia اور چھوٹی ventana de gracia شور والی ٹیلیمیٹری کو فوراً ظاہر کرتی ہے۔ reintentos کم رکھیں تاکہ غلط manifiesta جلد سامنے آئیں۔ |
| **Puesta en escena** | `4` | `3` | `3` | `4000` | `600` | پروڈکشن ڈیفالٹس کو منعکس کرتا ہے اور pares exploratorios کے لیے جگہ چھوڑتا ہے۔ |
| **Canarias** | `6` | `3` | `3` | `5000` | `900` | ڈیفالٹس کے مطابق؛ `telemetry_region` سیٹ کریں تاکہ ڈیش بورڈز کینری ٹریفک الگ دکھا سکیں۔ |
| **Disponibilidad general (GA)** | `None` (تمام elegible استعمال کریں) | `4` | `4` | `5000` | `900` | عارضی خرابیوں کو جذب کرنے کے لیے reintento اور umbrales de fallo بڑھائیں جبکہ آڈٹس ڈٹرمنزم برقرار رکھیں۔ |- `scoreboard.weight_scale` ڈیفالٹ `10_000` پر رہتا ہے جب تک downstream سسٹم کسی اور resolución entera کا تقاضا نہ کرے۔ اسکیل بڑھانے سے proveedor de pedidos نہیں بدلتی؛ صرف زیادہ گھنا distribución de crédito بنتا ہے۔
- مراحل کے درمیان منتقلی میں JSON bundle محفوظ کریں اور `--scoreboard-out` استعمال کریں تاکہ آڈٹ ٹریل میں درست پیرامیٹر سیٹ ریکارڈ ہو۔

## 2. Marcador کی ہائیجین

Manifiesto del marcador کی ضروریات، anuncios de proveedores اور ٹیلیمیٹری کو یکجا کرتا ہے۔
آگے بڑھنے سے پہلے:1. **ٹیلیمیٹری کی تازگی چیک کریں۔** یقینی بنائیں کہ `--telemetry-json` میں حوالہ شدہ instantáneas
   مقررہ ventana de gracia کے اندر ریکارڈ ہوئے ہوں۔ `telemetry_grace_secs` سے پرانی entradas
   `TelemetryStale { last_updated }` کے ساتھ فیل ہوں گی۔ اسے parada brusca سمجھیں اور
   ٹیلیمیٹری ایکسپورٹ اپڈیٹ کیے بغیر آگے نہ بڑھیں۔
2. **Elegibilidad وجوہات دیکھیں۔** `--scoreboard-out=/var/lib/sorafs/scoreboards/preflight.json`
   کے ذریعے آرٹیفیکٹس محفوظ کریں۔ ہر entrada میں `eligibility` بلاک ہوتا ہے جو ناکامی
   کی درست وجہ بتاتا ہے۔ discrepancia de capacidad یا anuncios caducados کو اوور رائیڈ نہ کریں؛
   carga útil ascendente درست کریں۔
3. **Peso ڈیلٹاز کا جائزہ لیں۔** `normalised_weight` کو پچھلے lanzamiento سے موازنہ کریں۔
   10% سے زیادہ تبدیلیاں جان بوجھ کر publicidad/telemetría تبدیلیوں سے ہم آہنگ ہونی چاہئیں
   اور registro de implementación میں نوٹ ہونی چاہئیں۔
4. **آرٹیفیکٹس آرکائیو کریں۔** `scoreboard.persist_path` سیٹ کریں تاکہ ہر رن میں
   فائنل instantánea del marcador محفوظ ہو۔ اسے lanzamiento ریکارڈ کے ساتھ manifiesto اور
   paquete de telemetría کے ساتھ جوڑیں۔
5. **Mezcla de proveedores کا ثبوت ریکارڈ کریں۔** `scoreboard.json` کی metadatos اور متعلقہ
   `summary.json` o `provider_count`, `gateway_provider_count` y `provider_mix`
   لیبل لازماً ہو تاکہ جائزہ لینے والے ثابت کر سکیں کہ رن `direct-only`, `gateway-only`
   یا `mixed` تھا۔ La puerta de enlace captura los datos `provider_count=0` y `provider_mix="gateway-only"`
   ہونا چاہیے، جبکہ mixto رنز میں دونوں سورسز کے لیے غیر صفر cuenta ضروری ہیں۔`cargo xtask sorafs-adoption-check` ان فیلڈز کو نافذ کرتا ہے (اور no coincide پر فیل ہوتا ہے),
   لہٰذا اسے ہمیشہ `ci/check_sorafs_orchestrator_adoption.sh` یا اپنے script de captura کے ساتھ
   چلائیں تاکہ `adoption_report.json` paquete de pruebas بنے۔ جب Torii gateways شامل ہوں تو
   `gateway_manifest_id`/`gateway_manifest_cid` Metadatos del marcador کو میں رکھیں تاکہ adopción
   sobre de manifiesto de puerta کو mezcla de proveedor capturado سے جوڑ سکے۔

فیلڈز کی تفصیلی تعریف کے لیے
`crates/sorafs_car/src/scoreboard.rs` a la estructura de resumen CLI (a `sorafs_cli fetch --json-out`
سے نکلتی ہے) دیکھیں۔

## CLI اور SDK فلیگ ریفرنس

`sorafs_cli fetch` (دیکھیں `crates/sorafs_car/src/bin/sorafs_cli.rs`)
Envoltorio `iroha_cli app sorafs fetch` (`crates/iroha_cli/src/commands/sorafs.rs`) ایک ہی
superficie de configuración del orquestador شیئر کرتے ہیں۔ evidencia de implementación یا canónica
repetición de partidos کرنے کے لیے یہ banderas استعمال کریں:

Referencia de marca de fuente múltiple compartida (ayuda de CLI اور docs کو صرف اسی فائل میں ترمیم کر کے sync رکھیں):- `--max-peers=<count>` proveedores elegibles کی تعداد محدود کرتا ہے جو marcador فلٹر سے گزرتے ہیں۔ خالی چھوڑیں تاکہ تمام proveedores elegibles سے stream ہو، اور `1` صرف تب سیٹ کریں جب جان بوجھ کر respaldo de fuente única آزمانا ہو۔ SDK کے `maxPeers` perilla سے ہم آہنگ (`SorafsGatewayFetchOptions.maxPeers`, `SorafsGatewayFetchOptions.max_peers`).
- `--retry-budget=<count>` `FetchOptions` کے límite de reintentos por fragmento تک فارورڈ ہوتا ہے۔ تجویز کردہ اقدار کے لیے ٹیوننگ گائیڈ کے lanzamiento جدول کو استعمال کریں؛ evidencia جمع کرنے والی CLI رنز کو Valores predeterminados del SDK سے میچ ہونا چاہیے۔
- `--telemetry-region=<label>` Prometheus `sorafs_orchestrator_*` سیریز (اور OTLP Relays) پر región/env لیبل لگاتا ہے تاکہ ڈیش بورڈز lab, staging, canary اور GA ٹریفک الگ کر سکیں۔
- `--telemetry-json=<path>` marcador کے لیے instantánea referenciada inyectar کرتا ہے۔ Marcador JSON کے ساتھ محفوظ کریں تاکہ آڈیٹرز رن replay کر سکیں (اور `cargo xtask sorafs-adoption-check --require-telemetry` یہ ثابت کر سکے کہ کون سا captura de flujo OTLP کو feed کر رہا تھا)۔
- `--local-proxy-*` (`--local-proxy-mode`, `--local-proxy-norito-spool`, `--local-proxy-kaigi-spool`, `--local-proxy-kaigi-policy`) ganchos para observadores de puentes کو فعال کرتے ہیں۔ جب سیٹ ہو تو Orchestrator مقامی Norito/Kaigi proxy کے ذریعے fragmentos de flujo کرتا ہے تاکہ clientes del navegador, cachés de guardia اور Kaigi Rooms کو وہی recibos ملیں جو Rust emite کرتا ہے۔
- `--scoreboard-out=<path>` (اختیاری `--scoreboard-now=<unix_secs>` کے ساتھ) instantánea de elegibilidad محفوظ کرتا ہے۔ محفوظ شدہ JSON کو ہمیشہ ticket de lanzamiento میں telemetría referenciada اور artefactos manifiestos کے ساتھ جوڑیں۔- `--deny-provider name=ALIAS` / `--boost-provider name=ALIAS:delta` metadatos publicitarios کے اوپر determinista ایڈجسٹمنٹس لگاتے ہیں۔ ان banderas کو صرف ensayos کے لیے استعمال کریں؛ پروڈکشن rebaja کو artefactos de gobernanza کے ذریعے جانا چاہیے تاکہ ہر نوڈ وہی paquete de políticas نافذ کرے۔
- `--provider-metrics-out` / `--chunk-receipts-out` métricas de estado por proveedor y recibos por fragmentos محفوظ کرتے ہیں؛ evidencia de adopción جمع کرتے وقت دونوں artefactos ضرور شامل کریں۔

مثال (شائع شدہ accesorio استعمال کرتے ہوئے):

```bash
sorafs_cli fetch \
  --plan fixtures/sorafs_orchestrator/multi_peer_parity_v1/plan.json \
  --gateway-provider gw-alpha=... \
  --telemetry-source-label otlp::staging \
  --scoreboard-out artifacts/sorafs_orchestrator/latest/scoreboard.json \
  --json-out artifacts/sorafs_orchestrator/latest/summary.json \
  --provider-metrics-out artifacts/sorafs_orchestrator/latest/provider_metrics.json \
  --chunk-receipts-out artifacts/sorafs_orchestrator/latest/chunk_receipts.json

cargo xtask sorafs-adoption-check \
  --scoreboard artifacts/sorafs_orchestrator/latest/scoreboard.json \
  --summary artifacts/sorafs_orchestrator/latest/summary.json
```

SDK configuración کو `SorafsGatewayFetchOptions` کے ذریعے Cliente Rust
(`crates/iroha/src/client.rs`), enlaces JS (`javascript/iroha_js/src/sorafs.js`) y
Swift SDK (`IrohaSwift/Sources/IrohaSwift/SorafsOptions.swift`) میں استعمال کرتے ہیں۔
ان helpers کو CLI defaults کے ساتھ lock-step رکھیں تاکہ آپریٹرز automatización میں
پالیسیوں کو بغیر personalizado ترجمہ لیئرز کے کاپی کر سکیں۔

## 3. Recuperar پالیسی ٹیوننگ

`FetchOptions` reintentos, simultaneidad y verificación کو کنٹرول کرتا ہے۔ ٹیوننگ کے وقت:- **Reintentos:** `per_chunk_retry_limit` کو `4` سے اوپر بڑھانے سے recovery وقت بڑھتا ہے مگر fallos del proveedor چھپ سکتے ہیں۔ بہتر ہے `4` کو techo رکھا جائے اور rotación de proveedores پر بھروسہ کیا جائے تاکہ کمزور intérpretes سامنے آئیں۔
- **Umbral de error:** `provider_failure_threshold` طے کرتا ہے کہ proveedor کب سیشن کے باقی حصے کے لیے desactivar ہو۔ اس ویلیو کو پالیسی سے ہم آہنگ رکھیں: اگر umbral de presupuesto de reintento سے کم ہو تو Orchestrator تمام reintentos ختم ہونے سے پہلے par کو نکال دیتا ہے۔
- **Concurrencia:** `global_parallel_limit` کو `None` رکھیں جب تک مخصوص ماحول rangos anunciados کو saturar نہ کر سکے۔ اگر سیٹ کریں تو ویلیو proveedores کے presupuestos de transmisión کے مجموعے سے ≤ ہو تاکہ hambre نہ ہو۔
- **Alterna de verificación:** `verify_lengths` اور `verify_digests` پروڈکشن میں فعال رہنے چاہئیں۔ یہ flotas de proveedores mixtos میں determinismo کی ضمانت دیتے ہیں؛ انہیں صرف entornos de fuzzing aislados میں ہی بند کریں۔

## 4. ٹرانسپورٹ اور انانیمٹی اسٹیجنگ

Posición de postura: `rollout_phase`, `anonymity_policy`, `transport_policy`:- `rollout_phase="snnet-5"` کو ترجیح دیں اور política de anonimato predeterminada کو SNNet-5 hitos کے ساتھ چلنے دیں۔ `anonymity_policy_override` صرف تب استعمال کریں جب gobernanza نے directiva firmada جاری کیا ہو۔
- `transport_policy="soranet-first"` Línea base رکھیں جب تک SNNet-4/5/5a/5b/6a/7/8/12/13 🈺 ہوں
  (دیکھیں `roadmap.md`). `transport_policy="direct-only"` صرف دستاویزی downgrades یا simulacros de cumplimiento کے لیے استعمال کریں، اور Revisión de cobertura de PQ کے بعد ہی `transport_policy="soranet-strict"` پر جائیں—اس سطح پر صرف relés clásicos رہیں تو فوری fallan ہوگا۔
- `write_mode="pq-only"` Ruta de escritura (SDK, orquestador, herramientas de gobernanza) PQ تقاضے پورے کر سکے۔ implementaciones کے دوران `write_mode="allow-downgrade"` رکھیں تاکہ ہنگامی ردعمل rutas directas پر انحصار کر سکے جبکہ degradación de telemetría کو نشان زد کرے۔
- Selección de guardia اور puesta en escena del circuito Directorio SoraNet پر منحصر ہیں۔ instantánea firmada `relay_directory` فراہم کریں اور `guard_set` caché محفوظ کریں تاکہ guardia de abandono طے شدہ ventana de retención میں رہے۔ `sorafs_cli fetch` کے ذریعے لاگ ہونے والا evidencia de implementación de huellas dactilares de caché کا حصہ ہے۔

## 5. Rebajar los ganchos de cumplimiento

دو ذیلی نظام دستی مداخلت کے بغیر پالیسی نافذ کرنے میں مدد دیتے ہیں:- **Remediación de degradación** (`downgrade_remediation`): `handshake_downgrade_total` ایونٹس کی نگرانی کرتا ہے اور `window_secs` کے اندر `threshold` تجاوز ہونے پر proxy local کو `target_mode` پر مجبور کرتا ہے ( solo metadatos) ۔ ڈیفالٹس (`threshold=3`, `window=300`, `cooldown=900`) برقرار رکھیں جب تک انسیڈنٹ ریویوز مختلف پیٹرن نہ بتائیں۔ کسی بھی anular کو registro de implementación میں دستاویز کریں اور `sorafs_proxy_downgrade_state` کو ڈیش بورڈز پر ٹریک کریں۔
- **Política de cumplimiento** (`compliance`): jurisdicción اور exclusiones manifiestas listas de exclusión voluntaria administradas por la gobernanza کے ذریعے آتے ہیں۔ Paquete de کنفیگریشن anulaciones ad-hoc del paquete اس کے بجائے `governance/compliance/soranet_opt_outs.json` کے لیے actualización firmada طلب کریں اور JSON generado دوبارہ implementación کریں۔

دونوں سسٹمز کے لیے، نتیجہ خیز کنفیگریشن paquete محفوظ کریں اور اسے liberación de evidencia میں شامل کریں تاکہ آڈیٹرز desencadenantes de degradación کو ٹریس کر سکیں۔

## 6. ٹیلیمیٹری اور ڈیش بورڈز

رول آؤٹ بڑھانے سے پہلے تصدیق کریں کہ درج ذیل سگنلز target ماحول میں فعال ہیں:- `sorafs_orchestrator_fetch_failures_total{reason="no_healthy_providers"}` —
  کینری مکمل ہونے کے بعد صفر ہونا چاہیے۔
- `sorafs_orchestrator_retries_total`
  `sorafs_orchestrator_retry_ratio` — کینری کے دوران 10% سے کم پر مستحکم ہوں اور GA کے بعد 5% سے کم رہیں۔
- `sorafs_orchestrator_policy_events_total` — متوقع etapa de implementación کی تصدیق کرتا ہے (etiqueta `stage`) اور `outcome` کے ذریعے apagones ریکارڈ کرتا ہے۔
- `sorafs_orchestrator_pq_candidate_ratio` /
  `sorafs_orchestrator_pq_deficit_ratio` — پالیسی توقعات کے مقابلے میں Relé PQ سپلائی ٹریک کرتے ہیں۔
- `telemetry::sorafs.fetch.*` لاگ ٹارگٹس — مشترکہ لاگ ایگریگیٹر پر جائیں اور `status=failed` کے لیے محفوظ تلاشیں ہوں۔

`dashboards/grafana/sorafs_fetch_observability.json` سے canonical Grafana tablero de instrumentos لوڈ کریں
(پورٹل میں **SoraFS → Obtener observabilidad** کے تحت ایکسپورٹ شدہ), تاکہ selectores de región/manifiesto,
Mapa de calor de reintento del proveedor, histogramas de latencia de fragmentos, contadores de bloqueo y SRE
quemados کے دوران دیکھتا ہے۔ Reglas de Alertmanager número `dashboards/alerts/sorafs_fetch_rules.yml`
Sintaxis de `scripts/telemetry/test_sorafs_fetch_alerts.sh` y Prometheus
ویلیڈیٹ کریں (ayudante `promtool test rules` کو لوکل یا Docker میں چلاتا ہے)۔ Transferencias de alerta
کے لیے اسی enrutamiento بلاک کی ضرورت ہے جو اسکرپٹ پرنٹ کرتا ہے تاکہ آپریٹرز ثبوت کو lanzamiento
ٹکٹ سے جوڑ سکیں۔

### ٹیلیمیٹری quemado ورک فلو

Elemento de la hoja de ruta **SF-6e** کے تحت 30 دن کی grabación de telemetría درکار ہے اس سے پہلے کہ
ملٹی سورس آرکسٹریٹر GA ڈیفالٹس پر چلا جائے۔ ریپو اسکرپٹس کے ذریعے ہر دن کے لیے
ریپروڈیوس ایبل آرٹیفیکٹس بنائیں:

1. `ci/check_sorafs_orchestrator_adoption.sh` کو perillas de entorno de grabación کے ساتھ چلائیں۔ Nombre:

   ```bash
   SORAFS_BURN_IN_LABEL=canary-week-1 \
   SORAFS_BURN_IN_REGION=us-east-1 \
   SORAFS_BURN_IN_MANIFEST=manifest-v4 \
   SORAFS_BURN_IN_DAY=7 \
   SORAFS_BURN_IN_WINDOW_DAYS=30 \
   ci/check_sorafs_orchestrator_adoption.sh
   ```Ayudante `fixtures/sorafs_orchestrator/multi_peer_parity_v1` ری پلے کرتا ہے،
   `scoreboard.json`, `summary.json`, `provider_metrics.json`, `chunk_receipts.json`,
   اور `adoption_report.json` کو `artifacts/sorafs_orchestrator/<timestamp>/` میں لکھتا ہے،
   اور `cargo xtask sorafs-adoption-check` کے ذریعے کم از کم los proveedores elegibles hacen cumplir la کرتا ہے۔
2. جب burn-in ویری ایبلز موجود ہوں تو اسکرپٹ `burn_in_note.json` بھی بناتا ہے، جس میں label،
   índice del día, identificación del manifiesto, fuente de telemetría y resúmenes de artefactos محفوظ ہوتے ہیں۔ lanzamiento de اسے
   iniciar sesión میں شامل کریں تاکہ واضح ہو کہ 30 روزہ ونڈو کا کون سا دن کس capturar سے پورا ہوا۔
3. اپڈیٹ شدہ Grafana بورڈ (`dashboards/grafana/sorafs_fetch_observability.json`) کو puesta en escena/producción
   espacio de trabajo میں امپورٹ کریں، etiqueta grabada لگائیں، اور تصدیق کریں کہ ہر پینل ٹیسٹ شدہ
   manifiesto/región کے نمونے دکھاتا ہے۔
4. جب بھی `dashboards/alerts/sorafs_fetch_rules.yml` بدلے، `scripts/telemetry/test_sorafs_fetch_alerts.sh`
   (یا `promtool test rules …`) چلائیں تاکہ métricas de desgaste de enrutamiento de alerta کے مطابق ہونا دستاویزی ہو۔
5. ڈیش بورڈ instantánea, salida de prueba de alerta اور `telemetry::sorafs.fetch.*` لاگ ٹیل کو آرکسٹریٹر
   آرٹیفیکٹس کے ساتھ محفوظ کریں تاکہ gobernanza بغیر sistemas en vivo سے métricas اٹھائے evidencia
   ری پلے کر سکے۔

## 7. رول آؤٹ چیک لسٹ1. Configuración del candidato CI میں کے ساتھ marcadores دوبارہ بنائیں اور artefactos کو control de versiones میں رکھیں۔
2. ہر ماحول (laboratorio, puesta en escena, canario, producción) میں artefactos deterministas چلائیں اور `--scoreboard-out` اور `--json-out` artefactos کو lanzamiento ریکارڈ کے ساتھ منسلک کریں۔
3. de guardia انجینئر کے ساتھ ٹیلیمیٹری ڈیش بورڈز ریویو کریں، اور یقینی بنائیں کہ اوپر دیے گئے تمام métricas کے muestras en vivo موجود ہیں۔
4. حتمی کنفیگریشن پاتھ (عام طور پر `iroha_config` کے ذریعے) اور registro de gobernanza کے git commit کو ریکارڈ کریں جو anuncios اور cumplimiento کے لیے استعمال ہوا۔
5. rastreador de lanzamiento اپڈیٹ کریں اور SDK ٹیموں کو نئے ڈیفالٹس سے آگاہ کریں تاکہ کلائنٹ انٹیگریشنز سیدھ میں رہیں۔

اس گائیڈ کی پیروی آرکسٹریٹر ڈیپلائمنٹس کو ڈٹرمنسٹک اور آڈٹ ایبل رکھتی ہے جبکہ
reintentar presupuestos, capacidad del proveedor, postura de privacidad کو ٹیون کرنے کے لیے واضح
فیڈبیک لوپس فراہم کرتی ہے۔