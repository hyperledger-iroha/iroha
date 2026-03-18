---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/direct-mode-pack.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: paquete de modo directo
título: SoraFS paquete alternativo de modo directo (SNNet-5a)
sidebar_label: paquete de modo directo
descripción: SNNet-5a en modo directo SoraFS y Torii/QUIC modo directo Configuración en modo directo Comprobaciones de cumplimiento y pasos de implementación
---

:::nota مستند ماخذ
:::

Circuitos SoraNet SoraFS کے لیے transporte predeterminado ہیں، مگر elemento de hoja de ruta **SNNet-5a** ایک respaldo regulado کا تقاضا کرتا ہے تاکہ implementación de anonimato مکمل ہونے تک operadores acceso de lectura determinista برقرار رکھ سکیں۔ Paquete de perillas CLI/SDK, perfiles de configuración, pruebas de cumplimiento, lista de verificación de implementación, transportes de privacidad, SoraFS y modo Torii/QUIC directo. میں چلانے کے لیے درکار ہیں۔

یہ puesta en escena alternativa اور entornos de producción regulados پر لاگو ہوتا ہے جب تک SNNet-5 سے SNNet-9 اپنے puertas de preparación پاس نہ کر لیں۔ نیچے والے artefactos کو معمول کے SoraFS implementación colateral سوئچ کر سکیں۔

## 1. Banderas CLI y SDK- `sorafs_cli fetch --transport-policy=direct-only ...` deshabilita la programación del relé کرتا ہے اور Los transportes Torii/QUIC aplican کرتا ہے۔ Ayuda CLI اب `direct-only` کو valor aceptado کے طور پر دکھاتی ہے۔
- SDK `OrchestratorConfig::with_transport_policy(TransportPolicy::DirectOnly)` configurado کرنا چاہیے جب بھی وہ "modo directo" alternar exposición کریں۔ `iroha::ClientOptions` o `iroha_android` enlaces generados automáticamente y enumeración hacia adelante کرتے ہیں۔
- Arneses de puerta de enlace (`sorafs_fetch`, enlaces de Python) compartidos Norito Ayudantes JSON کے ذریعے análisis de alternancia solo directo کر سکتے ہیں تاکہ automatización کو ایک جیسا comportamiento ملے۔

Runbooks orientados a socios میں documento de marca کریں اور alternancias de funciones کو variables de entorno کے بجائے `iroha_config` کے ذریعے cable کریں۔

## 2. Perfiles de políticas de puerta de enlace

La configuración determinista del orquestador persiste کرنے کے لیے Norito JSON استعمال کریں۔ `docs/examples/sorafs_direct_mode_policy.json` میں perfil de ejemplo یہ codifica کرتا ہے:

- `transport_policy: "direct_only"` — Los proveedores rechazan la publicidad y los transportes de retransmisión SoraNet anuncian la publicidad
- `max_providers: 2`: pares directos کو سب سے puntos finales Torii/QUIC confiables تک محدود کرتا ہے۔ Asignaciones de cumplimiento regional کے مطابق ajustar کریں۔
- `telemetry_region: "regulated-eu"` — métricas emitidas کو etiqueta کرتا ہے تاکہ paneles de control/ejecuciones de respaldo de auditorías کو الگ پہچان سکیں۔
- Presupuestos de reintento conservadores (`retry_budget: 2`, `provider_failure_threshold: 3`) Máscara de puertas de enlace mal configuradas نہ ہوں۔JSON کو `sorafs_cli fetch --config` (automatización) یا Enlaces SDK (`config_from_json`) کے ذریعے carga کریں، پھر operadores کے سامنے exposición de políticas کریں۔ Pistas de auditoría کے لیے salida del marcador (`persist_path`) محفوظ کریں۔

