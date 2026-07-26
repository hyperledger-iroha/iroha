---
lang: es
direction: ltr
source: docs/portal/docs/devportal/observability.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Наблюдаемость и аналитика портала

Tarjeta completa DOCS-SORA Analizadores de sondas, sondas sintéticas y automatizaciones
проверки битых ссылок для каждого vista previa de la compilación. En esta descripción de infraestructura,
которая поставляется вместе с порталом, чтобы операторы могли подключить MONITORING
без утечки данных посетителей.

## Тегирование релиза

- Instale `DOCS_RELEASE_TAG=<identifier>` (respaldo en `GIT_COMMIT` o `dev`) en el portal de la tienda.
  Значение внедряется в `<meta name="sora-release">`, чтобы sondas y tableros de instrumentos могли отличать
  развертывания.
- `npm run build` contiene `build/release.json` (descarga `scripts/write-checksums.mjs`), где описаны
  тег, marca de tiempo y opcionalmente `DOCS_RELEASE_SOURCE`. Este archivo se muestra en los artículos de vista previa
  и упоминается в отчете verificador de enlaces.

## Analítica de privacidad personal

- Introduzca `DOCS_ANALYTICS_ENDPOINT=<https://collector.example/ingest>` para un vehículo eléctrico de gran tamaño.
  Los archivos `{ event, path, locale, release, ts }` se almacenan sin referencia o con metadanos IP y por
  Si utiliza `navigator.sendBeacon`, no bloqueará la navegación.
- Управляйте выборкой через `DOCS_ANALYTICS_SAMPLE_RATE` (0-1). Трекер хранит последний отправленный camino
  и никогда не отправляет дубликаты событий для одной навигации.
- Realización de registros en `src/components/AnalyticsTracker.jsx` y montajes globales en `src/theme/Root.js`.

## Problemas sintéticos- `npm run probe:portal` para eliminar GET en marcas típicas (`/`, `/norito/overview`,
  `/reference/torii-swagger`, y т.д.) y prueba, qué metada es `sora-release` соответствует
  `--expect-release` (o `DOCS_RELEASE_TAG`). Ejemplo:

```bash
PORTAL_BASE_URL="https://docs.staging.sora" \
DOCS_RELEASE_TAG="preview-42" \
npm run probe:portal -- --expect-release=preview-42
```

Сбои показываются по path, что упрощает gate CD по успеху проб.

## Автоматизация битых ссылок

- `npm run check:links` escanea el `build/sitemap.xml`, utiliza el archivo local para borrar los mapas
  (contratación alternativa `index.html`), y un poco de `build/link-report.json`, una versión actualizada de las metadanas, historias,
  ошибки и SHA-256 отпечаток `checksums.sha256` (выставлен как `manifest.id`), чтобы каждый отчет можно
  было связать с манифестом артефакта.
- El script se guarda con códigos no deseados, cuando se bloquea una página y el CI puede bloquear las respuestas.
  при устаревших или сломанных маршрутах. Отчеты содержат кандидатные пути, которые пытались открыть, что
  помогает проследить регрессию маршрутизации до дерева docs.

## Дашборд Grafana y alertas- `dashboards/grafana/docs_portal.json` publica el documento Grafana **Publicación del portal de documentos**.
  Она включает следующие PANELи:
  - *Rechazos de puerta de enlace (5m)* использует `torii_sorafs_gateway_refusals_total` в разрезе
    `profile`/`reason`, чтобы SRE pueden bloquear estrategias de impulso de políticas o сбои токенов.
  - *Resultados de actualización de caché de alias* y *Edad de prueba de alias p90* отслеживают `torii_sorafs_alias_cache_*`,
    чтобы подтвердить наличие свежих перед DNS cut over.
  - *Recuentos de manifiestos de registro de pines* y luego *Recuento de alias activos* para aumentar el registro de pines pendientes y otras funciones
    число alias, чтобы gobernanza могла аудировать каждый релиз.
  - *Caducidad de la puerta de enlace TLS (horas)* подсвечивает приближение истечения Puerta de enlace de publicación de certificados TLS
    (alerta de 72 h).
  - *Resultados del SLA de replicación* y *Replicación pendiente* следят за телеметрией
    `torii_sorafs_replication_*`, чтобы убедиться, что все реплики соответствуют GA после публикации.
- Utilice una plantilla permanente (`profile`, `reason`), para obtener más información
  perfil de publicación `docs.sora` исследовать всплески по всем шлюзам.
- El enrutamiento PagerDuty utiliza paneles de tablero para el almacenamiento: alertas
  `DocsPortal/GatewayRefusals`, `DocsPortal/AliasCache` y `DocsPortal/TLSExpiry` se fusionaron,
  когда соответствующие серии выходят за порог. Consulte la alerta del runbook en esta página,
  Estos ingenieros de guardia pueden activar el sistema Prometheus.

## Сводим вместе1. Во время `npm run build` установите переменные окружения release/analytics y дайте post-build шагу
   escriba `checksums.sha256`, `release.json` y `link-report.json`.
2. Introduzca `npm run probe:portal` para obtener la vista previa del nombre de host en `--expect-release`, que se conecta con el tema.
   Сохраните stdout для Publishing чеклиста.
3. Introduzca `npm run check:links`, para abrir el mapa del sitio y descargarlo
   El formato JSON creado se muestra en la vista previa de los artefactos. CI кладет последний отчет в
   `artifacts/docs_portal/link-report.json`, este paquete de pruebas de gobernanza puede descargarse desde la compilación de logotipos.
4. Mejorar el punto final de análisis con un recopilador que preserva la privacidad (ingesta de OTEL plausible y autohospedada y otros)
   Y tenga en cuenta qué documentos de frecuencia de muestreo están disponibles para el registro de datos y cómo se interpretan correctamente los paneles de control.
   счетчики.
5. CI para implementar y ejecutar flujos de trabajo de vista previa/implementación
   (`.github/workflows/docs-portal-preview.yml`,
   `.github/workflows/docs-portal-deploy.yml`), поэтому локальные ensayos en seco должны покрывать
   только поведение, связанное с секретами.