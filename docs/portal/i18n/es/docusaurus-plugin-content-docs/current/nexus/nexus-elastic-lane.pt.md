---
lang: es
direction: ltr
source: docs/portal/docs/nexus/nexus-elastic-lane.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: nexus-elástico-carril
título: Provisionamento de carril elástico (NX-7)
sidebar_label: Aprovisionamiento de carril elástico
descripción: Flujo de bootstrap para crear manifests de lane Nexus, entradas de catálogo y evidencia de rollout.
---

:::nota Fuente canónica
Esta página espelha `docs/source/nexus_elastic_lane.md`. Mantenha ambas as copias alinhadas ate que o barrido de localización chegue ao portal.
:::

# Kit de aprovisionamiento de carril elástico (NX-7)

> **Artículo de la hoja de ruta:** NX-7 - herramientas de aprovisionamiento de carril elástico  
> **Estado:** herramientas completas: manifiestos de gera, fragmentos de catálogo, cargas útiles Norito, pruebas de humo,
> e o helper de bundle de load-test agora costura gating de latencia por slot + manifests de evidencia para que as rodadas de carga de validadores
> possam ser publicadas sem scripting sob medida.

Esta guía leva operadores con el novo helper `scripts/nexus_lane_bootstrap.sh` que automatiza la generación de manifiestos de lane, fragmentos de catálogo de lane/dataspace y evidencia de rollout. El objetivo es facilitar la creación de nuevas líneas Nexus (públicas o privadas) sin editar manualmente varios archivos sin volver a derivar la geometría del catálogo manualmente.

## 1. Requisitos previos1. Aprobación de gobernanza para el alias de lane, dataspace, conjunto de validadores, tolerancia a falhas (`f`) e política de asentamiento.
2. Una lista final de validadores (ID de contacto) y una lista de espacios de nombres protegidos.
3. Acceda al repositorio de configuración del nodo para poder anexar los fragmentos generados.
4. Caminhos para o registro de manifests de lane (veja `nexus.registry.manifest_directory` e `cache_directory`).
5. Contactos de telemetría/controles de PagerDuty para el carril, para que os alertas estén conectadas tan pronto como el carril esté en línea.

## 2. Gere artefatos de lane

Ejecute el ayudante desde la raíz del repositorio:

```bash
scripts/nexus_lane_bootstrap.sh \
  --lane-alias "Payments Lane" \
  --lane-id 3 \
  --dataspace-alias payments \
  --governance-module parliament \
  --settlement-handle xor_global \
  --validator i105... \
  --validator i105... \
  --validator i105... \
  --protected-namespace payments \
  --description "High-throughput interbank payments lane" \
  --dataspace-description "Payments dataspace" \
  --route-instruction finance::pacs008 \
  --encode-space-directory \
  --space-directory-out artifacts/nexus/payments_lane/payments.manifest.to \
  --telemetry-contact payments-ops@sora.org \
  --output-dir artifacts/nexus/payments_lane
```

Banderas chave:- `--lane-id` debe corresponder al índice de la nueva entrada en `nexus.lane_catalog`.
- `--dataspace-alias` e `--dataspace-id/hash` controlan la entrada del catálogo del espacio de datos (por padrao usa o id del carril cuando se omite).
- `--validator` puede repetirse o llenarse de `--validators-file`.
- `--route-instruction` / `--route-account` emite regras de roteamento prontas para colar.
- `--metadata key=value` (o `--telemetry-contact/channel/runbook`) captura contactos de runbook para que los paneles muestren los corretos de los propietarios.
- `--allow-runtime-upgrades` + `--runtime-upgrade-*` agregan el gancho de runtime-upgrade al manifiesto cuando el carril solicita controles establecidos por el operador.
- `--encode-space-directory` cambia automáticamente a `cargo xtask space-directory encode`. Combine con `--space-directory-out` cuando desee que el archivo `.to` esté codificado para otro lugar por defecto.

El script produce tres artefactos dentro de `--output-dir` (por padrao o directorio actual), pero un cuarto opcional cuando la codificación está habilitada:1. `<slug>.manifest.json`: manifiesto del carril que contiene el quórum de validadores, espacios de nombres protegidos y metadados opcionales del gancho de actualización en tiempo de ejecución.
2. `<slug>.catalog.toml` - un fragmento TOML con `[[nexus.lane_catalog]]`, `[[nexus.dataspace_catalog]]` y quaisquer registros de roteamento solicitados. Garanta que `fault_tolerance` está definida en la entrada de dataspace para dimensionar o comité de lane-relay (`3f+1`).
3. `<slug>.summary.json` - resumen de auditoría descrevendo a geometria (slug, segmentos, metadados) mais os passos de rollout requeridos y o comando exato de `cargo xtask space-directory encode` (em `space_directory_encode.command`). Anexe esse JSON ao ticket de onboarding como evidencia.
4. `<slug>.manifest.to` - certificado cuando `--encode-space-directory` está habilitado; pronto para el flujo `iroha app space-directory manifest publish` a Torii.

Use `--dry-run` para visualizar los JSON/snippets sin grabar archivos e `--force` para sobrescrever artefatos existentes.