Perillas de control del lado de la puerta de enlace `docs/examples/sorafs_gateway_direct_mode.toml` میں درج ہیں۔ یہ plantilla `iroha app sorafs gateway direct-mode enable` کی salida کو espejo کرتا ہے، sobre/verificaciones de admisión deshabilitar کرتا ہے، cable predeterminado de límite de velocidad کرتا ہے، اور `direct_mode` tabla کو nombres de host derivados del plan اور manifiesto resúmenes سے poblar کرتا ہے۔ Gestión de configuración میں confirmación de fragmento کرنے سے پہلے valores de marcador de posición کو اپنے plan de implementación سے reemplazar کریں۔

## 3. Conjunto de pruebas de cumplimiento

Preparación en modo directo اب Orchestrator اور CLI cajas دونوں میں cobertura شامل کرتی ہے:- `direct_only_policy_rejects_soranet_only_providers` اس بات کو یقینی بناتا ہے کہ `TransportPolicy::DirectOnly` تیزی سے fail کرے جب ہر anuncio candidato صرف Soporte de retransmisiones SoraNet کرتا ہو۔【crates/sorafs_orchestrator/src/lib.rs:7238】
- `direct_only_policy_prefers_direct_transports_when_available` یہ یقینی بناتا ہے کہ Torii/QUIC transports موجود ہوں تو انہی کو استعمال کیا جائے اور SoraNet retransmite la sesión سے خارج رکھا جائے۔【crates/sorafs_orchestrator/src/lib.rs:7285】
- `direct_mode_policy_example_is_valid` `docs/examples/sorafs_direct_mode_policy.json` analizar کرتا ہے تاکہ documentos auxiliares کے ساتھ alineados رہیں۔【crates/sorafs_orchestrator/src/lib.rs:7509】【docs/examples/sorafs_direct_mode_policy.json:1】
- `fetch_command_respects_direct_transports` `sorafs_cli fetch --transport-policy=direct-only` کو se burló de la puerta de enlace Torii کے خلاف چلتا ہے، جو ambientes regulados کے لیے prueba de humo فراہم کرتا ہے جہاں pin de transporte directo ہوتے ہیں۔【crates/sorafs_car/tests/sorafs_cli.rs:2733】
- `scripts/sorafs_direct_mode_smoke.sh` اسی comando کو política JSON اور persistencia del marcador کے ساتھ wrap کرتا ہے تاکہ automatización de implementación ہو سکے۔

Las actualizaciones publican کرنے سے پہلے suite enfocada چلائیں:

```bash
cargo test -p sorafs_orchestrator direct_only_policy
cargo test -p sorafs_car --features cli fetch_command_respects_direct_transports
```

اگر compilación del espacio de trabajo cambios ascendentes کی وجہ سے falla ہو تو error de bloqueo کو `status.md` میں registro کریں اور dependencia ponerse al día ہونے پر دوبارہ چلائیں۔

## 4. Humos automatizadosصرف Regresiones específicas del entorno de cobertura de CLI نہیں دکھاتی (desviación de la política de puerta de enlace یا desajustes manifiestos)۔ ایک ayudante de humo dedicado `scripts/sorafs_direct_mode_smoke.sh` میں ہے جو `sorafs_cli fetch` کو política de orquestador de modo directo, persistencia del marcador, اور captura de resumen کے ساتھ wrap کرتا ہے۔

Uso de ejemplo:

