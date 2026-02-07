---
lang: es
direction: ltr
source: docs/portal/docs/nexus/nexus-bootstrap-plan.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: plan-bootstrap-nexus
título: Bootstrap y observabilidad de Sora Nexus
descripción: Plan operativo para poner en línea el cluster central de validadores Nexus antes de agregar servicios SoraFS y SoraNet.
---

:::nota Fuente canónica
Esta página refleja `docs/source/soranexus_bootstrap_plan.md`. Mantenga ambas copias alineadas hasta que las versiones localizadas lleguen al portal.
:::

# Plan de bootstrap y observabilidad de Sora Nexus

## Objetivos
- Levantar la red base de validadores/observadores de Sora Nexus con llaves de gobernanza, APIs de Torii y monitoreo de consenso.
- Validar servicios centrales (Torii, consenso, persistencia) antes de habilitar despliegues piggyback de SoraFS/SoraNet.
- Establecer flujos de trabajo de CI/CD y paneles/alertas de observabilidad para asegurar la salud de la red.

##Requisitos previos
- Material de llaves de gobernanza (multisig del consejo, llaves de comité) disponible en HSM o Vault.
- Infraestructura base (clusters Kubernetes o nodos bare-metal) en regiones primaria/secundaria.
- Configuración bootstrap actualizada (`configs/nexus/bootstrap/*.toml`) que refleja los últimos parámetros de consenso.##Entornos de rojo
- Operar dos entornos Nexus con prefijos de red distintos:
- **Sora Nexus (mainnet)** - prefijo de red de produccion `nexus`, hospedando la gobernanza canónica y servicios piggyback de SoraFS/SoraNet (chain ID `0x02F1` / UUID `00000000-0000-0000-0000-000000000753`).
- **Sora Testus (testnet)** - prefijo de red de staging `testus`, que espera la configuración de mainnet para pruebas de integración y validación pre-release (chain UUID `809574f5-fee7-5e69-bfcf-52451e42d50f`).
- Mantener archivos génesis separados, llaves de gobernanza y huellas de infraestructura para cada entorno. Testus actúa como el banco de pruebas de todos los rollouts SoraFS/SoraNet antes de promover a Nexus.
- Las tuberías de CI/CD deben desplegar primero en Testus, ejecutar pruebas de humo automatizadas, y requerir promoción manual a Nexus una vez que pasen los cheques.
- Los paquetes de configuración de referencia viven en `configs/soranexus/nexus/` (mainnet) y `configs/soranexus/testus/` (testnet), cada uno con `config.toml`, `genesis.json` y directorios de admisión Torii de ejemplo.## Paso 1 - Revisión de configuración
1. Auditar la documentación existente:
   - `docs/source/nexus/architecture.md` (consenso, diseño de Torii).
   - `docs/source/nexus/deployment_checklist.md` (requisitos de infraestructura).
   - `docs/source/nexus/governance_keys.md` (procedimientos de custodia de llaves).
2. Validar que los archivos génesis (`configs/nexus/genesis/*.json`) se alineen con el roster actual de validadores y los pesos de sizing.
3. Confirmar parámetros de red:
   - Tamano de comité de consenso y quórum.
   - Intervalo de bloques / umbrales de finalización.
   - Puertos de servicio Torii y certificados TLS.

## Paso 2 - Despliegue del cluster bootstrap
1. Aprovisionar nodos validadores:
   - Desplegar instancias `irohad` (validadores) con volúmenes persistentes.
   - Asegurar reglas de firewall que permitan el tráfico de consenso y Torii entre nodos.
2. Iniciar servicios Torii (REST/WebSocket) en cada validador con TLS.
3. Desplegar nodos observadores (solo lectura) para resiliencia adicional.
4. Ejecutar scripts de bootstrap (`scripts/nexus_bootstrap.sh`) para distribuir génesis, iniciar consenso y registrar nodos.
5. Ejecutar pruebas de humo:
   - Enviar transacciones de prueba vía Torii (`iroha_cli tx submit`).
   - Verificar producción/finalidad de bloques mediante telemetría.
   - Revisar replicacion del libro mayor entre validadores/observadores.## Paso 3 - Gobernanza y gestión de llaves
1. Cargar la configuración multifirma del consejo; confirmar que las propuestas de gobernanza se puedan enviar y ratificar.
2. Almacenar de forma segura las llaves de consenso/comité; configurar copias de seguridad automáticas con registro de acceso.
3. Configurar procedimientos de rotación de llaves de emergencia (`docs/source/nexus/key_rotation.md`) y verificar el runbook.

## Paso 4 - Integración de CI/CD
1. Configurar tuberías:
   - Build y publicación de imágenes de validator/Torii (GitHub Actions o GitLab CI).
   - Validación automatizada de configuración (lint de genesis, verificación de firmas).
   - Pipelines de despliegue (Helm/Kustomize) para clusters de staging y producción.
2. Implementar pruebas de humo en CI (levantar cluster efimero, correr suite canonica de transacciones).
3. Agregar scripts de rollback para implementar fallos y documentar runbooks.## Paso 5 - Observabilidad y alertas
1. Desplegar el stack de monitoreo (Prometheus + Grafana + Alertmanager) por región.
2. Recopilar métricas centrales:
  - `nexus_consensus_height`, `nexus_finality_lag`, `torii_request_duration_seconds`, `validator_peer_count`.
   - Registros vía Loki/ELK para servicios Torii y consenso.
3. Paneles de control:
   - Salud de consenso (altura de bloque, finalización, estado de pares).
   - Latencia/tasa de error de API Torii.
   - Transacciones de gobernanza y estado de propuestas.
4. Alertas:
   - Paro de producción de bloques (>2 intervalos de bloque).
   - Conteo de pares por debajo del quórum.
   - Picos en la tasa de error de Torii.
   - Backlog de la cola de propuestas de gobernanza.

## Paso 6 - Validación y traspaso
1. Ejecutar validación de extremo a extremo:
   - Enviar una propuesta de gobernanza (p. ej., cambio de parámetro).
   - Procesarla vía aprobación del consejo para asegurar que el oleoducto de gobernanza funcione.
   - Ejecutar diff del estado del libro mayor para asegurar consistencia.
2. Documentar el runbook para on-call (respuesta a incidentes, failover, escalado).
3. Comunicar la disponibilidad a los equipos de SoraFS/SoraNet; confirmar que los desplegables piggyback puedan apuntar a nodos Nexus.## Lista de verificación de implementación
- [ ] Auditorio de génesis/configuración completada.
- [ ] Nodos validadores y observadores desplegados con consenso saludable.
- [ ] Llaves de gobernanza cargadas, propuesta probada.
- [ ] Pipelines CI/CD corriendo (construcción + despliegue + pruebas de humo).
- [ ] Paneles de observabilidad activos con alertas.
- [ ] Documentación de handoff entregada a equipos downstream.