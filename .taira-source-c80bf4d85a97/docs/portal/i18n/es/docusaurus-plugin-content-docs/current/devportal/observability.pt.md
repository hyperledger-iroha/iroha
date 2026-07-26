---
lang: es
direction: ltr
source: docs/portal/docs/devportal/observability.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Observabilidade e Analytics del portal.

El roadmap DOCS-SORA exige análisis, sondas sintéticas y automatización de enlaces quebrados
para cada construcción de vista previa. Esta nota documenta a infraestructura que ahora acompaña al portal
para que los operadores conecten el monitoreo sin dejar datos de visitantes.

## Etiquetado de lanzamiento

- Defina `DOCS_RELEASE_TAG=<identifier>` (respaldo para `GIT_COMMIT` o `dev`) ao
  construir o portal. El valor e inyectado en `<meta name="sora-release">`
  para que las sondas y los paneles distingan las implementaciones.
- `npm run build` emite `build/release.json` (escrito por
  `scripts/write-checksums.mjs`) descrevendo la etiqueta, la marca de tiempo y la
  `DOCS_RELEASE_SOURCE` opcional. O mesmo archivo e empacotado nos artefatos de vista previa e
  referenciado pelo relatorio do link checker.

## Analytics con preservacao de privacidade

- Configurar `DOCS_ANALYTICS_ENDPOINT=<https://collector.example/ingest>` para
  habilitar el nivel del rastreador. Contenido de cargas útiles `{ event, path, locale, release, ts }`
  sem metadatos de referencia o IP, e `navigator.sendBeacon` y usados siempre que sea posible
  para evitar bloquear navegación.
- Control de muestreo com `DOCS_ANALYTICS_SAMPLE_RATE` (0-1). O rastreador armazena
  El último camino enviado y nunca emite eventos duplicados para a mesma navegación.
- A implementacao fica em `src/components/AnalyticsTracker.jsx` e e montada
  globalmente vía `src/theme/Root.js`.

##Sondas sinteticas- `npm run probe:portal` dispara solicitudes GET contra rotas comunes
  (`/`, `/norito/overview`, `/reference/torii-swagger`, etc.) y verifique si
  La metaetiqueta `sora-release` corresponde a `--expect-release` (o `DOCS_RELEASE_TAG`).
  Ejemplo:

```bash
PORTAL_BASE_URL="https://docs.staging.sora" \
DOCS_RELEASE_TAG="preview-42" \
npm run probe:portal -- --expect-release=preview-42
```

Falhas sao reportadas por path, facilitando gatear o CD pelo sucesso dos probes.

## Automacao de enlaces quebrados

- `npm run check:links` varre `build/sitemap.xml`, garantía de que cada entrada mapeia para
  um archivo local (checando fallbacks `index.html`), y escreve
  `build/link-report.json` contiene metadatos de publicación, total, falhas e impresión
  SHA-256 de `checksums.sha256` (exposta como `manifest.id`) para que cada relatorio posible
  Ser ligado al manifiesto del artefato.
- El script termina con el código nao-zero cuando falta una página, así como un CI puede bloquear
  libera em rotas antigas o quebradas. Os relatorios citam os caminhos candidatos tentados,
  o que ayuda a rastrear regresos de roteamento de volta para arvore de docs.

## Panel de control Grafana y alertas- `dashboards/grafana/docs_portal.json` placa pública Grafana **Publicación del portal de documentos**.
  Ele incluye los siguientes dolores:
  - *Rechazos de puerta de enlace (5m)* usa `torii_sorafs_gateway_refusals_total` com escopo
    `profile`/`reason` para que SRE detecte empujes de ruinas políticas o falhas de tokens.
  - *Resultados de actualización de caché de alias* y *Edad de prueba de alias p90* acompanham
    `torii_sorafs_alias_cache_*` para probar que existen pruebas recientes antes de un corte
    sobre el DNS.
  - *Recuentos de manifiestos de registro de PIN* y estadísticas *Recuento de alias activos* espelham o
    backlog do pin-registry y el total de alias para que un gobierno pueda auditar
    cada lanzamiento.
  - *Caducidad de la puerta de enlace TLS (horas)* destaca cuando el certificado TLS de la puerta de enlace de publicación
    se aproxima do vencimento (limiar de alerta em 72 h).
  - *Resultados del SLA de replicación* y *Replicación pendiente* acompañan a la telemetría
    `torii_sorafs_replication_*` para garantizar que todas las réplicas estén disponibles
    patamar GA apos a publicacao.
- Use as variaveis de template embutidas (`profile`, `reason`) para enfocar no perfil
  de publicación `docs.sora` o investigar picos en todos los gateways.
- El enrutamiento de PagerDuty usa los dolores del tablero como evidencia: alertas
  `DocsPortal/GatewayRefusals`, `DocsPortal/AliasCache` e `DocsPortal/TLSExpiry`
  disparam quando a serie corresponsal ultrapassa seus limiares. liga o runbookdo alerta a esta página para que o on-call consiga repetir as queries exatas do Prometheus.

## Juntando todo

1. Durante `npm run build`, defina as variaveis de ambiente de release/analytics e
   deixe o pos-build emite `checksums.sha256`, `release.json` e
   `link-report.json`.
2. Rode `npm run probe:portal` contra el nombre de host de vista previa com
   `--expect-release` conectado a la etiqueta mesmo. Salve o stdout para una lista de verificación de publicación.
3. Rode `npm run check:links` para falhar rapido em entradas quebradas del mapa del sitio y archivo
   El relato JSON generado junto con los artefatos de vista previa. Un depósito de CI o
   Último relato en `artifacts/docs_portal/link-report.json` para que a gobernador.
   bajo el paquete de evidencias directamente dos registros de construcción.
4. Encaminhe o endpoint de Analytics para su coletor com preservacao de privacidade (Plausible,
   OTEL ingiere autohospedado, etc.) y garantiza que como taxas de amostragem estejam documentadas por
   Lanzamiento para que los paneles interpreten los volúmenes correctamente.
5. A CI ja conecta esses passos nos flujos de trabajo de vista previa/implementación
   (`.github/workflows/docs-portal-preview.yml`,
   `.github/workflows/docs-portal-deploy.yml`), estos son los ensayos ubicados en lugares tan precisos
   Cobrir comportamiento específico de secretos.