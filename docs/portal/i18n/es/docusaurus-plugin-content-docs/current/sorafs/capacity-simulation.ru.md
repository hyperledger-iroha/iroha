---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/capacity-simulation.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: simulación de capacidad
título: Ранбук симуляции емкости SoraFS
sidebar_label: Ранбук симуляции емкости
descripción: Simuladores de teclados SF-2c con dispositivos integrados, accesorios Prometheus y tableros de instrumentos Grafana.
---

:::nota Канонический источник
Esta página contiene la letra `docs/source/sorafs/runbooks/sorafs_capacity_simulation.md`. Deje copias sincronizadas de los documentos de Sphinx no incluidos en el presupuesto.
:::

Esta opción se puede utilizar para insertar simulaciones de impresoras SF-2c y visualizar las métricas de alta calidad. En las actualizaciones de seguridad, implementamos conmutación por error y soluciones de corte de extremo a extremo, implementamos configuraciones determinadas en `docs/examples/sorafs_capacity_simulation/`. Cargas útiles емкости по-прежнему используют `sorafs_manifest_stub capacity`; Utilice `iroha app sorafs toolkit pack` para el manifiesto/CAR de los automóviles.

## 1. Сгенерировать CLI-артефакты

```bash
cd $REPO_ROOT/docs/examples/sorafs_capacity_simulation
./run_cli.sh ./artifacts
```

`run_cli.sh` incluye `sorafs_manifest_stub capacity`, qué cargas útiles de Norito, bloques base64, archivos Torii y archivos JSON para:

- Трех деклараций провайдеров, участвующих в сценарии переговоров по квотам.
- Одного распоряжения о репликации, распределяющего манифест между провайдерами.
- Снимков телеметрии для базовой линии до сбоя, интервала сбоя and восстановления failover.
- La carga útil se realiza mediante corte después del modo de corte.Estos artefactos se encuentran en `./artifacts` (puede consultarse según el catálogo de medicamentos según el argumento). Pruebe los archivos `_summary.json` para este contacto.

## 2. Agregar resultados y verificar métricas

```bash
./analyze.py --artifacts ./artifacts
```

Formulario de analizador:

- `capacity_simulation_report.json`: configuración agregada, conmutación por error eliminada y metadatos esporádicos.
- `capacity_simulation.prom`: archivo de texto métrico Prometheus (`sorafs_simulation_*`), que sirve para el exportador y nodo del recopilador de archivos de texto o para un trabajo de raspado.

Primeras configuraciones de scrape Prometheus:

```yaml
scrape_configs:
  - job_name: sorafs-capacity-sim
    scrape_interval: 15s
    static_configs:
      - targets: ["localhost:9100"]
        labels:
          scenario: "capacity-sim"
    metrics_path: /metrics
    params:
      format: ["prometheus"]
```

Utilice el recopilador de archivos de texto en `capacity_simulation.prom` (para el exportador de nodos instalado en el catálogo, consulte `--collector.textfile.directory`).

## 3. Importar tablero Grafana

1. En Grafana importe `dashboards/grafana/sorafs_capacity_simulation.json`.
2. Utilice la fuente de datos `Prometheus` para realizar el scrape.
3. Proverte los paneles:
   - **Asignación de cuota (GiB)** requiere compromiso/asignación de la cuenta del usuario.
   - **Failover Trigger** activa *Failover Active*, que establece métricas.
   - **Caída del tiempo de actividad durante una interrupción** está disponible actualmente en el controlador `alpha`.
   - **Porcentaje de barra diagonal solicitada** Visualice las soluciones más efectivas en los dispositivos específicos.

## 4. Ожидаемые проверки- `sorafs_simulation_quota_total_gib{scope="assigned"}` a partir de `600`, cuando el compromiso de confirmación es >=600.
- `sorafs_simulation_failover_triggered` coloca `1`, un parámetro métrico proporcionado por `beta`.
- `sorafs_simulation_slash_requested` coloca `0.15` (barra del 15%) para el identificador del proveedor `alpha`.

Introduzca `cargo test -p sorafs_car --features cli --test capacity_simulation_toolkit` para conectar los dispositivos conectados a la CLI.