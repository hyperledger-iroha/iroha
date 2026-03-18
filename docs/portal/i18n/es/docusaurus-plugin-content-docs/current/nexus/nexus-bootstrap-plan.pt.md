---
lang: es
direction: ltr
source: docs/portal/docs/nexus/nexus-bootstrap-plan.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: plan-bootstrap-nexus
título: Bootstrap y observabilidad de Sora Nexus
descripción: Plano operativo para colocar o cluster central de validadores Nexus online antes de agregar servicios SoraFS y SoraNet.
---

:::nota Fuente canónica
Esta página refleja `docs/source/soranexus_bootstrap_plan.md`. Mantenha as duas copias alinhadas ate que as versoes localizadas cheguem ao portal.
:::

# Plano de bootstrap y observabilidad de Sora Nexus

## Objetivos
- Levantar una red base de validadores/observadores Sora Nexus con llaves de gobierno, APIs Torii y monitoreo de consenso.
- Validar servicios centrales (Torii, consenso, persistencia) antes de habilitar despliega piggyback SoraFS/SoraNet.
- Establecer flujos de trabajo de CI/CD y paneles/alertas de observabilidad para garantizar la seguridad de la red.

##Requisitos previos
- Material de llaves de gobierno (multisig do conselho, chaves de comité) disponible en HSM o Vault.
- Infraestructura base (clusters Kubernetes o bare-metal) en regiones primaria/secundaria.
- Configuración de bootstrap actualizada (`configs/nexus/bootstrap/*.toml`) reflejando los parámetros de consenso más recientes.## Ambientes de red
- Operar dos ambientes Nexus con prefijos de red distintos:
- **Sora Nexus (mainnet)** - prefijo de red de producción `nexus`, alojando gobierno canónico y servicios a cuestas SoraFS/SoraNet (ID de cadena `0x02F1` / UUID `00000000-0000-0000-0000-000000000753`).
- **Sora Testus (testnet)** - prefijo de red de staging `testus`, implementando la configuración de mainnet para pruebas de integración y validación previa al lanzamiento (cadena UUID `809574f5-fee7-5e69-bfcf-52451e42d50f`).
- Manter arquivos génesis separados, chaves de gobernanza e huellas de infraestructura para cada ambiente. Testus atua como campo de pruebas para rollouts SoraFS/SoraNet antes de promover para Nexus.
- Las tuberías de CI/CD deben implementarse primero en Testus, ejecutar pruebas de humo automatizadas y exigir el manual de promoción para Nexus cuando se pasan las comprobaciones.
- Paquetes de configuración de referencia viven en `configs/soranexus/nexus/` (mainnet) e `configs/soranexus/testus/` (testnet), cada uno de los cuales contiene `config.toml`, `genesis.json` y directorios de admisión Torii de ejemplo.## Etapa 1 - Revisión de configuración
1. Auditoría documental existente:
   - `docs/source/nexus/architecture.md` (consenso, diseño de Torii).
   - `docs/source/nexus/deployment_checklist.md` (requisitos de infraestructura).
   - `docs/source/nexus/governance_keys.md` (procedimientos de custodia de chaves).
2. Validar que arquivos genesis (`configs/nexus/genesis/*.json`) alinham con o roster actual de validadores e pesos de sizing.
3. Confirmar parámetros de red:
   - Tamanho do comite de consenso e quorum.
   - Intervalo de bloques / límites de finalidad.
   - Portas do servico Torii y certificados TLS.

## Etapa 2: implementar el arranque del clúster
1. Provisionarnos validadores:
   - Implementar instancias `irohad` (validadores) con volúmenes persistentes.
   - Garantir que regras de firewall permitam trafego de consenso e Torii entre nos.
2. Inicie los servicios Torii (REST/WebSocket) en cada validador con TLS.
3. Deploy de nos observadores (somente leitura) para resiliencia adicional.
4. Ejecute scripts de bootstrap (`scripts/nexus_bootstrap.sh`) para distribuir génesis, iniciar consenso y registrar nos.
5. Ejecutar pruebas de humo:
   - Enviar transacoes de teste vía Torii (`iroha_cli tx submit`).
   - Verificar producción/finalidad de blocos vía telemetría.
   - Revisar la replicación del libro mayor entre validadores/observadores.## Etapa 3 - Gobernanza y gestao de chaves
1. Carregar configuracao multisig do conselho; confirmar que las propuestas de gobierno pueden ser submetidas y ratificadas.
2. Armazenar com seguranca chaves de consenso/comite; configurar copias de seguridad automáticas con registro de acceso.
3. Configurar procedimientos de rotación de chaves de emergencia (`docs/source/nexus/key_rotation.md`) y verificar el runbook.

## Etapa 4 - Integraçao CI/CD
1. Configurar tuberías:
   - Construir y publicar el validador de imágenes/Torii (GitHub Actions o GitLab CI).
   - Validacao automatizada de configuracao (lint de genesis, verificacao de assinaturas).
   - Pipelines de implementación (Helm/Kustomize) para clusters de staging y producción.
2. Implementar pruebas de humo no CI (subir cluster efemero, rodar suite canonica de transacoes).
3. Agregar scripts de reversión para implementaciones con otros runbooks documentados.## Etapa 5 - Observabilidade y alertas
1. Implemente la pila de monitoreo (Prometheus + Grafana + Alertmanager) por región.
2. Coletar métricas centrales:
  - `nexus_consensus_height`, `nexus_finality_lag`, `torii_request_duration_seconds`, `validator_peer_count`.
   - Registros vía Loki/ELK para servicios Torii y consenso.
3. Paneles de control:
   - Saude do consenso (altura de bloque, finalidade, estatus de pares).
   - Latencia y taxones de error de API Torii.
   - Transacciones de gobierno y estatus de propuestas.
4. Alertas:
   - Parada de producao de blocos (>2 intervalos de bloco).
   - Queda no número de pares bajo el quórum.
   - Picos na taxa de error de Torii.
   - Backlog da fila de propuestas de gobierno.

## Etapa 6 - Validacao y traspaso
1. Rodar validación de extremo a extremo:
   - Submeter proposta degobernanca (ej. mudanca de parametro).
   - Procesar la aprobación del consejo para garantizar que la tubería de gobierno funcione.
   - Rodar diferencia de estado del libro mayor para garantizar consistencia.
2. Documentar o runbook para on-call (resposta a incidentes, failover, scaling).
3. Comunicar prontidao para equipes SoraFS/SoraNet; Confirmar que despliega piggyback podem apontar para nos Nexus.## Lista de verificación de implementación
- [ ] Auditorio de génesis/configuracao concluida.
- [ ] Nos validadores y observadores implementados con consenso saudavel.
- [ ] Chaves degobernanza carregadas, propuesta testada.
- [ ] Tuberías CI/CD rodando (construcción + despliegue + pruebas de humo).
- [ ] Paneles de observabilidade activos con alertas.
- [] Documentacao de handoff entregue aos times downstream.