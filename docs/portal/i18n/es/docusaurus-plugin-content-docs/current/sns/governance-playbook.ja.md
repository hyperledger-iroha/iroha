---
lang: ja
direction: ltr
source: docs/portal/i18n/es/docusaurus-plugin-content-docs/current/sns/governance-playbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 17e91e63a71ff1825a030d4ad4743fb42e77b2c5581e9c8b4f108c7f4f9bb26a
source_last_modified: "2026-01-20T13:31:25+00:00"
translation_last_reviewed: 2026-01-30
---

---
id: governance-playbook
lang: es
direction: ltr
source: docs/portal/docs/sns/governance-playbook.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

:::note Fuente canonica
Esta pagina refleja `docs/source/sns/governance_playbook.md` y ahora sirve como
la copia canonica del portal. El archivo fuente persiste para PRs de traduccion.
:::

# Playbook de gobernanza del Servicio de Nombres Sora (SN-6)

**Estado:** Borrador 2026-03-24 - referencia viva para la preparacion SN-1/SN-6  
**Enlaces del roadmap:** SN-6 "Compliance & Dispute Resolution", SN-7 "Resolver & Gateway Sync", politica de direcciones ADDR-1/ADDR-5  
**Prerequisitos:** Esquema de registro en [`registry-schema.md`](./registry-schema.md), contrato de API de registrador en [`registrar-api.md`](./registrar-api.md), guia UX de direcciones en [`address-display-guidelines.md`](./address-display-guidelines.md), y reglas de estructura de cuentas en [`docs/account_structure.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/account_structure.md).

Este playbook describe como los cuerpos de gobernanza del Sora Name Service (SNS)
adoptan cartas, aprueban registros, escalan disputas, y prueban que los estados de
resolver y gateway permanecen sincronizados. Cumple el requisito del roadmap de
que la CLI `sns governance ...`, los manifiestos Norito y los artefactos de
auditoria compartan una unica referencia operativa antes de N1 (lanzamiento
publico).

## 1. Alcance y audiencia

El documento esta dirigido a:

- Miembros del Consejo de Gobernanza que votan cartas, politicas de sufijos y
  resultados de disputas.
- Miembros de la junta de guardianes que emiten congelamientos de emergencia y
  revisan reversiones.
- Stewards de sufijos que operan colas de registrador, aprueban subastas y
  gestionan repartos de ingresos.
- Operadores de resolver/gateway responsables de la propagacion SoraDNS,
  actualizaciones GAR y guardrails de telemetria.
- Equipos de cumplimiento, tesoreria y soporte que deben demostrar que toda
  accion de gobernanza dejo artefactos Norito auditables.

Cubre las fases de beta cerrada (N0), lanzamiento publico (N1) y expansion (N2)
listadas en `roadmap.md`, vinculando cada flujo de trabajo con la evidencia,
 dashboards y rutas de escalamiento requeridas.

## 2. Roles y mapa de contactos

| Rol | Responsabilidades principales | Artefactos y telemetria principales | Escalacion |
|------|-------------------------------|-------------------------------------|------------|
| Consejo de Gobernanza | Redacta y ratifica cartas, politicas de sufijos, veredictos de disputa y rotaciones de stewards. | `docs/source/sns/governance_addenda/`, `artifacts/sns/governance/*`, votos del consejo almacenados via `sns governance charter submit`. | Presidencia del consejo + tracker de agenda de gobernanza. |
| Junta de Guardianes | Emite congelamientos soft/hard, canones de emergencia y revisiones de 72 h. | Tickets de guardian emitidos por `sns governance freeze`, manifiestos de override en `artifacts/sns/guardian/*`. | Guardia on-call (<=15 min ACK). |
| Stewards de sufijo | Operan colas del registrador, subastas, niveles de precio y comunicacion con clientes; reconocen cumplimientos. | Politicas de steward en `SuffixPolicyV1`, hojas de referencia de precios, acuses de steward junto a memorandos regulatorios. | Lider del programa de stewards + PagerDuty por sufijo. |
| Operaciones de registrador y facturacion | Operan endpoints `/v1/sns/*`, reconcilian pagos, emiten telemetria y mantienen snapshots de CLI. | API del registrador ([`registrar-api.md`](./registrar-api.md)), metricas `sns_registrar_status_total`, pruebas de pago en `artifacts/sns/payments/*`. | Duty manager del registrador y enlace de tesoreria. |
| Operadores de resolver y gateway | Mantienen SoraDNS, GAR y estado del gateway alineados con eventos del registrador; transmiten metricas de transparencia. | [`docs/source/soradns/deterministic_hosts.md`](../../../source/soradns/deterministic_hosts.md), [`docs/source/reports/soradns_transparency.md`](../../../source/reports/soradns_transparency.md), `dashboards/alerts/soradns_transparency_rules.yml`. | SRE del resolver on-call + puente ops de gateway. |
| Tesoreria y Finanzas | Aplica reparto 70/30, carve-outs de referidos, declaraciones fiscales/tesoreria y atestaciones SLA. | Manifiestos de acumulacion de ingresos, exportaciones Stripe/tesoreria, apendices KPI trimestrales en `docs/source/sns/regulatory/`. | Controller de finanzas + oficial de cumplimiento. |
| Enlace de Cumplimiento y Regulacion | Rastrea obligaciones globales (EU DSA, etc.), actualiza covenants KPI y presenta divulgaciones. | Memos regulatorios en `docs/source/sns/regulatory/`, decks de referencia, entradas `ops/drill-log.md` para ensayos tabletop. | Lider de programa de cumplimiento. |
| Soporte / SRE On-call | Maneja incidentes (colisiones, drift de facturacion, caidas de resolver), coordina mensajes a clientes y es dueno de runbooks. | Plantillas de incidente, `ops/drill-log.md`, evidencia de laboratorio, transcripciones Slack/war-room en `incident/`. | Rotacion on-call SNS + gestion SRE. |

## 3. Artefactos canonicos y fuentes de datos

| Artefacto | Ubicacion | Proposito |
|----------|----------|---------|
| Carta + anexos KPI | `docs/source/sns/governance_addenda/` | Cartas firmadas con control de version, covenants KPI y decisiones de gobernanza referenciadas por votos de CLI. |
| Esquema de registro | [`registry-schema.md`](./registry-schema.md) | Estructuras Norito canonicas (`NameRecordV1`, `SuffixPolicyV1`, `RevenueAccrualEventV1`). |
| Contrato del registrador | [`registrar-api.md`](./registrar-api.md) | Payloads REST/gRPC, metricas `sns_registrar_status_total` y expectativas de governance hook. |
| Guia UX de direcciones | [`address-display-guidelines.md`](./address-display-guidelines.md) | Renderizados canonicos IH58 (preferido) y comprimidos (segunda mejor opcion) reflejados por wallets/explorers. |
| Docs SoraDNS / GAR | [`docs/source/soradns/deterministic_hosts.md`](../../../source/soradns/deterministic_hosts.md), [`docs/source/reports/soradns_transparency.md`](../../../source/reports/soradns_transparency.md) | Derivacion deterministica de hosts, flujo de tailer de transparencia y reglas de alertas. |
| Memos regulatorios | `docs/source/sns/regulatory/` | Notas de ingreso jurisdiccional (p. ej., EU DSA), acuses de steward, anexos plantilla. |
| Drill log | `ops/drill-log.md` | Registro de ensayos de caos e IR requeridos antes de salir de fase. |
| Almacen de artefactos | `artifacts/sns/` | Pruebas de pago, tickets de guardian, diffs de resolver, exportaciones KPI y salida firmada de `sns governance ...`. |

Todas las acciones de gobernanza deben referenciar al menos un artefacto en la
 tabla anterior para que los auditores reconstruyan el rastro de decision en 24
 horas.

## 4. Playbooks de ciclo de vida

### 4.1 Mociones de carta y steward

| Paso | Dueno | CLI / Evidencia | Notas |
|------|-------|----------------|-------|
| Redactar addendum y deltas KPI | Rapporteur del consejo + lider de steward | Plantilla Markdown en `docs/source/sns/governance_addenda/YY/` | Incluir IDs de covenant KPI, hooks de telemetria y condiciones de activacion. |
| Presentar propuesta | Presidencia del consejo | `sns governance charter submit --input SN-CH-YYYY-NN.md` (produce `CharterMotionV1`) | La CLI emite manifiesto Norito en `artifacts/sns/governance/<id>/charter_motion.json`. |
| Voto y reconocimiento de guardianes | Consejo + guardianes | `sns governance ballot cast --proposal <id>` y `sns governance guardian-ack --proposal <id>` | Adjuntar minutos con hash y pruebas de quorum. |
| Aceptacion de steward | Programa de stewards | `sns governance steward-ack --proposal <id> --signature <file>` | Requerido antes de cambiar politicas de sufijos; guardar sobre en `artifacts/sns/governance/<id>/steward_ack.json`. |
| Activacion | Ops del registrador | Actualizar `SuffixPolicyV1`, refrescar caches del registrador, publicar nota en `status.md`. | Timestamp de activacion en `sns_governance_activation_total`. |
| Audit log | Cumplimiento | Agregar entrada a `docs/source/sns/regulatory/<jurisdiction>/<cycle>.md` y drill log si hubo tabletop. | Incluir referencias a dashboards de telemetria y diffs de politica. |

### 4.2 Registros, subastas y aprobaciones de precio

1. **Preflight:** El registrador consulta `SuffixPolicyV1` para confirmar nivel de
   precio, terminos disponibles y ventanas de gracia/redencion. Mantener hojas de
   precios sincronizadas con la tabla de niveles 3/4/5/6-9/10+ (nivel base +
   coeficientes de sufijo) documentada en el roadmap.
2. **Subastas sealed-bid:** Para pools premium, ejecutar el ciclo 72 h commit /
   24 h reveal via `sns governance auction commit` / `... reveal`. Publicar la
   lista de commits (solo hashes) en `artifacts/sns/auctions/<name>/commit.json`
   para que los auditores verifiquen la aleatoriedad.
3. **Verificacion de pago:** Los registradores validan `PaymentProofV1` contra el
   reparto de tesoreria (70% tesoreria / 30% steward con carve-out de referido <=10%).
   Guardar el JSON Norito en `artifacts/sns/payments/<tx>.json` y enlazarlo en la
   respuesta del registrador (`RevenueAccrualEventV1`).
4. **Hook de gobernanza:** Adjuntar `GovernanceHookV1` para nombres premium/guarded
   con referencias a ids de propuesta del consejo y firmas de steward. Hooks
   faltantes resultan en `sns_err_governance_missing`.
5. **Activacion + sync de resolver:** Una vez que Torii emite el evento de
   registro, disparar el tailer de transparencia del resolver para confirmar
   que el nuevo estado GAR/zone se propago (ver 4.5).
6. **Divulgacion al cliente:** Actualizar el ledger orientado al cliente
   (wallet/explorer) via los fixtures compartidos en
   [`address-display-guidelines.md`](./address-display-guidelines.md), asegurando
   que los renderizados IH58 y comprimidos coincidan con la guia de copia/QR.

### 4.3 Renovaciones, facturacion y reconciliacion de tesoreria

- **Flujo de renovacion:** Los registradores aplican las ventanas de gracia de
  30 dias + redencion de 60 dias especificadas en `SuffixPolicyV1`. Despues de 60
  dias, la secuencia de reapertura holandesa (7 dias, tarifa 10x decayendo 15%/dia)
  se activa automaticamente via `sns governance reopen`.
- **Reparto de ingresos:** Cada renovacion o transferencia crea un
  `RevenueAccrualEventV1`. Las exportaciones de tesoreria (CSV/Parquet) deben
  reconciliar con estos eventos diariamente; adjuntar pruebas en
  `artifacts/sns/treasury/<date>.json`.
- **Carve-outs de referidos:** Porcentajes de referido opcionales se rastrean por
  sufijo agregando `referral_share` a la politica de steward. Los registradores
  emiten el split final y guardan manifiestos de referido junto a la prueba de pago.
- **Cadencia de reportes:** Finanzas publica anexos KPI mensuales (registros,
  renovaciones, ARPU, uso de disputas/bond) en
  `docs/source/sns/regulatory/<suffix>/YYYY-MM.md`. Los dashboards deben tomar
  de las mismas tablas exportadas para que los numeros de Grafana coincidan con
  la evidencia del ledger.
- **Revision KPI mensual:** El checkpoint del primer martes empareja al lider de
  finanzas, steward de turno y PM del programa. Abrir el [SNS KPI dashboard](./kpi-dashboard.md)
  (embed del portal de `sns-kpis` / `dashboards/grafana/sns_suffix_analytics.json`),
  exportar las tablas de throughput y revenue del registrador, registrar deltas en
  el anexo y adjuntar los artefactos al memo. Disparar un incidente si la revision
  encuentra brechas SLA (ventanas de freeze >72 h, picos de error del registrador,
  drift de ARPU).

### 4.4 Congelamientos, disputas y apelaciones

| Fase | Dueno | Accion y evidencia | SLA |
|-------|-------|-------------------|-----|
| Solicitud de freeze soft | Steward / soporte | Presentar ticket `SNS-DF-<id>` con pruebas de pago, referencia de bond de disputa y selector(es) afectados. | <=4 h desde el ingreso. |
| Ticket de guardian | Junta de guardianes | `sns governance freeze --selector <IH58> --reason <text> --until <ts>` produce `GuardianFreezeTicketV1`. Guardar el JSON del ticket en `artifacts/sns/guardian/<id>.json`. | <=30 min ACK, <=2 h ejecucion. |
| Ratificacion del consejo | Consejo de gobernanza | Aprobar o rechazar congelamientos, documentar decision enlazada al ticket de guardian y digest del bond de disputa. | Proxima sesion del consejo o voto asincrono. |
| Panel de arbitraje | Cumplimiento + steward | Convocar panel de 7 jurados (segun roadmap) con boletas hasheadas via `sns governance dispute ballot`. Adjuntar recibos de voto anonimizados al paquete de incidente. | Veredicto <=7 dias despues del deposito de bond. |
| Apelacion | Guardianes + consejo | Las apelaciones duplican el bond y repiten el proceso de jurados; registrar manifiesto Norito `DisputeAppealV1` y referenciar ticket primario. | <=10 dias. |
| Descongelar y remediar | Registrador + ops de resolver | Ejecutar `sns governance unfreeze --selector <IH58> --ticket <id>`, actualizar estado del registrador y propagar diffs GAR/resolver. | Inmediato despues del veredicto. |

Los canones de emergencia (congelamientos activados por guardianes <=72 h) siguen
el mismo flujo pero requieren revision retroactiva del consejo y una nota de
transparencia en `docs/source/sns/regulatory/`.

### 4.5 Propagacion de resolver y gateway

1. **Hook de evento:** Cada evento de registro emite al stream de eventos del
   resolver (`tools/soradns-resolver` SSE). Ops de resolver se suscriben y
   registran diffs via el tailer de transparencia
   (`scripts/telemetry/run_soradns_transparency_tail.sh`).
2. **Actualizacion de plantilla GAR:** Gateways deben actualizar plantillas GAR
   referenciadas por `canonical_gateway_suffix()` y re-firmar la lista
   `host_pattern`. Guardar diffs en `artifacts/sns/gar/<date>.patch`.
3. **Publicacion de zonefile:** Usar el zonefile skeleton descrito en
   `roadmap.md` (name, ttl, cid, proof) y empujarlo a Torii/SoraFS. Archivar el
   JSON Norito en `artifacts/sns/zonefiles/<name>/<version>.json`.
4. **Chequeo de transparencia:** Ejecutar `promtool test rules dashboards/alerts/tests/soradns_transparency_rules.test.yml`
   para asegurar que las alertas permanecen verdes. Adjuntar la salida de texto
   de Prometheus al reporte de transparencia semanal.
5. **Auditoria de gateway:** Registrar muestras de headers `Sora-*` (politica de
   cache, CSP, digest de GAR) y adjuntarlas al log de gobernanza para que
   operadores puedan probar que el gateway sirvio el nuevo nombre con los
   guardrails previstos.

## 5. Telemetria y reportes

| Senal | Fuente | Descripcion / Accion |
|--------|--------|----------------------|
| `sns_registrar_status_total{result,suffix}` | Torii registrar handlers | Contador de exito/error para registros, renovaciones, congelamientos, transferencias; alerta cuando `result="error"` sube por sufijo. |
| `torii_request_duration_seconds{route="/v1/sns/*"}` | Metricas de Torii | SLOs de latencia para handlers API; alimenta dashboards basados en `torii_norito_rpc_observability.json`. |
| `soradns_bundle_proof_age_seconds` y `soradns_bundle_cid_drift_total` | Tailer de transparencia del resolver | Detecta pruebas obsoletas o drift de GAR; guardrails definidos en `dashboards/alerts/soradns_transparency_rules.yml`. |
| `sns_governance_activation_total` | Governance CLI | Contador incrementado cuando un charter/addendum se activa; se usa para reconciliar decisiones del consejo vs addenda publicadas. |
| `guardian_freeze_active` gauge | Guardian CLI | Rastrea ventanas de freeze soft/hard por selector; paginar a SRE si el valor queda en `1` mas alla del SLA declarado. |
| KPI annex dashboards | Finanzas / Docs | Rollups mensuales publicados junto a memos regulatorios; el portal los embebe via [SNS KPI dashboard](./kpi-dashboard.md) para que stewards y reguladores accedan a la misma vista de Grafana. |

## 6. Requisitos de evidencia y auditoria

| Accion | Evidencia a archivar | Almacen |
|--------|----------------------|---------|
| Cambio de carta / politica | Manifiesto Norito firmado, transcript CLI, diff KPI, acuse de steward. | `artifacts/sns/governance/<proposal-id>/` + `docs/source/sns/governance_addenda/`. |
| Registro / renovacion | Payload `RegisterNameRequestV1`, `RevenueAccrualEventV1`, prueba de pago. | `artifacts/sns/payments/<tx>.json`, logs de API del registrador. |
| Subasta | Manifiestos commit/reveal, semilla de aleatoriedad, spreadsheet de calculo de ganador. | `artifacts/sns/auctions/<name>/`. |
| Congelar / descongelar | Ticket de guardian, hash de voto del consejo, URL de log de incidente, plantilla de comunicacion al cliente. | `artifacts/sns/guardian/<ticket>/`, `incident/<date>-sns-*.md`. |
| Propagacion de resolver | Zonefile/GAR diff, extracto JSONL del tailer, snapshot Prometheus. | `artifacts/sns/resolver/<date>/` + reportes de transparencia. |
| Ingreso regulatorio | Memo de ingreso, tracker de deadlines, acuse de steward, resumen de cambios KPI. | `docs/source/sns/regulatory/<jurisdiction>/<cycle>.md`. |

## 7. Checklist de gates de fase

| Fase | Criterios de salida | Bundle de evidencia |
|-------|--------------------|--------------------|
| N0 - Beta cerrada | Esquema de registro SN-1/SN-2, CLI de registrador manual, drill de guardian completado. | Motion de carta + ACK de steward, logs de dry-run del registrador, reporte de transparencia del resolver, entrada en `ops/drill-log.md`. |
| N1 - Lanzamiento publico | Subastas + tiers de precio fijo activos para `.sora`/`.nexus`, registrador self-service, auto-sync de resolver, dashboards de facturacion. | Diff de hoja de precios, resultados CI del registrador, anexo de pagos/KPI, salida de tailer de transparencia, notas de ensayo de incidente. |
| N2 - Expansion | `.dao`, APIs de reseller, portal de disputas, scorecards de steward, dashboards de analitica. | Capturas del portal, metricas SLA de disputas, exportes de scorecards de steward, charter de gobernanza actualizado con politicas de reseller. |

Las salidas de fase requieren drills tabletop registrados (registro happy path,
freeze, outage de resolver) con artefactos adjuntos en `ops/drill-log.md`.

## 8. Respuesta a incidentes y escalamiento

| Trigger | Severidad | Dueno inmediato | Acciones obligatorias |
|---------|----------|----------------|-----------------------|
| Drift de resolver/GAR o pruebas obsoletas | Sev 1 | Resolver SRE + junta de guardianes | Paginar on-call del resolver, capturar salida del tailer, decidir si congelar nombres afectados, publicar estado cada 30 min. |
| Caida de registrador, fallo de facturacion o errores API generalizados | Sev 1 | Duty manager del registrador | Detener nuevas subastas, cambiar a CLI manual, notificar stewards/tesoreria, adjuntar logs de Torii al doc de incidente. |
| Disputa de nombre unico, mismatch de pago o escalamiento de cliente | Sev 2 | Steward + lider de soporte | Recopilar pruebas de pago, decidir si hace falta freeze soft, responder al solicitante dentro del SLA, registrar resultado en tracker de disputa. |
| Hallazgo de auditoria de cumplimiento | Sev 2 | Enlace de cumplimiento | Redactar plan de remediacion, archivar memo en `docs/source/sns/regulatory/`, agendar sesion de consejo de seguimiento. |
| Drill o ensayo | Sev 3 | PM del programa | Ejecutar escenario guionado desde `ops/drill-log.md`, archivar artefactos, marcar gaps como tareas del roadmap. |

Todos los incidentes deben crear `incident/YYYY-MM-DD-sns-<slug>.md` con tablas
de ownership, logs de comandos y referencias a la evidencia producida en este
playbook.

## 9. Referencias

- [`registry-schema.md`](./registry-schema.md)
- [`registrar-api.md`](./registrar-api.md)
- [`address-display-guidelines.md`](./address-display-guidelines.md)
- [`docs/account_structure.md`](../../../account_structure.md)
- [`docs/source/soradns/deterministic_hosts.md`](../../../source/soradns/deterministic_hosts.md)
- [`docs/source/reports/soradns_transparency.md`](../../../source/reports/soradns_transparency.md)
- `ops/drill-log.md`
- `roadmap.md` (SNS, DG, ADDR sections)

Mantener este playbook actualizado cuando cambien el texto de cartas, las
superficies de CLI o los contratos de telemetria; las entradas del roadmap que
referencian `docs/source/sns/governance_playbook.md` deben coincidir siempre con
la revision mas reciente.
