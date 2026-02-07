---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/reports/sf6-security-review.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: SF-6 سیکیورٹی ریویو
resumen: firma sin llave, transmisión de pruebas, manifiestos, canalizaciones, seguimiento
---

# SF-6 سیکیورٹی ریویو

**Ventana de evaluación:** 2026-02-10 → 2026-02-18  
**Revisión líder:** Gremio de ingeniería de seguridad (`@sec-eng`), Grupo de trabajo de herramientas (`@tooling-wg`)  
**Alcance:** SoraFS CLI/SDK (`sorafs_cli`, `sorafs_car`, `sorafs_manifest`), API de transmisión de prueba, manejo de manifiestos Torii, Integración Sigstore/OIDC, ganchos de liberación CI ۔  
**Artefactos:**  
- Fuente CLI y pruebas (`crates/sorafs_car/src/bin/sorafs_cli.rs`)  
- Manejadores de manifiesto/prueba Torii (`crates/iroha_torii/src/sorafs/api.rs`)  
- Liberación de automatización (`ci/check_sorafs_cli_release.sh`, `scripts/release_sorafs_cli.sh`)  
- Arnés de paridad determinista (`crates/sorafs_car/tests/sorafs_cli.rs`, [SoraFS Informe de paridad GA de Orchestrator](./orchestrator-ga-parity.md))

## Metodología1. **Talleres de modelado de amenazas** نے estaciones de trabajo para desarrolladores, sistemas CI اور Torii nodos کے لیے mapa de capacidades del atacante کیں۔  
2. **Revisión de código** ے superficies de credenciales (intercambio de tokens OIDC, firma sin llave), validación del manifiesto Norito y contrapresión de transmisión de prueba پر فوکس کیا۔  
3. **Pruebas dinámicas** Repetición de manifiestos de dispositivos, modos de falla que simulan (repetición de token, manipulación de manifiesto, secuencias de prueba truncadas), arnés de paridad y unidades fuzz personalizadas.  
4. **Inspección de configuración** con valores predeterminados de `iroha_config`, manejo de indicadores CLI y scripts de lanzamiento, validación, determinismo, ejecuciones auditables, etc.  
5. **Entrevista de proceso** ے flujo de remediación, rutas de escalada اور captura de evidencia de auditoría کو Tooling WG کے propietarios de la versión کے ساتھ confirmar کیا۔

## Resumen de hallazgos| identificación | Gravedad | Área | Encontrar | Resolución |
|----|----------|------|---------|------------|
| SF6-SR-01 | Alto | Firma sin llave | OIDC Audiencia de token valores predeterminados Plantillas de CI Implícito Reproducción entre inquilinos Reproducción entre inquilinos | ganchos de liberación اور plantillas CI میں `--identity-token-audience` کی aplicación explícita شامل کی گئی ([proceso de liberación](../developer-releases.md), `docs/examples/sorafs_ci.md`). audiencia omite ہونے پر CI اب falla ہوتا ہے۔ |
| SF6-SR-02 | Medio | Transmisión de prueba | Rutas de contrapresión نے buffers de suscriptor ilimitados قبول کیے، جس سے agotamiento de la memoria ممکن تھی۔ | Los tamaños de canal limitados `sorafs_cli proof stream` imponen کرتا ہے، truncamiento determinista کے ساتھ Registro de resúmenes Norito کرتا ہے اور interrupción de transmisión Torii fragmentos de respuesta de espejo کود کرنے کے لیے اپڈیٹ کیا گیا (`crates/iroha_torii/src/sorafs/api.rs`). |
| SF6-SR-03 | Medio | Presentación manifiesta | CLI no manifiesta قبول کیے بغیر planes de fragmentos integrados verifica کیے جب `--plan` موجود نہ تھا۔ | `sorafs_cli manifest submit` اب CAR digests دوبارہ calcular اور comparar کرتا ہے جب تک `--expect-plan-digest` دیا نہ جائے، desajustes rechazar کرتا ہے اور sugerencias de corrección دکھاتا ہے۔ Los casos de éxito/fracaso de las pruebas cubren کرتے ہیں (`crates/sorafs_car/tests/sorafs_cli.rs`). || SF6-SR-04 | Bajo | Pista de auditoría | Lista de verificación de lanzamiento میں سیکیورٹی ریویو کے لیے registro de aprobación firmado شامل نہیں تھا۔ | [proceso de liberación](../developer-releases.md) میں سیکشن شامل کیا گیا جو revisar hashes de notas اور URL del ticket de cierre کو GA سے پہلے adjuntar کرنے کا تقاضا کرتا ہے۔ |

