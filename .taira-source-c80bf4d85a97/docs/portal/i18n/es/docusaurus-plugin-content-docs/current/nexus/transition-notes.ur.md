---
lang: es
direction: ltr
source: docs/portal/docs/nexus/transition-notes.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: notas-de-transición-nexus
título: Nexus ٹرانزیشن نوٹس
descripción: `docs/source/nexus_transition_notes.md` کا آئینہ، جو Phase B ٹرانزیشن ثبوت، آڈٹ شیڈول، اور میٹیگیشنز کو کور کرتا ہے۔
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Nexus ٹرانزیشن نوٹس

یہ لاگ **Fase B - Nexus Cimientos de transición** کے باقی کام کو اس وقت تک ٹریک کرتا ہے جب تک multicarril لانچ چیک لسٹ مکمل نہ ہو جائے۔ یہ `roadmap.md` میں hitos اندراجات کی تکمیل کرتا ہے اور B1-B4 میں حوالہ دی گئی evidencia کو ایک جگہ رکھتا ہے تاکہ gobernanza, SRE اور SDK لیڈز ایک ہی fuente de verdad شیئر کر سکیں۔

## اسکوپ اور cadencia

- rastreo enrutado, barandillas de telemetría (B1/B2), gobernanza, configuración delta (B3), seguimientos de ensayo de lanzamiento de varios carriles (B4), seguimiento de ruta
- یہ عارضی cadencia نوٹ کی جگہ لیتا ہے جو پہلے یہاں تھا؛ Q1 2026 آڈٹ کے بعد تفصیلی رپورٹ `docs/source/nexus_routed_trace_audit_report_2026q1.md` میں ہے، جبکہ یہ صفحہ جاری شیڈول اور mitigación رجسٹر رکھتا ہے۔
- ہر ruta de seguimiento ونڈو، gobernanza ووٹ، یا ensayo de lanzamiento کے بعد ٹیبلز اپ ڈیٹ کریں۔ جب artefactos حرکت کریں تو نئی جگہ کو اسی صفحے میں ریفلیکٹ کریں تاکہ documentos posteriores (estado, paneles, portales SDK) ایک مستحکم ancla سے لنک کر سکیں۔