## 3. Aplique como mudancas1. Copie el manifiesto JSON para `nexus.registry.manifest_directory` configurado (y para el directorio de caché y el registro de paquetes remotos). Comité o archivo se manifiesta sao versionados no su repositorio de configuración.
2. Anexo o fragmento de catálogo en `config/config.toml` (o no `config.d/*.toml` apropiado). Garanta que `nexus.lane_count` seja pelo menos `lane_id + 1` e atualize quaisquer `nexus.routing_policy.rules` que devam apontar para o novo lane.
3. Codifique (se voce pulou `--encode-space-directory`) y publique o manifest no Space Directory usando o comando capturado sin resumen (`space_directory_encode.command`). Isso produz o payload `.manifest.to` que o Torii espera e registra evidencia para auditores; envidia vía `iroha app space-directory manifest publish`.
4. Ejecute `irohad --sora --config path/to/config.toml --trace-config` y archive a sayda do trace no ticket de rollout. Esto prueba que una nueva geometría corresponde a slug/segmentos de Kura gerados.
5. Reinicie os validadores atribuidos ao lane quando as mudancas de manifest/catalogo estiverem implantadas. Mantenha o resumen JSON sin ticket para auditorias futuras.

## 4. Monte un paquete de distribución del registro

Empacote el manifiesto generado y la superposición para que los operadores puedan distribuir datos de gobierno de líneas sin editar configuraciones en cada host. El asistente de agrupación de manifiestos de copia para el diseño canónico, produce una superposición opcional del catálogo de gobierno para `nexus.registry.cache_directory` y puede emitir un tarball para transferencias fuera de línea:```bash
scripts/nexus_lane_registry_bundle.sh \
  --manifest artifacts/nexus/payments_lane/payments.manifest.json \
  --output-dir artifacts/nexus/payments_lane/registry_bundle \
  --default-module parliament \
  --module name=parliament,module_type=parliament,param.quorum=2 \
  --bundle-out artifacts/nexus/payments_lane/registry_bundle.tar.gz
```

Saidas:

1. `manifests/<slug>.manifest.json` - copie estos archivos para o `nexus.registry.manifest_directory` configurado.
2. `cache/governance_catalog.json` - coloque en `nexus.registry.cache_directory`. Cada entrada `--module` tendrá una definición de módulo enchufable, lo que permitirá intercambiar el módulo de gobierno (NX-2) para actualizar o superponer la caché al editar `config.toml`.
3. `summary.json`: incluye hashes, metadados de superposición e instrucciones para operadores.
4. Opcional `registry_bundle.tar.*` - pronto para SCP, S3 o rastreadores de artefatos.

Sincronice el directorio completo (o el archivo) para cada validador, extraiga los hosts con espacio de aire y copie los manifiestos + superposición de caché para sus caminos de registro antes de reiniciar el Torii.

## 5. Pruebas de humo de validadores

Después de que o Torii reinicie, ejecute o novo helper de smoke para verificar se o lane reporta `manifest_ready=true`, se as métricas exponen un contagio esperado de lanes y se o calibre de sellado esta limpio. Lanes que exigem manifests devem expor um `manifest_path` nao vazio; El ayudante ahora falla inmediatamente cuando el camino falta para que cada despliegue NX-7 incluya una evidencia del manifiesto assinado:

```bash
scripts/nexus_lane_smoke.py \
  --status-url https://torii.example.com/v2/sumeragi/status \
  --metrics-url https://torii.example.com/metrics \
  --lane-alias payments \
  --expected-lane-count 3 \
  --min-da-quorum 0.95 \
  --max-oracle-staleness 75 \
  --expected-oracle-twap 60 \
  --oracle-twap-tolerance 5 \
  --max-oracle-haircut-bps 75 \
  --min-settlement-buffer 0.25 \
  --min-block-height 1000 \
  --max-finality-lag 4 \
  --max-settlement-backlog 0.5 \
  --max-headroom-events 0 \
  --max-slot-p95 1000 \
  --max-slot-p99 1100 \
  --min-slot-samples 10
```Adicione `--insecure` para probar ambientes autofirmado. O script sai com codigo nao zero se o lane estiver ausente, sealed ou se metricas/telemetria divergirem dos valores esperados. Utilice las perillas `--min-block-height`, `--max-finality-lag`, `--max-settlement-backlog` e `--max-headroom-events` para mantener la telemetría por carril (altura de bloque/finalidade/backlog/headroom) dentro de sus límites operativos y combinar con `--max-slot-p95` / `--max-slot-p99` (más `--min-slot-samples`) para importar las metas de duración de la ranura NX-18 sin ayuda.

Para validar air-gapped (o CI) puede reproducir una respuesta Torii capturada al acceder a un endpoint en vivo:

```bash
scripts/nexus_lane_smoke.py \
  --status-file fixtures/nexus/lanes/status_ready.json \
  --metrics-file fixtures/nexus/lanes/metrics_ready.prom \
  --lane-alias core \
  --lane-alias payments \
  --expected-lane-count 3 \
  --min-da-quorum 0.95 \
  --max-oracle-staleness 75 \
  --expected-oracle-twap 60 \
  --oracle-twap-tolerance 5 \
  --max-oracle-haircut-bps 75 \
  --min-settlement-buffer 0.25 \
  --min-block-height 1000 \
  --max-finality-lag 4 \
  --max-settlement-backlog 0.5 \
  --max-headroom-events 0 \
  --max-slot-p95 1000 \
  --max-slot-p99 1100 \
  --min-slot-samples 10
```

Los accesorios grabados en `fixtures/nexus/lanes/` reflejan los artefatos producidos por el ayudante de bootstrap para que novos manifests possam ser lintados sem scripting sob medida. Un CI ejecuta el mesmo fluxo a través de `ci/check_nexus_lane_smoke.sh` e `ci/check_nexus_lane_registry_bundle.sh` (alias: `make check-nexus-lanes`) para probar que el ayudante de smoke NX-7 continúa siendo compatible con el formato de carga útil publicado y garantiza que los resúmenes/overlays hacen paquete de reproducción de archivos.