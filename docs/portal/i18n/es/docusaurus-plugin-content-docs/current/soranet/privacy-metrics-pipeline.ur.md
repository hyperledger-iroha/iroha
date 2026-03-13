---
lang: es
direction: ltr
source: docs/portal/docs/soranet/privacy-metrics-pipeline.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: canalización-de-métricas-de-privacidad
título: SoraNet پرائیویسی میٹرکس پائپ لائن (SNNet-8)
sidebar_label: پرائیویسی میٹرکس پائپ لائن
descripción: SoraNet کے relés اور orquestadores کے لئے پرائیویسی محفوظ telemetría جمع کرنا۔
---

:::nota مستند ماخذ
`docs/source/soranet/privacy_metrics_pipeline.md` کی عکاسی کرتا ہے۔ پرانے docs کے ختم ہونے تک دونوں نقول ہم وقت رکھیں۔
:::

# SoraNet پرائیویسی میٹرکس پائپ لائن

Tiempo de ejecución del relé SNNet-8 کے لئے پرائیویسی کو مدنظر رکھنے والی telemetría سطح متعارف کراتا ہے۔ relé اب apretón de manos اور circuito واقعات کو ایک منٹ کے cubos میں جمع کرتا ہے اور صرف موٹے Prometheus contadores برآمد کرتا ہے، جس سے انفرادی circuitos no enlazables رہتے ہیں جبکہ آپریٹرز کو قابلِ عمل بصیرت ملتی ہے۔

## Agregador کا خلاصہ- implementación de tiempo de ejecución `tools/soranet-relay/src/privacy.rs` میں `PrivacyAggregator` کے طور پر موجود ہے۔
- cubos کو reloj de pared منٹ کے حساب سے key کیا جاتا ہے (`bucket_secs`, predeterminado 60 segundos) اور انہیں ایک محدود ring (`max_completed_buckets`, predeterminado 120) میں رکھا جاتا ہے۔ acciones de recopilador اپنا محدود trabajo pendiente رکھتے ہیں (`max_share_lag_buckets`, predeterminado 12) تاکہ پرانے Prio windows suprimió depósitos کے طور پر descarga ہوں اور pérdida de memoria یا recopiladores atascados چھپ نہ جائیں۔
- `RelayConfig::privacy` براہ راست `PrivacyConfig` Mapa de میں ہوتا ہے اور perillas de sintonización (`bucket_secs`, `min_handshakes`, `flush_delay_buckets`, `force_flush_buckets`, `max_completed_buckets`, `max_share_lag_buckets`, `expected_shares`) ظاہر کرتا ہے۔ Valores predeterminados del tiempo de ejecución de producción رکھتا ہے جبکہ Umbrales de agregación segura SNNet-8a متعارف کراتا ہے۔
- tiempo de ejecución ماڈیولز ayudantes escritos کے ذریعے eventos ریکارڈ کرتے ہیں: `record_circuit_accepted`, `record_circuit_rejected`, `record_throttle`, `record_throttle_cooldown`, `record_capacity_reject`, `record_active_sample`, `record_verified_bytes`, y `record_gar_category`۔

## Punto final de administración de retransmisiónآپریٹرز `GET /privacy/events` کے ذریعے relé کے administrador oyente کو encuesta کر کے observaciones sin procesar لے سکتے ہیں۔ یہ JSON delimitado por nueva línea de punto final (`application/x-ndjson`) y cargas útiles de `SoranetPrivacyEventV1` ہوتے ہیں جو اندرونی `PrivacyEventBuffer` سے espejo ہوتے ہیں۔ buffer تازہ ترین eventos کو `privacy.event_buffer_capacity` entradas تک رکھتا ہے (predeterminado 4096) اور read پر Drain ہو جاتا ہے، اس لئے raspadores کو espacios سے بچنے کیلئے کافی بار encuesta کرنا چاہئے۔ eventos, apretón de manos, acelerador, ancho de banda verificado, circuito activo, señales GAR, contadores Prometheus y colectores descendentes پرائیویسی محفوظ migas de pan محفوظ کر سکیں یا flujos de trabajo de agregación seguros کو feed کریں۔