## Resumen de evidencia (2026 Q1-Q2)| ورک اسٹریم | ثبوت | مالکان | اسٹیٹس | نوٹس |
|------------|----------|----------|--------|-------|
| **B1 - Auditorías de seguimiento enrutado** | `docs/source/nexus_routed_trace_audit_report_2026q1.md`, `docs/examples/nexus_audit_outcomes/` | @operaciones de telemetría, @gobernanza | مکمل (primer trimestre de 2026) | تین آڈٹ ونڈوز ریکارڈ ہوئیں؛ `TRACE-CONFIG-DELTA` کا TLS retraso Q2 repetición میں بند ہوا۔ |
| **B2 - Remediación de telemetría y barandillas** | `docs/source/nexus_telemetry_remediation_plan.md`, `docs/source/telemetry.md`, `dashboards/alerts/nexus_audit_rules.yml` | @sre-core, @telemetry-ops | مکمل | paquete de alertas, política de bot de diferencias, tamaño de lote de OTLP (registro `nexus.scheduler.headroom` + panel de espacio libre Grafana) فراہم ہو چکے ہیں؛ کوئی واورز اوپن نہیں۔ |
| **B3 - Configurar aprobaciones delta** | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md`, `defaults/nexus/config.toml`, `defaults/nexus/genesis.json` | @release-eng, @governance | مکمل | GOV-2026-03-19 ووٹ ریکارڈ ہے؛ paquete firmado نیچے والے paquete de telemetría کو feed کرتا ہے۔ |
| **B4 - Ensayo de lanzamiento de varios carriles** | `docs/source/runbooks/nexus_multilane_rehearsal.md`, `docs/source/project_tracker/nexus_rehearsal_2026q1.md`, `artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json`, `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json`, `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` | @nexus-core, @sre-core | مکمل (segundo trimestre de 2026) | Repetición canary del segundo trimestre y mitigación del retraso de TLS manifiesto del validador + `.sha256` con ranuras 912-936, semilla de carga de trabajo `NEXUS-REH-2026Q2` para volver a ejecutar el archivo hash de perfil TLS y capturar |

## سہ ماہی ruta de seguimiento آڈٹ شیڈول| ID de seguimiento | ونڈو (UTC) | نتیجہ | نوٹس |
|----------|--------------|---------|-------|
| `TRACE-LANE-ROUTING` | 2026-02-17 09:00-09:45 | پاس | Admisión a cola P95 ہدف <=750 ms سے کافی نیچے رہا۔ کوئی ایکشن درکار نہیں۔ |
| `TRACE-TELEMETRY-BRIDGE` | 2026-02-24 10:00-10:45 | پاس | Hashes de reproducción OTLP `status.md` کے ساتھ منسلک ہیں؛ SDK diff bot paridad y deriva cero کی تصدیق کی۔ |
| `TRACE-CONFIG-DELTA` | 2026-03-01 12:00-12:30 | حل شدہ | TLS retraso Q2 repetición میں بند ہوا؛ `NEXUS-REH-2026Q2` paquete de telemetría کے میں hash de perfil TLS `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb` ریکارڈ ہے (دیکھیں `artifacts/nexus/tls_profile_rollout_2026q2/`) اور صفر rezagados ہیں۔ |
| `TRACE-MULTILANE-CANARY` | 2026-05-05 09:12-10:14 | پاس | Semilla de carga de trabajo `NEXUS-REH-2026Q2`; paquete de telemetría + manifiesto/resumen `artifacts/nexus/rehearsals/2026q1/` میں (rango de ranura 912-936) y agenda `artifacts/nexus/rehearsals/2026q2/` میں ہے۔ |

آنے والے سہ ماہیوں میں نئی قطاریں شامل کریں اور جب ٹیبل موجودہ سہ ماہی سے بڑا ہو جائے تو مکمل اندراجات کو apéndice میں منتقل کریں۔ ruta-traza رپورٹس یا minutos de gobierno سے اس سیکشن کو `#quarterly-routed-trace-audit-schedule` anclaje کے ذریعے ریفرنس کریں۔

## Mitigaciones y elementos pendientes| آئٹم | تفصیل | مالک | ہدف | اسٹیٹس / نوٹس |
|------|-------------|-------|--------|----------------|
| `NEXUS-421` | `TRACE-CONFIG-DELTA` کے دوران پیچھے رہ جانے والے Perfil TLS کی propagación مکمل کریں، volver a ejecutar la captura de evidencia کریں، اور registro de mitigación بند کریں۔ | @release-eng, @sre-core | Q2 2026 seguimiento enrutado ونڈو | بند - Hash de perfil TLS `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb` کو `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json` + `.sha256` میں captura کیا گیا؛ volver a ejecutar نے cero rezagados کی تصدیق کی۔ |
| Preparación `TRACE-MULTILANE-CANARY` | Ensayo del segundo trimestre Paquete de telemetría Paquete de accesorios Paquete de arneses SDK validar reutilización del asistente | @telemetry-ops, Programa SDK | 2026-04-30 convocatoria de planificación | مکمل - agenda `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` میں محفوظ ہے جس میں ranura/metadatos de carga de trabajo شامل ہے؛ rastreador de reutilización de arneses میں نوٹ ہے۔ |
| Rotación de resumen del paquete de telemetría | ہر ensayo/lanzamiento سے پہلے `scripts/telemetry/validate_nexus_telemetry_pack.py` چلائیں اور digests کو config delta tracker کے ساتھ لاگ کریں۔ | @operaciones de telemetría | ہر candidato de liberación | مکمل - `telemetry_manifest.json` + `.sha256` `artifacts/nexus/rehearsals/2026q1/` میں جاری ہوئے (rango de ranura `912-936`, semilla `NEXUS-REH-2026Q2`); rastreador de resúmenes اور índice de evidencia میں کاپی کیے گئے۔ |

