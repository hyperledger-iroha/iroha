---
lang: es
direction: ltr
source: docs/portal/docs/devportal/observability.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# پورٹل آبزرویبیلٹی اور اینالٹکس

DOCS-SORA Vista previa de compilación, análisis, sondas sintéticas y automatización de enlaces rotos
یہ نوٹ وہ plomería بیان کرتا ہے جو اب پورٹل کے ساتھ barco ہوتی ہے تاکہ operadores de monitoreo جوڑ سکیں
بغیر datos de visitantes لیک کئے۔

## Liberar etiquetado

- `DOCS_RELEASE_TAG=<identifier>` سیٹ کریں (respaldo `GIT_COMMIT` یا `dev`) جب پورٹل build ہو۔
  ویلیو `<meta name="sora-release">` میں inyectar ہوتی ہے تاکہ sondas اور implementaciones de paneles کو distinguir کر سکیں۔
- `npm run build` `build/release.json` emite کرتا ہے (جسے `scripts/write-checksums.mjs` لکھتا ہے)
  جو etiqueta, marca de tiempo, اور opcional `DOCS_RELEASE_SOURCE` کو describir کرتا ہے۔ یہی فائل vista previa de artefactos paquete میں
  ہوتی ہے اور verificador de enlaces رپورٹ میں referir ہوتی ہے۔

## Análisis que preservan la privacidad

- `DOCS_ANALYTICS_ENDPOINT=<https://collector.example/ingest>` configurar کریں تاکہ rastreador ligero habilitar ہو۔
  cargas útiles میں `{ event, path, locale, release, ts }` شامل ہوتے ہیں بغیر referente یا Metadatos IP کے، اور
  `navigator.sendBeacon` جہاں ممکن ہو استعمال ہوتا ہے تاکہ bloque de navegación نہ ہو۔
- `DOCS_ANALYTICS_SAMPLE_RATE` (0-1) کے ساتھ control de muestreo کریں۔ ruta del último envío del rastreador ذخیرہ کرتا ہے
  اور ایک ہی navegación کے لئے eventos duplicados emiten نہیں کرتا۔
- implementación `src/components/AnalyticsTracker.jsx` میں ہے اور `src/theme/Root.js` کے ذریعے montaje global ہوتی ہے۔

## Sondas sintéticas- `npm run probe:portal` عام rutas کے خلاف GET solicitudes بھیجتا ہے
  (`/`, `/norito/overview`, `/reference/torii-swagger`, وغیرہ) اور verificar کرتا ہے کہ
  `sora-release` metaetiqueta `--expect-release` (یا `DOCS_RELEASE_TAG`) سے coincide con کرتا ہے۔ Nombre:

```bash
PORTAL_BASE_URL="https://docs.staging.sora" \
DOCS_RELEASE_TAG="preview-42" \
npm run probe:portal -- --expect-release=preview-42
```

Fallos ہر ruta کے حساب سے informe ہوتے ہیں، جس سے puerta de CD کرنا آسان ہو جاتا ہے۔

## Automatización de enlaces rotos

- `npm run check:links` `build/sitemap.xml` escaneo کرتا ہے، ہر entrada کو archivo local سے mapa ہونا یقینی بناتا ہے
  (`index.html` respaldos چیک کرتا ہے), y `build/link-report.json` لکھتا ہے جس میں metadatos de lanzamiento, totales, fallas,
  اور `checksums.sha256` کا SHA-256 huella digital شامل ہوتا ہے (جو `manifest.id` کے طور پر exponen ہوتا ہے) تاکہ ہر رپورٹ
  manifiesto de artefacto سے جوڑی جا سکے۔
- script distinto de cero پر salida کرتا ہے جب کوئی صفحہ falta ہو، اس لئے CI پرانی یا rutas rotas پر lanzamientos روک سکتا ہے۔
  informes, rutas de candidatos, citar, probar, regresiones de enrutamiento, árbol de documentos, seguimiento, seguimiento

## Grafana panel de control y alertas- Placa `dashboards/grafana/docs_portal.json` Grafana **Publicación del portal de documentos** publicar کرتا ہے۔
  اس میں یہ paneles شامل ہیں:
  - *Rechazos de puerta de enlace (5 m)* `torii_sorafs_gateway_refusals_total` کو `profile`/`reason` کے alcance کے ساتھ استعمال کرتا ہے
    La política de SRE خراب impulsa یا fallas de token detectan کر سکیں۔
  - *Resultados de actualización de caché de alias* اور *Edad de prueba de alias p90* `torii_sorafs_alias_cache_*` کو track کرتے ہیں
    تاکہ DNS cortado sobre سے پہلے pruebas nuevas موجود ہونے کا ثبوت ملے۔
  - *Recuentos de manifiestos de registro de pines* y *Recuento de alias activos* acumulación de registros de pines estadísticos y alias totales کو reflejan کرتے ہیں
    تاکہ gobernanza ہر lanzamiento کو auditoría کر سکے۔
  - *Caducidad de TLS de puerta de enlace (horas)* اس وقت resaltado کرتا ہے جب puerta de enlace de publicación کا Caducidad del certificado TLS کے قریب ہو
    (umbral de alerta 72 h)۔
  - *Resultados del SLA de replicación* اور *Replicación pendiente* Telemetría `torii_sorafs_replication_*` پر نظر رکھتے ہیں
    تاکہ publicar کے بعد تمام réplicas de la barra GA پر ہوں۔
- variables de plantilla integradas (`profile`, `reason`) استعمال کریں تاکہ `docs.sora` perfil de publicación پر focus کیا جا سکے
  یا تمام puertas de enlace میں picos investigar کئے جا سکیں۔
- Paneles del panel de enrutamiento de PagerDuty کو evidencia کے طور پر استعمال کرتا ہے: alertas
  `DocsPortal/GatewayRefusals`, `DocsPortal/AliasCache`, y `DocsPortal/TLSExpiry` تب fire serie کرتے ہیں جب متعلقہ
  umbrales cruzados کرے۔ runbook de alerta کو اسی صفحے سے enlace کریں تاکہ ingenieros de guardia consultas exactas Prometheus reproducción کر سکیں۔## Juntándolo

1. `npm run build` Conjunto de variables de entorno de lanzamiento/análisis de کے دوران کریں اور paso posterior a la compilación کو
   `checksums.sha256`, `release.json`, y `link-report.json` emiten کرنے دیں۔
2. vista previa del nombre de host کے خلاف `npm run probe:portal` چلائیں اور `--expect-release` کو اسی etiqueta سے cable کریں۔
   lista de verificación de publicación کے لئے stdout محفوظ کریں۔
3. `npm run check:links` Error en las entradas del mapa del sitio rotas, error en la generación del informe JSON
   vista previa de artefactos کے ساتھ archivo کریں۔ Último informe de CI کو `artifacts/docs_portal/link-report.json` میں drop کرتا ہے
   تاکہ registros de compilación de gobernanza سے paquete de evidencia سیدھا descargar کر سکے۔
4. punto final de análisis کو اپنے recopilador que preserva la privacidad (Plausible, ingesta de OTEL autohospedado وغیرہ) کی طرف forward کریں
   اور ہر lanzamiento کے لئے tasas de muestreo documento کریں تاکہ tableros cuenta کو درست interpretar کریں۔
5. CI پہلے ہی vista previa/implementación de flujos de trabajo میں ان pasos کو wire کر چکا ہے
   (`.github/workflows/docs-portal-preview.yml`,
   `.github/workflows/docs-portal-deploy.yml`) ، اس لئے simulacros locales میں صرف cubierta de comportamiento específico de secretos کرنا ہوتا ہے۔