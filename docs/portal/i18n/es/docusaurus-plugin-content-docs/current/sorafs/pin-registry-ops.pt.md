---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/pin-registry-ops.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: pin-registry-ops
título: Operações do Registro de PIN
sidebar_label: Funciones del registro de PIN
Descripción: Monitorear y seleccionar el Registro de PIN de SoraFS y las métricas de SLA de replicación.
---

:::nota Fuente canónica
España `docs/source/sorafs/runbooks/pin_registry_ops.md`. Mantenha ambas as versos sincronizadas comió una aposentadora da documentacao herdada do Sphinx.
:::

## Visao general

Este runbook documenta como monitorear y hacer triagem do Pin Registry do SoraFS y sus acuerdos de nivel de servicio (SLA) de replicación. Las métricas se originan en `iroha_torii` y se exportan a través de Prometheus en el espacio de nombres `torii_sorafs_*`. Torii demostramos el estado del registro en un intervalo de 30 segundos en el segundo plano, y los paneles de control permanecen actualizados cuando ningún operador está consultando los puntos finales `/v1/sorafs/pin/*`. Importe el tablero curado (`docs/source/grafana_sorafs_pin_registry.json`) para un diseño de Grafana pronto para usarlo que mapeia directamente para los secos abajo.

## Referencia de métricas| Métrica | Etiquetas | Descripción |
| ------- | ------ | --------- |
| `torii_sorafs_registry_manifests_total` | `status` (`pending` \| `approved` \| `retired`) | Inventario de manifiestos en cadena por estado del ciclo de vida. |
| `torii_sorafs_registry_aliases_total` | - | Contagem de alias de manifiestos activos registrados no registro. |
| `torii_sorafs_registry_orders_total` | `status` (`pending` \| `completed` \| `expired`) | Backlog de órdenes de replicación segmentadas por estado. |
| `torii_sorafs_replication_backlog_total` | - | Gauge de conveniencia que espelha ordens `pending`. |
| `torii_sorafs_replication_sla_total` | `outcome` (`met` \| `missed` \| `pending`) | Contabilidade de SLA: `met` conta órdenes concluidas dentro de la fecha límite, `missed` agrega conclusoes tardias + expiracoes, `pending` espelha órdenes pendientes. |
| `torii_sorafs_replication_completion_latency_epochs` | `stat` (`avg` \| `p95` \| `max` \| `count`) | Latencia agregada de conclusao (epocas entre emissao e conclusao). |
| `torii_sorafs_replication_deadline_slack_epochs` | `stat` (`avg` \| `p95` \| `max` \| `count`) | Janelas de folga de órdenes pendentes (deadline menos epoca de emissao). |

Todos los medidores reinician una instantánea, entre ellos los paneles deben mostrarse en cadencia `1m` o más rápida.

## Panel de control GrafanaEl panel JSON incluye varias configuraciones que codifican los flujos de trabajo de los operadores. As consultas abajo sirven como referencia rápida caso voce prefira montar gráficos sob medida.

1. **Ciclo de vida de manifiestos** - `torii_sorafs_registry_manifests_total` (agrupado por `status`).
2. **Tendencia del catálogo de alias** - `torii_sorafs_registry_aliases_total`.
3. **Fila de órdenes por status** - `torii_sorafs_registry_orders_total` (agrupado por `status`).
4. **Backlog vs órdenes vencidas** - combina `torii_sorafs_replication_backlog_total` e `torii_sorafs_registry_orders_total{status="expired"}` para mostrar saturación.
5. **Razao de éxito de SLA** -

   ```promql
   sum(torii_sorafs_replication_sla_total{outcome="met"})
   /
   clamp_min(
     sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}),
     1
   )
   ```

6. **Latencia vs falta de fecha límite** - sobrepoe `torii_sorafs_replication_completion_latency_epochs{stat="p95"}` e `torii_sorafs_replication_deadline_slack_epochs{stat="avg"}`. Utilice las transformaciones de Grafana para agregar vistas `min_over_time` cuando necesite el piso absoluto de seguimiento, por ejemplo:

   ```promql
   min_over_time(torii_sorafs_replication_deadline_slack_epochs{stat="avg"}[15m])
   ```

7. **Órdenes perdidas (taxa 1h)** -

   ```promql
   sum(increase(torii_sorafs_replication_sla_total{outcome="missed"}[1h]))
   ```

##Limiares de alerta- **Suceso de SLA  0**
  - Limitar: `increase(torii_sorafs_registry_orders_total{status="expired"}[5m]) > 0`
  - Acao: Inspeccionar manifiestos de gobierno para confirmar la deserción de proveedores.
- **p95 de conclusao > folga media de fecha límite**
  - Limitar: `torii_sorafs_replication_completion_latency_epochs{stat="p95"} > torii_sorafs_replication_deadline_slack_epochs{stat="avg"}`
  - Acao: Verificar se os proveedores están cumpliendo antes de los dos plazos; considerar retribuicoes.

### Registros de Prometheus de ejemplo