## Configuración del relé

Configuración del relé فائل کے `privacy` سیکشن کے ذریعے پرائیویسی cadencia de telemetría ایڈجسٹ کرتے ہیں:

```json
{
  "mode": "Entry",
  "listen": "0.0.0.0:443",
  "privacy": {
    "bucket_secs": 60,
    "min_handshakes": 12,
    "flush_delay_buckets": 1,
    "force_flush_buckets": 6,
    "max_completed_buckets": 120,
    "max_share_lag_buckets": 12,
    "expected_shares": 2
  }
}
```

Valores predeterminados de campo Especificación SNNet-8 کے مطابق ہیں اور load کے وقت validar ہوتے ہیں:| Campo | Descripción | Predeterminado |
|-------|-------------|---------|
| `bucket_secs` | ہر ventana de agregación کی چوڑائی (segundos)۔ | `60` |
| `min_handshakes` | کم از کم recuento de contribuyentes جس کے بعد los contadores de depósitos emiten کر سکتا ہے۔ | `12` |
| `flush_delay_buckets` | enjuague کی کوشش سے پہلے مکمل cubos کی تعداد۔ | `1` |
| `force_flush_buckets` | el cubo suprimido emite کرنے سے پہلے زیادہ سے زیادہ عمر۔ | `6` |
| `max_completed_buckets` | محفوظ شدہ acumulación de depósitos (memoria ilimitada سے بچاتا ہے)۔ | `120` |
| `max_share_lag_buckets` | supresión سے پہلے acciones de coleccionista کی ventana de retención۔ | `12` |
| `expected_shares` | combinar کرنے سے پہلے درکار Acciones de coleccionista de Prio۔ | `2` |
| `event_buffer_capacity` | flujo de administración کیلئے Registro de eventos de NDJSON۔ | `4096` |

`force_flush_buckets` کو `flush_delay_buckets` سے کم کرنا، umbrales کو صفر کرنا، یا protector de retención کو بند کرنا اب falla de validación کرتا ہے تاکہ ایسی implementaciones سے بچا جائے جو fuga de telemetría por retransmisión کریں۔

`event_buffer_capacity` کی حد `/admin/privacy/events` کو بھی encuadernado کرتی ہے، تاکہ raspadores غیر معینہ مدت تک پیچھے نہ رہ جائیں۔

## Acciones de coleccionista de PrioLos colectores duales SNNet-8a emiten کرتا ہے جو los depósitos Prio secretos compartidos emiten کرتے ہیں۔ orquestador اب `/privacy/events` NDJSON flujo کو `SoranetPrivacyEventV1` entradas اور `SoranetPrivacyPrioShareV1` acciones دونوں کیلئے analizar کرتا ہے اور انہیں `SoranetSecureAggregator::ingest_prio_share` میں adelante کرتا ہے۔ cubos اس وقت emiten ہوتے ہیں جب `PrivacyBucketConfig::expected_shares` contribuciones آ جائیں، جو relé comportamiento کی عکاسی ہے۔ comparte کو alineación del cubo اور forma del histograma کیلئے validar کیا جاتا ہے، پھر انہیں `SoranetPrivacyBucketMetricsV1` میں combinar کیا جاتا ہے۔ Recuento de protocolo de enlace combinado `min_contributors` سے نیچے چلا جائے تو cubo کو `suppressed` کے طور پر export کیا جاتا ہے، جو comportamiento del agregador en retransmisión جیسا ہے۔ ventanas suprimidas اب `suppression_reason` etiqueta emite کرتے ہیں تاکہ آپریٹرز `insufficient_contributors`, `collector_suppressed`, `collector_window_elapsed`, اور `forced_flush_window_elapsed` کے درمیان فرق کر سکیں جب brechas de telemetría کی تشخیص کر رہے ہوں۔ `collector_window_elapsed` کی وجہ تب بھی fire ہوتی ہے جب Prio comparte `max_share_lag_buckets` سے زیادہ دیر رہیں، جس سے colectores atascados نظر آتے ہیں اور memoria de acumuladores obsoletos میں نہیں رہتے۔