```bash
./scripts/sorafs_direct_mode_smoke.sh \
  --config docs/examples/sorafs_direct_mode_smoke.conf \
  --provider name=gw-regulated,provider-id=001122...,base-url=https://gw.example/direct/,stream-token=BASE64
```- یہ script CLI flags اور clave=valor archivos de configuración دونوں کو respect کرتا ہے (دیکھیں `docs/examples/sorafs_direct_mode_smoke.conf`)۔ چلانے سے پہلے resumen de manifiesto اور entradas de anuncios del proveedor کو valores de producción سے poblar کریں۔
- `--policy` predeterminado طور پر `docs/examples/sorafs_direct_mode_policy.json` ہے، مگر `sorafs_orchestrator::bindings::config_to_json` سے بننے والا کوئی بھی Orchestrator JSON دیا جا سکتا ہے۔ Política de CLI کو `--orchestrator-config=PATH` کے ذریعے قبول کرتا ہے، جس سے ejecuciones reproducibles ممکن ہوتی ہیں بغیر flags ہاتھ سے tune کیے۔
- جب `sorafs_cli` `PATH` میں نہ ہو تو helper اسے `sorafs_orchestrator` crate سے (perfil de lanzamiento) build کرتا ہے تاکہ pistas de humo enviadas plomería en modo directo کو ejercicio کریں۔
- Salidas:
  - Carga útil ensamblada (`--output`, por defecto `artifacts/sorafs_direct_mode/payload.bin`).
  - Resumen de recuperación (`--summary`, carga útil predeterminada کے ساتھ) جس میں región de telemetría اور informes del proveedor شامل ہوتے ہیں جو evidencia de implementación بنتے ہیں۔
  - Política de instantáneas del marcador JSON میں دیے گئے path پر persist ہوتا ہے (مثلاً `fetch_state/direct_mode_scoreboard.json`)۔ اسے resumen کے ساتھ cambiar tickets میں archivo کریں۔- Automatización de la puerta de adopción: buscar مکمل ہونے کے بعد helper `cargo xtask sorafs-adoption-check` چلاتا ہے جس میں marcador persistente اور rutas de resumen استعمال ہوتے ہیں۔ Valor predeterminado del quórum requerido طور پر línea de comando پر دیے گئے proveedores کی تعداد ہے؛ بڑی muestra چاہیے تو `--min-providers=<n>` anular کریں۔ Resumen de informes de adopción کے ساتھ لکھے جاتے ہیں (`--adoption-report=<path>` ubicación personalizada دے سکتا ہے) اور helper default طور پر `--require-direct-only` (alternativa کے مطابق) اور `--require-telemetry` جب متعلقہ bandera دیا جائے، pasar کرتا ہے۔ اضافی xtask args کے لیے `XTASK_SORAFS_ADOPTION_FLAGS` استعمال کریں (مثلاً downgrade aprobado کے دوران `--allow-single-source` تاکہ gate fallback کو tolera بھی کرے اور hacer cumplir بھی)۔ صرف diagnóstico local میں `--skip-adoption-check` استعمال کریں؛ hoja de ruta کے مطابق ہر ejecución regulada en modo directo میں paquete de informes de adopción لازمی ہے۔

## 5. Lista de verificación de implementación1. **Congelación de configuración:** perfil JSON en modo directo کو `iroha_config` repositorio میں store کریں اور hash کو cambiar ticket میں درج کریں۔
2. **Auditoría de puerta de enlace:** modo directo پر switch کرنے سے پہلے تصدیق کریں کہ Puntos finales Torii TLS, capacidad TLV y registro de auditoría aplicar کر رہے ہیں۔ Perfil de política de puerta de enlace کو operadores کے لیے publicar کریں۔
3. **Aprobación de cumplimiento:** libro de estrategias actualizado کو revisores normativos/de cumplimiento کے ساتھ compartir کریں اور anonimato superpuesto سے باہر چلانے کی aprobaciones capturar کریں۔
4. **Ejecución en seco:** conjunto de pruebas de cumplimiento چلائیں اور staging buscar proveedores confiables Torii کے خلاف کریں۔ Resultados del marcador اور Archivo de resúmenes CLI کریں۔
5. **Corte de producción:** anuncio de cambio de ventana کریں، `transport_policy` کو `direct_only` پر flip کریں (اگر `soranet-first` منتخب کیا تھا) اور monitor de paneles de modo directo کریں (latencia `sorafs_fetch`, contadores de fallas del proveedor) ۔ documento del plan de reversión کریں تاکہ SNNet-4/5/5a/5b/6a/7/8/12/13 `roadmap.md:532` میں graduado ہونے پر SoraNet-first واپس جا سکیں۔
6. **Revisión posterior al cambio:** instantáneas del marcador, obtener resúmenes, resultados de monitoreo, ticket de cambio, adjuntar `status.md` Fecha de entrada en vigor y actualización de anomalías کریں۔Lista de verificación کو `sorafs_node_ops` runbook کے ساتھ رکھیں تاکہ operadores interruptor en vivo سے پہلے ensayo de flujo de trabajo کر سکیں۔ جب SNNet-5 GA تک پہنچے تو producción telemetría میں paridad confirmar کرنے کے بعد reserva retirar کریں۔

