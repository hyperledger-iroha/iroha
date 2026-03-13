---
lang: es
direction: ltr
source: docs/portal/docs/nexus/nexus-default-lane-quickstart.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: nexus-carril-predeterminado-inicio rápido
título: Carril predeterminado de inicio de Быстрый старт (NX-5)
sidebar_label: carril predeterminado de inicio rápido
descripción: Configure y proteja el carril predeterminado alternativo en Nexus, el Torii y el SDK pueden insertar lane_id en carriles públicos.
---

:::nota Канонический источник
Esta página está escrita `docs/source/quickstart/default_lane.md`. Deje copias sincronizadas, pero no las coloque en el portal.
:::

# Быстрый старт carril predeterminado (NX-5)

> **Hoja de ruta del sitio:** NX-5: carril público predeterminado de integración. El método actual incluye el respaldo `nexus.routing_policy.default_lane`, los componentes REST/gRPC Torii y el SDK de código abierto más populares `lane_id`, когда трафик относится к канонической carril público. Esto proporciona a los operadores un catálogo de información, un respaldo en `/status` y un cliente disponible до конца.

## Предварительные требования

- Сборка Sora/Nexus para `irohad` (запуск `irohad --sora --config ...`).
- Para acceder a la configuración del repositorio, elimine las secciones `nexus.*`.
- `iroha_cli`, настроенный на целевой кластер.
- `curl`/`jq` (o activado) para la carga útil del programador `/status` en Torii.

## 1. Descripción del carril del catálogo y el espacio de datosMejore carriles y espacios de datos, que son los que mejor se adaptan a cada conjunto. El siguiente fragmento (utilizado en `defaults/nexus/config.toml`) registra tres carriles públicos y alias compartidos para el espacio de datos:

```toml
[nexus]
lane_count = 3

[[nexus.lane_catalog]]
index = 0
alias = "core"
description = "Primary execution lane"
dataspace = "universal"

[[nexus.lane_catalog]]
index = 1
alias = "governance"
description = "Governance & parliament traffic"
dataspace = "governance"

[[nexus.lane_catalog]]
index = 2
alias = "zk"
description = "Zero-knowledge attachments"
dataspace = "zk"

[[nexus.dataspace_catalog]]
alias = "universal"
id = 0
description = "Single-lane data space"
fault_tolerance = 1

[[nexus.dataspace_catalog]]
alias = "governance"
id = 1
description = "Governance proposals & manifests"
fault_tolerance = 1

[[nexus.dataspace_catalog]]
alias = "zk"
id = 2
description = "Zero-knowledge proofs and attachments"
fault_tolerance = 1
```

El cable `index` es único y no apto. Espacios de datos de identificación: это 64-bitные значения; в примерах выше используются те же числовые значения, что и индексы lane, для наглядности.

## 2. Задать дефолты маршрутизации и опциональные переопределения

La sección `nexus.routing_policy` mejora el carril de respaldo y pospone la actualización de la configuración de la instalación префиксов аккаунтов. Si no está disponible, el programador realizará transmisiones en `default_lane` e `default_dataspace`. El enrutador lógico está conectado a `crates/iroha_core/src/queue/router.rs` y la política principal es la de Torii REST/gRPC.

```toml
[nexus.routing_policy]
default_lane = 0                # use the "core" lane when no rules match
default_dataspace = "universal"    # reuse the public dataspace for the fallback

[[nexus.routing_policy.rules]]
lane = 1
dataspace = "governance"
[nexus.routing_policy.rules.matcher]
instruction = "governance"
description = "Route governance instructions to the governance lane"

[[nexus.routing_policy.rules]]
lane = 2
dataspace = "zk"
[nexus.routing_policy.rules.matcher]
instruction = "smartcontract::deploy"
description = "Route contract deployments to the zk lane for proof tracking"
```


## 3. Запустить ноду с примененной политикой

```bash
IROHA_CONFIG=/path/to/nexus/config.toml
irohad --sora --config "${IROHA_CONFIG}"
```

No es necesario iniciar una política de privacidad desde el principio. Любые ошибки валидации (отсутствующие индексы, дублирующиеся alias, некорректные ids dataspaces) всплывают до начала gossip.