## Puntos finales de ingesta Torii

Torii اب دو puntos finales HTTP activados por telemetría فراہم کرتا ہے تاکہ relés اور colectores کسی transporte personalizado کے بغیر observaciones hacia adelante کر سکیں:- `POST /v2/soranet/privacy/event` ایک `RecordSoranetPrivacyEventDto` carga útil قبول کرتا ہے۔ cuerpo میں `SoranetPrivacyEventV1` کے ساتھ ایک opcional `source` etiqueta شامل ہوتا ہے۔ Torii solicitar perfil de telemetría کے مطابق validar کرتا ہے، evento ریکارڈ کرتا ہے، اور HTTP `202 Accepted` کے ساتھ ایک Norito Sobre JSON y ventana de depósito calculada (`bucket_start_unix`, `bucket_duration_secs`) y modo de retransmisión
- `POST /v2/soranet/privacy/share` ایک `RecordSoranetPrivacyShareDto` carga útil قبول کرتا ہے۔ cuerpo میں `SoranetPrivacyPrioShareV1` اور opcional `forwarded_by` sugerencia ہوتا ہے تاکہ آپریٹرز flujos de colector کا auditoría کر سکیں۔ Envíos HTTP `202 Accepted` کے ساتھ Norito Sobre JSON y کرتی ہیں جو ventana de depósito, اور sugerencia de supresión کو resumen کرتا ہے؛ fallas de validación telemetría `Conversion` respuesta سے mapa ہوتے ہیں تاکہ recopiladores کے درمیان manejo determinista de errores برقرار رہے۔ orquestador کا bucle de eventos اب sondeo de retransmisión کے دوران یہ acciones emiten کرتا ہے، جس سے Torii کا Prio acumulador en depósitos de relé کے ساتھ sincronización رہتا ہے۔

دونوں perfil de telemetría de terminales کی پابندی کرتے ہیں: métricas deshabilitadas ہونے پر `503 Service Unavailable` واپس کرتے ہیں۔ clientes Norito binario (`application/x.norito`) یا Norito JSON (`application/x.norito+json`) cuerpos بھیج سکتے ہیں؛ servidor estándar Torii extractores کے ذریعے formato خود بخود negociar کرتا ہے۔

