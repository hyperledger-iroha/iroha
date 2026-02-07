---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/multi-source-rollout.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: implementación de múltiples fuentes
título: ملٹی سورس رول آؤٹ اور پرووائیڈر بلیک لسٹنگ رن بک
sidebar_label: ملٹی سورس رول آؤٹ رن بک
descripción: مرحلہ وار ملٹی سورس رول آؤٹس اور ہنگامی پرووائیڈر بلیک لسٹنگ کے لیے آپریشنل چیک لسٹ۔
---

:::nota مستند ماخذ
یہ صفحہ `docs/source/sorafs/runbooks/multi_source_rollout.md` کی عکاسی کرتا ہے۔ جب تک پرانا ڈاکیومنٹیشن سیٹ ریٹائر نہ ہو جائے، دونوں نقول کو ہم آہنگ رکھیں۔
:::

## مقصد

یہ رن بک SRE اور آن کال انجینئرز کو دو اہم ورک فلو میں رہنمائی کرتی ہے:

1. ملٹی سورس آرکسٹریٹر کو کنٹرولڈ ویوز میں رول آؤٹ کرنا۔
2. موجودہ سیشنز کو غیر مستحکم کیے بغیر خراب کارکردگی والے پرووائیڈرز کو بلیک لسٹ کرنا یا ان کی ترجیح کم کرنا۔

یہ فرض کرتی ہے کہ SF-6 کے تحت فراہم کردہ pila de orquestación پہلے ہی ڈیپلائے ہے (`sorafs_orchestrator`, API de rango de fragmentos de puerta de enlace, اور exportadores de telemetría).

> **مزید دیکھیں:** [آرکسٹریٹر آپریشنز رن بک](./orchestrator-ops.md) فی رن طریقۂ کار (captura del marcador, مرحلہ وار alternancias de despliegue, reversión) میں تفصیل دیتی ہے۔ لائیو تبدیلیوں کے دوران دونوں حوالوں کو ساتھ استعمال کریں۔

## 1. قبل از عمل توثیق1. **گورننس ان پٹس کی تصدیق کریں۔**
   - تمام امیدوار پرووائیڈرز کو cargas útiles de capacidad de rango اور presupuestos de flujo کے ساتھ Sobres `ProviderAdvertV1` شائع کرنے چاہئیں۔ `/v1/sorafs/providers` سے ویلیڈیٹ کریں اور متوقع capacidad فیلڈز سے موازنہ کریں۔
   - tasas de latencia/fallo فراہم کرنے والے instantáneas de telemetría ہر canary رن سے پہلے 15 منٹ سے کم پرانے ہونے چاہئیں۔
2. **کنفیگریشن اسٹیج کریں۔**
   - Formato JSON en capas `iroha_config` Formato de archivo:

     ```toml
     [torii.sorafs.orchestrator]
     config_path = "/etc/iroha/sorafs/orchestrator.json"
     ```

     Implementación de JSON میں سے متعلق حدود (`max_providers`, reintentar presupuestos) اپڈیٹ کریں۔ puesta en escena/producción میں وہی فائل دیں تاکہ فرق کم رہے۔
3. **accesorios canónicos چلائیں۔**
   - variables de entorno de manifiesto/token سیٹ کریں اور deterministic fetch چلائیں:

     ```bash
     sorafs_cli fetch \
       --plan fixtures/sorafs_manifest/ci_sample/payload.plan.json \
       --manifest-id "$CANARY_MANIFEST_ID" \
       --provider name=alpha,provider-id="$PROVIDER_ALPHA_ID",base-url=https://gw-alpha.example,stream-token="$PROVIDER_ALPHA_TOKEN" \
       --provider name=beta,provider-id="$PROVIDER_BETA_ID",base-url=https://gw-beta.example,stream-token="$PROVIDER_BETA_TOKEN" \
       --provider name=gamma,provider-id="$PROVIDER_GAMMA_ID",base-url=https://gw-gamma.example,stream-token="$PROVIDER_GAMMA_TOKEN" \
       --max-peers=3 \
       --retry-budget=4 \
       --scoreboard-out artifacts/canary.scoreboard.json \
       --json-out artifacts/canary.fetch.json
     ```

     variables de entorno Resumen de carga útil del manifiesto (hexadecimal) اور ہر canary پرووائیڈر کے لیے tokens de flujo codificados en base64 شامل ہونے چاہئیں۔
   - `artifacts/canary.scoreboard.json` کو پچھلے lanzamiento سے comparar کریں۔ کوئی نیا غیر اہل پرووائیڈر یا وزن میں >10% تبدیلی reseña مانگتی ہے۔
4. **ٹیلیمیٹری کی وائرنگ چیک کریں۔**
   - `docs/examples/sorafs_fetch_dashboard.json` میں Grafana exportar کھولیں۔ آگے بڑھنے سے پہلے یقینی بنائیں کہ `sorafs_orchestrator_*` puesta en escena de métricas میں rellenar ہو رہی ہیں۔

## 2. ہنگامی پرووائیڈر بلیک لسٹنگ

جب کوئی پرووائیڈر خراب trozos فراہم کرے، مستقل tiempos de espera دے، یا comprobaciones de cumplimiento میں فیل ہو تو یہ طریقہ کار اپنائیں۔1. **ثبوت محفوظ کریں۔**
   - تازہ ترین resumen de búsqueda ایکسپورٹ کریں (salida `--json-out` کا) ۔ ناکام índices de fragmentos, پرووائیڈر alias, اور digerir desajustes ریکارڈ کریں۔
   - `telemetry::sorafs.fetch.*` objetivos سے متعلقہ لاگ extractos محفوظ کریں۔
