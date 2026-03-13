---
lang: es
direction: ltr
source: docs/portal/docs/sns/governance-playbook.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::nota مستند ماخذ
یہ صفحہ `docs/source/sns/governance_playbook.md` کی عکاسی کرتا ہے اور اب پورٹل
کی کینونیکل کاپی ہے۔ سورس فائل ترجمہ PRs کے لئے برقرار رہتی ہے۔
:::

# Servicio de nombres Sora گورننس پلی بک (SN-6)

**حالت:** 2026-03-24 مسودہ - SN-1/SN-6 تیاری کے لئے زندہ حوالہ  
**روڈمیپ لنکس:** SN-6 "Cumplimiento y resolución de disputas", SN-7 "Resolver y sincronización de puerta de enlace", ADDR-1/ADDR-5 ایڈریس پالیسی  
**پیشگی شرائط:** رجسٹری اسکیمہ [`registry-schema.md`](./registry-schema.md) میں، رجسٹرار API معاہدہ [`registrar-api.md`](./registrar-api.md) میں، ایڈریس UX رہنمائی [`address-display-guidelines.md`](./address-display-guidelines.md) میں، اور اکاؤنٹ اسٹرکچر قواعد [`docs/account_structure.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/account_structure.md) میں۔

یہ پلی بک بتاتی ہے کہ Sora Name Service (SNS) کی گورننس باڈیز کیسے چارٹر اپناتی
ہیں، رجسٹریشن منظور کرتی ہیں، تنازعات کو escalar کرتی ہیں، اور ثابت کرتی ہیں کہ
resolver اور gateway کی حالتیں ہم آہنگ رہتی ہیں۔ یہ روڈمیپ کی اس ضرورت کو پورا
کرتی ہے کہ `sns governance ...` CLI, Norito manifiesta اور artefactos de auditoría N1
(عوامی لانچ) سے پہلے ایک ہی آپریٹر ریفرنس شیئر کریں۔

## 1. دائرہ کار اور سامعین

یہ دستاویز ان کے لئے ہے:- Consejo de Gobernanza کے اراکین جو چارٹرز، sufijo پالیسیوں اور تنازعہ نتائج پر ووٹ دیتے ہیں۔
- Tablero guardián کے اراکین جو ہنگامی se congela جاری کرتے ہیں اور reversiones کا جائزہ لیتے ہیں۔
- Administradores de sufijos جو registrador کی قطاریں چلاتے ہیں، subastas منظور کرتے ہیں، اور divisiones de ingresos سنبھالتے ہیں۔
- Resolver/gateway آپریٹرز جو SoraDNS پھیلاؤ، GAR اپڈیٹس اور telemetry guardrails کے ذمہ دار ہیں۔
- Cumplimiento, tesorería اور apoyo ٹیمیں جنہیں دکھانا ہوتا ہے کہ ہر گورننس کارروائی نے auditoría کے قابل Norito artefactos چھوڑے۔

یہ `roadmap.md` میں درج بند-بیٹا (N0), عوامی لانچ (N1) اور توسیع (N2) مراحل کو
کور کرتی ہے، ہر ورک فلو کو درکار شواہد، paneles de control اور escalada راستوں سے
جوڑتی ہے۔

## 2. کردار اور رابطہ نقشہ| کردار | بنیادی ذمہ داریاں | Objetos y telemetría | Escalada |
|------|--------------------|-------------------------------|-----------|
| Consejo de Gobierno | چارٹرز، sufijo پالیسیوں، تنازعہ فیصلوں اور rotaciones de mayordomo کی تدوین و توثیق۔ | `docs/source/sns/governance_addenda/`, `artifacts/sns/governance/*`, boletas del consejo جو `sns governance charter submit` سے محفوظ ہوتے ہیں۔ | Presidente del consejo + rastreador de expedientes de gobernanza۔ |
| Junta de Guardianes | Congelaciones suaves/duras, ہنگامی cánones, اور 72 h reviews جاری کرتا ہے۔ | Tickets de Guardian, `sns governance freeze`, anulan manifiestos, `artifacts/sns/guardian/*`, میں لاگ ہوتے ہیں۔ | Rotación de guardia de guardia (<=15 min ACK)۔ |
| Mayordomos de sufijo | colas de registradores, subastas, niveles de precios y comunicaciones con clientes چلاتے ہیں؛ reconocimientos de cumplimiento دیتے ہیں۔ | Políticas del administrador `SuffixPolicyV1` میں، hojas de referencia de precios, اور reconocimientos del administrador جو memorandos regulatorios کے ساتھ محفوظ ہوتے ہیں۔ | Líder del programa Steward + sufijo مخصوص PagerDuty۔ |
| Operaciones de registro y facturación | Puntos finales `/v2/sns/*` Conciliación de pagos Telemetría Emisión de instantáneas CLI | API de registrador ([`registrar-api.md`](./registrar-api.md)), métricas `sns_registrar_status_total`, pruebas de pago y `artifacts/sns/payments/*` میں محفوظ ہیں۔ | Gerente de servicio de registro اور enlace de tesorería۔ || Operadores de resolución y puerta de enlace | SoraDNS, GAR اور estado de puerta de enlace کو eventos de registrador کے ساتھ alineados رکھتے ہیں؛ flujo de métricas de transparencia کرتے ہیں۔ | [`docs/source/soradns/deterministic_hosts.md`](../../../source/soradns/deterministic_hosts.md), [`docs/source/reports/soradns_transparency.md`](../../../source/reports/soradns_transparency.md), `dashboards/alerts/soradns_transparency_rules.yml`. | Resolver SRE de guardia + puente de operaciones de puerta de enlace۔ |
| Tesorería y Finanzas | División de ingresos 70/30, exclusiones de referencias, declaraciones de impuestos/tesorería y certificaciones de SLA | Manifiestos de acumulación de ingresos, Exportaciones de tesorería/Stripe, اور سہ ماہی Apéndices de KPI `docs/source/sns/regulatory/` میں۔ | Controlador financiero + oficial de cumplimiento۔ |
| Enlace regulatorio y de cumplimiento | عالمی ذمہ داریوں (EU DSA وغیرہ) کو ٹریک کرتا ہے، KPI covenants اپڈیٹ کرتا ہے، اور divulgaciones فائل کرتا ہے۔ | Memos reglamentarios `docs/source/sns/regulatory/` میں، mazos de referencia، اور ensayos de mesa کے لئے `ops/drill-log.md` entradas۔ | Líder del programa de cumplimiento۔ |
| Soporte / SRE de guardia | incidentes (colisiones, desviaciones en la facturación, interrupciones en la resolución) | Plantillas de incidentes, `ops/drill-log.md`, evidencia de laboratorio preparada, Transcripciones de Slack/war-room y `incident/` میں محفوظ ہیں۔ | Rotación de guardias SNS + gestión SRE۔ |

## 3. Artefactos کینونیکل اور ڈیٹا ذرائع| Artefacto | مقام | مقصد |
|----------|------|------|
| Carta + Anexos de KPI | `docs/source/sns/governance_addenda/` | ورژن کنٹرول شدہ دستخط شدہ چارٹرز، KPI covenants, اور گورننس فیصلے جو CLI votes سے ریفرنس ہوتے ہیں۔ |
| Esquema de registro | [`registry-schema.md`](./registry-schema.md) | Tarjeta gráfica Norito (`NameRecordV1`, `SuffixPolicyV1`, `RevenueAccrualEventV1`) ۔ |
| Contrato de registrador | [`registrar-api.md`](./registrar-api.md) | Cargas útiles REST/gRPC, métricas `sns_registrar_status_total`, gancho de gobernanza |
| Guía de dirección UX | [`address-display-guidelines.md`](./address-display-guidelines.md) | Representaciones de کینونیکل I105 (ترجیحی) / comprimido (`sora`) (`sora`, segundo mejor) جو billeteras/exploradores میں جھلکتے ہیں۔ |
| Documentos SoraDNS/GAR | [`docs/source/soradns/deterministic_hosts.md`](../../../source/soradns/deterministic_hosts.md), [`docs/source/reports/soradns_transparency.md`](../../../source/reports/soradns_transparency.md) | Derivación de host determinista, flujo de trabajo de cola de transparencia, reglas de alerta |
| Memorandos reglamentarios | `docs/source/sns/regulatory/` | Notas de admisión jurisdiccionales (مثلا EU DSA), reconocimientos del administrador, anexos de plantilla ۔ |
| Registro de perforación | `ops/drill-log.md` | Caos اور ensayos de IR کا ریکارڈ جو salidas de fase سے پہلے لازم ہیں۔ |
| Almacenamiento de artefactos | `artifacts/sns/` | Pruebas de pago, tickets de guardián, diferencias de resolución, exportaciones de KPI, `sns governance ...` y salida CLI firmada |

تمام گورننس اقدامات کو اوپر کی جدول میں سے کم از کم ایک artefacto کا حوالہ دینا
چاہیے تاکہ auditores 24 گھنٹوں میں sendero de decisión دوبارہ بنا سکیں۔

## 4. لائف سائیکل پلی بکس### 4.1 Estatuto y mociones del delegado

| مرحلہ | مالک | CLI / Evidencia | نوٹس |
|-------|------|----------------|------|
| Borrador de adenda sobre deltas de KPI | Relator del Consejo + líder delegado | Plantilla de rebajas `docs/source/sns/governance_addenda/YY/` میں محفوظ | ID de convenio de KPI, ganchos de telemetría y condiciones de activación. |
| Envío de propuesta | Presidente del consejo | `sns governance charter submit --input SN-CH-YYYY-NN.md` (`CharterMotionV1` بناتا ہے) | CLI Norito manifiesto `artifacts/sns/governance/<id>/charter_motion.json` میں محفوظ کرتی ہے۔ |
| Votar اور reconocimiento de tutor | Consejo + guardianes | `sns governance ballot cast --proposal <id>` y `sns governance guardian-ack --proposal <id>` | Minutas hash اور pruebas de quórum منسلک کریں۔ |
| Aceptación del mayordomo | Programa de mayordomos | `sns governance steward-ack --proposal <id> --signature <file>` | Políticas de sufijo تبدیل کرنے سے پہلے لازم؛ `artifacts/sns/governance/<id>/steward_ack.json` Sobre میں ریکارڈ کریں۔ |
| Activación | Operaciones de registro | `SuffixPolicyV1` اپڈیٹ کریں، cachés del registrador ریفریش کریں، `status.md` میں نوٹ شائع کریں۔ | Marca de tiempo de activación `sns_governance_activation_total` میں لاگ ہوتا ہے۔ |
| Registro de auditoría | Cumplimiento | `docs/source/sns/regulatory/<jurisdiction>/<cycle>.md` Registro de perforación میں اندراج کریں اگر de mesa ہوا ہو۔ | Paneles de telemetría y diferencias de políticas کی حوالہ جات شامل کریں۔ |

### 4.2 Registro, subasta y aprobaciones de precios1. **Comprobación previa:** Registrador `SuffixPolicyV1` کو دیکھ کر nivel de precios, دستیاب términos اور
   ventanas de gracia/redención کی تصدیق کرتا ہے۔ قیمت کی شیٹس کو hoja de ruta میں درج
   3/4/5/6-9/10+ nivel ٹیبل (nivel base + coeficientes de sufijo) کے مطابق sincronizado رکھیں۔
2. **Subastas de oferta sellada:** Grupos premium کے لئے Ciclo de confirmación de 72 h / ciclo de revelación de 24 h
   `sns governance auction commit` / `... reveal` کے ذریعے چلائیں۔ cometer فہرست
   (hashes) `artifacts/sns/auctions/<name>/commit.json` میں شائع کریں تاکہ
   los auditores verifican la aleatoriedad کر سکیں۔
3. **Verificación de pago:** Registradores `PaymentProofV1` کو divisiones de tesorería کے خلاف
   validar کرتے ہیں (70% tesorería / 30% administrador, exclusión de referencia <= 10%). Norito JSON
   `artifacts/sns/payments/<tx>.json` Respuesta del registrador
   (`RevenueAccrualEventV1`) میں لنک کریں۔
4. **Gancho de gobernanza:** Premium/protegido ناموں کے لئے `GovernanceHookV1` لگائیں جس میں
   ID de propuesta del consejo y firmas del administrador ہوں۔ Faltan ganchos سے
   `sns_err_governance_missing` آتا ہے۔
5. **Activación + sincronización del resolutor:** جیسے ہی Torii evento de registro بھیجے، transparencia del resolutor
   tailer چلائیں تاکہ نیا GAR/estado de zona پھیلنے کی تصدیق ہو (4.5 دیکھیں)۔
6. **Divulgación del cliente:** Libro mayor de cara al cliente (billetera/explorador) کو
   [`address-display-guidelines.md`](./address-display-guidelines.md) کی accesorios compartidos کے
   ذریعے اپڈیٹ کریں، اور یقینی بنائیں کہ I105 اور copia de renderizados comprimidos (`sora`) / QR
   orientación سے میل کھاتے ہیں۔### 4.3 Renovaciones, facturación y conciliación de tesorería

- **Flujo de trabajo de renovación:** Registradores `SuffixPolicyV1` میں دی گئی 30 دن gracia + 60 دن
  ventanas de canje نافذ کرتے ہیں۔ 60 دن بعد Secuencia de reapertura holandesa (7 دن، 10x tarifa جو
  15%/دن کم ہوتی ہے) خودکار طور پر `sns governance reopen` سے چلتی ہے۔
- **División de ingresos:** ہر renovación یا transferencia `RevenueAccrualEventV1` بناتا ہے۔ Exportaciones del Tesoro
  (CSV/Parquet) کو روزانہ ان eventos کے ساتھ reconciliar کرنا چاہیے؛ pruebas
  `artifacts/sns/treasury/<date>.json` میں منسلک کریں۔
- **Exclusiones de referencia:** اختیاری referencia فیصد sufijo کے حساب سے `referral_share` کو
  política de administrador میں شامل کر کے ٹریک کیے جاتے ہیں۔ La división final de los registradores emite کرتے ہیں
  اور manifiestos de referencia کو comprobante de pago کے ساتھ رکھتے ہیں۔
- **Cadencia de informes:** Anexos de KPI de Finanzas (registros, renovaciones, ARPU, disputas/fianzas)
  utilización) `docs/source/sns/regulatory/<suffix>/YYYY-MM.md` میں پوسٹ کرتا ہے۔ Paneles de control
  انہی tablas de exportación سے چلیں تاکہ Grafana نمبرز evidencia del libro mayor سے میچ کریں۔
- **Revisión mensual de KPI:** پہلے منگل کا líder financiero de punto de control, administrador de turno اور programa PM کو جوڑتا ہے۔
  [Panel de KPI de SNS](./kpi-dashboard.md) کھولیں (incrustación del portal `sns-kpis` /
  `dashboards/grafana/sns_suffix_analytics.json`), rendimiento del registrador + tablas de ingresos
  exportar کریں، deltas anexo میں لاگ کریں، اور artefactos memo کے ساتھ لگائیں۔ revisión de اگر
  Infracciones de SLA (congelar ventanas >72 h, picos de errores del registrador, deriva de ARPU) پائے تو incidente
  desencadenar کریں۔### 4.4 Congelaciones, disputas y apelaciones

| مرحلہ | مالک | Acción y Pruebas | Acuerdo de Nivel de Servicio |
|-------|------|--------------------|-----|
| Solicitud de congelación suave | Mayordomo / apoyo | Ticket `SNS-DF-<id>` incluye comprobantes de pago, referencia de fianza en disputa, selector(es) afectado(s) | <=4 h de ingesta سے۔ |
| Boleto de guardián | Junta de guardianes | `sns governance freeze --selector <I105> --reason <text> --until <ts>` `GuardianFreezeTicketV1` بناتا ہے۔ Ticket JSON `artifacts/sns/guardian/<id>.json` میں رکھیں۔ | <=30 min ACK, <=2 h de ejecución۔ |
| Ratificación del Consejo | Consejo de gobierno | Congelar aprobar/rechazar کریں، ticket de tutor اور resumen de bonos de disputa کے ساتھ فیصلہ ڈاکیومنٹ کریں۔ | اگلا sesión del consejo یا votación asíncrona۔ |
| Panel de arbitraje | Cumplimiento + administrador | Panel de 7 jurados (hoja de ruta کے مطابق) بلائیں، votos hash `sns governance dispute ballot` کے ذریعے جمع کریں۔ Paquete de incidentes de recibos de voto anónimo میں لگائیں۔ | Veredicto <= 7 días después del depósito de fianza۔ |
| Apelación | Guardián + consejo | Fianza de apelación کو دوگنا کرتی ہیں اور proceso de jurado دہراتی ہیں؛ Norito manifiesto `DisputeAppealV1` ریکارڈ کریں اور ticket primario ریفرنس کریں۔ | <=10 días۔ |
| Descongelar y remediar | Registrador + operaciones de resolución | `sns governance unfreeze --selector <I105> --ticket <id>` چلائیں، estado del registrador اپڈیٹ کریں، اور GAR/resolver diffs propagate کریں۔ | Veredicto کے فوراً بعد۔ |

Cánones de emergencia (congelaciones activadas por el guardián <=72 h) بھی اسی flow کو فالو کرتے ہیں
لیکن revisión retroactiva del consejo اور `docs/source/sns/regulatory/` میں transparencia
nota درکار ہے۔### 4.5 Resolver y propagación de puerta de enlace

1. **Gancho de evento:** ہر flujo de eventos de resolución de eventos de registro (`tools/soradns-resolver` SSE) پر
   emitir ہوتا ہے۔ Operaciones de resolución suscribirse کر کے transparencia tailer
   (`scripts/telemetry/run_soradns_transparency_tail.sh`) کے ذریعے diferencias ریکارڈ کرتے ہیں۔
2. **Actualización de plantilla GAR:** Gateways کو `canonical_gateway_suffix()` سے consulte ہونے والے plantillas GAR
   اپڈیٹ کرنے ہوں گے اور `host_pattern` lista کو دوبارہ signo کرنا ہوگا۔ diferencias
   `artifacts/sns/gar/<date>.patch` میں رکھیں۔
3. **Publicación de Zonefile:** `roadmap.md` میں بیان کردہ esqueleto de archivo de zona (nombre, ttl, cid, prueba)
   Presione el botón Torii/SoraFS para presionar el botón Norito JSON
   `artifacts/sns/zonefiles/<name>/<version>.json` Archivo میں کریں۔
4. **Verificación de transparencia:** `promtool test rules dashboards/alerts/tests/soradns_transparency_rules.test.yml`
   چلائیں تاکہ alertas verde رہیں۔ Prometheus salida de texto کو informe de transparencia semanal کے ساتھ منسلک کریں۔
5. **Auditoría de puerta de enlace:** Ejemplos de encabezado `Sora-*` (política de caché, resumen de CSP, GAR) ریکارڈ کریں اور انہیں
   registro de gobernanza کے ساتھ adjuntar کریں تاکہ آپریٹرز ثابت کر سکیں کہ puerta de enlace نے نیا نام
   مطلوبہ barandillas کے ساتھ sirven کیا۔

## 5. Telemetría e informes| Señal | Fuente | Descripción / Acción |
|--------|--------|----------------------|
| `sns_registrar_status_total{result,suffix}` | Controladores de registradores Torii | رجسٹریشن، renovaciones, congelaciones, transferencias کے لئے contador de éxito/error؛ جب `result="error"` sufijo کے حساب سے بڑھ جائے تو alerta۔ |
| `torii_request_duration_seconds{route="/v2/sns/*"}` | Métricas Torii | Controladores de API para SLO de latencia `torii_norito_rpc_observability.json` سے بنے paneles de control کو feed کرتا ہے۔ |
| `soradns_bundle_proof_age_seconds` y `soradns_bundle_cid_drift_total` | Tailer de transparencia de resolución | پرانے pruebas یا GAR deriva کو detectar کرتا ہے؛ barandillas `dashboards/alerts/soradns_transparency_rules.yml` میں define ہیں۔ |
| `sns_governance_activation_total` | CLI de gobernanza | ہر activación de estatutos/anexos پر بڑھنے والا contador؛ decisiones del consejo کو adiciones publicadas کے ساتھ conciliar کرنے میں استعمال۔ |
| Calibre `guardian_freeze_active` | CLI de guardián | ہر selector کے لئے pista de ventanas de congelación suave/dura کرتا ہے؛ اگر `1` SLA سے زیادہ رہے تو SRE کو página کریں۔ |
| Paneles de control del anexo KPI | Finanzas / Documentos | ماہانہ rollups جو memorandos regulatorios کے ساتھ شائع ہوتے ہیں؛ portal انہیں [Panel de KPI de SNS](./kpi-dashboard.md) کے ذریعے incrustar کرتا ہے تاکہ mayordomos اور reguladores ایک ہی Grafana ver تک پہنچیں۔ |

## 6. Pruebas y requisitos de auditoría| Acción | Pruebas para archivar | Almacenamiento |
|--------|--------------------|---------|
| Cambio de estatuto/política | Manifiesto Norito firmado, transcripción CLI, diferencia de KPI, reconocimiento del administrador۔ | `artifacts/sns/governance/<proposal-id>/` + `docs/source/sns/governance_addenda/`. |
| Registro / renovación | Carga útil `RegisterNameRequestV1`, `RevenueAccrualEventV1`, comprobante de pago۔ | `artifacts/sns/payments/<tx>.json`, registros de API del registrador۔ |
| Subasta | Confirmar/revelar manifiestos, semilla de aleatoriedad, hoja de cálculo de cálculo del ganador۔ | `artifacts/sns/auctions/<name>/`. |
| Congelar/descongelar | Boleto de Guardian, hash de votación del consejo, URL de registro de incidentes, plantilla de comunicaciones de clientes۔ | `artifacts/sns/guardian/<ticket>/`, `incident/<date>-sns-*.md`. |
| Propagación del resolver | Zonefile/GAR diff, extracto JSONL de cola, instantánea Prometheus۔ | `artifacts/sns/resolver/<date>/` + informes de transparencia۔ |
| Ingesta reglamentaria | Memo de admisión, rastreador de fechas límite, reconocimiento del delegado, resumen de cambios de KPI ۔ | `docs/source/sns/regulatory/<jurisdiction>/<cycle>.md`. |

## 7. Lista de verificación de la puerta de fase| Fase | Criterios de salida | Paquete de pruebas |
|-------|---------------|-----------------|
| N0 - Beta cerrada | Esquema de registro SN-1/SN-2, CLI de registrador manual, taladro guardián مکمل۔ | Moción de estatuto + administrador ACK, registros de ejecución en seco del registrador, informe de transparencia del solucionador, entrada `ops/drill-log.md` ۔ |
| N1 - Lanzamiento público | Subastas + niveles de precio fijo disponibles para `.sora`/`.nexus`, registrador de autoservicio, sincronización automática de resolución, paneles de facturación ۔ | Diferencia de la hoja de precios, resultados de CI del registrador, anexo de pago/KPI, resultados del seguimiento de transparencia, notas de ensayo de incidentes ۔ |
| N2 - Ampliación | `.dao`, API de revendedor, portal de disputas, cuadros de mando del administrador, paneles de análisis ۔ | Capturas de pantalla del portal, métricas de disputas de SLA, exportaciones de cuadros de mando del administrador, carta de gobernanza actualizada que hace referencia a las políticas de revendedor. |

Salidas de fase کو ریکارڈ شدہ ejercicios de mesa (registro camino feliz, congelación, interrupción del resolutor) درکار ہیں جن کے
artefactos `ops/drill-log.md` میں منسلک ہوں۔

## 8. Respuesta al incidente y escalada| Gatillo | Gravedad | Propietario inmediato | Acciones obligatorias |
|---------|----------|-----------------|-------------------|
| Deriva del resolver/GAR یا pruebas obsoletas | Septiembre 1 | Resolver SRE + tablero guardián | resolutor de guardia کو página کریں، salida de cola captura کریں، متاثرہ نام congelar کرنے کا فیصلہ کریں، ہر 30 min پر actualización de estado دیں۔ |
| Interrupción del registrador, falla de facturación, y errores generalizados de API | Septiembre 1 | Gerente de servicio de registrador | Subastas روکیں، manual CLI پر سوئچ کریں، mayordomos/tesorería کو اطلاع دیں، Torii registros کو documento de incidente میں adjuntar کریں۔ |
| Disputa de un solo nombre, discrepancia en el pago, یا escalada de clientes | Septiembre 2 | Mayordomo + líder de soporte | comprobantes de pago جمع کریں، congelamiento suave کی ضرورت طے کریں، SLA کے اندر solicitante کو جواب دیں، rastreador de disputas میں نتیجہ لاگ کریں۔ |
| Hallazgo de la auditoría de cumplimiento | Septiembre 2 | Enlace de cumplimiento | plan de remediación تیار کریں، `docs/source/sns/regulatory/` میں memorando فائل کریں، sesión del consejo de seguimiento شیڈول کریں۔ |
| Taladro یا ensayo | Septiembre 3 | Programa PM | `ops/drill-log.md` کے escenario con guión کو چلائیں، archivo de artefactos کریں، lagunas کو tareas de hoja de ruta کے طور پر لیبل کریں۔ |

تمام incidentes کو `incident/YYYY-MM-DD-sns-<slug>.md` بنانا ہوگا جس میں tablas de propiedad,
registros de comando, اور اس پلی بک میں تیار ہونے والے evidencia کے referencias ہوں۔

## 9. حوالہ جات-[`registry-schema.md`](./registry-schema.md)
- [`registrar-api.md`](./registrar-api.md)
-[`address-display-guidelines.md`](./address-display-guidelines.md)
-[`docs/account_structure.md`](../../../account_structure.md)
-[`docs/source/soradns/deterministic_hosts.md`](../../../source/soradns/deterministic_hosts.md)
-[`docs/source/reports/soradns_transparency.md`](../../../source/reports/soradns_transparency.md)
- `ops/drill-log.md`
- `roadmap.md` (secciones SNS, DG, ADDR)

جب بھی redacción del estatuto, superficies CLI یا contratos de telemetría بدلیں تو اس پلی بک کو
اپڈیٹ رکھیں؛ entradas de la hoja de ruta جو `docs/source/sns/governance_playbook.md` کو
ریفرنس کرتی ہیں انہیں ہمیشہ تازہ ترین ورژن سے میل کھانا چاہیے۔