## Configurar la integración del paquete delta- Resumen de diferencias canónicas `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` ہے۔ جب نئے `defaults/nexus/*.toml` یا cambios de génesis آئیں تو پہلے tracker اپ ڈیٹ کریں اور پھر خلاصہ یہاں شامل کریں۔
- Paquete de telemetría de ensayo de paquetes de configuración firmados کو feed کرتے ہیں۔ یہ pack, جو `scripts/telemetry/validate_nexus_telemetry_pack.py` سے validar ہوتا ہے، config delta evidencia کے ساتھ publicar ہونا چاہیے تاکہ operadores B4 میں استعمال ہونے والے بالکل وہی artefactos de repetición کر سکیں۔
- Iroha 2 paquetes de carriles Opciones de configuración: `nexus.enabled = false` Configuraciones de carril/espacio de datos/anulaciones de enrutamiento y rechazo de paquetes El perfil Nexus (`--sora`) habilita نہ ہو، اس لئے plantillas de un solo carril سے `nexus.*` secciones ہٹا دیں۔
- registro de votos de gobernanza (GOV-2026-03-19) کو tracker اور اس نوٹ دونوں سے لنک رکھیں تاکہ مستقبل کے votos اسی فارمیٹ کو دوبارہ دریافت کیے بغیر استعمال کر سکیں۔

## Seguimiento del ensayo de lanzamiento- `docs/source/runbooks/nexus_multilane_rehearsal.md` canary پلان، lista de participantes y pasos de reversión کو محفوظ کرتا ہے؛ جب topología de carril یا exportadores de telemetría بدلیں تو runbook اپ ڈیٹ کریں۔
- `docs/source/project_tracker/nexus_rehearsal_2026q1.md` 9 اپریل کے ensayo کے ہر artefacto کو لسٹ کرتا ہے اور اب Notas/agenda de preparación del segundo trimestre بھی رکھتا ہے۔ مستقبل کے ensayos اسی rastreador میں شامل کریں تاکہ evidencia monótona رہے۔
- Fragmentos del recopilador OTLP اور Grafana exportaciones (دیکھیں `docs/source/telemetry.md`) تب publicar کریں جب guía de procesamiento por lotes del exportador بدلے؛ Actualización del primer trimestre نے alertas de margen de maniobra سے بچنے کے لئے tamaño de lote کو 256 muestras تک بڑھایا۔
- CI de varios carriles/prueba de evidencia ریفرنس کو ریٹائر کیا؛ `defaults/nexus/config.toml` کے hash (`nexus.enabled = true`, blake2b `d69eefa2abb8886b0f3e280e88fe307a907cfe88053b5d60a1d459a5cf8549e1`) کو tracker کے ساتھ sincronización رکھیں جب paquetes de ensayo ریفریش ہوں۔

## Ciclo de vida del carril de ejecución- Planes de ciclo de vida del carril de ejecución اب enlaces de espacio de datos validan کرتے ہیں اور Kura/la reconciliación de almacenamiento por niveles falla ہونے پر abort کر دیتے ہیں، جس سے catalog بغیر تبدیلی کے رہتا ہے۔ ayudantes carriles relés en caché poda poda síntesis de libro mayor de fusión reutilización de pruebas
- Asistentes de configuración/ciclo de vida Nexus (`State::apply_lane_lifecycle`, `Queue::apply_lane_lifecycle`) Se aplican varios planes Se aplican carriles Se reinicia Se agrega/retira سکے؛ enrutamiento, instantáneas de TEU, registros de manifiesto, plan, recarga, recarga
- Operadores کے لئے رہنمائی: اگر plan falló ہو تو faltan espacios de datos یا raíces de almacenamiento چیک کریں جو بن نہیں سکتے (directorios raíz fría por niveles/carril Kura)۔ rutas base درست کریں اور دوبارہ کوشش کریں؛ کامیاب planes carril/espacio de datos diferencia de telemetría دوبارہ emitir کرتے ہیں تاکہ paneles de control نئی topología دکھا سکیں۔

