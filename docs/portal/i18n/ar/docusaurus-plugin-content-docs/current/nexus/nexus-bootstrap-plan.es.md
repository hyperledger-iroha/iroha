---
lang: ar
direction: rtl
source: docs/portal/docs/nexus/nexus-bootstrap-plan.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
id: nexus-bootstrap-plan
title: Bootstrap y observabilidad de Sora Nexus
description: Plan operativo para poner en linea el cluster central de validadores Nexus antes de agregar servicios SoraFS y SoraNet.
---

:::note Fuente canonica
Esta pagina refleja `docs/source/soranexus_bootstrap_plan.md`. Manten ambas copias alineadas hasta que las versiones localizadas lleguen al portal.
:::

# Plan de bootstrap y observabilidad de Sora Nexus

## Objetivos
- Levantar la red base de validadores/observadores de Sora Nexus con llaves de gobernanza, APIs de Torii y monitoreo de consenso.
- Validar servicios centrales (Torii, consenso, persistencia) antes de habilitar despliegues piggyback de SoraFS/SoraNet.
- Establecer workflows de CI/CD y dashboards/alertas de observabilidad para asegurar la salud de la red.

## Prerequisitos
- Material de llaves de gobernanza (multisig del consejo, llaves de comite) disponible en HSM o Vault.
- Infraestructura base (clusters Kubernetes o nodos bare-metal) en regiones primaria/secundaria.
- Configuracion bootstrap actualizada (`configs/nexus/bootstrap/*.toml`) que refleje los ultimos parametros de consenso.

## Entornos de red
- Operar dos entornos Nexus con prefijos de red distintos:
- **Sora Nexus (mainnet)** - prefijo de red de produccion `nexus`, hospedando la gobernanza canonica y servicios piggyback de SoraFS/SoraNet (chain ID `0x02F1` / UUID `00000000-0000-0000-0000-000000000753`).
- **Sora Testus (testnet)** - prefijo de red de staging `testus`, que espeja la configuracion de mainnet para pruebas de integracion y validacion pre-release (chain UUID `809574f5-fee7-5e69-bfcf-52451e42d50f`).
- Mantener archivos genesis separados, llaves de gobernanza y huellas de infraestructura para cada entorno. Testus actua como el banco de pruebas de todos los rollouts SoraFS/SoraNet antes de promover a Nexus.
- Las pipelines de CI/CD deben desplegar primero en Testus, ejecutar smoke tests automatizados, y requerir promocion manual a Nexus una vez que pasen los checks.
- Los bundles de configuracion de referencia viven en `configs/soranexus/nexus/` (mainnet) y `configs/soranexus/testus/` (testnet), cada uno con `config.toml`, `genesis.json` y directorios de admision Torii de ejemplo.

## Paso 1 - Revision de configuracion
1. Auditar la documentacion existente:
   - `docs/source/nexus/architecture.md` (consenso, layout de Torii).
   - `docs/source/nexus/deployment_checklist.md` (requisitos de infraestructura).
   - `docs/source/nexus/governance_keys.md` (procedimientos de custodia de llaves).
2. Validar que los archivos genesis (`configs/nexus/genesis/*.json`) se alineen con el roster actual de validadores y los pesos de staking.
3. Confirmar parametros de red:
   - Tamano de comite de consenso y quorum.
   - Intervalo de bloques / umbrales de finalizacion.
   - Puertos de servicio Torii y certificados TLS.

## Paso 2 - Despliegue del cluster bootstrap
1. Aprovisionar nodos validadores:
   - Desplegar instancias `irohad` (validadores) con volumnes persistentes.
   - Asegurar reglas de firewall que permitan trafico de consenso y Torii entre nodos.
2. Iniciar servicios Torii (REST/WebSocket) en cada validador con TLS.
3. Desplegar nodos observadores (solo lectura) para resiliencia adicional.
4. Ejecutar scripts de bootstrap (`scripts/nexus_bootstrap.sh`) para distribuir genesis, iniciar consenso y registrar nodos.
5. Ejecutar smoke tests:
   - Enviar transacciones de prueba via Torii (`iroha_cli tx submit`).
   - Verificar produccion/finalidad de bloques mediante telemetria.
   - Revisar replicacion del ledger entre validadores/observadores.

## Paso 3 - Gobernanza y gestion de llaves
1. Cargar la configuracion multisig del consejo; confirmar que las propuestas de gobernanza se puedan enviar y ratificar.
2. Almacenar de forma segura las llaves de consenso/comite; configurar backups automaticos con logging de acceso.
3. Configurar procedimientos de rotacion de llaves de emergencia (`docs/source/nexus/key_rotation.md`) y verificar el runbook.

## Paso 4 - Integracion de CI/CD
1. Configurar pipelines:
   - Build y publicacion de imagenes de validator/Torii (GitHub Actions o GitLab CI).
   - Validacion automatizada de configuracion (lint de genesis, verificacion de firmas).
   - Pipelines de despliegue (Helm/Kustomize) para clusters de staging y produccion.
2. Implementar smoke tests en CI (levantar cluster efimero, correr suite canonica de transacciones).
3. Agregar scripts de rollback para despliegues fallidos y documentar runbooks.

## Paso 5 - Observabilidad y alertas
1. Desplegar el stack de monitoreo (Prometheus + Grafana + Alertmanager) por region.
2. Recopilar metricas centrales:
  - `nexus_consensus_height`, `nexus_finality_lag`, `torii_request_duration_seconds`, `validator_peer_count`.
   - Logs via Loki/ELK para servicios Torii y consenso.
3. Dashboards:
   - Salud de consenso (altura de bloque, finalizacion, estado de peers).
   - Latencia/tasa de error de Torii API.
   - Transacciones de gobernanza y estado de propuestas.
4. Alertas:
   - Paro de produccion de bloques (>2 intervalos de bloque).
   - Conteo de peers por debajo del quorum.
   - Picos en la tasa de error de Torii.
   - Backlog de la cola de propuestas de gobernanza.

## Paso 6 - Validacion y handoff
1. Ejecutar validacion end-to-end:
   - Enviar una propuesta de gobernanza (p. ej., cambio de parametro).
   - Procesarla via aprobacion del consejo para asegurar que el pipeline de gobernanza funciona.
   - Ejecutar diff del estado del ledger para asegurar consistencia.
2. Documentar el runbook para on-call (respuesta a incidentes, failover, escalado).
3. Comunicar la disponibilidad a los equipos de SoraFS/SoraNet; confirmar que los despliegues piggyback puedan apuntar a nodos Nexus.

## Checklist de implementacion
- [ ] Auditoria de genesis/configuracion completada.
- [ ] Nodos validadores y observadores desplegados con consenso saludable.
- [ ] Llaves de gobernanza cargadas, propuesta probada.
- [ ] Pipelines CI/CD corriendo (build + deploy + smoke tests).
- [ ] Dashboards de observabilidad activos con alertas.
- [ ] Documentacion de handoff entregada a equipos downstream.
