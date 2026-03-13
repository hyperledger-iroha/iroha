---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/provider-advert-rollout.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: "SoraFS پرووائیڈر anuncio رول آؤٹ اور مطابقتی پلان"
---

> [`docs/source/sorafs/provider_advert_rollout.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_rollout.md) سے ماخوذ۔

# SoraFS پرووائیڈر anuncio رول آؤٹ اور مطابقتی پلان

یہ پلان anuncios de proveedores permisivos سے مکمل طور پر gobernados `ProviderAdvertV1`
corte de superficie کو کوآرڈی نیٹ کرتا ہے جو recuperación de fragmentos de múltiples fuentes کے لیے
ضروری ہے۔ یہ تین entregables پر فوکس کرتا ہے:

- **Guía del operador.** وہ قدم بہ قدم اقدامات جو proveedores de almacenamiento کو ہر gate فلپ ہونے
  سے پہلے مکمل کرنا ہیں۔
- **Cobertura de telemetría.** Paneles de control, alertas, observabilidad y operaciones.
  کرتے ہیں تاکہ نیٹ ورک صرف anuncios compatibles قبول کرے۔
  SDK y herramientas ٹیمیں اپنی lanzamientos پلان کر سکیں۔

یہ implementación [hoja de ruta de migración SoraFS] (./migration-roadmap) کی Hitos de SF-2b/2c
کے ساتھ align ہے اور فرض کرتا ہے کہ [política de admisión de proveedores](./provider-admission-policy)
پہلے سے نافذ ہے۔

## Cronología de la fase

| Fase | Ventana (objetivo) | Comportamiento | Acciones del operador | Enfoque de observabilidad |
|-------|-----------------|-----------|------------------|-------------------|

## Lista de verificación del operador1. **Anuncios de inventario.** ہر anuncio publicado کی فہرست بنائیں اور ریکارڈ کریں:
   - Ruta de la envolvente gobernante (equivalente en producción `defaults/nexus/sorafs_admission/...` یا).
   - anuncio `profile_id` y `profile_aliases`.
   - lista de capacidades (کم از کم `torii_gateway` اور `chunk_range_fetch`).
   - Bandera `allow_unknown_capabilities` (جب TLV reservados por el proveedor ہوں تو ضروری ہے)۔
2. **Regenerar con herramientas del proveedor.**
   - اپنے editor de anuncios del proveedor سے carga útil دوبارہ بنائیں، اور یقینی بنائیں:
     - `profile_id=sorafs.sf1@1.0.0`
     - `capability=chunk_range_fetch` اور واضح `max_span`
     - GREASE TLVs کی صورت میں `allow_unknown_capabilities=<true|false>`
   - `/v2/sorafs/providers` اور `sorafs_fetch` سے validar کریں؛ desconocido
     capacidades کی advertencias کو triaje کریں۔
3. **Validar la preparación para múltiples fuentes.**
   - `sorafs_fetch` کو `--provider-advert=<path>` کے ساتھ چلائیں؛ Por `chunk_range_fetch`
     نہ ہونے پر CLI falla کرتا ہے اور ignoró capacidades desconocidas کے لیے advertencias دیتا ہے۔
     Informe JSON محفوظ کریں اور registros de operaciones کے ساتھ archivo کریں۔
4. **Renovaciones de etapa.**
   - aplicación de puerta de enlace (R2) سے کم از کم 30 دن پہلے `ProviderAdmissionRenewalV1`
     sobres جمع کریں۔ renovaciones میں identificador canónico اور conjunto de capacidades برقرار
     رہنا چاہئے؛ صرف juego, puntos finales y metadatos بدلے۔
5. **Comunicarse con equipos dependientes.**
   - Los propietarios del SDK lanzan دینا ہوں گی جو los anuncios rechazan ہونے پر operadores کو advertencias دکھائیں۔- Anuncio de transición de fase de DevRel ہر کرے؛ enlaces del tablero y lógica de umbral شامل کریں۔
6. **Instalar paneles y alertas.**
   - Exportación Grafana امپورٹ کریں اور **SoraFS / Implementación del proveedor** کے تحت رکھیں، UID
     `sorafs-provider-admission` رکھیں۔
   - یقینی بنائیں کہ reglas de alerta puesta en escena اور producción میں compartida
     `sorafs-advert-rollout` canal de notificación پر جائیں۔

## Telemetría y paneles

یہ métricas پہلے ہی `iroha_telemetry` کے ذریعے دستیاب ہیں:

- `torii_sorafs_admission_total{result,reason}` — aceptado, rechazado y advertencia
  resultados گنتا ہے۔ razones میں `missing_envelope`, `unknown_capability`, `stale`
  اور `policy_violation` شامل ہیں۔

Exportación Grafana: [`docs/source/grafana_sorafs_admission.json`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/grafana_sorafs_admission.json).
فائل کو repositorio de paneles compartidos (`observability/dashboards`) میں importar کریں اور
شائع کرنے سے پہلے صرف datasource UID اپڈیٹ کریں۔

Carpeta Grafana **SoraFS / Implementación del proveedor** UID estable
`sorafs-provider-admission` کے ساتھ publicar ہوتا ہے۔ reglas de alerta
`sorafs-admission-warn` (advertencia) y `sorafs-admission-reject` (crítico)
پہلے سے `sorafs-advert-rollout` política de notificación استعمال کرنے کے لیے configurada ہیں؛
Lista de destinos Panel de control JSON Editar Punto de contacto
actualizar کریں۔

Paneles Grafana recomendados:| Paneles | Consulta | Notas |
|-------|-------|-------|
| **Tasa de resultados de admisión** | `sum by(result)(rate(torii_sorafs_admission_total[5m]))` | Gráfico de pila: aceptar vs advertir vs rechazar دکھاتا ہے۔ advertir > 0.05 * total (advertencia) یا rechazar > 0 (crítico) پر alerta۔ |
| **Proporción de advertencia** | `sum(rate(torii_sorafs_admission_total{result="warn"}[5m])) / sum(rate(torii_sorafs_admission_total[5m]))` | Serie temporal de una sola línea, umbral del buscapersonas y feed کرتی ہے (tasa de advertencia del 5 % del 15 %) ۔ |
| **Motivos del rechazo** | `sum by(reason)(rate(torii_sorafs_admission_total{result="reject"}[5m]))` | triaje de runbook کے لیے؛ medidas de mitigación کے لنکس شامل کریں۔ |
| **Actualizar deuda** | `sum(rate(torii_sorafs_admission_total{reason="stale"}[1h]))` | fecha límite de actualización falta de proveedores de کرنے والے کو ظاہر کرتا ہے؛ registros de caché de descubrimiento کے ساتھ referencia cruzada کریں۔ |

Paneles de control manuales کے لیے Artefactos CLI:

- `sorafs_fetch --provider-metrics-out` ہر proveedor کے لیے `failures`, `successes`,
  اور `disabled` contadores لکھتا ہے۔ ensayos del orquestador کو monitor کرنے کے لیے
  Paneles ad-hoc میں importar کریں۔
- Informe JSON کے `chunk_retry_rate` اور `provider_failure_rate` limitación de campos یا
  Síntomas de carga útil obsoleta دکھاتے ہیں جو اکثر admisión rechaza سے پہلے آتے ہیں۔

### Diseño del tablero Grafana

Tablero dedicado de observabilidad ایک — **SoraFS Admisión del proveedor
Lanzamiento** (`sorafs-provider-admission`) — **SoraFS / Lanzamiento del proveedor** کے تحت
publicar کرتا ہے، اور اس کے ID de panel canónico یہ ہیں:- Panel 1: *Tasa de resultados de admisión* (área apilada, یونٹ "ops/min").
- Panel 2 — *Relación de advertencia* (serie única) ، اظہار
  `suma(tasa(torii_sorafs_admission_total{result="warn"}[5m])) /
   suma(tasa(torii_sorafs_admission_total[5m]))`.
- Panel 3 — *Motivos de rechazo* (`reason` کے حساب سے series temporales), `rate(...[5m])`
  کے مطابق ordenar کی گئی۔
- Panel 4 — *Actualizar deuda* (estadísticas), اوپر والی consulta de tabla کو mirror کرتا ہے اور
  libro mayor de migración سے حاصل کردہ fechas límite de actualización de anuncios کے ساتھ anotado ہے۔

Repositorio de paneles de infraestructura de esqueleto JSON Mیں `observability/dashboards/sorafs_provider_admission.json`
پر copiar یا crear کریں، پھر صرف fuente de datos UID اپڈیٹ کریں؛ ID de panel y reglas de alerta
نیچے runbooks میں referenciados ہیں، اس لیے انہیں renumeración کرنے سے پہلے documentos اپڈیٹ کریں۔

سہولت کے لیے ریپو `docs/source/grafana_sorafs_admission.json` میں tablero de referencia
definición دیتا ہے؛ لوکل testing کے لیے اسے اپنے Grafana carpeta میں copiar کر لیں۔

### Prometheus reglas de alerta

مندرجہ ذیل grupo de reglas کو `observability/prometheus/sorafs_admission.rules.yml`
میں شامل کریں (اگر یہ پہلا SoraFS grupo de reglas ہے تو فائل بنائیں) اور Prometheus
configuración سے incluye کریں۔ `<pagerduty>` کو اپنے etiqueta de enrutamiento de guardia سے بدلیں۔

```yaml
groups:
  - name: torii_sorafs_admission
    rules:
      - alert: SorafsProviderAdvertWarnFlood
        expr: sum(rate(torii_sorafs_admission_total{result="warn"}[5m])) /
              sum(rate(torii_sorafs_admission_total[5m])) > 0.05
        for: 15m
        labels:
          severity: warning
          route: <pagerduty>
        annotations:
          summary: "SoraFS provider adverts generating warnings"
          description: |
            Warn outcomes exceeded 5% of all admissions for 15 minutes.
            Inspect panel 3 on the sorafs/provider-admission dashboard and
            coordinate advert rotation with the affected operator.
      - alert: SorafsProviderAdvertReject
        expr: increase(torii_sorafs_admission_total{result="reject"}[5m]) > 0
        for: 5m
        labels:
          severity: critical
          route: <pagerduty>
        annotations:
          summary: "SoraFS provider adverts rejected"
          description: |
            Provider adverts have been rejected for the last five minutes.
            Check panel 4 (rejection reasons) and rotate envelopes before
            the refresh deadline elapses.
```

`scripts/check_prometheus_rules.sh observability/prometheus/sorafs_admission.rules.yml`
چلا کر تصدیق کریں کہ sintaxis `promtool check rules` پاس کر رہا ہے۔

## Comunicación y manejo de incidentes- **Anuncio publicitario de estado semanal.** Métricas de admisión de DevRel, advertencias pendientes, fechas límite, fechas límite, fechas límite
- **Respuesta a incidentes.** اگر `reject` alertas فائر ہوں تو de guardia انجینئر:
  1. Descubrimiento Torii (`/v2/sorafs/providers`) سے búsqueda de anuncios infractores کریں۔
  2. canalización del proveedor میں validación de anuncios دوبارہ چلائیں اور `/v2/sorafs/providers` سے comparar کریں تاکہ error reproducir ہو۔
  3. proveedor کے ساتھ coordinar کریں تاکہ اگلی fecha límite de actualización سے پہلے rotación de anuncios ہو جائے۔
- **El cambio se congela.** R1/R2 کے دوران esquema de capacidad میں تبدیلیاں نہ کریں جب تک rollout کمیٹی منظوری نہ دے؛ Pruebas de GREASE کو ہفتہ وار ventana de mantenimiento میں cronograma کریں اور libro mayor de migración میں registro کریں۔

## Referencias

- [SoraFS Protocolo de cliente/nodo](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/sorafs_node_client_protocol.md)
- [Política de admisión de proveedores](./provider-admission-policy)
- [Hoja de ruta de migración](./migration-roadmap)
- [Extensiones de múltiples fuentes de anuncios de proveedores](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_multisource.md)