## Telemetría NPoS y evidencia de contrapresión

Ensayo de lanzamiento de fase B retro نے capturas de telemetría determinista مانگے تھے جو ثابت کریں کہ NPoS marcapasos اور capas de chismes اپنی contrapresión حدود میں رہتے ہیں۔ `integration_tests/tests/sumeragi_npos_performance.rs` y el arnés de integración y los escenarios, las métricas y los resúmenes JSON (`sumeragi_baseline_summary::<scenario>::...`) emiten لوکل چلانے کے لئے:

```bash
cargo test -p integration_tests sumeragi_npos_performance -- --nocapture
````SUMERAGI_NPOS_STRESS_PEERS`, `SUMERAGI_NPOS_STRESS_COLLECTORS_K` یا `SUMERAGI_NPOS_STRESS_REDUNDANT_SEND_R` سیٹ کریں تاکہ زیادہ estrés y topologías دیکھ سکیں؛ valores predeterminados B4 میں استعمال ہونے والے 1 s/`k=3` colector پروفائل کو reflect کرتے ہیں۔| Escenario/prueba | Cobertura | Telemetría clave |
| --- | --- | --- |
| `npos_baseline_1s_k3_captures_metrics` | tiempo de bloque de ensayo کے ساتھ 12 rondas چلاتا ہے تاکہ Sobres de latencia EMA, profundidades de cola اور medidores de envío redundante ریکارڈ ہوں، پھر paquete de evidencia serializar کیا جاتا ہے۔ | `sumeragi_phase_latency_ema_ms`, `sumeragi_collectors_k`, `sumeragi_redundant_send_r`, `sumeragi_bg_post_queue_depth*`. |
| `npos_queue_backpressure_triggers_metrics` | cola de transacciones کو بھر کر یقینی بناتا ہے کہ los aplazamientos de admisión activan de manera determinista ہوں اور la exportación de contadores de capacidad/saturación de la cola کرے۔ | `sumeragi_tx_queue_depth`, `sumeragi_tx_queue_capacity`, `sumeragi_tx_queue_saturated`, `sumeragi_pacemaker_backpressure_deferrals_total`, `sumeragi_rbc_backpressure_deferrals_total`. |
| `npos_pacemaker_jitter_within_band` | fluctuación del marcapasos اور ver muestras de tiempos de espera کرتا ہے جب تک +/-125 permille بینڈ نافذ ہونے کا ثبوت نہ ملے۔ | `sumeragi_pacemaker_jitter_ms`, `sumeragi_pacemaker_view_timeout_target_ms`, `sumeragi_pacemaker_jitter_frac_permille`. |
| `npos_rbc_store_backpressure_records_metrics` | Cargas útiles de RBC Almacenar Suave/Duro Empujar Sesiones Contadores de bytes Contadores de bytes Estabilizar دکھایا جا سکے۔ | `sumeragi_rbc_store_pressure`, `sumeragi_rbc_store_sessions`, `sumeragi_rbc_store_bytes`, `sumeragi_rbc_backpressure_deferrals_total`. |
| `npos_redundant_send_retries_update_metrics` | retransmite fuerza کرتا ہے تاکہ medidores de relación de envío redundante اور contadores de recolectores en el objetivo آگے بڑھیں، اور ظاہر ہو کہ retro y telemetría de extremo a extremo جڑی ہے۔ | `sumeragi_collectors_targeted_current`, `sumeragi_redundant_sends_total`. |
| `npos_rbc_chunk_loss_fault_reports_backlog` | los fragmentos espaciados deterministamente caen کرتا ہے تاکہ monitores de acumulación خاموش drenaje کے بجائے fallas aumentan کریں۔ | `sumeragi_rbc_backlog_sessions_pending`, `sumeragi_rbc_backlog_chunks_total`, `sumeragi_rbc_backlog_chunks_max`. |جب بھی gobernanza یہ ثبوت مانگے کہ alarmas de contrapresión topología de ensayo سے coincidencia کرتے ہیں، arnés کے پرنٹ شدہ Líneas JSON کے ساتھ Prometheus scrape بھی منسلک کریں۔

