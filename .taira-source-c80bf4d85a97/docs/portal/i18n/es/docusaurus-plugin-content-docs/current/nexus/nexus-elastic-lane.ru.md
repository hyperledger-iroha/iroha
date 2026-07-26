---
lang: es
direction: ltr
source: docs/portal/docs/nexus/nexus-elastic-lane.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: nexus-elástico-carril
título: Эластичное выделение carril (NX-7)
sidebar_label: carril de ropa elástica
descripción: Programa Bootstrap para la implementación de catálogos y archivos adjuntos en el carril Nexus.
---

:::nota Канонический источник
Esta página está escrita `docs/source/nexus_elastic_lane.md`. Deje copias sincronizadas, pero no todas las copias estarán disponibles en el portal.
:::

# Набор инструментов для эластичного выделения lane (NX-7)

> **Hoja de ruta del punto:** NX-7 - carril de instrumentos de ropa elástica  
> **Estado:** Instrumentos de seguridad: manifiestos genéricos, catálogos de fragmentos, cargas útiles Norito, pruebas de humo,
> un asistente para el paquete de prueba de carga que ayuda a configurar la latencia de la ranura + Manifestantes de los validadores, los programas de validación
> можно было публиковать без кастомных скриптов.

Esto proporciona a los operadores un nuevo ayudante `scripts/nexus_lane_bootstrap.sh`, un generador automático de carriles y fragmentos implementación del carril/espacio de datos y del catálogo. Цель - легко поднимать новые Nexus lanes (públicas o privadas) без ручного редактирования нескольких файлов и без ручного пересчета catálogo de geometrías.

## 1. Предварительные требования1. Gobernanza del carril de alias, espacio de datos, validación de datos, tolerancia a fallos (`f`) y resolución de políticas.
2. Validadores finales de cuentas (ID de cuenta) y espacios de nombres únicos.
3. Al descargar el repositorio de configuración de los usuarios, podrá agregar fragmentos de pantalla.
4. Пути для реестра манифестов lane (см. `nexus.registry.manifest_directory` y `cache_directory`).
5. Los contactos de los televisores/PagerDuty manejan el carril, todas las alertas se pueden conectar automáticamente.

## 2. Сгенерируйте артефакты carril

Запустите helper из корня репозитория:

```bash
scripts/nexus_lane_bootstrap.sh \
  --lane-alias "Payments Lane" \
  --lane-id 3 \
  --dataspace-alias payments \
  --governance-module parliament \
  --settlement-handle xor_global \
  --validator <i105-account-id> \
  --validator <i105-account-id> \
  --validator <i105-account-id> \
  --protected-namespace payments \
  --description "High-throughput interbank payments lane" \
  --dataspace-description "Payments dataspace" \
  --route-instruction finance::pacs008 \
  --encode-space-directory \
  --space-directory-out artifacts/nexus/payments_lane/payments.manifest.to \
  --telemetry-contact payments-ops@sora.org \
  --output-dir artifacts/nexus/payments_lane
```

Ключевые флаги:

- `--lane-id` se actualiza con el índice de nuevas entradas en `nexus.lane_catalog`.
- `--dataspace-alias` e `--dataspace-id/hash` amplían el espacio de datos más pequeño del catálogo (con el carril de identificación implementado).
- `--validator` puede insertar o escribir en `--validators-file`.
- `--route-instruction` / `--route-account` выводят готовые к вставке правила маршрутизации.
- `--metadata key=value` (или `--telemetry-contact/channel/runbook`) Runbook de contactos de software, paneles de control que se encuentran en lugares remotos.
- `--allow-runtime-upgrades` + `--runtime-upgrade-*` incorpora la actualización del tiempo de ejecución del gancho en el controlador, junto con el carril de la red de control del operador.
- `--encode-space-directory` автоматически вызывает `cargo xtask space-directory encode`. Utilice `--space-directory-out` o conecte el código `.to` a otro lado.El script contiene tres artefactos en `--output-dir` (en el catálogo de textos completos) y una codificación opcional en varios idiomas:

1. `<slug>.manifest.json`: muestra el carril con validadores de quórum, elimina espacios de nombres y actualizaciones de tiempo de ejecución de gancho opcionales.
2. `<slug>.catalog.toml` - TOML-fragment с `[[nexus.lane_catalog]]`, `[[nexus.dataspace_catalog]]` и запрошенными правилами маршрутизации. Tenga en cuenta que `fault_tolerance` está en un espacio de datos reducido y que puede configurar el comité de relé de carril (`3f+1`).
3. `<slug>.summary.json` - аудит-сводка с геометрией (slug, сеgmentы, метаданные), требуемыми шагами rollout и точной командой `cargo xtask space-directory encode` (bajo `space_directory_encode.command`). Utilice este código JSON para la incorporación de datos a la plataforma.
4. `<slug>.manifest.to` - появляется при `--encode-space-directory`; готов для потока Torii `iroha app space-directory manifest publish`.