تمام ventana de revisión de hallazgos altos/medios کے دوران arreglar ہوئیں اور موجود paridad arnés سے validar ہوئیں۔ کوئی problemas críticos latentes باقی نہیں۔

## Validación de controles- **Alcance de la credencial:** Plantillas de CI predeterminadas para audiencia explícita y afirmaciones del emisor. CLI اور Release Helper `--identity-token-audience` کے بغیر `--identity-token-provider` کے falla rápidamente ہوتے ہیں۔  
- **Repetición determinista:** Las pruebas actualizadas cubren los flujos de envío de manifiestos positivos/negativos کرتے ہیں، یہ یقینی بناتے ہیں کہ resúmenes no coincidentes fallas no deterministas رہیں اور network کو touch کرنے سے پہلے superficie ہوں۔  
- **Prueba de contrapresión de transmisión:** Torii Elementos PoR/PoTR, canales limitados, secuencias de flujo, CLI, muestras de latencia truncadas + ejemplos de fallas, crecimiento ilimitado de suscriptores. ہے جبکہ resúmenes deterministas برقرار رکھتا ہے۔  
- **Observabilidad:** Contadores de transmisión de prueba (`torii_sorafs_proof_stream_*`) Resúmenes de CLI abortan la captura de motivos کرتے ہیں، جس سے operadores کے لیے auditoría de migas de pan ملتے ہیں۔  
- **Documentación:** Guías para desarrolladores ([índice de desarrolladores](../developer-index.md), [referencia CLI](../developer-cli.md)), indicadores de seguridad sensibles y flujos de trabajo de escalada کو واضح کرتے ہیں۔

## Adiciones a la lista de verificación de lanzamiento

Los gerentes de publicación کو GA candidato promueven کرتے وقت درج ذیل evidencia **لازمی** adjuntar کرنا ہوگا:1. تازہ ترین سیکیورٹی ریویو memo کا hash (یہ دستاویز)۔  
2. ticket de remediación rastreado کا enlace (مثال: `governance/tickets/SF6-SR-2026.md`)۔  
3. `scripts/release_sorafs_cli.sh --manifest ... --bundle-out ... --signature-out ...` کا salida جس میں argumentos explícitos de audiencia/emisor دکھیں۔  
4. arnés de paridad کے registros capturados (`cargo test -p sorafs_car -- --nocapture sorafs_cli::proof_stream::bounded_channels`) ۔  
5. تصدیق کہ Torii notas de la versión میں contadores de telemetría de transmisión de prueba limitada شامل ہیں۔

اوپر دیے گئے artefactos جمع نہ کرنا Aprobación de GA کو روکتا ہے۔

**Hashes de artefactos de referencia (aprobación del 20 de febrero de 2026):**

- `sf6_security_review.md` — `66001d0b53d8e7ed5951a07453121c075dea931ca44c11f1fcd1571ed827342a`

## Seguimientos destacados

- **Actualización del modelo de amenazas:** یہ ریویو ہر سہ ماہی یا بڑے Adiciones de indicadores CLI سے پہلے دوبارہ کریں۔  
- **Cobertura de fuzzing:** Codificaciones de transporte de transmisión de prueba کو `fuzz/proof_stream_transport` کے ذریعے fuzz کیا جاتا ہے، جو identity, gzip, deflate اور zstd payloads کو cover کرتا ہے۔  
- **Ensayo de incidente:** compromiso simbólico اور reversión del manifiesto کو simular کرنے والی ejercicio del operador شیڈول کریں، تاکہ documentos میں procedimientos practicados reflejan ہوں۔

## Aprobación

- Representante del Gremio de Ingeniería de Seguridad: @sec-eng (2026-02-20)  
- Representante del Grupo de Trabajo sobre Herramientas: @tooling-wg (2026-02-20)

Aprobaciones firmadas کو paquete de artefactos de lanzamiento کے ساتھ محفوظ کریں۔