---
lang: es
direction: ltr
source: docs/portal/docs/sns/governance-playbook.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::nota Fuente canónica
Esta pagina refleja `docs/source/sns/governance_playbook.md` y ahora sirve como
la copia canónica del portal. El archivo fuente persiste para PRs de traducción.
:::

# Playbook de gobernanza del Servicio de Nombres Sora (SN-6)

**Estado:** Borrador 2026-03-24 - referencia viva para la preparacion SN-1/SN-6  
**Enlaces del roadmap:** SN-6 "Cumplimiento y resolución de disputas", SN-7 "Resolver & Gateway Sync", política de direcciones ADDR-1/ADDR-5  
**Requisitos previos:** Esquema de registro en [`registry-schema.md`](./registry-schema.md), contrato de API de registrador en [`registrar-api.md`](./registrar-api.md), guía UX de direcciones en [`address-display-guidelines.md`](./address-display-guidelines.md), y reglas de estructura de cuentas en [`docs/account_structure.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/account_structure.md).

Este playbook describe como los cuerpos de gobernanza del Sora Name Service (SNS)
adoptan cartas, aprueban registros, escalan disputas, y prueban que los estados de
resolver y gateway permanecen sincronizados. Cumple el requisito del roadmap de
que la CLI `sns governance ...`, los manifiestos Norito y los artefactos de
auditoria compartan una única referencia operativa antes de N1 (lanzamiento
público).

## 1. Alcance y audiencia

El documento esta dirigido a:- Miembros del Consejo de Gobernanza que votan cartas, políticas de sufijos y
  resultados de disputas.
- Miembros de la junta de guardianes que emiten congelamientos de emergencia y
  reversiones revisadas.
- Stewards de sufijos que operan colas de registrador, aprueban subastas y
  gestionan repartos de ingresos.
- Operadores de resolutor/gateway responsables de la propagación SoraDNS,
  actualizaciones GAR y guardrails de telemetria.
- Equipos de cumplimiento, tesorería y soporte que deben demostrar que toda
  acción de gobernanza dejo artefactos Norito auditables.

Cubre las fases de beta cerrada (N0), lanzamiento público (N1) y expansión (N2)
listadas en `roadmap.md`, vinculando cada flujo de trabajo con la evidencia,
 Dashboards y rutas de escalada requeridas.

## 2. Roles y mapa de contactos| papel | Responsabilidades principales | Artefactos y telemetria principales | Escalación |
|------|-------------------------------|-------------------------------|------------|
| Consejo de Gobernanza | Redacta y ratifica cartas, políticas de sufijos, veredictos de disputa y rotaciones de stewards. | `docs/source/sns/governance_addenda/`, `artifacts/sns/governance/*`, votos del consejo almacenados vía `sns governance charter submit`. | Presidencia del consejo + tracker de agenda de gobernanza. |
| Junta de Guardianes | Emite congelamientos blandos/duros, cañones de emergencia y revisión de 72 h. | Tickets de guardian emitidos por `sns governance freeze`, manifiestos de override en `artifacts/sns/guardian/*`. | Guardia de guardia (<=15 min ACK). |
| Mayordomos de sufijo | Operan colas del registrador, subastas, niveles de precio y comunicación con clientes; reconocen cumplimientos. | Politicas de steward en `SuffixPolicyV1`, hojas de referencia de precios, acuses de steward junto a memorandos regulatorios. | Lider del programa de stewards + PagerDuty por sufijo. |
| Operaciones de registrador y facturación | Operan endpoints `/v2/sns/*`, concilian pagos, emiten telemetría y mantienen instantáneas de CLI. | API del registrador ([`registrar-api.md`](./registrar-api.md)), métricas `sns_registrar_status_total`, pruebas de pago en `artifacts/sns/payments/*`. | Duty manager del registrador y enlace de tesoreria. || Operadores de resolución y puerta de enlace | Mantienen SoraDNS, GAR y estado del gateway alineados con eventos del registrador; transmiten métricas de transparencia. | [`docs/source/soradns/deterministic_hosts.md`](../../../source/soradns/deterministic_hosts.md), [`docs/source/reports/soradns_transparency.md`](../../../source/reports/soradns_transparency.md), `dashboards/alerts/soradns_transparency_rules.yml`. | SRE del resolutor de guardia + puente ops de gateway. |
| Tesorería y Finanzas | Aplica reparto 70/30, carve-outs de referidos, declaraciones fiscales/tesoreria y atestaciones SLA. | Manifiestos de acumulación de ingresos, exportaciones Stripe/tesoreria, apéndices KPI trimestrales en `docs/source/sns/regulatory/`. | Controller de finanzas + oficial de cumplimiento. |
| Enlace de Cumplimiento y Regulación | Rastrea obligaciones globales (EU DSA, etc.), actualiza covenants KPI y presenta divulgaciones. | Memos regulatorios en `docs/source/sns/regulatory/`, decks de referencia, entradas `ops/drill-log.md` para ensayos tabletop. | Líder de programa de cumplimiento. |
| Soporte / SRE de guardia | Maneja incidentes (colisiones, deriva de facturacion, caidas de resolver), coordina mensajes a clientes y es dueno de runbooks. | Plantillas de incidente, `ops/drill-log.md`, evidencia de laboratorio, transcripciones Slack/war-room en `incident/`. | Rotacion de guardia SNS + gestion SRE. |

## 3. Artefactos canónicos y fuentes de datos| Artefacto | Ubicación | propuesta |
|----------|----------|---------|
| Carta + anexos KPI | `docs/source/sns/governance_addenda/` | Cartas firmadas con control de versión, convenios KPI y decisiones de gobernanza referenciadas por votos de CLI. |
| Esquema de registro | [`registry-schema.md`](./registry-schema.md) | Estructuras canónicas Norito (`NameRecordV1`, `SuffixPolicyV1`, `RevenueAccrualEventV1`). |
| Contrato del registrador | [`registrar-api.md`](./registrar-api.md) | Cargas útiles REST/gRPC, métricas `sns_registrar_status_total` y expectativas de Governance Hook. |
| Guía UX de direcciones | [`address-display-guidelines.md`](./address-display-guidelines.md) | Renderizados canónicos I105 (preferido) y comprimidos (segunda mejor opción) reflejados por wallets/explorers. |
| Documentos SoraDNS / GAR | [`docs/source/soradns/deterministic_hosts.md`](../../../source/soradns/deterministic_hosts.md), [`docs/source/reports/soradns_transparency.md`](../../../source/reports/soradns_transparency.md) | Derivación determinística de hosts, flujo de cola de transparencia y reglas de alertas. |
| Memos regulatorios | `docs/source/sns/regulatory/` | Notas de ingreso jurisdiccional (p. ej., EU DSA), acuses de Steward, anexos plantilla. |
| Registro de perforación | `ops/drill-log.md` | Registro de ensayos de caos e IR requeridos antes de salir de fase. |
| Almacén de artefactos | `artifacts/sns/` | Pruebas de pago, tickets de guardian, diffs de resolutor, exportaciones KPI y salida firmada de `sns governance ...`. |Todas las acciones de gobernanza deben referenciar al menos un artefacto en la
 tabla anterior para que los auditores reconstruyan el rastro de decisión en 24
 horas.

## 4. Libros de jugadas del ciclo de vida

### 4.1 Mociones de carta y mayordomo

| Paso | Dueño | CLI/Evidencia | Notas |
|------|-------|----------------|-------|
| Redactar addendum y deltas KPI | Relator del consejo + líder de mayordomo | Plantilla Markdown en `docs/source/sns/governance_addenda/YY/` | Incluir ID de KPI de convenio, ganchos de telemetría y condiciones de activación. |
| Presentar propuesta | Presidencia del consejo | `sns governance charter submit --input SN-CH-YYYY-NN.md` (produce `CharterMotionV1`) | La CLI emite manifiesto Norito en `artifacts/sns/governance/<id>/charter_motion.json`. |
| Voto y reconocimiento de tutores | Consejo + guardianes | `sns governance ballot cast --proposal <id>` y `sns governance guardian-ack --proposal <id>` | Adjuntar minutos con hash y pruebas de quorum. |
| Aceptación de mayordomo | Programa de azafatas | `sns governance steward-ack --proposal <id> --signature <file>` | Requerido antes de cambiar políticas de sufijos; guardar sobre en `artifacts/sns/governance/<id>/steward_ack.json`. |
| Activación | Operaciones del registrador | Actualizar `SuffixPolicyV1`, actualizar cachés del registrador, publicar nota en `status.md`. | Marca de tiempo de activación en `sns_governance_activation_total`. |
| Registro de auditoría | Cumplimiento | Agregar entrada a `docs/source/sns/regulatory/<jurisdiction>/<cycle>.md` y Drill Log si hubo tabletop. | Incluir referencias a paneles de telemetría y diferencias políticas. |

### 4.2 Registros, subastas y aprobaciones de precio1. **Preflight:** El registrador consulta `SuffixPolicyV1` para confirmar el nivel de
   precio, términos disponibles y ventanas de gracia/redención. Mantener hojas de
   precios sincronizados con la tabla de niveles 3/4/5/6-9/10+ (nivel base +
   coeficientes de sufijo) documentada en el roadmap.
2. **Subastas de oferta sellada:** Para pools premium, ejecutar el ciclo 72 h commit /
   Revelación de 24 h a través de `sns governance auction commit` / `... reveal`. Publicar la
   lista de confirmaciones (hashes solo) en `artifacts/sns/auctions/<name>/commit.json`
   para que los auditores verifiquen la aleatoriedad.
3. **Verificación de pago:** Los registradores validan `PaymentProofV1` contra el
   reparto de tesoreria (70% tesoreria / 30% steward con carve-out de referido <=10%).
   Guarde el JSON Norito en `artifacts/sns/payments/<tx>.json` y enlacelo en la
   respuesta del registrador (`RevenueAccrualEventV1`).
4. **Hook de gobernanza:** Adjuntar `GovernanceHookV1` para nombres premium/guarded
   con referencias a ids de propuesta del consejo y firmas de steward. Ganchos
   faltantes resultan en `sns_err_governance_missing`.
5. **Activación + sincronización de resolución:** Una vez que Torii emite el evento de
   registro, disparar el tailer de transparencia del resolutor para confirmar
   que el nuevo estado GAR/zone se propago (ver 4.5).
6. **Divulgacion al cliente:** Actualizar el libro mayor orientado al cliente
   (wallet/explorer) a través de los dispositivos compartidos en[`address-display-guidelines.md`](./address-display-guidelines.md), asegurando
   que los renderizados I105 y comprimidos coinciden con la guia de copia/QR.

### 4.3 Renovaciones, facturación y reconciliación de tesorería- **Flujo de renovación:** Los registradores aplican las ventanas de gracia de
  30 días + redención de 60 días especificadas en `SuffixPolicyV1`. Después de 60
  dias, la secuencia de reapertura holandesa (7 dias, tarifa 10x decayendo 15%/dia)
  se activa automáticamente vía `sns governance reopen`.
- **Reparto de ingresos:** Cada renovación o transferencia crea un
  `RevenueAccrualEventV1`. Las exportaciones de tesorería (CSV/Parquet) deben
  reconciliar con estos eventos diariamente; adjuntar pruebas en
  `artifacts/sns/treasury/<date>.json`.
- **Carve-outs de referidos:** Porcentajes de referidos opcionales se rastrean por
  sufijo agregando `referral_share` a la politica de steward. Los registradores
  emiten el split final y guardan manifiestos de referido junto a la prueba de pago.
- **Cadencia de reportes:** Finanzas publica anexos KPI mensuales (registros,
  renovaciones, ARPU, uso de disputas/bond) en
  `docs/source/sns/regulatory/<suffix>/YYYY-MM.md`. Los tableros deben tomar
  de las mismas tablas exportadas para que los numeros de Grafana coincidan con
  la evidencia del libro mayor.
- **Revisión KPI mensual:** El checkpoint del primer martes empareja al líder de
  finanzas, steward de turno y PM del programa. Abrir el [Panel de KPI de SNS](./kpi-dashboard.md)
  (insertar el portal de `sns-kpis` / `dashboards/grafana/sns_suffix_analytics.json`),
  exportar las tablas de throughput e ingresos del registrador, registrador deltas enel anexo y adjuntar los artefactos al memo. Disparar un incidente si la revisión
  encuentra brechas SLA (ventanas de congelación >72 h, picos de error del registrador,
  deriva de ARPU).

### 4.4 Congelamientos, disputas y apelaciones| Fase | Dueño | Acción y evidencia | Acuerdo de Nivel de Servicio |
|-------|-------|-------------------|-----|
| Solicitud de congelación suave | Azafato / soporte | Boleto de presentador `SNS-DF-<id>` con pruebas de pago, referencia de bond de disputa y selector(es) afectados. | <=4 h desde el ingreso. |
| Boleto de guardián | Junta de guardianes | `sns governance freeze --selector <I105> --reason <text> --until <ts>` produce `GuardianFreezeTicketV1`. Guarde el JSON del ticket en `artifacts/sns/guardian/<id>.json`. | <=30 min ACK, <=2 h de ejecución. |
| Ratificación del consejo | Consejo de gobernanza | Aprobar o rechazar congelamientos, decisión documental enlazada al ticket de guardian y digest del bond de disputa. | Próxima sesión del consejo o voto asincrono. |
| Panel de arbitraje | Cumplimiento + mayordomo | Convocar panel de 7 jurados (segun roadmap) con boletas hasheadas vía `sns governance dispute ballot`. Adjuntar recibos de voto anonimizados al paquete de incidente. | Veredicto <=7 días después del deposito de bono. |
| Apelación | Guardianes + consejo | Las apelaciones duplican el vínculo y repiten el proceso de jurados; registrador manifiesto Norito `DisputeAppealV1` y referenciar ticket primario. | <=10 días. |
| Descongelar y remediar | Registrador + operaciones de resolución | Ejecutar `sns governance unfreeze --selector <I105> --ticket <id>`, actualizar estado del registrador y propagar diffs GAR/resolver. | Inmediato despues del veredicto. |Los cañones de emergencia (congelamientos activados por guardianes <=72 h) siguen
el mismo flujo pero requiere revisión retroactiva del consejo y una nota de
transparencia en `docs/source/sns/regulatory/`.

### 4.5 Propagación de resolución y puerta de enlace

1. **Hook de evento:** Cada evento de registro emite al stream de eventos del
   solucionador (`tools/soradns-resolver` SSE). Ops de resolver se suscriben y
   registran diferencias a través del tailer de transparencia
   (`scripts/telemetry/run_soradns_transparency_tail.sh`).
2. **Actualización de plantilla GAR:** Los gateways deben actualizar plantillas GAR
   referenciadas por `canonical_gateway_suffix()` y refirmar la lista
   `host_pattern`. Guardar diferencias en `artifacts/sns/gar/<date>.patch`.
3. **Publicación de Zonefile:** Usar el esqueleto de Zonefile descrito en
   `roadmap.md` (nombre, ttl, cid, prueba) y empujarlo a Torii/SoraFS. Archivar el
   JSON Norito y `artifacts/sns/zonefiles/<name>/<version>.json`.
4. **Chequeo de transparencia:** Ejecutar `promtool test rules dashboards/alerts/tests/soradns_transparency_rules.test.yml`
   para asegurar que las alertas permanezcan verdes. Adjuntar la salida de texto
   de Prometheus al informe de transparencia semanal.
5. **Auditoria de gateway:** Registrar muestras de headers `Sora-*` (politica de
   cache, CSP, digest de GAR) y adjuntarlas al log de gobernanza para que
   los operadores puedan probar que el gateway sirvio el nuevo nombre con los
   barandillas previstas.

## 5. Telemetria y reportes| señal | Fuente | Descripción / Acción |
|--------|--------|----------------------|
| `sns_registrar_status_total{result,suffix}` | Controladores de registradores Torii | Contador de exito/error para registros, renovaciones, congelamientos, transferencias; alerta cuando `result="error"` sube por sufijo. |
| `torii_request_duration_seconds{route="/v2/sns/*"}` | Métricas de Torii | SLO de latencia para controladores API; Alimenta cuadros de mando basados ​​en `torii_norito_rpc_observability.json`. |
| `soradns_bundle_proof_age_seconds` y `soradns_bundle_cid_drift_total` | Tailer de transparencia del resolutor | Detecta pruebas obsoletas o deriva de GAR; barandillas definidas en `dashboards/alerts/soradns_transparency_rules.yml`. |
| `sns_governance_activation_total` | CLI de gobernanza | Contador incrementado cuando un charter/addendum se activa; se usa para reconciliar decisiones del consejo vs addenda publicadas. |
| Calibre `guardian_freeze_active` | CLI de guardián | Rastrea ventanas de congelación suave/dura por selector; paginar a SRE si el valor queda en `1` mas alla del SLA declarado. |
| Paneles de control del anexo KPI | Finanzas / Documentos | Rollups mensuales publicados junto a memos regulatorios; el portal los embebe via [SNS KPI Dashboard](./kpi-dashboard.md) para que stewards y reguladores accedan a la misma vista de Grafana. |

## 6. Requisitos de evidencia y auditoria| Acción | Evidencia a archivar | Almacén |
|--------|----------------------|---------|
| Cambio de carta / política | Manifiesto Norito firmado, transcripción CLI, diferencia KPI, acusación de administrador. | `artifacts/sns/governance/<proposal-id>/` + `docs/source/sns/governance_addenda/`. |
| Registro / renovación | Carga útil `RegisterNameRequestV1`, `RevenueAccrualEventV1`, prueba de pago. | `artifacts/sns/payments/<tx>.json`, registros de API del registrador. |
| Subasta | Manifiestos commit/reveal, semilla de aleatoriedad, hoja de cálculo de cálculo de ganador. | `artifacts/sns/auctions/<name>/`. |
| Congelar / descongelar | Ticket de guardián, hash de voto del consejo, URL de registro de incidente, plantilla de comunicación al cliente. | `artifacts/sns/guardian/<ticket>/`, `incident/<date>-sns-*.md`. |
| Propagación de resolución | Zonefile/GAR diff, extrae JSONL del tailer, instantánea Prometheus. | `artifacts/sns/resolver/<date>/` + informes de transparencia. |
| Ingreso regulatorio | Memo de ingreso, rastreador de fechas límite, acusación de mayordomo, resumen de cambios KPI. | `docs/source/sns/regulatory/<jurisdiction>/<cycle>.md`. |

## 7. Lista de verificación de puertas de fase| Fase | Criterios de salida | Paquete de evidencia |
|-------|--------------------|--------------------|
| N0 - Beta cerrada | Esquema de registro SN-1/SN-2, CLI de registrador manual, taladro de tutor completado. | Motion de carta + ACK de Steward, logs de dry-run del registrador, reporte de transparencia del resolutor, entrada en `ops/drill-log.md`. |
| N1 - Lanzamiento público | Subastas + niveles de precio fijo activos para `.sora`/`.nexus`, registrador self-service, auto-sync de resolutor, paneles de facturación. | Diff de hoja de precios, resultados CI del registrador, anexo de pagos/KPI, salida de tailer de transparencia, notas de ensayo de incidente. |
| N2 - Ampliación | `.dao`, API de revendedor, portal de disputas, cuadros de mando de administrador, paneles de análisis. | Capturas del portal, métricas SLA de disputas, exportaciones de scorecards de steward, charter de gobernanza actualizado con políticas de reseller. |

Las salidas de fase requieren taladros registrados de mesa (registro happy path,
congelación, interrupción de resolución) con artefactos adjuntos en `ops/drill-log.md`.

## 8. Respuesta a incidentes y escalada| Gatillo | Severidad | Dueno inmediato | Acciones obligatorias |
|---------|----------|----------------|-----------------------|
| Deriva de resolución/GAR o pruebas obsoletas | Septiembre 1 | Resolver SRE + junta de guardianes | Paginar on-call del resolutor, capturar salida del tailer, decidir si congelar nombres afectados, publicar estado cada 30 min. |
| Caida de registrador, fallo de facturación o errores API generalizados | Septiembre 1 | Gerente de turno del registrador | Detener nuevas subastas, cambiar un manual CLI, notificar a stewards/tesoreria, adjuntar registros de Torii al doc de incidente. |
| Disputa de nombre único, discrepancia de pago o escalada de cliente | Septiembre 2 | Steward + líder de soporte | Recopilar pruebas de pago, decidir si hace falta congelación suave, responder al solicitante dentro del SLA, registrar resultado en tracker de disputa. |
| Hallazgo de auditoria de cumplimiento | Septiembre 2 | Enlace de cumplimiento | Redactar plan de remediacion, archivar memo en `docs/source/sns/regulatory/`, agendar sesión de consejo de seguimiento. |
| Taladro o ensayo | Septiembre 3 | PM del programa | Ejecutar escenario guionado desde `ops/drill-log.md`, archivar artefactos, marcar gaps como tareas del roadmap. |

Todos los incidentes deben crear `incident/YYYY-MM-DD-sns-<slug>.md` con tablas
de propiedad, logs de comandos y referencias a la evidencia producida en este
libro de jugadas.

## 9. Referencias-[`registry-schema.md`](./registry-schema.md)
-[`registrar-api.md`](./registrar-api.md)
-[`address-display-guidelines.md`](./address-display-guidelines.md)
-[`docs/account_structure.md`](../../../account_structure.md)
- [`docs/source/soradns/deterministic_hosts.md`](../../../source/soradns/deterministic_hosts.md)
-[`docs/source/reports/soradns_transparency.md`](../../../source/reports/soradns_transparency.md)
- `ops/drill-log.md`
- `roadmap.md` (secciones SNS, DG, ADDR)

Mantener este playbook actualizado cuando cambien el texto de cartas, las
superficies de CLI o los contratos de telemetría; las entradas del roadmap que
referencian `docs/source/sns/governance_playbook.md` deben coincidir siempre con
La revisión más reciente.