## 4. Подтвердить состояние gobernancia del carril

Después de esta nueva conexión, utilice el ayudante de CLI, cómo controlar, cómo bloquear el carril predeterminado (descarga de manifiesto) y conectarse al tráfico. Сводный вид выводит по одной строке на lane:

```bash
iroha_cli app nexus lane-report --summary
```

Salida de ejemplo:

```
Lane  Alias            Module           Status  Quorum  Validators  Detail
   0  core             parliament       ready      05           07  manifest ok
   1  governance       parliament       ready      05           07  manifest ok
   2  zk               parliament       sealed     03           05  manifest required
```Si el carril predeterminado es `sealed`, consulte el runbook para controlar los carriles antes de cada uno de ellos y así distribuir todo el tráfico. La bandera `--fail-on-sealed` está conectada a CI.

## 5. Проверить carga útil de estado Torii

El `/status` elimina y bloquea la política, y el programador de carriles es pequeño. Utilice `curl`/`jq` para saber cómo instalar y proteger el carril alternativo. Telemetro publico:

```bash
curl -s http://127.0.0.1:8080/status | jq '.nexus.routing_policy'
```

Salida de muestra:

```json
{
  "default_lane": 0,
  "default_dataspace": "universal",
  "rules": [
    {"lane": 1, "dataspace": "governance", "matcher": {"instruction": "governance"}},
    {"lane": 2, "dataspace": "zk", "matcher": {"instruction": "smartcontract::deploy"}}
  ]
}
```

Aquí se pueden configurar varios programas programadores para el carril `0`:

```bash
curl -s http://127.0.0.1:8080/status \
  | jq '.nexus_scheduler_lane_teu_status[] | select(.lane_id == 0)
        | {lane_id, alias, dataspace_alias, committed, manifest_ready, scheduler_utilization_pct}'
```

Esto permite que la instantánea de TEU, los alias metadanos y el manifiesto de banderas se ajusten a su configuración. La carga útil está implementada en los paneles Grafana para la ingesta de carriles del tablero.

## 6. Проверить дефолтное поведение клиента- **Rust/CLI.** `iroha_cli` и клиентский crate Rust опускают поле `lane_id`, когда вы не передаете `--lane-id` / `LaneSelector`. El enrutador de cola está conectado a `default_lane`. Desactive las banderas `--lane-id`/`--dataspace-id` para conectarse al carril no predeterminado.
- **JS/Swift/Android.** Las versiones SDK disponibles incluyen `laneId`/`lane_id` opciones y alternativas de respaldo en la configuración, disponibles en `/status`. Держите политику маршрутизации синхронизированной между puesta en escena y producción, чтобы мобильным приложениям не требовались аварийные перенастройки.
- **Pruebas de canalización/SSE.** Filtros de transmisión de datos predeterminados `tx_lane_id == <u32>` (см. `docs/source/pipeline.md`). Puede conectar `/v2/pipeline/events/transactions` con este filtro, cómo colocarlo, cómo limpiarlo, o no en el carril de respaldo. identificación

## 7. Observabilidad y ganchos de gobernanza- `/status` incluye los anuncios `nexus_lane_governance_sealed_total` e `nexus_lane_governance_sealed_aliases`, que alertan a Alertmanager y que contienen un manifiesto de carril. Estas alertas se encuentran en devnet.
- El programador de televisores y el control del panel de control de los carriles (`dashboards/grafana/nexus_lanes.json`) crean un alias/slug en el catálogo. Если вы переименовываете alias, переименуйте и соответствующие директории Kura, чтобы аудиторы сохраняли детерминированные пути (отслеживается по NX-1).
- Modificación de los carriles predeterminados para desactivar el plan de reversión. Зафиксируйте hash manifest and доказательства gobernancia рядом с этим quickstart в вашем операторском runbook, чтобы будущие ротации не гадали требуемое состояние.

Para estos programas, puede seleccionar el sistema `nexus.routing_policy.default_lane` para configurar el SDK y desactivarlo. унаследованные кодовые пути в сети de un solo carril.