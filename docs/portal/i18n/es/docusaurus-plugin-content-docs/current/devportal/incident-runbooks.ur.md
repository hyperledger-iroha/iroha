---
lang: es
direction: ltr
source: docs/portal/docs/devportal/incident-runbooks.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# انسیڈنٹ رن بکس اور رول بیک ڈرلز

## مقصد

روڈمیپ آئٹم **DOCS-9** قابل عمل playbooks اور ensayo پلان کا تقاضا کرتا ہے تاکہ
پورٹل آپریٹرز ڈلیوری فیلئرز سے بغیر اندازے کے ریکور کر سکیں۔ یہ نوٹ تین ہائی سگنل
انسیڈنٹس - Implementaciones de ناکام, degradación de la replicación, اور interrupciones de análisis - کو کور کرتا ہے
اور simulacros trimestrales کو دستاویزی بناتا ہے جو ثابت کرتے ہیں کہ reversión de alias اور validación sintética
اب بھی de extremo a extremo کام کرتے ہیں۔

### متعلقہ مواد

- [`devportal/deploy-guide`](./deploy-guide) — empaquetado, firma, flujo de trabajo de promoción de alias۔
- [`devportal/observability`](./observability) — etiquetas de lanzamiento, análisis, sondas جن کا نیچے حوالہ ہے۔
- `docs/source/sorafs_node_client_protocol.md`
  y [`sorafs/pin-registry-ops`](../sorafs/pin-registry-ops)
  — telemetría de registro y umbrales de escalada ۔
- Ayudantes `docs/portal/scripts/sorafs-pin-release.sh` y `npm run probe:*`
  جو چیک لسٹس میں ریفرنس ہیں۔

### Herramientas de telemetría y herramientas| Señal / Herramienta | مقصد |
| ------------- | ------- |
| `torii_sorafs_replication_sla_total` (cumplido/incumplido/pendiente) | paradas de replicación y detección de violaciones de SLA کرتا ہے۔ |
| `torii_sorafs_replication_backlog_total`, `torii_sorafs_replication_completion_latency_epochs` | profundidad del trabajo pendiente اور latencia de finalización کو clasificación کے لئے cuantificar کرتا ہے۔ |
| `torii_sorafs_gateway_refusals_total`, `torii_sorafs_manifest_submit_total{status="error"}` | fallas en el lado de la puerta de enlace دکھاتا ہے جو اکثر خراب implementar کے بعد آتے ہیں۔ |
| `npm run probe:portal` / `npm run probe:tryit-proxy` | sondas sintéticas جو libera puerta کرتے ہیں اور reversiones validan کرتے ہیں۔ |
| `npm run check:links` | puerta de enlace roto; ہر mitigación کے بعد استعمال ہوتا ہے۔ |
| `sorafs_cli manifest submit ... --alias-*` (`scripts/sorafs-pin-release.sh` کے ذریعے) | mecanismo de promoción/reversión de alias۔ |
| Placa `Docs Portal Publishing` Grafana (`dashboards/grafana/docs_portal.json`) | rechazos/alias/TLS/telemetría de replicación کو agregado کرتا ہے۔ Alertas de PagerDuty, paneles, pruebas, referencias |

## Runbook: implementación de ناکام یا خراب artefacto

### شروع ہونے کی شرائط

- Las sondas de vista previa/producción fallan (`npm run probe:portal -- --expect-release=...`).
- Alertas Grafana en `torii_sorafs_gateway_refusals_total` یا
  Lanzamiento de `torii_sorafs_manifest_submit_total{status="error"}` کے بعد۔
- Promoción de alias de control de calidad manual کے فوراً بعد rutas rotas یا Pruébelo fallas de proxy نوٹ کرے۔

### فوری روک تھام1. **Las implementaciones se congelan:** Canalización de CI کو `DEPLOY_FREEZE=1` سے marca کریں (entrada de flujo de trabajo de GitHub)
   یا pausa en el trabajo de Jenkins کریں تاکہ مزید artefactos نہ نکلیں۔