## Actualizar lista de verificación

1. نئی routed-trace ونڈوز شامل کریں اور جب سہ ماہی بدلے تو پرانی اندراجات منتقل کریں۔
2. Seguimiento de Alertmanager کے بعد tabla de mitigación اپ ڈیٹ کریں، چاہے ticket de acción بند کرنا ہی کیوں نہ ہو۔
3. جب config deltas بدلیں، tracker, یہ نوٹ، اور resúmenes de paquetes de telemetría کی فہرست کو اسی pull request میں اپ ڈیٹ کریں۔
4. کسی بھی نئے artefacto de ensayo/telemetría کو یہاں لنک کریں تاکہ مستقبل کی actualizaciones de la hoja de ruta ایک ہی دستاویز کو ریفرنس کریں، بکھری ہوئی ad-hoc نوٹس نہیں۔

## Índice de evidencia| اثاثہ | مقام | نوٹس |
|-------|----------|-------|
| Informe de auditoría de seguimiento enrutado (primer trimestre de 2026) | `docs/source/nexus_routed_trace_audit_report_2026q1.md` | Evidencia de fase B1 کے لئے fuente canónica؛ پورٹل پر `docs/portal/docs/nexus/nexus-routed-trace-audit-2026q1.md` Espejo retrovisor ہے۔ |
| Configurar rastreador delta | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` | Resúmenes de diferencias de TRACE-CONFIG-DELTA, iniciales de los revisores, registro de votos de GOV-2026-03-19 شامل ہیں۔ |
| Plan de remediación de telemetría | `docs/source/nexus_telemetry_remediation_plan.md` | paquete de alertas, tamaño de lote OTLP, اور B2 سے جڑی barreras de presupuesto de exportación کو دستاویز کرتا ہے۔ |
| Rastreador de ensayos de varios carriles | `docs/source/project_tracker/nexus_rehearsal_2026q1.md` | 9 artefactos de ensayo, manifiesto/resumen del validador, notas/agenda del segundo trimestre y evidencia de reversión |
| Manifiesto/resumen del paquete de telemetría (más reciente) | `artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json` (+ `.sha256`) | rango de ranura 912-936, semilla `NEXUS-REH-2026Q2` اور paquetes de gobernanza کے artefactos hashes ریکارڈ کرتا ہے۔ |
| Manifiesto de perfil TLS | `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json` (+ `.sha256`) | Repetición del segundo trimestre کے دوران captura شدہ منظور شدہ TLS perfil hash؛ apéndices de seguimiento enrutado میں citar کریں۔ |
| Agenda TRACE-MULTILANO-CANARIA | `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` | Planificación de ensayos del segundo trimestre نوٹس (ونڈو، rango de espacios, semilla de carga de trabajo, propietarios de acciones) ۔ |
| Lanzamiento del runbook de ensayo | `docs/source/runbooks/nexus_multilane_rehearsal.md` | puesta en escena -> ejecución -> reversión کے لئے عملی checklist؛ topología de carril یا guía del exportador بدلنے پر اپ ڈیٹ کریں۔ |
| Validador de paquetes de telemetría | `scripts/telemetry/validate_nexus_telemetry_pack.py` | B4 retro میں حوالہ دیا گیا CLI؛ paquete بدلنے پر resúmenes کو tracker کے ساتھ آرکائیو کریں۔ || Regresión multicarril | `ci/check_nexus_multilane.sh` + `integration_tests/tests/nexus/multilane_router.rs` | configuraciones de varios carriles `nexus.enabled = true` ثابت کرتا ہے، Sora catalog hashes محفوظ رکھتا ہے، اور `ConfigLaneRouter` کے ذریعے lane-local Provisión de rutas de registro de combinación/kura (`blocks/lane_{id:03}_{slug}`) کر کے resúmenes de artefactos شائع کرتا ہے۔ |