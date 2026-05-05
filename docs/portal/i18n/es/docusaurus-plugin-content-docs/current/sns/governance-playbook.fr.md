---
lang: es
direction: ltr
source: docs/portal/docs/sns/governance-playbook.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::nota Fuente canónica
Esta página se refleja `docs/source/sns/governance_playbook.md` y se mantiene en mantenimiento.
copia canónica del portal. El archivo fuente persiste para las relaciones públicas de traducción.
:::

# Guía de gestión del servicio de nombres de Sora (SN-6)

**Estatuto:** Redige 2026-03-24 - referencia vivante pour la preparación SN-1/SN-6  
**Liens du roadmap:** SN-6 "Cumplimiento y resolución de disputas", SN-7 "Resolver & Gateway Sync", política de direcciones ADDR-1/ADDR-5  
**Requisitos previos:** Schema du registre dans [`registry-schema.md`](./registry-schema.md), contrato de API del registrador dans [`registrar-api.md`](./registrar-api.md), guía UX de direcciones dans [`address-display-guidelines.md`](./address-display-guidelines.md), y regles la estructura de cuentas en [`docs/account_structure.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/account_structure.md).

Este libro de jugadas decrit comment les organes de gouvernance du Sora Name Service (SNS)
adoptent des chartes, approuvent des enregistrements, escaladent les litiges, et
Esto provoca que los estados del solucionador y de la puerta de enlace se restablezcan. Estoy satisfecho
La exigencia de la hoja de ruta según la CLI `sns governance ...`, los manifiestos.
Norito et les artefactos de auditoría participan en una referencia única cote operator
avant N1 (público lancement).

## 1. Portée y público

El documento disponible:- Miembros del Consejo de Gobierno que votan sobre las cartas y las políticas de
  suffixe et les resultats de litige.
- Miembros del consejo de tutores que emiten geles de urgencia y examinadores
  les reversiones.
- Stewards de suffixe qui gerent les files du registrador, approuvent les encheres
  et gerent les partages de revenus.
- Operadores resolutores/gateway responsables de la propagación SoraDNS, des mises a
  jour GAR, et des garde-fous de telemetrie.
- Equipes de conformite, tresorerie et support qui doivent demontrer que chaque
  action de gouvernance a laisse des artefactos Norito auditables.

La cobertura de las fases de beta cerrada (N0), lanzamiento público (N1) y expansión (N2)
enumeradas en `roadmap.md` en función de cada flujo de trabajo con requisitos previos,
Dashboards et vías de escalada.

## 2. Roles y carta de contacto| Rol | Responsabilidades principales | Artefactos y principios de telemetría | Escalada |
|------|--------------------------------|------------------------------------|---------|
| Consejo de Gobierno | Redige et ratifie les chartes, politiques de suffixe, veredictos de litige et rotaciones de mayordomos. | `docs/source/sns/governance_addenda/`, `artifacts/sns/governance/*`, boletines del consejo de valores vía `sns governance charter submit`. | President du conseil + suivi du docket de gouvernance. |
| Consejo de guardianes | Emet des gels soft/hard, canons d'urgence, et revues 72 h. | Tickets guardian emis par `sns governance freeze`, manifestes d'override consignas sous `artifacts/sns/guardian/*`. | Guardián de rotación de guardia (<=15 min ACK). |
| Mayordomos de sufijo | Gestionar los archivos del registrador, los archivos, los niveles de precio y el cliente de comunicación; reconnaissent les conformités. | Políticas de steward dans `SuffixPolicyV1`, fichas de referencia de precios, agradecimientos de steward stockes a cote des memos reglementaires. | Líder del administrador del programa + PagerDuty por sufijo. |
| Registrador de operaciones y facturación | Abrir los puntos finales `/v1/sns/*`, conciliar los pagos, activar la telemetría y mantener las instantáneas CLI. | Registrador API ([`registrar-api.md`](./registrar-api.md)), métricas `sns_registrar_status_total`, archivos previos de pago en `artifacts/sns/payments/*`. | Gerente de turno del registrador y tresorerie de enlace. || Operadores de resolución y puerta de enlace | Mantenimiento de SoraDNS, GAR y el estado de la puerta de enlace alineados con los eventos del registrador; Difundir las medidas de transparencia. | [`docs/source/soradns/deterministic_hosts.md`](../../../source/soradns/deterministic_hosts.md), [`docs/source/reports/soradns_transparency.md`](../../../source/reports/soradns_transparency.md), `dashboards/alerts/soradns_transparency_rules.yml`. | SRE resolver de guardia + puerta de enlace de operaciones de puente. |
| Tesorería y finanzas | Aplique la repartición 70/30, las carve-outs de referidos, los depósitos fiscales/tresorerie y las atestados SLA. | Manifiestos de acumulación de ingresos, exportaciones Stripe/tresorerie, apéndices KPI trimestriels sous `docs/source/sns/regulatory/`. | Controleur de finanzas + responsable conformé. |
| Enlace conforme y reglamentario | Traje las obligaciones globales (EU DSA, etc.), cumplió un día los convenios KPI y depose des divulgations. | Notas reglamentarias en `docs/source/sns/regulatory/`, decks de referencia, entradas `ops/drill-log.md` para mesa de ensayo. | Liderar el programa de conformidad. |
| Soporte / SRE de guardia | Gestionar los incidentes (colisiones, derivación de facturación, paneles de resolución), coordinar el cliente de comunicación y poseer los runbooks. | Modeles d'incident, `ops/drill-log.md`, preuves de labo mises en scene, transcripciones Slack/war-room archivees sous `incident/`. | Rotación de guardia SNS + gestión SRE. |

## 3. Artefactos canónicos y fuentes de donnees| Artefacto | Emplazamiento | Objetivo |
|----------|-------------|---------|
| Tabla + adiciones KPI | `docs/source/sns/governance_addenda/` | Cartas firmadas con control de versión, acuerdos KPI y decisiones de gobierno referenciadas por los votos CLI. |
| Esquema de registro | [`registry-schema.md`](./registry-schema.md) | Estructuras canónicas Norito (`NameRecordV1`, `SuffixPolicyV1`, `RevenueAccrualEventV1`). |
| Contrato de registro | [`registrar-api.md`](./registrar-api.md) | Cargas útiles REST/gRPC, métricas `sns_registrar_status_total` y attentes des hooks de gouvernance. |
| Guía UX de direcciones | [`address-display-guidelines.md`](./address-display-guidelines.md) | Rendus canoniques i105 (preferido) y compresas (segunda opción) reproducidos por billeteras/exploradores. |
| Documentos SoraDNS / GAR | [`docs/source/soradns/deterministic_hosts.md`](../../../source/soradns/deterministic_hosts.md), [`docs/source/reports/soradns_transparency.md`](../../../source/reports/soradns_transparency.md) | La derivación determina los hosts, el flujo de trabajo del colador de transparencia y las reglas de alerta. |
| Notas reglamentarias | `docs/source/sns/regulatory/` | Notes d'accueil par juridiction (ex. EU DSA), agradecimientos del administrador, anexos de modelo. |
| Diario de ejercicios | `ops/drill-log.md` | Journal des rehearsals caos et IR requis avant sorties de stage. |
| Almacenamiento de artefactos | `artifacts/sns/` | Preuves de pago, guardián de tickets, resolución de diferencias, KPI de exportaciones y clasificación CLI firmado por el producto `sns governance ...`. |Todas las acciones de gobernanza deben hacer referencia a menos de un artefacto del
tableau ci-dessus afin que les auditeurs puissent reconstruire la trace de
decisión en 24 horas.

## 4. Libros de jugadas del ciclo de vida

### 4.1 Mociones de charte et Steward| Etapa | Propietario | CLI / Preuve | Notas |
|-------|-------------|--------------|-------|
| Redigerar el anexo y los deltas KPI | Ponente del consejo + administrador principal | Plantilla Markdown stocke sous `docs/source/sns/governance_addenda/YY/` | Incluya los ID de KPI del acuerdo, ganchos de telemetría y condiciones de activación. |
| Soumettre la propuesta | Presidente del consejo | `sns governance charter submit --input SN-CH-YYYY-NN.md` (producto `CharterMotionV1`) | La CLI emite un manifiesto Norito almacenado en `artifacts/sns/governance/<id>/charter_motion.json`. |
| Voto y reconocimiento tutor | Consejo + tutores | `sns governance ballot cast --proposal <id>` y `sns governance guardian-ack --proposal <id>` | Joindre les minutes hashs et les preuves de quorum. |
| Azafato de aceptación | Delegado del programa | `sns governance steward-ack --proposal <id> --signature <file>` | Requis avant changement des politiques de suffixe; registre el sobre bajo `artifacts/sns/governance/<id>/steward_ack.json`. |
| Activación | Operaciones del registrador | Mettre a jour `SuffixPolicyV1`, rafraichir les caches du registrador, publier una note dans `status.md`. | Marca de tiempo del registro de activación en `sns_governance_activation_total`. |
| Diario de auditoría | Conforme | Ajouter une entree a `docs/source/sns/regulatory/<jurisdiction>/<cycle>.md` et au diario de taladro si efecto de mesa. | Incluya referencias en paneles de telemetría y diferencias políticas. |

### 4.2 Aprobaciones de registro, registro y precio1. **Comprobación previa:** El registrador interroga `SuffixPolicyV1` para confirmar el nivel
   de precio, les termes disponibles et les fenetres de gracia/redención. garder les
   fichas de precios sincronizadas con el cuadro de niveles 3/4/5/6-9/10+ (nivel
   de base + coeficientes de sufijo) documente dans le roadmap.
2. **Oferta sellada:** Para la prima de los pools, ejecute el ciclo de compromiso de 72 h /
   Revelación de 24 h a través de `sns governance auction commit` / `... reveal`. Publicar la lista
   des commits (hashes únicos) en `artifacts/sns/auctions/<name>/commit.json`
   afin que les auditeurs puissent verifier l'aleatoire.
3. **Verificación de pago:** Los registradores válidos `PaymentProofV1` par rapport
   aux repartitions de tresorerie (70% tresorerie / 30% mayordomo con carve-out de
   referencia <=10%). Almacenar el JSON Norito en `artifacts/sns/payments/<tx>.json`
   et le lier dans la reponse du registrador (`RevenueAccrualEventV1`).
4. **Hook de gouvernance:** Adjunto `GovernanceHookV1` para los nombres premium/guarded
   en referencia a los identificadores de proposición del consejo y a las firmas del administrador.
   Les ganchos manquants declencent `sns_err_governance_missing`.
5. **Activación + resolución de sincronización:** Una vez que Torii emet el evento de registro,
   Declencher le tailer de transparence du resolutor pour confirmer que le nouvel
   etat GAR/zone s'est propage (ver 4.5).
6. **Cliente de divulgación:** Mettre a jour le ledger oriente client (wallet/explorer)a través de los accesorios compartidos en [`address-display-guidelines.md`](./address-display-guidelines.md),
   En s'assurant que les rendus i105 et comprime las guías auxiliares correspondientes copia/QR.

### 4.3 Renovaciones, facturación y conciliación de tesorería- **Flujo de trabajo de renovación:** Les registradores aplican la ventana de gracia de
  30 días + la ventana de redención de 60 días especificada en `SuffixPolicyV1`.
  Después de 60 días, la secuencia de recuperación holandesa (7 días, frais 10x
  decroissant de 15%/jour) se reduce automáticamente a través de `sns governance reopen`.
- **Repartition des revenus:** Chaque renouvellement ou transfert cree un
  `RevenueAccrualEventV1`. Les exports de tresorerie (CSV/Parquet) doivent
  reconciliador ces evenements quotidiennement; joindre les preuves a
  `artifacts/sns/treasury/<date>.json`.
- **Exclusiones de referencia:** Los porcentajes de opciones de referencia son posteriores
  par suffixe en ajoutant `referral_share` a la politique steward. Los registradores
  emettent le split final et stockent les manifestes de reference a cote de la
  preuve de pago.
- **Cadencia de informes:** La finanzas públicas de los anexos KPI mensuelles
  (inscripciones, renovaciones, ARPU, utilización de litigios/bonos) sous
  `docs/source/sns/regulatory/<suffix>/YYYY-MM.md`. Los paneles de control no
  s'appuyer sur les memes table exportees afin que les chiffres Grafana
  corresponsal aux preuves du ledger.
- **Revue KPI mensuelle:** Le checkpoint du premier mardi associe le lead Finance,
  le steward de service et le PM program. Abrir el [panel de KPI de SNS](./kpi-dashboard.md)
  (incrustar portal de `sns-kpis` / `dashboards/grafana/sns_suffix_analytics.json`),exportador de tablas de rendimiento + ingresos del registrador, consignador de deltas
  dans l'annexe, et joindre les artefactos au memo. Declencher un incidente si la
  revista trouve des incumplimientos SLA (fenetres de congelación >72 h, pics d'erreurs du
  registrador, derivar ARPU).

### 4.4 Geles, litigios y apelaciones| Fase | Propietario | Acción y lucha | Acuerdo de Nivel de Servicio |
|-------|--------------|------------------|-----|
| Demanda de gel suave | Mayordomo / apoyo | Deposer un ticket `SNS-DF-<id>` avec preuves de paiement, reference de bond de litige et selecteur(s) afecto(s). | <=4 h después del plato principal. |
| Guardián de entradas | Consejo tutor | `sns governance freeze --selector <i105> --reason <text> --until <ts>` producto `GuardianFreezeTicketV1`. Guarde el JSON del billete en `artifacts/sns/guardian/<id>.json`. | <=30 min ACK, <=2 h de ejecución. |
| Ratificación del consejo | Consejo de Gobierno | Aprobar o rechazar los geles, documentar la decisión con el gravamen frente al ticket guardian y el resumen del vínculo de litigio. | Prochaine session du conseil ou vote asincrono. |
| Panel de arbitraje | Conformite + azafato | Convoque un panel de 7 jurados (según la hoja de ruta) con los boletines hashes a través de `sns governance dispute ballot`. Joindre les recus de vote anonymises au paquet d'incident. | Veredicto <=7 días después del depósito del bono. |
| Apelación | Guardián + consejo | Les appels doublent le bond et repetent le processus des jurors; Registre el manifiesto Norito `DisputeAppealV1` y haga referencia al ticket primario. | <=10 días. |
| Degel y remediación | Registrador + solucionador de operaciones | El ejecutor `sns governance unfreeze --selector <i105> --ticket <id>`, ingresa al día el estado del registrador y propaga las diferencias GAR/resolver. | Inmediato después del veredicto. |Les canons d'urgence (gels declenches par guardian <=72 h) después del flujo del meme
mais exigente una revista retroactiva del consejo y una nota de transparencia bajo
`docs/source/sns/regulatory/`.

### 4.5 Resolución de propagación y puerta de enlace

1. **Hook d'evenement:** Chaque event de registre emet vers le flux d'evenements
   solucionador (`tools/soradns-resolver` SSE). Les ops resolutor s'abonnent et
   registrar las diferencias a través del tailer de transparencia
   (`scripts/telemetry/run_soradns_transparency_tail.sh`).
2. **Mise a jour du template GAR:** Les gateways doivent mettre a jour les templates
   Referencias GAR par `canonical_gateway_suffix()` y re-signer la liste
   `host_pattern`. Stocker les diffs dans `artifacts/sns/gar/<date>.patch`.
3. **Publicación del archivo de zona:** Utilice el esqueleto de archivo de zona decrit dans
   `roadmap.md` (nombre, ttl, cid, prueba) y le pousser vers Torii/SoraFS. Archivador
   el JSON Norito en `artifacts/sns/zonefiles/<name>/<version>.json`.
4. **Verificación de transparencia:** Ejecutor `promtool test rules dashboards/alerts/tests/soradns_transparency_rules.test.yml`
   Para asegurar que las alertas estén verdes. Joindre la sortie texte
   Prometheus en informe de transparencia hebdomadaire.
5. **Puerta de enlace de auditoría:** Registrador de echantillons d'en-tetes `Sora-*` (caché de políticas,
   CSP, digest GAR) et les joindre au journal de gouvernance afin que les
   Los operadores pueden probar que le gateway a servi le nouveau nom avec les
   Garde-fous prevus.

## 5. Telemetría e informes| Señal | Fuente | Descripción / Acción |
|--------|--------|----------------------|
| `sns_registrar_status_total{result,suffix}` | Registrador de gestores Torii | Compteur succes/erreur pour enregistrements, renouvellements, gels, transferts; Alerta cuando `result="error"` aumenta por sufijo. |
| `torii_request_duration_seconds{route="/v1/sns/*"}` | Métricas Torii | SLO de latencia para la API de controladores; Alimente des Dashboards issus de `torii_norito_rpc_observability.json`. |
| `soradns_bundle_proof_age_seconds` y `soradns_bundle_cid_drift_total` | Tailer de resolución de transparencia | Detecte des preuves perimees ou des derives GAR; garde-fous define en `dashboards/alerts/soradns_transparency_rules.yml`. |
| `sns_governance_activation_total` | Gobernanza CLI | Compteur incrementa a cada activación de charte/adendum; utilizar para reconciliar las decisiones del consejo frente a las adendas públicas. |
| Calibre `guardian_freeze_active` | Guardián CLI | Se adaptan a las ventanas de gel blandas/duras seleccionadas por el seleccionador; página SRE si el valor restante `1` au-dela du SLA declara. |
| Paneles de control KPI anexos | Finanzas / Documentos | Rollups mensuels publica con les memos reglementaires; El portal se integra a través del [Panel de KPI de SNS] (./kpi-dashboard.md) para que los administradores y reguladores accedan al meme vue Grafana. |

## 6. Exigencias de prueba y auditoría| Acción | Evidencia un archivador | Material de archivo |
|--------|---------------------|----------|
| Cambio de carta / política | Firma del manifiesto Norito, CLI de transcripción, KPI de diferenciación, administrador de reconocimiento. | `artifacts/sns/governance/<proposal-id>/` + `docs/source/sns/governance_addenda/`. |
| Registro / renovación | Carga útil `RegisterNameRequestV1`, `RevenueAccrualEventV1`, antes de pago. | `artifacts/sns/payments/<tx>.json`, registros API del registrador. |
| Enchere | Manifiestos comprometer/revelar, grano de aleatoire, tabla de cálculo del gagnant. | `artifacts/sns/auctions/<name>/`. |
| Gel/degel | Ticket guardián, hash de voto del consejo, URL de registro de incidente, modelo de comunicación del cliente. | `artifacts/sns/guardian/<ticket>/`, `incident/<date>-sns-*.md`. |
| Resolvedor de propagación | Archivo de zona diferencial/GAR, extraído JSONL del minorista, instantánea Prometheus. | `artifacts/sns/resolver/<date>/` + informes de transparencia. |
| Regulador de ingesta | Memo d'accueil, rastreador de fechas límite, administrador de reconocimiento, KPI de cambios de currículum. | `docs/source/sns/regulatory/<jurisdiction>/<cycle>.md`. |

## 7. Lista de verificación de puerta de fase| Fase | Criterios de salida | Paquete de pruebas |
|-------|--------------------|------------------|
| N0 - Beta cerrada | Esquema de registro SN-1/SN-2, manual del registrador CLI, guardián del simulacro completo. | Motion de charte + ACK Steward, logs de simulacro del registrador, informe de resolución de transparencia, entrada en `ops/drill-log.md`. |
| N1 - Lanzamiento público | Encheres + niveles de precios fijos activos para `.sora`/`.nexus`, autoservicio de registrador, resolución de sincronización automática, paneles de facturación. | Diferencia de hoja de precio, resultados CI del registrador, pago anexo/KPI, salida del tailer de transparencia, notas del incidente del ensayo. |
| N2 - Ampliación | `.dao`, revendedor de API, portal de litigios, administrador de cuadros de mando, análisis de paneles. | Captura ecran du portail, métricas SLA de litige, exportaciones de cuadros de mando de administrador, charte de gouvernance mise a jour avec politiques reseller. |

Les salidas de fase exigentes de ejercicios de registro de mesa (parcours registre
happy path, gel, panne resolver) con artefactos adjuntos en `ops/drill-log.md`.

## 8. Respuesta aux incidentes y escalada| Declencheur | Severita | Propietario inmediato | Acciones obligatorias |
|-------------|----------|-----------------------|----------------------|
| Derive resolver/GAR ou preuves perimees | Septiembre 1 | SRE solucionador + consejo tutor | Localizador del solucionador de guardia, capturador de la salida, decididor si los nombres afectan deben ser geles, cartel un estatuto todos los 30 min. |
| Registrador de panel, control de facturación y errores generalizados de API | Septiembre 1 | Gerente de turno del registrador | Arreter les nouvelles encheres, basculer sur CLI manuel, notifier stewards/tresorerie, joindre les logs Torii au doc ​​d'incident. |
| Litigio sobre un solo nombre, desajuste de pago o escalada de clientes | Septiembre 2 | Steward + soporte líder | El cobrador de los preuves de paiement, determinador si un gel soft es necesario, responderá al demandeur dans le SLA, consignar le resultat dans le tracker de litige. |
| Constancia de auditoría de conformidad | Septiembre 2 | Enlace conforme | Rediger un plan de remediación, depositar un memo sous `docs/source/sns/regulatory/`, planificador una sesión de consejo de seguimiento. |
| Ejercicio o ensayo | Septiembre 3 | Programa PM | Ejecute el script de escenario después de `ops/drill-log.md`, archive los artefactos, etiquete los espacios en blanco como los tajos de la hoja de ruta. |Todos los incidentes no creo que `incident/YYYY-MM-DD-sns-<slug>.md` avec des
tablas de propiedad, registros de comandos y referencias previas
Produites tout au long de ce playbook.

## 9. Referencias

-[`registry-schema.md`](./registry-schema.md)
-[`registrar-api.md`](./registrar-api.md)
-[`address-display-guidelines.md`](./address-display-guidelines.md)
-[`docs/account_structure.md`](../../../account_structure.md)
- [`docs/source/soradns/deterministic_hosts.md`](../../../source/soradns/deterministic_hosts.md)
-[`docs/source/reports/soradns_transparency.md`](../../../source/reports/soradns_transparency.md)
- `ops/drill-log.md`
- `roadmap.md` (secciones SNS, DG, ADDR)

Guarde este libro de jugadas cada día que le texto de tablas, superficies CLI
o los contratos de telemetría cambian; les entrees du roadmap qui referencent
`docs/source/sns/governance_playbook.md` siempre corresponde a la
revisión actual.