2. **Captura de artefactos کریں:** compilación fallida کے `build/checksums.sha256`,
   `portal.manifest*.{json,to,bundle,sig}`, salida de sonda ڈاؤن لوڈ کریں تاکہ rollback
   عین resume la referencia de کو کرے۔
3. **Partes interesadas کو اطلاع دیں:** almacenamiento SRE, Docs/DevRel líder, اور responsable de gobernanza
   (خصوصاً جب `docs.sora` متاثر ہو)۔

### رول بیک طریقہ کار

1. manifiesto de último estado en buen estado (LKG) کی شناخت کریں۔ flujo de trabajo de producción انہیں
   `artifacts/devportal/<release>/sorafs/portal.manifest.to` میں اسٹور کرتا ہے۔
2. ayudante de envío سے alias کو اس manifest پر دوبارہ bind کریں:

```bash
cd docs/portal
./scripts/sorafs-pin-release.sh \
  --build-dir build \
  --artifact-dir artifacts/revert-$(date +%Y%m%d%H%M) \
  --sorafs-dir artifacts/revert-$(date +%Y%m%d%H%M)/sorafs \
  --pin-min-replicas 5 \
  --alias "docs-prod-revert" \
  --alias-namespace "${PIN_ALIAS_NAMESPACE}" \
  --alias-name "${PIN_ALIAS_NAME}" \
  --alias-proof "${PIN_ALIAS_PROOF_PATH}" \
  --torii-url "${TORII_URL}" \
  --submitted-epoch "$(date +%Y%m%d)" \
  --authority "${AUTHORITY}" \
  --private-key "${PRIVATE_KEY}" \
  --skip-submit

# swap in the LKG artefacts before submission
cp /secure/archive/lkg/portal.manifest.to artifacts/.../sorafs/portal.manifest.to
cp /secure/archive/lkg/portal.manifest.bundle.json artifacts/.../sorafs/

cargo run -p sorafs_orchestrator --bin sorafs_cli -- \
  manifest submit \
  --manifest artifacts/.../sorafs/portal.manifest.to \
  --chunk-plan artifacts/.../sorafs/portal.plan.json \
  --torii-url "${TORII_URL}" \
  --authority "${AUTHORITY}" \
  --private-key "${PRIVATE_KEY}" \
  --alias-namespace "${PIN_ALIAS_NAMESPACE}" \
  --alias-name "${PIN_ALIAS_NAME}" \
  --alias-proof "${PIN_ALIAS_PROOF_PATH}" \
  --metadata rollback_from="${FAILED_RELEASE}" \
  --summary-out artifacts/.../sorafs/rollback.submit.json
```

3. resumen de reversión کو ticket de incidente میں LKG اور resúmenes de manifiesto fallidos کے ساتھ ریکارڈ کریں۔

### توثیق

1. `npm run probe:portal -- --expect-release=${LKG_TAG}`.
2. `npm run check:links`.
3. `sorafs_cli manifest verify-signature ...` y `sorafs_cli proof verify ...`
   (guía de implementación دیکھیں) تاکہ تصدیق ہو کہ manifiesto repromocionado archivado CAR کے ساتھ coincidencia کرتا ہے۔
4. `npm run probe:tryit-proxy` Aplicación Proxy de preparación Try-It کی بحالی یقینی ہو۔

### واقعے کے بعد

1. causa raíz سمجھ میں آنے کے بعد ہی canalización de implementación دوبارہ فعال کریں۔
2. [`devportal/deploy-guide`](./deploy-guide) میں entradas "Lecciones aprendidas" کو نئے puntos سے بھر دیں، اگر ہوں۔
3. conjunto de pruebas fallido (sonda, verificador de enlaces, etc.) کے لئے defectos فائل کریں۔

## Runbook - ریپلیکیشن میں گراوٹ