## Métricas Prometheusہر cubo exportado میں `mode` (`entry`, `middle`, `exit`) اور `bucket_start` etiquetas ہوتے ہیں۔ Varias familias de métricas emiten کی جاتی ہیں:| Métrica | Descripción |
|--------|-------------|
| `soranet_privacy_circuit_events_total{kind}` | taxonomía de apretón de manos جس میں `kind={accepted,pow_rejected,downgrade,timeout,other_failure,capacity_reject}` شامل ہے۔ |
| `soranet_privacy_throttles_total{scope}` | contadores de aceleración جن میں `scope={congestion,cooldown,emergency,remote_quota,descriptor_quota,descriptor_replay}` ہے۔ |
| `soranet_privacy_throttle_cooldown_millis_{sum,count}` | apretones de manos estrangulados سے آنے والی cooldown مدتوں کا مجموعہ۔ |
| `soranet_privacy_verified_bytes_total` | pruebas de medición ciegas سے ancho de banda verificado ۔ |
| `soranet_privacy_active_circuits_{avg,max}` | ہر cubo میں circuitos activos کا اوسط اور زیادہ سے زیادہ۔ |
| `soranet_privacy_rtt_millis{percentile}` | Estimaciones de percentiles de RTT (`p50`, `p90`, `p99`) ۔ |
| `soranet_privacy_gar_reports_total{category_hash}` | Contadores del Informe de acción de gobernanza con hash, resumen de categoría, clave ہوتے ہیں۔ |
| `soranet_privacy_bucket_suppressed` | وہ depósitos جو umbral de contribuyente پورا نہ ہونے کی وجہ سے روکے گئے۔ |
| `soranet_privacy_pending_collectors{mode}` | acumuladores compartidos de coleccionista جو combinar ہونے کے منتظر ہیں، modo relé کے حساب سے گروپ کیے گئے۔ |
| `soranet_privacy_suppression_total{reason}` | contadores de cubos suprimidos جن میں `reason={insufficient_contributors,collector_suppressed,collector_window_elapsed,forced_flush_window_elapsed}` شامل ہیں تاکہ brechas de privacidad de los paneles کو نسبت دے سکیں۔ |
| `soranet_privacy_snapshot_suppression_ratio` | آخری drenaje کا proporción suprimida/drenada (0-1), presupuestos de alerta کیلئے مفید۔ |
| `soranet_privacy_last_poll_unixtime` | آخری کامیاب encuesta کا marca de tiempo UNIX (alerta de inactividad del recopilador کو چلانے کیلئے)۔ |
| `soranet_privacy_collector_enabled` | calibre جو اس وقت `0` ہو جاتا ہے جب recopilador de privacidad بند ہو یا inicio ہونے میں ناکام ہو (alerta de recopilador deshabilitado کیلئے)۔ || `soranet_privacy_poll_errors_total{provider}` | fallos de sondeo جو alias de retransmisión کے حساب سے گروپ ہوتے ہیں (errores de decodificación, fallos HTTP, یا غیر متوقع códigos de estado پر بڑھتے ہیں)۔ |

جن cubos میں observaciones نہیں ہوتیں وہ خاموش رہتے ہیں، جس سے paneles صاف رہتے ہیں اور ventanas sin relleno نہیں بنتیں۔

## Orientación operativa1. **Paneles** - Métricas y métricas `mode` y `window_start` کے حساب سے گروپ کر کے چارٹ کریں۔ ventanas faltantes کو resaltado کریں تاکہ recopilador یا relé مسائل سامنے آئیں۔ `soranet_privacy_suppression_total{reason}` استعمال کریں تاکہ brechas کی clasificación میں déficit de contribuyentes اور supresión impulsada por recopiladores کے درمیان فرق ہو سکے۔ Activo Grafana اب ایک dedicado **"Razones de supresión (5m)"** panel فراہم کرتا ہے جو ان contadores سے چلتا ہے، ساتھ ہی ایک **"Suppressed Bucket %"** stat `sum(soranet_privacy_bucket_suppressed) / count(...)` فی selección حساب کرتا ہے تاکہ آپریٹرز incumplimientos presupuestarios ایک نظر میں دیکھ سکیں۔ Serie **"Collector Share Backlog"** (`soranet_privacy_pending_collectors`) اور **"Snapshot Suppression Ratio"** estadísticas de recopiladores atascados اور ejecuciones automatizadas کے دوران deriva del presupuesto کو نمایاں کرتے ہیں۔
2. **Alertas**: contadores seguros para la privacidad, alarmas: picos de rechazo de PoW, frecuencia de enfriamiento, deriva de RTT, rechazos de capacidad. چونکہ ہر cubo کے اندر contadores monótonos ہوتے ہیں، سادہ reglas basadas en tasas اچھا کام کرتے ہیں۔
3. **Respuesta a incidentes** - پہلے datos agregados پر انحصار کریں۔ جب گہرے depuración کی ضرورت ہو تو relés سے instantáneas del depósito reproducción کروائیں یا pruebas de medición ciegas دیکھیں، registros de tráfico sin procesar جمع نہ کریں۔
4. **Retención** - `max_completed_buckets` سے تجاوز سے بچنے کیلئے کافی بار scrape کریں۔ exportadores کو Salida Prometheus کو fuente canónica سمجھنا چاہئے اور forward کرنے کے بعد depósitos locales حذف کر دینے چاہئیں۔## Análisis de supresión y ejecuciones automatizadas