## 6. Evidencia y requisitos de la puerta de adopción

Capturas en modo directo کو ابھی بھی La puerta de adopción SF-6c satisface کرنا ہوتا ہے۔ ہر ejecutar کے لیے marcador, resumen, sobre de manifiesto اور paquete de informes de adopción کریں تاکہ `cargo xtask sorafs-adoption-check` validar postura alternativa کر سکے۔ Puerta de campos faltantes کو falla کرا دیتے ہیں، اس لیے cambiar tickets میں registro de metadatos esperado کریں۔- **Metadatos de transporte:** `scoreboard.json` کو `transport_policy="direct_only"` declarar کرنا چاہیے (اور جب downgrade force ہو تو `transport_policy_override=true` flip کریں)۔ Campos de política de anonimato کو emparejados رکھیں چاہے وہ valores predeterminados heredan کریں، تاکہ revisores دیکھ سکیں کہ plan de anonimato por etapas سے desviación ہوا یا نہیں۔
- **Contadores de proveedores:** Sesiones de solo puerta de enlace, `provider_count=0` persisten, `gateway_provider_count=<n>` y Torii, proveedores que rellenan JSON کو ہاتھ سے editar نہ کریں: Los recuentos de CLI/SDK derivan کرتا ہے اور puerta de adopción y capturas rechazadas کرتا ہے جن میں falta división ہو۔
- **Evidencia manifiesta:** Las puertas de enlace Torii están firmadas y firmadas `--gateway-manifest-envelope <path>` (equivalente a SDK) pasan کریں تاکہ `gateway_manifest_provided` کے ساتھ `gateway_manifest_id`/`gateway_manifest_cid` `scoreboard.json` Registro de میں ہوں۔ یقینی بنائیں کہ `summary.json` میں وہی `manifest_id`/`manifest_cid` موجود ہوں؛ کسی بھی فائل میں par نہ ہو تو verificación de adopción fallida ہو جائے گا۔
- **Expectativas de telemetría:** جب captura de telemetría کے ساتھ ہو تو gate کو `--require-telemetry` کے ساتھ چلائیں تاکہ las métricas del informe de adopción emiten ہونے کا ثبوت دے۔ Ensayos con espacio de aire میں bandera omitir کیا جا سکتا ہے، مگر CI اور cambiar boletos کو documento de ausencia کرنا چاہیے۔

Ejemplo:

```bash
cargo xtask sorafs-adoption-check \
  --scoreboard fetch_state/direct_mode_scoreboard.json \
  --summary fetch_state/direct_mode_summary.json \
  --allow-single-source \
  --require-direct-only \
  --json-out artifacts/sorafs_direct_mode/adoption_report.json \
  --require-telemetry
````adoption_report.json` کو marcador, resumen, sobre de manifiesto اور paquete de troncos de humo کے ساتھ adjuntar کریں۔ یہ artefactos Trabajo de adopción de CI (`ci/check_sorafs_orchestrator_adoption.sh`) کی aplicación کو espejo کرتے ہیں اور degradaciones de modo directo کو auditable رکھتے ہیں۔