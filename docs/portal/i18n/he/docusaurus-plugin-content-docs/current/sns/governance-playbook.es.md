---
lang: he
direction: rtl
source: docs/portal/docs/sns/governance-playbook.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::שימו לב פואנטה קנוניקה
Esta pagina refleja `docs/source/sns/governance_playbook.md` y ahora sirve como
la copia canonica del portal. El archivo fuente persiste para PRs de traducion.
:::

# Playbook de gobernanza del Servicio de Nombres Sora (SN-6)

**Estado:** Borrador 2026-03-24 - referencia viva para la preparacion SN-1/SN-6  
**מצופות מפת הדרכים:** SN-6 "תאימות ופתרון מחלוקות", SN-7 "Resolver & Gateway Sync", מדיניות הנחיות ADDR-1/ADDR-5  
**דרישות מוקדמות:** Esquema de registro en [`registry-schema.md`](./registry-schema.md), contrato de API de registrador en [`registrar-api.md`](./registrar-api.md), guia UX de directions en [`address-display-guidelines.md`](./address-display-guidelines.md), y reglas de estructura de cuentas en [`docs/account_structure.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/account_structure.md).

ספר המשחקים של אסטה מתאר como los cuerpos de gobernanza del Sora Name Service (SNS)
adoptan cartas, Aprueban registros, escalan disputas, y prueban que los estados de
resolver y gateway permanecen sincronizados. Cumple el requisito del roadmap de
que la CLI `sns governance ...`, los manifiestos Norito y los artefactos de
auditoria compartan una unica referencia operativa antes de N1 (lanzamiento
פובליקו).

## 1. Alcance y audiencia

תעודת זהות:

- Miembros del Consejo de Gobernanza que votan cartas, politicas de sufijos y
  resultados de disputas.
- Miembros de la Junta de Guardianes que emiten congelamientos de emergencia y
  reversiones revisan.
- דיילים דה סופיגוס que operan colas de registrador, Aprueban Subastas y
  gestionan repartos de ingresos.
- מפעילי הפתרון/השער אחראים להפצת SoraDNS,
  actualizaciones GAR y מעקות בטיחות de telemetria.
- Equipos de cumplimiento, tesoreria y soporte que deben demostrar que toda
  accion de gobernanza dejo artefactos Norito auditables.

Cubre las fases de beta cerrada (N0), lanzamiento publico (N1) והרחבה (N2)
listadas en `roadmap.md`, vinculando cada flujo de trabajo con la evidencia,
 לוחות מחוונים y rutas de escalamiento requeridas.

## 2. תפקידים ומפות אנשי קשר| רול | Responsabilidades principales | Artefactos y telemetria principales | Escalacion |
|------|---------------------------------|----------------------------------------|------------|
| Consejo de Gobernanza | Redacta y ratifica cartas, politicas de sufijos, veredictos de disputa y rotaciones de stewards. | `docs/source/sns/governance_addenda/`, `artifacts/sns/governance/*`, votos del consejo almacenados via `sns governance charter submit`. | Presidencia del consejo + tracker de agenda de gobernanza. |
| Junta de Guardianes | Emite congelamientos רך/קשה, קנונים חירום ושינויים של 72 שעות. | כרטיסים של אפוטרופוס פור `sns governance freeze`, מניפיסטוס של ביטול ב-`artifacts/sns/guardian/*`. | Guardia כוננות (<=15 דקות ACK). |
| דיילים דה סופיג'ו | אופר קולס דל רישום, תחתונים, נקודות מחיר ותקשורת עם לקוחות; reconocen cumplimientos. | Politicas de steward en `SuffixPolicyV1`, hojas de referencia de precios, הטענות של הדייל לתזכירי תקנות. | Lider del programa de stewards + PagerDuty por sufijo. |
| Operaciones de registrador y facturacion | נקודות קצה אופנתיות `/v2/sns/*`, פקודות מתפייסות, פולטות טלמטריה וצילומי מצב של CLI. | API del registrador ([`registrar-api.md`](./registrar-api.md)), מדדים `sns_registrar_status_total`, פרועbas de pago en `artifacts/sns/payments/*`. | מנהל תורנות של רישום y enlace de tesoreria. |
| Operadores de resolver y gateway | Mantienen SoraDNS, GAR y estado del gateway alineados con eventos del registrador; transmiten metricas de transparencia. | [`docs/source/soradns/deterministic_hosts.md`](../../../source/soradns/deterministic_hosts.md), [`docs/source/reports/soradns_transparency.md`](../../../source/reports/soradns_transparency.md), `dashboards/alerts/soradns_transparency_rules.yml`. | SRE del resolver on-call + puente ops de gateway. |
| Tesoreria y Finanzas | Aplica reparto 70/30, carve-outs de referidos, declaraciones fiscales/tesoreria y atestaciones SLA. | Manifiestos de acumulacion de ingresos, exportaciones Stripe/tesoreria, apendices KPI trimestrales en `docs/source/sns/regulatory/`. | Controller de finanzas + oficial de cumplimiento. |
| Enlace de Cumplimiento y Regulacion | Rastrea obligaciones globales (EU DSA, וכו'), אמנות מציאות KPI y presenta divulgaciones. | תזכירים רגולטוריים ב-`docs/source/sns/regulatory/`, חפיסות רפרנס, מקורות `ops/drill-log.md` לשולחן העבודה. | Lider de programa de cumplimiento. |
| Soporte / SRE כוננות | תקריות מנג'ה (התנגשויות, סחף דה facturacion, פתרונות הגנה), קואורדינאות נסיעות ולקוחות ומשחקי ריצה. | Plantillas de incidente, `ops/drill-log.md`, evidencia de laboratorio, transcripciones Slack/war-room en `incident/`. | Rotacion כוננות SNS + התנועה SRE. |

## 3. Artefactos canonicos y fuentes de datos| Artefacto | Ubicacion | פרופוזיטו |
|--------|--------|--------|
| Carta + anexos KPI | `docs/source/sns/governance_addenda/` | Cartas firmadas con control de version, covenants KPI y decisiones de gobernanza referenciadas por votos de CLI. |
| Esquema de registro | [`registry-schema.md`](./registry-schema.md) | Estructuras Norito canonicas (`NameRecordV1`, `SuffixPolicyV1`, `RevenueAccrualEventV1`). |
| Contrato del registrador | [`registrar-api.md`](./registrar-api.md) | מטענים REST/gRPC, מדדים `sns_registrar_status_total` והוק ציפיות לממשל. |
| Guia UX de direcciones | [`address-display-guidelines.md`](./address-display-guidelines.md) | Renderizados canonicos I105 (preferido) y comprimidos (segunda mejor opcion) reflejados por ארנקים/חוקרים. |
| Docs SoraDNS / GAR | [`docs/source/soradns/deterministic_hosts.md`](../../../source/soradns/deterministic_hosts.md), [`docs/source/reports/soradns_transparency.md`](../../../source/reports/soradns_transparency.md) | Derivacion deterministica de hosts, flujo detailer de transparencia y regglas de alertas. |
| תזכירים רגולטוריים | `docs/source/sns/regulatory/` | Notas de ingreso jurisdiccional (עמ' ej., EU DSA), ecuses de steward, anexos plantilla. |
| יומן מקדחה | `ops/drill-log.md` | Registro de ensayos de caos e IR requeridos antes de salir de phase. |
| Almacen de artefactos | `artifacts/sns/` | Pruebas de pago, כרטיסים דה guardian, diffs de resolver, exportaciones KPI y salida firmada de `sns governance ...`. |

Todas las acciones de gobernanza deben referenciar al menos un artefacto en la
 tabla anterior para que los auditores reconstruyan el rastro de decision en 24
 הוראס.

## 4. Playbooks de ciclo de vida

### 4.1 Mociones de carta y דייל

| פאסו | דואנו | CLI / Evidencia | Notas |
|------|-------|----------------|-------|
| Redactar תוספת y deltas KPI | Rapporteur del consejo + lider de steward | Plantilla Markdown en `docs/source/sns/governance_addenda/YY/` | כולל מזהים של אמנה KPI, ווים לטלמטריה ותנאים להפעלה. |
| Presentar propuesta | Presidencia del consejo | `sns governance charter submit --input SN-CH-YYYY-NN.md` (לייצר `CharterMotionV1`) | La CLI emite manifiesto Norito en `artifacts/sns/governance/<id>/charter_motion.json`. |
| Voto y reconocimiento de guardianes | Consejo + אפוטרופוסים | `sns governance ballot cast --proposal <id>` y `sns governance guardian-ack --proposal <id>` | Adjuntar minutos con hash y pruebas de quorum. |
| Aceptacion de steward | Programa de stewards | `sns governance steward-ack --proposal <id> --signature <file>` | Requerido antes de cambiar politicas de sufijos; guardar sobre en `artifacts/sns/governance/<id>/steward_ack.json`. |
| Activacion | אופציות לרשום | אקטואליזר `SuffixPolicyV1`, מטמונים מחודשים של רישום, פרסום הודעות ב-`status.md`. | חותמת זמן הפעלה ב-`sns_governance_activation_total`. |
| יומן ביקורת | Cumplimiento | Agregar entrada a `docs/source/sns/regulatory/<jurisdiction>/<cycle>.md` y logging log si hubo שולחן. | כולל הפניות ללוחות מחוונים של טלמטריה והבדלים בפוליטיקה. |

### 4.2 רישומים, חלקים ואפרובאציונים1. **טיסה מוקדמת:** אל רשם החשבון `SuffixPolicyV1` עבור confirmar nivel de
   precio, terminos disponibles y ventanas de gracia/redencion. Mantener hojas de
   precios sincronizadas con la tabla de niveles 3/4/5/6-9/10+ (בסיס נייבל +
   coeficientes de sufijo) documentada en el מפת הדרכים.
2. **Subastas sealed-bid:** Para pools premium, ejecutar el ciclo 72 h commit /
   חשיפה של 24 שעות דרך `sns governance auction commit` / `... reveal`. Publicar la
   list de commits (hashes סולו) en `artifacts/sns/auctions/<name>/commit.json`
   para que los auditores verifiquen la aleatoriedad.
3. ** Verificacion de pago:** Los registradores validan `PaymentProofV1` contra el
   reparto de tesoreria (70% tesoreria / 30% דייל בהסתייגות <=10%).
   Guardar el JSON Norito en `artifacts/sns/payments/<tx>.json` y enlazarlo en la
   משוב הרשום (`RevenueAccrualEventV1`).
4. **Hook de gobernanza:** Adjuntar `GovernanceHookV1` para nombres premium/guarded
   con referencias a ids de propuesta del consejo y firmas de steward. ווים
   faltantes resultan en `sns_err_governance_missing`.
5. **הפעלה + פותר סנכרון:** זה לא משנה מה Torii emite el evento de
   registro, disparar el tailer de transparencia del resolver para confirmar
   que el nuevo estado GAR/zone se propago (גרסה 4.5).
6. **גילוי לקוחות:** עדכון לקוחות חשבונות
   (ארנק/חוקר) דרך los fixtures compartidos en
   [`address-display-guidelines.md`](./address-display-guidelines.md), asegurando
   que los renderizados I105 y comprimidos coincidan con la guia de copia/QR.

### 4.3 Renovaciones, facturacion y reconciliacion de tesoreria- **Flujo de renovacion:** Los registradores aplican las ventanas de gracia de
  30 dias + redencion de 60 dias especificadas en `SuffixPolicyV1`. Despues de 60
  dias, la secuencia de reapertura holandesa (7 דיאס, מחיר 10x decayendo 15%/dias)
  הפעל אוטומטית דרך `sns governance reopen`.
- **Reparto de ingresos:** Cada renovacion o transferencia crea un
  `RevenueAccrualEventV1`. Las exportaciones de tesoreria (CSV/Parquet) deben
  Reconciliar con estos eventos diariamente; adjuntar pruebas en
  `artifacts/sns/treasury/<date>.json`.
- **Carve-outs de referidos:** Porcentajes de referido opcionales se rastrean por
  sufijo agregando `referral_share` a la politica de steward. לוס רשומים
  emiten el split final y guardan manifiestos de referido junto a la prueba de pago.
- **Cadencia de reportes:** Finanzas publica anexos KPI mensuales (registros,
  renovaciones, ARPU, uso de disputas/bond) en
  `docs/source/sns/regulatory/<suffix>/YYYY-MM.md`. לוס לוחות מחוונים deben tomar
  de las mismas tablas exportadas para que los numeros de Grafana coincidan con
  להוכחות דל ספר.
- **עדכון KPI למחזור:** El checkpoint del primer martes empareja al lider de
  finanzas, steward de turno y PM del programa. Abrir el [לוח מחוונים SNS KPI](./kpi-dashboard.md)
  (הטמע את הפורטל של `sns-kpis` / `dashboards/grafana/sns_suffix_analytics.json`),
  Exportar Las Tablas de תפוקה y Revenue del Registrador, הרשם deltas en
  el anexo y adjuntar los artefactos al memo. Disparar un incidente si la revisie
  encuentra brechas SLA (ventanas de freeze >72 h, picos de error del registrador,
  drift de ARPU).

### 4.4 Congelamientos, disputas y apelaciones

| פאזה | דואנו | Accion y evidencia | SLA |
|-------|-------|------------------------|-----|
| Solicitud de freeze soft | דייל / soporte | Presentar כרטיס `SNS-DF-<id>` con pruebas de pago, referencia de bond de disputa y selector(es) afectados. | <=4 שעות לאחר מכן. |
| Ticket de Guardian | Junta de Guardianes | `sns governance freeze --selector <I105> --reason <text> --until <ts>` לייצר `GuardianFreezeTicketV1`. Guardar el JSON del ticket en `artifacts/sns/guardian/<id>.json`. | <=30 דקות ACK, <=2 שעות פליטה. |
| Ratificacion del Consejo | Consejo de gobernanza | Aprobar o rechazar congelamientos, החלטה תיעודית enlazada al ticket de guardian y digest del bond de disputa. | Proxima sesion del consejo o voto asincrono. |
| Panel de arbitraje | Cumplimiento + דייל | Convocar Panel de 7 Jurados (מפת דרכים) עם נקודות התקדמות דרך `sns governance dispute ballot`. Adjuntar recibos de voto anonimizados al paquete de incidente. | Veredicto <=7 dias despues del deposito de bond. |
| Apelacion | Guardianes + consejo | Las apelaciones duplican el bond y repiten el processo de jurados; רשם מניפיסטו Norito `DisputeAppealV1` y referenceciar ticket primario. | <=10 dias. |
| Descongelar y remediar | Registrador + ops de resolver | Ejecutar `sns governance unfreeze --selector <I105> --ticket <id>`, בפועל estado del registrador y propagar diffs GAR/resolver. | Inmediato despues del veredicto. |Los canones de emergencia (congelamientos activados por guardianes <=72 h) סיואן
el mismo flujo pero requieren revisie retroactiva del consejo y una not de
transparencia en `docs/source/sns/regulatory/`.

### 4.5 תפוצה של פתרון השער

1. **Hook de evento:** Cada evento de registro emite al stream de eventos del
   פותר (`tools/soradns-resolver` SSE). Ops de resolver se subscriben y
   הרשם משתנה דרך el tailer de transparencia
   (`scripts/telemetry/run_soradns_transparency_tail.sh`).
2. **Actualizacion de plantilla GAR:** Gateways deben actualizar plantillas GAR
   התייחסות ל-`canonical_gateway_suffix()` ו-re-firmar la list
   `host_pattern`. Guardar diffs en `artifacts/sns/gar/<date>.patch`.
3. **Publicacion de zonefile:** Usar el zonefile skeleton decrito en
   `roadmap.md` (שם, ttl, cid, הוכחה) y empujarlo a Torii/SoraFS. ארכיון אל
   JSON Norito en `artifacts/sns/zonefiles/<name>/<version>.json`.
4. **Chequeo de transparencia:** Ejecutar `promtool test rules dashboards/alerts/tests/soradns_transparency_rules.test.yml`
   para asegurar que las alertas permanecen verdes. Adjuntar la salida de texto
   de Prometheus al reporte de transparencia semanal.
5. **Auditoria de gateway:** הרשם מוestras de headers `Sora-*` (politica de
   cache, CSP, digest de GAR) y adjuntarlas al log de gobernanza para que
   המפעילים Puedan Probar que el gateway sirvio el nuevo nombre con los
   מעקות בטיחות previstos.

## 5. מדווחת Telemetria y

| סנאל | פואנטה | Descripcion / Accion |
|--------|--------|---------------------|
| `sns_registrar_status_total{result,suffix}` | מטפלי רשם Torii | Contador de exito/error para registros, renovaciones, congelamientos, transferencias; alerta cuando `result="error"` sube por sufijo. |
| `torii_request_duration_seconds{route="/v2/sns/*"}` | Metricas de Torii | SLOs de latencia para handlers API; לוחות מחוונים של alimenta basados ​​en `torii_norito_rpc_observability.json`. |
| `soradns_bundle_proof_age_seconds` y `soradns_bundle_cid_drift_total` | Tailer de transparencia del resolver | Detecta pruebas obsoletas o drift de GAR; מעקות בטיחות definidos en `dashboards/alerts/soradns_transparency_rules.yml`. |
| `sns_governance_activation_total` | ממשל CLI | Contador incrementado cuando un charter/addendum se active; se usa para reconciliar decisiones del consejo vs addenda publicadas. |
| מד `guardian_freeze_active` | גרדיאן CLI | Rastrea ventanas de freeze רך/קשה פור בורר; pager a SRE si el valor queda en `1` mas alla del SLA declarado. |
| לוחות מחוונים נספח KPI | Finanzas / Docs | Rollups mensuales publicados junto מזכרים regulatorios; el portal los embebe באמצעות [לוח המחוונים של SNS KPI](./kpi-dashboard.md) עבור דיילים ומנהלי תקנות אקדמאיים ב-Grafana. |

## 6. Requisitos de evidencia y auditoria| אקציון | Evidencia a archivar | אלמסן |
|--------|-----------------------------|--------|
| Cambio de carta / politica | Manifiesto Norito פיראדו, תמלול CLI, KPI שונה, אקוזה דייל. | `artifacts/sns/governance/<proposal-id>/` + `docs/source/sns/governance_addenda/`. |
| רישום / שיפוץ | מטען `RegisterNameRequestV1`, `RevenueAccrualEventV1`, prueba de pago. | `artifacts/sns/payments/<tx>.json`, יומני ממשק API של הרשום. |
| סובסטה | מתחייב/חושף, סמילה דה aleatoriedad, גיליון אלקטרוני של calculo de ganador. | `artifacts/sns/auctions/<name>/`. |
| Congelar / descongelar | Ticket de Guardian, hash de voto del consejo, URL de log de incidente, plantilla de comunicacion al cliente. | `artifacts/sns/guardian/<ticket>/`, `incident/<date>-sns-*.md`. |
| Propagacion de resolver | הפרש Zonefile/GAR, חילוץ JSONL של tailer, תמונת מצב Prometheus. | `artifacts/sns/resolver/<date>/` + דוחות שקיפות. |
| Ingreso regulatorio | תזכיר אינגרסו, עוקב אחר מועדים, משפט דייל, קורות חיים של KPI. | `docs/source/sns/regulatory/<jurisdiction>/<cycle>.md`. |

## 7. רשימת השערים לשלב

| פאזה | קריטריונים דה סלידה | Bundle de evidencia |
|-------|------------------------|------------------------|
| N0 - Beta cerrada | אספקת רישום SN-1/SN-2, מדריך לרשום CLI, תרגיל שומר מלא. | Motion de carta + ACK de steward, יומני ריצה יבשה של רישום, דיווח על שקיפות של פתרון, כתובה ב-`ops/drill-log.md`. |
| N1 - Lanzamiento publico | Subastas + tiers de precio fijo activos para `.sora`/`.nexus`, רשם שירות עצמי, סנכרון אוטומטי של פותר, לוחות מחוונים בפועל. | Diff de hoja de precios, תוצאות CI del registrador, anexo de pagos/KPI, salida de tailer de transparencia, notas de ensayo de incidente. |
| N2 - הרחבה | `.dao`, ממשקי API למפיץ, פורטל מחלוקת, כרטיסי ניקוד של דייל, לוחות מחוונים של אנליטיקה. | נתוני הפורטל, מדדי SLA למחלוקות, ייצוא כרטיסי ניקוד של דייל, אמנת הלכה למעשה עם מפיץ פוליטי. |

Las salidas de fase requieren מקדחים שולחניים registrados (registro happy path,
להקפיא, הפסקת פתרון) עם אמנות עזר ב-`ops/drill-log.md`.

## 8. תשובה לאירועים ואירועים| טריגר | Severidad | Dueno inmediato | Acciones obligatorias |
|--------|--------|----------------|------------------------|
| Drift de resolver/GAR o pruebas obsoletas | סב 1 | Resolver SRE + junta de guardianes | עמוד טלפוני ב-call של פותר, צילום טלפון, קביעת נקודות פרסום, פרסום שבועי 30 דקות. |
| מדריך הרשמה, ניסיון בהכללה של ממשק API | סב 1 | מנהל חובה דל רישום | מעכבים נובעים, מדריך ל-CLI, הודעות דיילים/טסוריה, יומני עזר של Torii אל מסמך תקרית. |
| Disputa de nombre unico, חוסר התאמה של פאגו או escalamiento de cliente | סב' 2 | דייל + lider de soporte | שחזור פריטים דה פאגו, הכרחי אם יש להקפיא רך, מגיב על בקשת שירות SLA, רשם תוצאות מעקב אחר מחלוקת. |
| Hallazgo de auditoria de cumplimiento | סב' 2 | Enlace de cumplimiento | Redactar Plan de Remediacion, תזכיר ארכיון ב-`docs/source/sns/regulatory/`, סדר היום של הסכם הסגירה. |
| Drill o ensayo | סוו 3 | PM del programa | Ejecutar escenario guionado desde `ops/drill-log.md`, חפצי ארכיון, פערי מרקאר como tareas del roadmap. |

Todos los incidentes deben crear `incident/YYYY-MM-DD-sns-<slug>.md` con tablas
דה בעלות, יומני דה קומנדוס y referencias a la evidencia producida en este
ספר משחק.

## 9. התייחסויות

- [`registry-schema.md`](./registry-schema.md)
- [`registrar-api.md`](./registrar-api.md)
- [`address-display-guidelines.md`](./address-display-guidelines.md)
- [`docs/account_structure.md`](../../../account_structure.md)
- [`docs/source/soradns/deterministic_hosts.md`](../../../source/soradns/deterministic_hosts.md)
- [`docs/source/reports/soradns_transparency.md`](../../../source/reports/soradns_transparency.md)
- `ops/drill-log.md`
- `roadmap.md` (קטעי SNS, DG, ADDR)

Mantener este playbook actualizado cuando cambien el texto de cartas, לאס
superficies de CLI o los contratos de telemetria; las entradas del roadmap que
referencian `docs/source/sns/governance_playbook.md` deben coincidir siempre con
la revision mas reciente.