SNNet-8 کی قبولیت اس بات پر منحصر ہے کہ recopiladores automatizados صحت مند رہیں اور supresión پالیسی حدود کے اندر رہے (ہر relé کیلئے کسی بھی 30 منٹ کی ventana میں ≤10% cubos)۔ اس gate کو پورا کرنے کیلئے درکار herramientas اب repo کے ساتھ آتا ہے؛ آپریٹرز کو اسے اپنی ہفتہ وار rituales میں شامل کرنا ہوگا۔ نئے Paneles de supresión Grafana نیچے دیے گئے Fragmentos de PromQL کو منعکس کرتے ہیں، جس سے de guardia ٹیموں کو visibilidad en vivo ملتی ہے اس سے پہلے کہ انہیں consultas manuales کی ضرورت پڑے۔

### Revisión de supresión کیلئے Recetas PromQL

آپریٹرز کو درج ذیل PromQL helpers ساتھ رکھنے چاہئیں؛ El panel de control Grafana compartido (`dashboards/grafana/soranet_privacy_metrics.json`) y las reglas de Alertmanager a las que se hace referencia son:

```promql
/* Suppression ratio per relay mode (30 minute window) */
(
  increase(soranet_privacy_suppression_total{reason=~"insufficient_contributors|collector_suppressed|collector_window_elapsed|forced_flush_window_elapsed"}[30m])
) /
clamp_min(
  increase(soranet_privacy_circuit_events_total{kind="accepted"}[30m]) +
  increase(soranet_privacy_suppression_total[30m]),
1
)
```

```promql
/* Detect new suppression spikes above the permitted minute budget */
increase(soranet_privacy_suppression_total{reason=~"insufficient_contributors|collector_window_elapsed|collector_suppressed"}[5m])
/
clamp_min(
  sum(increase(soranet_privacy_circuit_events_total{kind="accepted"}[5m])),
1
)
```

relación de salida استعمال کریں تاکہ **"% de depósito suprimido"** estadística پالیسی presupuesto سے نیچے رہے؛ detector de picos کو Alertmanager سے جوڑیں تاکہ contribuyentes کی تعداد غیر متوقع طور پر کم ہونے پر فوری فیڈبیک ملے۔

### CLI de informe de depósito sin conexión

espacio de trabajo میں `cargo xtask soranet-privacy-report` ایک وقتی NDJSON captura کیلئے دستیاب ہے۔ اسے ایک یا زیادہ el administrador de retransmisión exporta پر punto کریں:

```bash
cargo xtask soranet-privacy-report \
  --input artifacts/sorafs_privacy/relay-a.ndjson \
  --input artifacts/sorafs_privacy/relay-b.ndjson \
  --json-out artifacts/sorafs_privacy/relay_summary.json
```یہ captura de ayuda کو `SoranetSecureAggregator` کے ذریعے stream کرتا ہے، stdout پر resumen de supresión پرنٹ کرتا ہے، اور اختیاری طور پر `--json-out <path|->` کے ذریعے informe JSON estructurado لکھتا ہے۔ یہ coleccionista en vivo y perillas (`--bucket-secs`, `--min-contributors`, `--expected-shares`, وغیرہ) کو honor کرتا ہے، جس سے آپریٹرز problemas کی triage میں مختلف umbrales کے ساتھ capturas históricas repetición کر سکتے ہیں۔ Puerta de análisis de supresión SNNet-8 کو auditoría کے قابل رکھنے کیلئے JSON کو Grafana capturas de pantalla کے ساتھ adjuntar کریں۔

