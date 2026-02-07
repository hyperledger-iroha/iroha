---
lang: es
direction: ltr
source: docs/portal/docs/devportal/observability.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Observabilidad y análisis del portal

La hoja de ruta DOCS-SORA exige análisis, pruebas sintéticas y automatización de gravámenes.
Casos para cada compilación de vista previa. Cette note documente la plumberie livre avec le portail afin
Que los operadores puedan ramificar el monitoreo sin exponer las donaciones de los visitantes.

## Etiquetado de lanzamiento

- Definir `DOCS_RELEASE_TAG=<identifier>` (respaldo en `GIT_COMMIT` o `dev`) cuando du
  construir du portal. El valor está inyectado en `<meta name="sora-release">`
  Para que las sondas y los paneles de control distingan las implementaciones.
- `npm run build` género `build/release.json` (escrito par
  `scripts/write-checksums.mjs`) que decrit la etiqueta, la marca de tiempo y la
  `DOCS_RELEASE_SOURCE` opcional. Le meme fichier est embarque dans les artefactos vista previa y
  referencia par le rapport du link checker.

## Análisis de la vida privada

- Configurador `DOCS_ANALYTICS_ENDPOINT=<https://collector.example/ingest>` para activar
  le tracker leger. Las cargas útiles contienen `{ event, path, locale, release, ts }`
  sin metadatos de referencia o IP, y `navigator.sendBeacon` está utilizando lo posible
  Para evitar bloquear la navegación.
- Controle el cantillón con `DOCS_ANALYTICS_SAMPLE_RATE` (0-1). El rastreador conserva
  El último enviado de ruta y no hay eventos duplicados para la navegación de memes.
- La implementación se encuentra en `src/components/AnalyticsTracker.jsx` et est montee
  globalmente a través de `src/theme/Root.js`.

## Sondas sintéticas- `npm run probe:portal` emet des requetes GET sur des route courantes
  (`/`, `/norito/overview`, `/reference/torii-swagger`, etc.) y verifique que le
  La metaetiqueta `sora-release` corresponde a `--expect-release` (ou `DOCS_RELEASE_TAG`).
  Ejemplo:

```bash
PORTAL_BASE_URL="https://docs.staging.sora" \
DOCS_RELEASE_TAG="preview-42" \
npm run probe:portal -- --expect-release=preview-42
```

Les echecs sont rapportes par path, ce qui facilite le gate CD sur le succes des probes.

## Automatización de casos de gravámenes

- `npm run check:links` escanea `build/sitemap.xml`, asegúrese de que cada mapa entree vers un
  archivo local (verificando las reservas `index.html`), y escrito
  `build/link-report.json` contiene los metadatos de publicación, los totales, los echecs y
  El empreinte SHA-256 de `checksums.sha256` (expuesto como `manifest.id`) afin que cada
  rapport puisse etre rattache au manifeste d'artefact.
- El script devuelve un código distinto de cero cuando se realiza una página, después de que el CI puede bloquear
  les releases sur des route obsoletes ou cassees. Les rapports citant les chemins candidats
  tentes, ce qui aide a tracer les regressions de routage jusqu'a l'arbre de docs.

## Panel de control Grafana y alertas- `dashboards/grafana/docs_portal.json` publica el cuadro Grafana **Publicación del portal de documentos**.
  El contenido de los paneles siguientes:
  - *Rechazos de puerta de enlace (5 m)* utilizan el par de alcance `torii_sorafs_gateway_refusals_total`
    `profile`/`reason` para que el SRE detecte empujones políticos incorrectos o
    echecs de tokens.
  - *Resultados de actualización de caché de alias* y *Edad de prueba de alias p90* siguientes
    `torii_sorafs_alias_cache_*` para demostrar que las pruebas frescas existen antes de un corte
    sobre DNS.
  - *Recuentos de manifiestos de registro de pines* y la estadística *Recuento de alias activos* refleja el archivo
    backlog du pin-registry et le total des alias pour que la gouvernance puisse auditer
    cada liberación.
  - *Caducidad de TLS de la puerta de enlace (horas)* se cumple antes del acercamiento a la caducidad del certificado TLS del
    puerta de enlace de publicación (seuil d'alerte a 72 h).
  - *Resultados de replicación SLA* y *Replicación pendiente* vigilan la telemetría
    `torii_sorafs_replication_*` para asegurar que todas las réplicas respetan el
    publicación niveau GA después de la publicación.
- Utilice las variables de plantilla de enteros (`profile`, `reason`) para concentrarse en
  le profil de Publishing `docs.sora` o enqueter sur des pics sur l'ensemble des gateways.
- La ruta PagerDuty utiliza los paneles del tablero como antes: las alertas
  `DocsPortal/GatewayRefusals`, `DocsPortal/AliasCache` y `DocsPortal/TLSExpiry`se declencent lorsque la serie correspondiente depasse son seuil. Liez le runbook de
  Alerte a esta página para que el guardia pueda reanudar las solicitudes Prometheus exactas.

## Ensamblador todo

1. Colgante `npm run build`, define las variables de entorno de liberación/análisis y
   Laisser l'etape post-build emettre `checksums.sha256`, `release.json` y
   `link-report.json`.
2. Ejecutor `npm run probe:portal` con la vista previa del hotel con
   `--expect-release` rama en la etiqueta meme. Guarde la salida estándar para la lista de verificación
   de publicación.
3. Ejecutor `npm run check:links` para hacer eco rápidamente en las entradas de las cajas del mapa del sitio
   Y archive la relación JSON generada con la vista previa de los artefactos. La CI depone le
   dernier rapport dans `artifacts/docs_portal/link-report.json` pour que la gouvernance
   Puisse telecharger le bundle de preuves directement depuis les logs de build.
4. Router l'endpoint Analytics versus votre Collecteur respectueux de la vie privée (Plausible,
   OTEL ingiere auto-heberge, etc.) y asegura que los taux d'echantillonnage sont documentes
   Par la liberación a fin de que los paneles interpretan la corrección de los volúmenes.
5. El cable CI deja estas etapas a través de la vista previa/implementación de los flujos de trabajo
   (`.github/workflows/docs-portal-preview.yml`,
   `.github/workflows/docs-portal-deploy.yml`), donc les dry runs locaux ne doivent couvrir
   que le comportement specifique aux secrets.