Utilice `--dry-run`, coloque archivos JSON/fragmentos entre archivos de texto y `--force` para su impresión. существующих артефактов.

## 3. Примените изменения1. Escriba el manifiesto JSON en el archivo `nexus.registry.manifest_directory` (y en el directorio de caché, o en el registro de paquetes disponibles). Cambie el archivo o manifieste versiones en los repositorios de configuración.
2. Inserte el catálogo de fragmentos en `config/config.toml` (o en el módulo `config.d/*.toml`). Tenga en cuenta que `nexus.lane_count` no incluye `lane_id + 1` y elimina `nexus.routing_policy.rules`, que se encuentra en un nuevo carril.
3. Descargue (así como `--encode-space-directory`) y publique el manifiesto en Space Directory, implemente el comando en resumen (`space_directory_encode.command`). Este es el `.manifest.to`, el equipo Torii y el dispositivo del auditor; отправьте через `iroha app space-directory manifest publish`.
4. Introduzca `irohad --sora --config path/to/config.toml --trace-config` y guarde el rastro en el menú desplegable. Esto permite que esta nueva geometría se convierta en un segmento de slug/Kura.
5. Asegúrese de validar los validadores colocados en el carril después de desactivar el manifiesto/catálogo. Сохраните resumen JSON в тикете для будущих аудитов.

## 4. Соберите paquete распределения реестра

Al utilizar un manifiesto y una superposición personalizados, los operadores pueden transmitir la gobernanza de los carriles mediante configuraciones adecuadas en el servidor. El asistente muestra los manifiestos de copia en el diseño canónico, superpone el catálogo de gobernanza opcional para `nexus.registry.cache_directory` y puede cargar el tarball. офлайн-передачи:

```bash
scripts/nexus_lane_registry_bundle.sh \
  --manifest artifacts/nexus/payments_lane/payments.manifest.json \
  --output-dir artifacts/nexus/payments_lane/registry_bundle \
  --default-module parliament \
  --module name=parliament,module_type=parliament,param.quorum=2 \
  --bundle-out artifacts/nexus/payments_lane/registry_bundle.tar.gz
```

Выходы:1. `manifests/<slug>.manifest.json` - cópielo en el interior `nexus.registry.manifest_directory`.
2. `cache/governance_catalog.json` - coloque en `nexus.registry.cache_directory`. Bloqueo del módulo `--module` del módulo de gobernanza (NX-2) que contiene caché superposición вместо редактирования `config.toml`.
3. `summary.json` - содержит хеши, метаданные superposición e instrucciones para los operadores.
4. Opcionalmente `registry_bundle.tar.*` - готово для SCP, S3 o трекеров артефактов.

Sincronice todo el catálogo (o archivos) en el validador de archivos, descomprima las conexiones con espacio de aire y escriba el administrador + superposición de caché en este пути реестра перед перезапуском Torii.

## 5. Pruebas de humo validadas

Después de la medición Torii, instale un nuevo ayudante de humo, cómo verificar, qué carril tiene `manifest_ready=true`, medidas métricas. ожидаемое количество carriles y ancho de vía sellado чист. Lanes, которым нужны манифесты, обязаны иметь непустой `manifest_path`; ayudante, una gran cantidad de archivos adjuntos que se pueden implementar en el NX-7 es el siguiente manifiesto:

```bash
scripts/nexus_lane_smoke.py \
  --status-url https://torii.example.com/v1/sumeragi/status \
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
```Utilice `--insecure` para probar el documento autofirmado. Скрипт завершается ненулевым кодом, если lane отсутствует, sellado, o métricas/telemetrías с oжидаемыми значениями. Utilice `--min-block-height`, `--max-finality-lag`, `--max-settlement-backlog` y `--max-headroom-events` para ubicar el televisor en el carril (ver блока/финальность/backlog/headroom) en las ramas secundarias, y сочетайте их с `--max-slot-p95` / `--max-slot-p99` (и `--min-slot-samples`) para la sobлюдения El NX-18 en una ranura de almacenamiento no necesita ayuda.

Para un controlador con espacio de aire (o CI) se puede programar automáticamente desde el punto final Torii:

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

Los accesorios adjuntos en `fixtures/nexus/lanes/` incluyen artefactos, un asistente de arranque y nuevos manifiestos que pueden eliminar pelusas sin archivos. скриптов. CI funciona con estos dispositivos como `ci/check_nexus_lane_smoke.sh` e `ci/check_nexus_lane_registry_bundle.sh` (alias: `make check-nexus-lanes`), cómo utilizarlo y cómo ayudante de humo NX-7. Se incluyen la carga útil del formato público y el paquete de resúmenes/superposiciones.