2. **فوری anular لگائیں۔**
   - آرکسٹریٹر کو دیے گئے instantánea de telemetría میں پرووائیڈر کو penalizado مارک کریں (`penalty=true` سیٹ کریں یا `token_health` کو `0` پر abrazadera کریں)۔ اگلا construcción del marcador خود بخود پرووائیڈر کو excluir کر دے گا۔
   - pruebas de humo ad-hoc کے لیے `sorafs_cli fetch` میں `--deny-provider gw-alpha` پاس کریں تاکہ propagación de telemetría کا انتظار کیے بغیر ejercicio de ruta de falla ہو۔
   - متاثرہ ماحول میں اپڈیٹ شدہ paquete de telemetría/configuración دوبارہ implementar کریں (puesta en escena → canary → producción)۔ تبدیلی کو registro de incidentes میں دستاویز کریں۔
3. **anular ویلیڈیٹ کریں۔**
   - búsqueda de accesorios canónicos دوبارہ چلائیں۔ تصدیق کریں کہ marcador نے پرووائیڈر کو `policy_denied` وجہ کے ساتھ no elegible مارک کیا ہے۔
   - `sorafs_orchestrator_provider_failures_total` چیک کریں تاکہ انکار شدہ پرووائیڈر کے لیے کاؤنٹر مزید نہ بڑھے۔
4. **طویل مدتی پابندی بڑھائیں۔**
   - اگر پرووائیڈر >24 h کے لیے بلاک رہے گا تو اس کے anuncio کو rotar یا suspender کرنے کے لیے ticket de gobernanza بنائیں۔ ووٹ پاس ہونے تک lista de denegaciones برقرار رکھیں اور instantáneas de telemetría اپڈیٹ کرتے رہیں تاکہ پرووائیڈر دوبارہ marcador میں نہ آئے۔
5. **رول بیک پروٹوکول۔**- پرووائیڈر بحال کرنے کے لیے اسے lista de denegaciones سے ہٹا دیں، دوبارہ implementar کریں، اور نیا instantánea del marcador محفوظ کریں۔ تبدیلی کو incidente post mortem کے ساتھ منسلک کریں۔

## 3. مرحلہ وار رول آؤٹ پلان

| مرحلہ | دائرہ کار | لازمی سگنلز | Ir/No-ir معیار |
|-------|-----------|--------------|----------------|
| **Laboratorio** | Integración de productos sanitarios | cargas útiles de accesorios کے ساتھ دستی CLI buscar | Estos fragmentos son contadores de fallas del proveedor y una tasa de reintento <5%. |
| **Puesta en escena** | مکمل puesta en escena del plano de control | Grafana tablero de mandos reglas de alerta صرف solo advertencia موڈ میں | `sorafs_orchestrator_active_fetches` ہر ٹیسٹ رن کے بعد صفر پر واپس آئے؛ کوئی `warn/critical` الرٹ نہ لگے۔ |
| **Canarias** | پروڈکشن ٹریفک کا ≤10% | buscapersonas خاموش مگر ٹیلیمیٹری مانیٹر en tiempo real | tasa de reintento < 10%, fallas del proveedor صرف معلوم pares ruidosos تک محدود، histograma de latencia base de preparación ±20% کے مطابق۔ |
| **Disponibilidad general** | 100% رول آؤٹ | reglas del buscapersonas فعال | 24 h تک `NoHealthyProviders` کی صفر errores, tasa de reintento مستحکم، tablero paneles SLA سبز۔ |

ہر مرحلے میں:

1. مطلوبہ `max_providers` اور reintentar presupuestos کے ساتھ آرکسٹریٹر JSON اپڈیٹ کریں۔
2. accesorio canónico اور ماحول کے نمائندہ manifest کے خلاف `sorafs_cli fetch` یا Pruebas de integración del SDK چلائیں۔
3. marcador + artefactos de resumen محفوظ کریں اور registro de lanzamiento کے ساتھ منسلک کریں۔
4. اگلے مرحلے پر جانے سے پہلے de guardia انجینئر کے ساتھ paneles de telemetría ریویو کریں۔## 4. Observabilidad y ganchos de incidentes

- **Métricas:** یقینی بنائیں کہ Alertmanager `sorafs_orchestrator_fetch_failures_total{reason="no_healthy_providers"}` اور `sorafs_orchestrator_retries_total` کو مانیٹر کر رہا ہے۔ اچانک اسپائک عام طور پر یہ بتاتا ہے کہ پرووائیڈر لوڈ میں degrade ہو رہا ہے۔
- **Registros:** `telemetry::sorafs.fetch.*` apunta a کو مشترکہ لاگ ایگریگیٹر پر روٹ کریں۔ `event=complete status=failed` کے لیے محفوظ تلاشیں بنائیں تاکہ triage تیز ہو۔
- **Marcadores:** ہر artefacto del marcador کو طویل مدتی اسٹوریج میں محفوظ کریں۔ Revisiones de cumplimiento de JSON اور reversiones por etapas کے لیے rastro de evidencia بھی ہے۔
- **Paneles:** placa canónica Grafana (`docs/examples/sorafs_fetch_dashboard.json`) کو پروڈکشن فولڈر میں کلون کریں اور `docs/examples/sorafs_fetch_alerts.yaml` کی reglas de alerta کریں۔

## 5. Comunicación y documentación

- ہر denegar/impulsar تبدیلی کو registro de cambios de operaciones میں marca de tiempo, آپریٹر، وجہ اور متعلقہ incidente کے ساتھ لاگ کریں۔
- Pesos de proveedores y presupuestos de reintento.
- GA مکمل ہونے کے بعد `status.md` میں resumen de implementación اپڈیٹ کریں اور اس رن بک ریفرنس کو notas de la versión میں archivo کریں۔