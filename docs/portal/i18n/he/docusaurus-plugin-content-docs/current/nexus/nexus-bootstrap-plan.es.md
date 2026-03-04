---
lang: he
direction: rtl
source: docs/portal/docs/nexus/nexus-bootstrap-plan.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
מזהה: nexus-bootstrap-plan
כותרת: Bootstrap y observabilidad de Sora Nexus
תיאור: תוכנית הפעלה עבור שרתים ב-linea el cluster central de validadores Nexus לפני שירותים אגררים SoraFS ו- SoraNet.
---

:::שימו לב פואנטה קנוניקה
Esta pagina refleja `docs/source/soranexus_bootstrap_plan.md`. Manten ambas copias alineadas hasta que las versiones localizadas lleguen al portal.
:::

# Plan de bootstrap y observabilidad de Sora Nexus

## אובייקטיביות
- Levantar la red base de validadores/observadores de Sora Nexus con llaves de gobernanza, APIs de Torii y monitoreo de consenso.
- Validar servicios centrales (Torii, consenso, persistencia) antes de habilitar despliegues piggyback de SoraFS/SoraNet.
- קביעת זרימות עבודה של CI/CD y לוחות מחוונים/אזהרות התבוננות עבור אסיגורר לה סלוד דה לה אדום.

## דרישות מוקדמות
- Material de llaves de gobernanza (multisig del consejo, llaves de comite) זמינים ב-HSM o Vault.
- בסיס Infraestructura (אשכולות Kubernetes o nodos חשופות מתכת) en regiones primaria/secundaria.
- קונפיגורציית bootstrap actualizada (`configs/nexus/bootstrap/*.toml`) que releje los ultimos parametros de consenso.

## Entornos de red
- Operar dos entornos Nexus עם מאפיינים אדומים:
- **Sora Nexus (mainnet)** - prefijo de red de produccion `nexus`, hospedando la gobernanza canonica y servicios piggyback de SoraFS/SoraNet (מזהה שרשרת I108UID30X /U18UID30X `00000000-0000-0000-0000-000000000753`).
- **Sora Testus (testnet)** - Prefijo de red de staging `testus`, que espeja la configuracion de mainnet para pruebas de integracion yvalidacion pre-release (שרשרת UUID `809574f5-fee7-5e69-bfcf-52451e42d50f`).
- Mantener archivos genesis separados, llaves de gobernanza y huellas de infraestructura para cada entorno. Testus actua como el banco de pruebas de todos los rollouts SoraFS/SoraNet לפני קידום המכירות של Nexus.
- צינורות ה-CI/CD של CI/CD deben desplegar primero in Testus, ejecutar tests tests automatizados, and requesterir promocion manual a Nexus une vez que pasen los checks.
- Los bundles de configuracion de referencia viven en `configs/soranexus/nexus/` (mainnet) y `configs/soranexus/testus/` (testnet), cada uno con `config.toml`, `genesis.json` y directorios de ejplo de admision I0800000X.

## פסו 1 - עדכון תצורה
1. Auditar la documentacion existente:
   - `docs/source/nexus/architecture.md` (קונצנזו, פריסה דה Torii).
   - `docs/source/nexus/deployment_checklist.md` (דרישות תשתית).
   - `docs/source/nexus/governance_keys.md` (procedimientos de custodia de llaves).
2. Validar que los archivos genesis (`configs/nexus/genesis/*.json`) se alineen con el roster actual de validadores y los pesos de staking.
3. Confirmer parametros de red:
   - Tamano de comite de consenso y quorum.
   - Intervalo de bloques / Umbrales de finalizacion.
   - תעודות TLS Torii.## Paso 2 - Despliegue del cluster bootstrap
1. אישור אישורים:
   - Desplegar instancias `irohad` (validadores) con volumnes persistentes.
   - חומת אש אסגורית המאפשרת תחבורה קונצנזואלית ו-Torii המוקדמות.
2. התחל שירותים Torii (REST/WebSocket) ב-TLS.
3. Desplegar nodos observadores (סולו lectura) para resiliencia adicional.
4. סקריפטים של אתחול האתחול (`scripts/nexus_bootstrap.sh`) להפצה בראשית, הסכמה מוקדמת וציוני הרשם.
5. בדיקות עשן Ejecutar:
   - Enviar transacciones de prueba via Torii (`iroha_cli tx submit`).
   - אימות ייצור/סיום טלמטריה.
   - Revisar replicacion del Ledger entre validadores/observadores.

## פאסו 3 - Gobernanza y gestion de llaves
1. Cargar la configuracion multisig del consejo; confirmar que las propuestas de gobernanza se puedan enviar y ratificar.
2. Almacenar de forma segura las llaves de consenso/comite; הגדרת גיבויים אוטומטיים עם רישום כניסה.
3. קבע את הליך הסיבוב של חירום (`docs/source/nexus/key_rotation.md`) ובדוק את ספר ההפעלה.

## Paso 4 - Integracion de CI/CD
1. הגדר צינורות:
   - Build y publicacion de imagenes de validator/Torii (פעולות GitHub או GitLab CI).
   - Validacion automatizada de configuracion (לבין בראשית, אימות של חברות).
   - Pipelines despliegue (Helm/Kustomize) עבור אשכולות של בימוי ייצור.
2. בדיקות עשן מיושמות en CI (levantar cluster efimero, correr suite canonica de transacciones).
3. תסריטים אגרים של החזרה לאחור עבור ספרי ריצה דוקומנטריים.

## Paso 5 - Observabilidad y alertas
1. Desplegar el stack de monitoreo (Prometheus + Grafana + Alertmanager) עבור האזור.
2. מדדים מרכזיים חוזרים:
  - `nexus_consensus_height`, `nexus_finality_lag`, `torii_request_duration_seconds`, `validator_peer_count`.
   - יומנים דרך Loki/ELK para servicios Torii y consenso.
3. לוחות מחוונים:
   - Salud de consenso (אלטורה דה בלוק, finalizacion, estado de peers).
   - Latencia/tasa de error de Torii API.
   - Transacciones de gobernanza y estado de propuestas.
4. התראות:
   - Paro de produccion de bloques (>2 intervalos de bloque).
   - Conteo de peers por debajo del quorum.
   - Picos en la tasa de error de Torii.
   - Backlog de la cola de propuestas de gobernanza.

## פסו 6 - תוקף מסירה
1. אימות מקצה לקצה:
   - Enviar una propuesta de gobernanza (עמ' ej., cambio de parametro).
   - Procesarla via aprobacion del consejo para asegurar que el pipeline de gobernanza funciona.
   - Ejecutar diff del Estado del Ledger para asegurar consistentencia.
2. ספר תעודה עבור כוננות (תשובה לאירועים, תקלה, אסקלדו).
3. Comunicar la disponibilidad a los equipos de SoraFS/SoraNet; confirmar que los despliegues piggyback puedan apuntar a nodos Nexus.## רשימת בדיקה ליישום
- [ ] Auditoria de genesis/configuracion completada.
- [ ] Nodos validadores y observadores desplegados consenso saludable.
- [ ] Llaves de gobernanza cargadas, propuesta probada.
- [ ] צינורות CI/CD corriendo (בנייה + פריסה + בדיקות עשן).
- [ ] לוחות מחוונים של פעילים עם התראות.
- [ ] Documentacion de handoff entregada a equipos downstream.