### شروع ہونے کی شرائط- الرٹ: `sum(torii_sorafs_replication_sla_total{outcome="met"}) /
  abrazadera_min(suma(torii_sorafs_replication_sla_total{resultado=~"cumplido|perdido"}), 1) <
  0.95` 10 min
- `torii_sorafs_replication_backlog_total > 10` 10 pulgadas (`pin-registry-ops.md` دیکھیں)۔
- Gobernanza ریلیز کے بعد alias کی دستیابی سست ہونے کی رپورٹ کرے۔

### ابتدائی جانچ

1. [`sorafs/pin-registry-ops`](../sorafs/pin-registry-ops) paneles de control دیکھیں تاکہ معلوم ہو سکے کہ backlog
   کسی flota de proveedores de clase de almacenamiento یا تک محدود ہے یا نہیں۔
2. Registros Torii میں `sorafs_registry::submit_manifest` advertencias چیک کریں تاکہ معلوم ہو سکے کہ los envíos fallan ہو رہی ہیں یا نہیں۔
3. `sorafs_cli manifest status --manifest ...` کے ذریعے réplica de muestra de salud کریں (resultados por proveedor دکھاتا ہے)۔

### تخفیفی اقدامات

1. `scripts/sorafs-pin-release.sh` کے ذریعے زیادہ recuento de réplicas (`--pin-min-replicas 7`) کے ساتھ manifiesto دوبارہ جاری کریں
   Carga del programador کو زیادہ proveedores پر پھیلا دے۔ نیا resumen del registro de incidentes میں ریکارڈ کریں۔
2. اگر trabajo pendiente کسی ایک proveedor سے جڑا ہو تو programador de replicación کے ذریعے اسے عارضی طور پر deshabilitar کریں
   (`pin-registry-ops.md` میں documentado) اور نیا envío de manifiesto کریں جو دوسرے proveedores کو alias actualización کرنے پر مجبور کرے۔
3. اگر frescura del alias, paridad de replicación سے زیادہ crítico ہو تو alias کو پہلے سے manifiesto de گرم preparado (`docs-preview`) پر rebind کریں،
   پھر SRE کے atraso borrar کرنے کے بعد seguimiento manifiesto publicar کریں۔

### بحالی اور اختتام1. `torii_sorafs_replication_sla_total{outcome="missed"}` Monitor کریں تاکہ meseta de recuento ہو۔
2. `sorafs_cli manifest status` salida کو evidencia کے طور پر captura کریں کہ ہر réplica دوبارہ compatible ہے۔
3. backlog de replicación post-mortem کو فائل یا اپ ڈیٹ کریں (escalado de proveedores, ajuste de fragmentos, etc.) ۔

## Runbook - اینالیٹکس یا ٹیلیمیٹری آؤٹیج

### شروع ہونے کی شرائط

- `npm run probe:portal` کامیاب ہو لیکن paneles de control `AnalyticsTracker` eventos کو >15 minutos تک ingerir نہ کریں۔
- Eventos eliminados de revisión de privacidad میں غیر متوقع اضافہ رپورٹ کرے۔
- `npm run probe:tryit-proxy` `/probe/analytics` las rutas fallan

### جوابی اقدامات

1. Verifique las entradas en tiempo de compilación: `DOCS_ANALYTICS_ENDPOINT` o `DOCS_ANALYTICS_SAMPLE_RATE`
   artefacto de liberación fallida (`build/release.json`) میں۔
2. `DOCS_ANALYTICS_ENDPOINT` کو colector de preparación پر punto کر کے `npm run probe:portal` دوبارہ چلائیں تاکہ las cargas útiles del rastreador emiten کرنا ثابت ہو۔
3. Colectores abajo ہوں تو `DOCS_ANALYTICS_ENDPOINT=""` configurar کریں اور reconstruir کریں تاکہ cortocircuito del rastreador کرے؛ Línea de tiempo del incidente de la ventana de corte میں ریکارڈ کریں۔
4. validar کریں کہ `scripts/check-links.mjs` اب بھی `checksums.sha256` huella digital کرتا ہے
   (interrupciones de análisis کو validación del mapa del sitio *bloqueo* نہیں کرنا چاہیے)۔
5. recuperación del recopilador ہونے کے بعد `npm run test:widgets` چلائیں تاکہ pruebas unitarias auxiliares de análisis ejecutar ہوں پھر republicar کریں۔

### واقعے کے بعد1. [`devportal/observability`](./observability) Limitaciones del colector میں نئی ​​یا requisitos de muestreo اپ ڈیٹ کریں۔
2. Política de datos analíticos کے باہر eliminar یا redactar ہوا ہو تو aviso de gobernanza فائل کریں۔

## سہ ماہی استقامت کی مشقیں

Taladros دونوں **ہر trimestre کے پہلے منگل** (enero/abril/julio/octubre) کو چلائیں
یا کسی بڑے cambio de infraestructura کے فوراً بعد۔ artefactos کو
`artifacts/devportal/drills/<YYYYMMDD>/` کے تحت محفوظ کریں۔

| مشق | مراحل | شواہد |
| ----- | ----- | -------- |
| Reversión de alias کی مشق | 1. تازہ ترین manifiesto de producción کے ساتھ Reversión de "Implementación fallida" دوبارہ چلائیں۔2. las sondas pasan ہونے کے بعد producción پر volver a vincular کریں۔3. `portal.manifest.submit.summary.json` اور registros de sonda کو taladro فولڈر میں ریکارڈ کریں۔ | `rollback.submit.json`, salida de sonda, etiqueta de lanzamiento de ensayo ۔ |
| مصنوعی توثیق کا آڈٹ | 1. producción اور puesta en escena کے خلاف `npm run probe:portal` اور `npm run probe:tryit-proxy` چلائیں۔2. `npm run check:links` چلائیں اور `build/link-report.json` archivo کریں۔3. Paneles Grafana کے capturas de pantalla/exportaciones adjuntas کریں جو confirmación del éxito de la sonda کریں۔ | Registros de sonda + `link-report.json` جو huella digital manifiesta کا حوالہ دیتا ہے۔ |

Ejercicios perdidos کو Administrador de Docs/DevRel اور Revisión de gobernanza de SRE تک escalar کریں، کیونکہ hoja de ruta یہ تقاضا کرتا ہے کہ
reversión de alias اور sondas del portal کے صحت مند رہنے کا determinista, evidencia trimestral موجود ہو۔

## PagerDuty y coordinación de guardia- Servicio PagerDuty **Publicación del portal de documentos** `dashboards/grafana/docs_portal.json` سے پیدا ہونے والے alertas کی مالک ہے۔
  Como `DocsPortal/GatewayRefusals`, `DocsPortal/AliasCache`, y `DocsPortal/TLSExpiry` Docs/DevRel página principal de la página کرتے ہیں
  اور Almacenamiento SRE secundario ہوتا ہے۔
- página آنے پر `DOCS_RELEASE_TAG` شامل کریں، متاثرہ Grafana paneles کے capturas de pantalla adjuntar کریں، اور mitigación شروع کرنے سے پہلے
  salida de verificación de enlace/sonda کو notas del incidente میں enlace کریں۔
- mitigación (reversión y reimplementación) کے بعد `npm run probe:portal`, `npm run check:links` دوبارہ چلائیں، اور تازہ Grafana
  captura de instantáneas کریں جو métricas کو umbrales کے اندر دکھائیں۔ تمام evidencia incidente de PagerDuty کے ساتھ adjuntar کریں
  قبل ازیں کہ اسے resolver کیا جائے۔
- اگر دو alertas ایک ساتھ incendio ہوں (مثال کے طور پر TLS caducidad اور backlog), تو rechazos کو پہلے triaje کریں
  (publicación روکیں), procedimiento de reversión چلائیں، پھر Almacenamiento SRE کے ساتھ puente پر TLS/backlog صاف کریں۔