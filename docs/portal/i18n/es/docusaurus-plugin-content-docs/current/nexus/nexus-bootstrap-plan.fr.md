---
lang: es
direction: ltr
source: docs/portal/docs/nexus/nexus-bootstrap-plan.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: plan-bootstrap-nexus
título: Bootstrap y observabilidad Sora Nexus
Descripción: Plan operativo para conectar en línea el grupo central de validadores Nexus antes de agregar los servicios SoraFS y SoraNet.
---

:::nota Fuente canónica
Esta página refleja `docs/source/soranexus_bootstrap_plan.md`. Guarde las dos copias alineadas justo en el momento en que las versiones localizadas lleguen al portal.
:::

# Plan de arranque y observabilidad Sora Nexus

## Objetivos
- Mettre en place le reseau de validadores/observadores base Sora Nexus con claves de gobierno, API Torii y monitoreo del consenso.
- Validar los servicios de corazón (Torii, consenso, persistencia) antes de activar los despliegues a cuestas SoraFS/SoraNet.
- Establecer flujos de trabajo CI/CD y paneles/alertas de observabilidad para asegurar la seguridad del trabajo.

## Requisitos previos
- Material de claves de gobierno (multisig du conseil, claves de comité) disponibles en HSM o Vault.
- Infraestructura base (clusters Kubernetes o noeuds bare-metal) en las regiones primarias/secundarias.
- Configuración bootstrap mise a day (`configs/nexus/bootstrap/*.toml`) que refleja los últimos parámetros de consenso.## Investigación de entornos
- Operar dos entornos Nexus con prefijos de búsqueda distintos:
- **Sora Nexus (mainnet)** - prefijo de producción `nexus`, que protege el gobierno canónico y los servicios a cuestas SoraFS/SoraNet (ID de cadena `0x02F1` / UUID `00000000-0000-0000-0000-000000000753`).
- **Sora Testus (testnet)** - prefijo de investigación de preparación `testus`, espejo de la configuración de la red principal para las pruebas de integración y validación previa al lanzamiento (UUID de cadena `809574f5-fee7-5e69-bfcf-52451e42d50f`).
- Mantenimiento de los archivos génesis, de las claves de gobierno y de las empresas de infraestructura separadas para cada entorno. Testus sert de terreno de preuve pour todos los lanzamientos SoraFS/SoraNet antes de la promoción versión Nexus.
- Las tuberías CI/CD deben desplegar el acceso a las pruebas, ejecutar las pruebas de humo de forma automática y exigir un manual de promoción según Nexus una vez que se aprueben las comprobaciones.
- Los paquetes de configuración de referencia se encuentran en `configs/soranexus/nexus/` (mainnet) y `configs/soranexus/testus/` (testnet), cada uno de los contenidos, por ejemplo, `config.toml`, `genesis.json` y los repertorios de admisión Torii.## Etapa 1 - Revisión de configuración
1. Auditor de la documentación existente:
   - `docs/source/nexus/architecture.md` (consenso, diseño Torii).
   - `docs/source/nexus/deployment_checklist.md` (exigencias infra).
   - `docs/source/nexus/governance_keys.md` (procedimientos de guardia de llaves).
2. Valider que les fichiers genesis (`configs/nexus/genesis/*.json`) está alineado con la lista actual de validadores y pesos de apuesta.
3. Confirmar los parámetros encontrados:
   - Taille du comité de consenso y quórum.
   - Intervalle de bloc / seuils de finalite.
   - Puertos de servicio Torii y certificados TLS.

## Etapa 2: implementación del arranque del clúster
1. Provisioner les noeuds validadores:
   - Implementador de instancias `irohad` (validadores) con volúmenes persistentes.
   - Verificador de que las reglas de firewall autorizan el consenso de tráfico y Torii entre noeuds.
2. Compruebe los servicios Torii (REST/WebSocket) en cada validador con TLS.
3. Deployer des noeuds observateurs (conferencia única) para una resiliencia adicional.
4. Ejecute los scripts bootstrap (`scripts/nexus_bootstrap.sh`) para la génesis del distribuidor, marrer el consenso y registrar los nuevos.
5. Ejecutor de las pruebas de humo:
   - Soumettre des transacciones de test vía Torii (`iroha_cli tx submit`).
   - Verificador de la producción/finalización de bloques vía telemetría.
   - Verificador de la replicación del libro mayor entre validadores/observadores.## Etapa 3 - Gouvernance et gestion des cles
1. Cargador la configuración multifirma del consejo; confirmer que les propositions de gouvernance peuvent etre soumises et ratifiees.
2. Stocker de maniere securisee les cles de consenso/comité; Configurer des salvaguardas automáticas con registro de acceso.
3. Coloque los procedimientos de rotación de llaves de urgencia (`docs/source/nexus/key_rotation.md`) y verifique el runbook.

## Etapa 4 - Integración CI/CD
1. Configurar las canalizaciones:
   - Creación y publicación del validador de imágenes/Torii (GitHub Actions o GitLab CI).
   - Validación automática de configuración (génesis de pelusa, verificación de firmas).
   - Implementación de pipelines (Helm/Kustomize) para puesta en escena y producción de clusters.
2. Implementador de pruebas de humo en CI (realizador de un efeméride de clúster, ejecutor de la suite canónica de transacciones).
3. Agregar scripts de reversión para las tasas de implementación y documentar los runbooks.## Etapa 5 - Observabilite et alertas
1. Implemente la pila de monitoreo (Prometheus + Grafana + Alertmanager) por región.
2. Coleccionista de las métricas del corazón:
  - `nexus_consensus_height`, `nexus_finality_lag`, `torii_request_duration_seconds`, `validator_peer_count`.
   - Registros a través de Loki/ELK para los servicios Torii y consenso.
3. Paneles de control:
   - Sante du consenso (hauteur de bloc, finalité, statut des peers).
   - Latencia/error de API Torii.
   - Transacciones de gobierno y estatuto de proposiciones.
4. Alertas:
   - Arret de production de blocs (>2 intervalos de block).
   - Baisse du nombre de peers sous le quorum.
   - Fotos de errores de error Torii.
   - Atrasos en el expediente de propuestas de gobierno.

## Etapa 6 - Validación y transferencia
1. Ejecutar la validación de extremo a extremo:
   - Soumettre une proposition de gouvernance (p. ej. changement de parametre).
   - La faire passer par l'approbation du conseil pour s'assurer que le pipeline de gouvernance fonctionne.
   - Ejecutar una diferencia de estado del libro mayor para asegurar la coherencia.
2. Documentar el runbook para guardia (incidente de respuesta, conmutación por error, escalado).
3. Comunique la disponibilidad de equipos SoraFS/SoraNet; Confirme que las implementaciones piggyback pueden ser punteros frente a los nuevos Nexus.## Lista de verificación de implementación
- [ ] Auditoría de finalización de génesis/configuración.
- [ ] Noeuds validadores y observadores implementan con un consenso sain.
- [ ] Cles de gouvernance chargees, testador de la proposición.
- [ ] Pipelines CI/CD en marcha (construcción + despliegue + pruebas de humo).
- [] Paneles de observabilidad activa con alertas.
- [ ] Documentación de traspaso en vivo aux equipes downstream.