```yaml
groups:
  - name: sorafs-pin-registry
    rules:
      - alert: SorafsReplicationSlaDrop
        expr: sum(torii_sorafs_replication_sla_total{outcome="met"}) /
          clamp_min(sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}), 1) < 0.95
        for: 15m
        labels:
          severity: page
        annotations:
          summary: "SLA de replicacao SoraFS abaixo da meta"
          description: "A razao de sucesso do SLA ficou abaixo de 95% por 15 minutos."

      - alert: SorafsReplicationBacklogGrowing
        expr: torii_sorafs_replication_backlog_total > 10
        for: 10m
        labels:
          severity: page
        annotations:
          summary: "Backlog de replicacao SoraFS acima do limiar"
          description: "Ordens de replicacao pendentes excederam o budget configurado."

      - alert: SorafsReplicationExpiredOrders
        expr: increase(torii_sorafs_registry_orders_total{status="expired"}[5m]) > 0
        for: 0m
        labels:
          severity: ticket
        annotations:
          summary: "Ordens de replicacao SoraFS expiradas"
          description: "Pelo menos uma ordem de replicacao expirou nos ultimos cinco minutos."
```

## Flujo de triagem1. **Identificar causa**
   - Si los incumplimientos de SLA aumentan en cuanto el trabajo pendiente permanece bajo, foque no desempenho dos proveedores (falhas de PoR, conclusiones tardías).
   - Se o backlog crescer com misses estaveis, inspecione a admissao (`/v1/sorafs/pin/*`) para confirmar manifests aguardando aprovacao do conselho.
2. **Validar estado de proveedores**
   - Ejecute `iroha app sorafs providers list` y verifique se as capacidades anunciadas atendem aos requisitos de replicacao.
   - Verifique los medidores `torii_sorafs_capacity_*` para confirmar GiB provisionados y sucesso de PoR.
3. **Retribuir réplica**
   - Emita nuevas órdenes vía `sorafs_manifest_stub capacity replication-order` quando a folga do backlog (`stat="avg"`) cair abaixo de 5 epocas (o empacotamento de manifest/CAR usa `iroha app sorafs toolkit pack`).
   - Notifique a gobernador se os alias nao tiverem vinculantes ativos de manifiesto (quedas inesperadas em `torii_sorafs_registry_aliases_total`).
4. **Resultado documental**
   - Registre notas de incidentes en el registro de operaciones de SoraFS con marcas de tiempo y resúmenes de manifiestos afectados.
   - Actualice este runbook según los nuevos modos de falha o paneles de control introducidos.

## Plano de implementación

Siga este procedimiento en etapas para habilitar o soportar la política de caché de alias en producción:1. **Preparar configuración**
   - Actualizar `torii.sorafs_alias_cache` en `iroha_config` (usuario -> actual) con los TTL y janelas de graca acordados: `positive_ttl`, `refresh_window`, `hard_expiry`, `negative_ttl`, `revocation_ttl`, `rotation_max_age`, `successor_grace` e `governance_grace`. Los valores predeterminados corresponden a la política en `docs/source/sorafs_alias_policy.md`.
   - Para los SDK, distribuye los valores de sus camadas de configuración (`AliasCachePolicy::new(positive, refresh, hard, negative, revocation, rotation, successor, governance)` en enlaces Rust / NAPI / Python) para que el cumplimiento del cliente corresponda a la puerta de enlace.
2. **Ejecución en seco de la puesta en escena**
   - Implantar una mudanca de configuración en un cluster de puesta en escena que espelhe a topología de producción.
   - Ejecute `cargo xtask sorafs-pin-fixtures` para confirmar que os aparatos canónicos de alias aún no decodificam e fazem round-trip; Cualquier desajuste implica una deriva aguas arriba que debe ser resuelto primero.
   - Ejerza los puntos finales `/v1/sorafs/pin/{digest}` e `/v1/sorafs/aliases` con pruebas sintéticas cobrindo casos frescos, actualizar ventana, caducados y expirados. Valide los códigos HTTP, encabezados (`Sora-Proof-Status`, `Retry-After`, `Warning`) y campos del cuerpo JSON en este runbook.
3. **Habilitar em producción**
   - Implante una nueva configuración en janela padrao de mudancas. Aplique primero el Torii y luego reinicie gateways/servicos SDK para que el nodo confirme una nueva política en los registros.- Importe `docs/source/grafana_sorafs_pin_registry.json` a Grafana (para actualizar los paneles existentes) y corrija el problema de actualizar el caché de alias en el espacio de trabajo del NOC.
4. **Verificación pos-implementación**
   - Monitoree `torii_sorafs_alias_cache_refresh_total` e `torii_sorafs_alias_cache_age_seconds` durante 30 minutos. Picos nas curvas `error`/`expired` deben correlacionarse con janelas de refresco; Crescimento inesperado significa que los operadores deben inspeccionar pruebas de alias y saude dos proveedores antes de continuar.
   - Confirme que los registros del lado del cliente se muestran como mesmas decisoes de politica (los SDK emiten errores cuando prueba que están obsoletos o caducados). Ausencia de advertencias del cliente indica configuración incorrecta.
5. **Retroceso**
   - Se a emissao de alias atrasar e a janela de actualizar disparar con frecuencia, relajarse temporalmente a política aumentando `refresh_window` e `positive_ttl` na configuración, después de reimplantar. Mantenha `hard_expiry` intacto para que provas realmente stale ainda sejam rejeitadas.
   - Reverta para a configuración anterior restaurando o snapshot anterior de `iroha_config` se a telemetria continuar mostrando contagios `error` elevadas, entao abra um incidente para rastrear atrasos na geracao de alias.

## Materiales relacionados

- `docs/source/sorafs/pin_registry_plan.md` - hoja de ruta de implementación y contexto de gobernanza.
- `docs/source/sorafs/runbooks/sorafs_node_ops.md` - operaciones del trabajador de almacenamiento, complementa este libro de jugadas del registro.