### پہلی lista de verificación de ejecución automatizada

Gobernanza اب بھی ثبوت چاہتا ہے کہ پہلی presupuesto de supresión de ejecución de automatización میں رہی۔ ayudante اب `--max-suppression-ratio <0-1>` قبول کرتا ہے تاکہ CI یا آپریٹرز اس وقت fail fast کر سکیں جب cubos suprimidos ventana permitida سے بڑھ جائیں (predeterminado 10%) یا جب ابھی کوئی cubos موجود نہ ہوں۔ تجویز کردہ flujo:

1. punto(s) final(es) de administrador de retransmisión اور Orchestrator کے `/v2/soranet/privacy/event|share` stream سے NDJSON export کر کے `artifacts/sorafs_privacy/<relay>.ndjson` میں محفوظ کریں۔
2. presupuesto de políticas کے ساتھ ayudante چلائیں:

   ```bash
   cargo xtask soranet-privacy-report \
     --input artifacts/sorafs_privacy/relay-a.ndjson \
     --input artifacts/sorafs_privacy/relay-b.ndjson \
     --json-out artifacts/sorafs_privacy/relay_summary.json \
     --max-suppression-ratio 0.10
   ```کمانڈ relación observada پرنٹ کرتی ہے اور salida distinta de cero کرتی ہے جب el presupuesto excede ہو **یا** جب cubos ابھی تیار نہ ہوں، جس سے اشارہ ملتا ہے کہ telemetría ابھی ejecutar کیلئے پیدا نہیں ہوئی۔ métricas en vivo کو دکھانا چاہئے کہ `soranet_privacy_pending_collectors` صفر کی طرف drenaje ہو رہا ہے اور `soranet_privacy_snapshot_suppression_ratio` اسی presupuesto سے نیچے رہتا ہے جب correr چل رہا ہو۔
3. transporte predeterminado Salida JSON Registro CLI Paquete de evidencia SNNet-8 Archivo Archivo Revisores Revisores Artefactos

## Próximos pasos (SNNet-8a)

- recopiladores Prio duales, integran, comparten ingestión, tiempo de ejecución, relés y recopiladores, las cargas útiles `SoranetPrivacyBucketMetricsV1` consistentes emiten *(Hecho — `crates/sorafs_orchestrator/src/lib.rs` میں `ingest_privacy_payload` اور متعلقہ pruebas دیکھیں۔)*
- Panel de control Prometheus compartido JSON, reglas de alerta, brechas de supresión, salud del recopilador, apagones de anonimato y portada. *(Hecho: `dashboards/grafana/soranet_privacy_metrics.json`, `dashboards/alerts/soranet_privacy_rules.yml`, `dashboards/alerts/soranet_policy_rules.yml` y accesorios de validación دیکھیں۔)*
- `privacy_metrics_dp.md` میں بیان کردہ artefactos de calibración de privacidad diferencial تیار کریں، جن میں cuadernos reproducibles اور resúmenes de gobernanza شامل ہوں۔ *(Hecho — cuaderno اور artefactos `scripts/telemetry/run_privacy_dp.py` سے بنتے ہیں؛ CI contenedor `scripts/telemetry/run_privacy_dp_notebook.sh` cuaderno کو `.github/workflows/release-pipeline.yml` flujo de trabajo کے ذریعے چلاتا ہے؛ resumen de gobernanza `docs/source/status/soranet_privacy_dp_digest.md` میں فائل کیا گیا ہے۔)*موجودہ lanzamiento SNNet-8 کی بنیاد فراہم کرتی ہے: determinista, telemetría segura para la privacidad جو براہ راست موجودہ Prometheus raspadores اور paneles میں فٹ ہوتی ہے۔ artefactos de calibración de privacidad diferencial پر مرکوز ہے۔