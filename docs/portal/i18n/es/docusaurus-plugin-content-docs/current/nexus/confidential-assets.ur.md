---
lang: es
direction: ltr
source: docs/portal/docs/nexus/confidential-assets.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: Activos confidenciales اور Transferencias ZK
descripción: circulación blindada, registros اور controles del operador کے لئے Fase C کا plano۔
slug: /nexus/activos-confidenciales
---
<!--
SPDX-License-Identifier: Apache-2.0
-->
# Activos confidenciales اور Diseño de transferencia ZK

## Motivación
- flujos de activos protegidos opt-in فراہم کرنا تاکہ dominios شفاف circulación بدلے بغیر privacidad transaccional برقرار رکھ سکیں۔
- auditores اور operadores کو circuitos اور parámetros criptográficos کے لئے controles del ciclo de vida (activación, rotación, revocación) دینا۔

## Modelo de amenaza
- Validadores honestos pero curiosos ہیں: consenso fiel چلاتے ہیں لیکن libro mayor/estado کو inspeccionar کرنے کی کوشش کرتے ہیں۔
- Los observadores de la red bloquean datos y transacciones chismes دیکھتے ہیں؛ canales de chismes privados کا کوئی suposición نہیں۔
- Fuera de alcance: análisis de tráfico fuera del libro mayor, adversarios cuánticos (hoja de ruta PQ میں علیحدہ ٹریک), ataques de disponibilidad del libro mayor۔## Descripción general del diseño
- Activos موجودہ saldos transparentes کے علاوہ *grupo protegido* declarar کر سکتے ہیں؛ compromisos criptográficos de circulación blindada سے representan ہوتی ہے۔
- Notas `(asset_id, amount, recipient_view_key, blinding, rho)` کو encapsular کرتی ہیں، ساتھ:
  - Compromiso: `Comm = Pedersen(params_id || asset_id || amount || recipient_view_key || blinding)`.
  - Nulificador: `Null = Poseidon(domain_sep || nk || rho || asset_id || chain_id)`, orden de notas سے independiente۔
  - Carga útil cifrada: `enc_payload = AEAD_XChaCha20Poly1305(ephemeral_shared_key, note_plaintext)`.
- Transacciones con cargas útiles `ConfidentialTransfer` codificadas con Norito لاتی ہیں جن میں:
  - Aportes públicos: ancla Merkle, anuladores, nuevos compromisos, identificación de activos, versión del circuito.
  - Destinatarios y auditores opcionales para cargas útiles cifradas.
  - Prueba de conocimiento cero, conservación del valor, propiedad, autorización, certificación کرتی ہے۔
- Verificación de claves اور conjuntos de parámetros en registros del libro mayor کے ذریعے ventanas de activación کے ساتھ کنٹرول ہوتے ہیں؛ nodos desconocidos یا entradas revocadas کو referir کرنے والی pruebas validar کرنے سے انکار کرتے ہیں۔
- Encabezados de consenso resumen de funciones confidenciales activas پر confirmar کرتے ہیں تاکہ bloques صرف اسی وقت aceptar ہوں جب registro اور coincidencia de estado de parámetros کرے۔
- Construcción de prueba de pila Halo2 (Plonkish) استعمال کرتی ہے بغیر configuración confiable؛ Groth16 یا دیگر SNARK variantes v1 میں دانستہ طور پر no compatible ہیں۔

### Accesorios deterministasSobres para notas confidenciales اب `fixtures/confidential/encrypted_payload_v1.json` میں accesorio canónico کے ساتھ آتے ہیں۔ یہ conjunto de datos ایک مثبت v1 اور منفی muestras mal formadas پکڑتا ہے تاکہ SDK paridad de análisis ثابت کر سکیں۔ Pruebas de modelo de datos de Rust (`crates/iroha_data_model/tests/confidential_encrypted_payload_vectors.rs`) y Swift suite (`IrohaSwift/Tests/IrohaSwiftTests/ConfidentialEncryptedPayloadTests.swift`) accesorio para codificación Norito, superficies de error y códec de cobertura de regresión کے ارتقا کے ساتھ alineado رہتے ہیں۔

Los SDK de Swift incluyen pegamento JSON personalizado, emiten instrucciones de protección, incluyen compromiso de nota de 32 bytes, carga útil cifrada y metadatos de débito, incluye `ShieldRequest`. `/v1/pipeline/transactions` پر signo اور relé کرنے کے لئے `IrohaSDK.submit(shield:keypair:)` (یا `submitAndWait`) کال کریں۔ Longitudes de compromiso de ayuda validar کرتا ہے، `ConfidentialEncryptedPayload` کو Norito codificador میں hilo کرتا ہے، اور نیچے بیان کردہ `zk::Shield` diseño کو espejo کرتا ہے تاکہ billeteras Rust کے ساتھ lock-step رہیں۔## Compromisos de consenso y control de capacidades
- Los encabezados de bloque `conf_features = { vk_set_hash, poseidon_params_id, pedersen_params_id, conf_rules_version }` exponen کرتے ہیں؛ digerir hash de consenso میں حصہ لیتا ہے اور aceptación de bloque کے لئے vista de registro local سے coincidencia ہونا چاہئے۔
- Gobernanza مستقبل کے `activation_height` کے ساتھ `next_conf_features` پروگرام کر کے etapa de actualizaciones کر سکتی ہے؛ اس altura تک productores de bloques پچھلا resumen emitir کرتے رہتے ہیں۔
- Los nodos validadores کو `confidential.enabled = true` اور `assume_valid = false` کے ساتھ operan کرنا DEBEN ہے۔ El inicio verifica el conjunto del validador unirse کرنے سے انکار کرتے ہیں اگر کوئی شرط fail ہو یا local `conf_features` diverge ہو۔
- Metadatos del protocolo de enlace P2P اب `{ enabled, assume_valid, conf_features }` شامل کرتا ہے۔ funciones no compatibles anuncian pares `HandshakeConfidentialMismatch` کے ساتھ rechazan ہوتے ہیں اور rotación de consenso میں داخل نہیں ہوتے۔
- Observadores no validadores `assume_valid = true` سیٹ کر سکتے ہیں؛ وہ deltas confidenciales کو aplicar ciegamente کرتے ہیں مگر seguridad de consenso پر اثر نہیں ڈالتے۔## Políticas de activos
- ہر definición de activos میں creador یا gobernanza کی طرف سے conjunto کیا گیا `AssetConfidentialPolicy` ہوتا ہے:
  - `TransparentOnly`: modo predeterminado؛ صرف instrucciones transparentes (`MintAsset`, `TransferAsset` وغیرہ) permitidas ہیں اور operaciones blindadas rechazan ہوتے ہیں۔
  - `ShieldedOnly`: تمام emisión اور transferencias کو instrucciones confidenciales استعمال کرنا ہوں گے؛ `RevealConfidential` ممنوع ہے تاکہ balanzas عوامی نہ ہوں۔
  - `Convertible`: titulares نیچے بیان کردہ instrucciones de entrada/salida کے ذریعے transparente اور representaciones blindadas کے درمیان valor movimiento کر سکتے ہیں۔
- Las políticas restringidas del FSM siguen a کرتی ہیں تاکہ fondos varados نہ ہوں:
  - `TransparentOnly → Convertible` (piscina blindada habilitada)
  - `TransparentOnly → ShieldedOnly` (transición pendiente a ventana de conversión درکار)
  - `Convertible → ShieldedOnly` (retraso mínimo لازم)
  - `ShieldedOnly → Convertible` (plan de migración درکار تاکہ billetes blindados gastables رہیں)
  - `ShieldedOnly → TransparentOnly` no permitido ہے جب تک grupo protegido vacío نہ ہو یا notas pendientes de gobernanza کو unshield کرنے والی codificación de migración نہ کرے۔
- Instrucciones de gobierno `ScheduleConfidentialPolicyTransition` ISI کے ذریعے `pending_transition { new_mode, effective_height, previous_mode, transition_id, conversion_window }` set کرتی ہیں اور `CancelConfidentialPolicyTransition` سے cambios programados abortar کر سکتی ہیں۔ Validación de Mempool یقینی بناتی ہے کہ کوئی altura de transición de transacción کو straddle نہ کرے اور cambio de verificación de política a mitad de bloque ہونے پر inclusión determinista طور پر falla ہو۔- Transiciones pendientes نئے bloque کے شروع میں خودکار aplicar ہوتے ہیں: جب ventana de conversión de altura de bloque میں داخل ہو (ShieldedOnly actualizaciones کے لئے) یا `effective_height` پہنچے تو runtime `AssetConfidentialPolicy` actualizar کرتا ہے، `zk.policy` actualización de metadatos کرتا ہے اور entrada pendiente borrar کرتا ہے۔ اگر `ShieldedOnly` transición madura ہونے پر suministro transparente باقی ہو تو cancelación de cambio de tiempo de ejecución کر کے registro de advertencia کرتا ہے اور modo anterior برقرار رہتا ہے۔
- Perillas de configuración `policy_transition_delay_blocks` اور `policy_transition_window_blocks` aviso mínimo اور períodos de gracia hacer cumplir کرتے ہیں تاکہ billeteras cambiar کے آس پاس notas convertir کر سکیں۔
- Identificador de auditoría `pending_transition.transition_id` کے طور پر بھی کام کرتا ہے؛ gobernanza کو transiciones finalizar یا cancelar کرتے وقت اسے citar کرنا چاہئے تاکہ operadores informes de rampa de entrada/salida correlacionar کر سکیں۔
- `policy_transition_window_blocks` predeterminado 720 ہے (tiempo de bloqueo de 60 segundos پر تقریباً 12 گھنٹے)۔ Solicitudes de gobernanza de nodos جو aviso más corto چاہیں انہیں abrazadera کرتے ہیں۔
- Génesis manifiesta los flujos CLI actuales y las políticas pendientes exponen کرتے ہیں۔ Tiempo de ejecución de la lógica de admisión پر política پڑھ کر confirmar کرتی ہے کہ ہر instrucción confidencial autorizada ہے۔
- Lista de verificación de migración: نیچے “Secuenciación de migración” میں Milestone M0 کے مطابق plan de actualización por etapas دیکھیں۔

#### Torii کے ذریعے transiciones کی monitoreoCarteras اور auditors `GET /v1/confidential/assets/{definition_id}/transitions` کو encuesta کر کے activo `AssetConfidentialPolicy` دیکھتے ہیں۔ Carga útil JSON id de activo canónico, última altura de bloque observada, `current_mode`, altura en modo efectivo (ventanas de conversión según el informe `Convertible` کرتے ہیں), se esperaba Identificadores `vk_set_hash`/Poseidón/Pedersen شامل کرتا ہے۔ جب transición de gobernanza pendiente ہو تو respuesta میں یہ بھی ہوتا ہے:

- `transition_id` - `ScheduleConfidentialPolicyTransition` سے واپس ملنے والا auditoría handle۔
- `previous_mode`/`new_mode`.
- `effective_height`.
- `conversion_window` o `window_open_height` derivado (y billeteras de bloque y conversión de corte ShieldedOnly a través de conversión)

Respuesta de ejemplo:

```json
{
  "asset_id": "62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
  "block_height": 4217,
  "current_mode": "Convertible",
  "effective_mode": "Convertible",
  "vk_set_hash": "8D7A4B0A95AB1C33F04944F5D332F9A829CEB10FB0D0797E2D25AEFBAAF1155D",
  "poseidon_params_id": 7,
  "pedersen_params_id": 11,
  "pending_transition": {
    "transition_id": "BF2C6F9A4E9DF389B6F7E5E6B5487B39AE00D2A4B7C0FBF2C9FEF6D0A961C8ED",
    "previous_mode": "Convertible",
    "new_mode": "ShieldedOnly",
    "effective_height": 5000,
    "conversion_window": 720,
    "window_open_height": 4280
  }
}
```

Respuesta `404` کا مطلب ہے کہ definición de activo coincidente موجود نہیں۔ جب کوئی transición programada نہ ہو تو `pending_transition` campo `null` ہوتا ہے۔

### Máquina de estado de políticas| Modo actual | Modo siguiente | Requisitos previos | Manejo de altura efectiva | Notas |
|--------------------|------------------|-------------------------------------------------------------------------------|-------------------------------------------------------------------------------------------------------------------|-------------------------------------------------------------------------------------|
| Sólo transparente | Descapotable | Las entradas del registro de parámetros/verificador de gobernanza نے activan کئے ہوں۔ `ScheduleConfidentialPolicyTransition` enviar کریں جس میں `effective_height ≥ current_height + policy_transition_delay_blocks` ہو۔ | Transición بالکل `effective_height` پر ejecutar ہوتا ہے؛ piscina protegida فوراً دستیاب ہوتا ہے۔                   | شفاف flujos برقرار رکھتے ہوئے confidencialidad habilitar کرنے کا ruta predeterminada۔               |
| Sólo transparente | Sólo blindado | اوپر والی شرائط کے ساتھ `policy_transition_window_blocks ≥ 1` بھی۔                                                         | Runtime `effective_height - policy_transition_window_blocks` پر `Convertible` میں داخل ہوتا ہے؛ `effective_height` پر `ShieldedOnly` میں بدلتا ہے۔ | شفاف las instrucciones desactivan ہونے سے پہلے ventana de conversión determinista دیتا ہے۔   || Descapotable | Sólo blindado | `effective_height ≥ current_height + policy_transition_delay_blocks` کے ساتھ transición programada۔ La gobernanza DEBE auditar los metadatos کے ذریعے (`transparent_supply == 0`) certificar کرے؛ corte de tiempo de ejecución پر aplicar کرتا ہے۔ | اوپر جیسی semántica de ventanas۔ اگر `effective_height` پر suministro transparente distinto de cero ہو تو transición `PolicyTransitionPrerequisiteFailed` کے ساتھ abortar ہو جاتا ہے۔ | activo کو مکمل circulación confidencial میں bloqueo کرتا ہے۔                                     |
| Sólo blindado | Descapotable | Transición programada؛ کوئی retiro de emergencia activo نہیں (`withdraw_height` desarmado) ۔                                    | `effective_height` cambio de estado ہوتا ہے؛ revelar rampas دوبارہ کھلتی ہیں جبکہ billetes blindados válidos رہتے ہیں۔                           | ventanas de mantenimiento یا revisiones del auditor کے لئے۔                                          |
| Sólo blindado | Sólo transparente | Gobernanza کو `shielded_supply == 0` ثابت کرنا ہوگا یا firmado `EmergencyUnshield` etapa del plan کرنا ہوگا (firmas del auditor درکار)۔ | Runtime `effective_height` سے پہلے `Convertible` ventana کھولتا ہے؛ اس altura پر instrucciones confidenciales hard-fail ہو جاتی ہیں اور activo modo solo transparente میں واپس جاتا ہے۔ | salida de último recurso۔ اگر ventana کے دوران کوئی gasto de notas confidenciales ہو تو transición cancelación automática ہو جاتا ہے۔ || Cualquiera | Igual que la actual | `CancelConfidentialPolicyTransition` pendiente de cambio claro کرتا ہے۔                                                        | `pending_transition` فوراً eliminar ہوتا ہے۔                                                                          | status quo integridad کیلئے شامل۔                                             |

اوپر lista نہ ہونے والے transiciones gobernanza presentación پر rechazar ہوتے ہیں۔ Se aplica la transición programada en tiempo de ejecución کرنے سے پہلے verificación de requisitos previos کرتا ہے؛ activo fallido کو modo anterior میں واپس دھکیلتی ہے اور telemetría/bloqueo de eventos کے ذریعے `PolicyTransitionPrerequisiteFailed` emite کرتی ہے۔

### Secuenciación de migración2. **Preparar la transición:** `policy_transition_delay_blocks` کو respetar کرنے والے `effective_height` کے ساتھ `ScheduleConfidentialPolicyTransition` enviar کریں۔ `ShieldedOnly` کی طرف جاتے وقت ventana de conversión especificar کریں (`window ≥ policy_transition_window_blocks`)۔
3. **Publicar guía del operador:** واپس ملنے والا `transition_id` registro کریں اور runbook de entrada/salida circular کریں۔ Carteras اور auditores `/v1/confidential/assets/{id}/transitions` suscribirse کر کے ventana altura abierta جانتے ہیں۔
4. **Aplicación de ventanas:** política de tiempo de ejecución de ventana کھلتے ہی `Convertible` پر switch کرتا ہے، `PolicyTransitionWindowOpened { transition_id }` emite کرتا ہے، اور solicitudes de gobernanza en conflicto rechazan کرنا شروع کرتا ہے۔
5. **Finalizar o cancelar:** `effective_height` پر requisitos previos de transición de tiempo de ejecución verificar کرتا ہے (suministro transparente صفر، retiro de emergencia نہیں وغیرہ)۔ Política de éxito کو modo solicitado پر flip کرتا ہے؛ falla `PolicyTransitionPrerequisiteFailed` emitir کر کے pendiente de transición borrar کرتا ہے اور política sin cambios رہتی ہے۔
6. **Actualizaciones de esquema:** Transición de کے بعد Aumento de la versión del esquema de activos de gobernanza کرتی ہے (مثلاً `asset_definition.v2`) اور Manifest de herramientas CLI serializar کرتے y `confidential_policy` requieren کرتا ہے۔ Los operadores de documentos de actualización de Genesis, los validadores reinician, la configuración de políticas y las huellas digitales del registro agregan کرنے کی ہدایت دیتے ہیں۔Confidencialidad habilitada کے ساتھ شروع ہونے والی نئی redes política deseada کو genesis میں codificar کرتی ہیں۔ پھر بھی iniciar کے بعد cambio de modo کرتے وقت اوپر والی lista de verificación seguir کی جاتی ہے تاکہ ventanas de conversión deterministas رہیں اور billeteras کے پاس ajustar کرنے کا وقت ہو۔

### Activación y control de versiones del manifiesto Norito- Los manifiestos de Génesis DEBEN `confidential_registry_root` clave personalizada کیلئے `SetParameter` incluir کریں۔ Carga útil Norito JSON ہے جو `ConfidentialRegistryMeta { vk_set_hash: Option<String> }` سے coincide کرتا ہے: جب کوئی entrada del verificador فعال نہ ہو تو campo omitir کریں (`null`) ، ورنہ Cadena hexadecimal de 32 bytes (`0x…`) دیں جو manifiesto میں موجود instrucciones del verificador پر `compute_vk_set_hash` کے hash کے برابر ہو۔ Falta el parámetro یا no coincide el hash ہونے پر inicio de nodos سے انکار کرتے ہیں۔
- Versión de diseño de manifiesto `ConfidentialFeatureDigest::conf_rules_version` en línea incrustada کرتا ہے۔ Redes v1 کیلئے اسے `Some(1)` رہنا MUST ہے اور یہ `iroha_config::parameters::defaults::confidential::RULES_VERSION` کے برابر ہے۔ El conjunto de reglas evoluciona, aumenta constantemente, los manifiestos se regeneran, los binarios, se bloquean, se implementan versiones mezclan کرنے سے validadores `ConfidentialFeatureDigestMismatch` کے ساتھ bloques rechazan کرتے ہیں۔
- Los manifiestos de activación, las actualizaciones del registro, los cambios en el ciclo de vida de los parámetros y el paquete de transiciones de políticas DEBEN digerir de forma coherente:
  1. Mutaciones de registro planificadas (`Publish*`, `Set*Lifecycle`) کو vista de estado fuera de línea میں aplicar کریں اور `compute_confidential_feature_digest` سے cálculo de resumen posterior a la activación کریں۔
  2. Hash calculado کے ساتھ `SetParameter::custom(confidential_registry_root, {"vk_set_hash": "0x…"})` emite کریں تاکہ pares rezagados instrucciones de registro intermedio falta کرنے کے باوجود resumen correcto recuperar کر سکیں۔
  3. Las instrucciones `ScheduleConfidentialPolicyTransition` añaden کریں۔ ہر instrucción کو cita `transition_id` emitida por el gobierno کرنا لازم ہے؛ اسے بھولنے والے manifiesta el rechazo del tiempo de ejecución کرتا ہے۔4. Bytes de manifiesto, huella digital SHA-256 y plan de activación میں استعمال ہونے والا resumen محفوظ کریں۔ Los operadores تینوں artefactos verifican کر کے ہی votan دیتے ہیں تاکہ particiones سے بچا جا سکے۔
- جب lanzamiento کیلئے corte diferido درکار ہو، altura del objetivo کو parámetro personalizado complementario میں registro کریں (مثلاً `custom.confidential_upgrade_activation_height`) ۔ یہ auditores کو Norito prueba codificada دیتا ہے کہ validadores نے resumen de cambios سے پہلے ventana de aviso honor کیا۔## Ciclo de vida del verificador y de los parámetros
### Registro ZK
- Libro mayor `ZkVerifierEntry { vk_id, circuit_id, version, proving_system, curve, public_inputs_schema_hash, vk_hash, vk_len, max_proof_bytes, gas_schedule_id, activation_height, deprecation_height, withdraw_height, status, metadata_uri_cid, vk_bytes_cid }` اسٹور کرتا ہے جہاں `proving_system` فی الحال `Halo2` پر fijo ہے۔
- `(circuit_id, version)` pares globalmente únicos ہیں؛ metadatos del circuito de registro کے búsqueda کیلئے índice secundario رکھتی ہے۔ Par duplicado رجسٹر کرنے کی کوشش admisión پر rechazar ہوتی ہے۔
- `circuit_id` no vacío ہونا چاہئے اور `public_inputs_schema_hash` فراہم کرنا لازم ہے (عام طور پر verifier کے codificación canónica de entrada pública کا Blake2b-32 hash)۔ Admisión ان فیلڈز کے بغیر registros rechazados کرتا ہے۔
- Instrucciones de gobernanza میں شامل ہیں:
  - `PUBLISH`, entrada `Proposed` de solo metadatos agregar کرنے کیلئے۔
  - `ACTIVATE { vk_id, activation_height }`, límite de época پر programa de activación کرنے کیلئے۔
  - `DEPRECATE { vk_id, deprecation_height }`، آخری marca de altura کرنے کیلئے جہاں entrada de pruebas کو referencia کر سکیں۔
  - `WITHDRAW { vk_id, withdraw_height }`, parada de emergencia کیلئے؛ متاثرہ altura de retiro de activos کے بعد congelación de gastos confidenciales کرتے ہیں جب تک نئی entradas activadas نہ ہوں۔
- Génesis manifiesta خودکار طور پر `confidential_registry_root` parámetro personalizado emitir کرتے ہیں جس کا `vk_set_hash` entradas activas سے coincidencia کرتا ہے؛ nodo de validación کے unión de consenso سے پہلے resumen کو estado del registro local کے خلاف verificación cruzada کرتی ہے۔- Registro del verificador یا actualización کیلئے `gas_schedule_id` ضروری ہے؛ verificación aplicar کرتی ہے کہ entrada de registro `Active` ہو، `(circuit_id, version)` índice میں ہو، اور Pruebas de Halo2 میں `OpenVerifyEnvelope` ہو جس کا `circuit_id`, `vk_hash`, y `public_inputs_schema_hash` registro de registro سے coincide con کرے۔

### Claves de demostración
- Prueba de claves fuera del libro mayor رہتے ہیں مگر identificadores de direcciones de contenido (`pk_cid`, `pk_hash`, `pk_len`) کے ذریعے refer کیے جاتے ہیں جو metadatos del verificador کے ساتھ publicar ہوتے ہیں۔
- SDK de billetera PK recuperación de datos کر کے hashes verificar کرتے ہیں اور caché local میں رکھتے ہیں۔

### Parámetros de Pedersen y Poseidón
- Registros (`PedersenParams`, `PoseidonParams`) controles del ciclo de vida del verificador reflejan کرتی ہیں، ہر ایک میں `params_id`, generadores/constantes کے hashes, alturas de activación/desaprobación/retirada ہوتے ہیں۔## Ordenamiento determinista y anuladores
- ہر activo `CommitmentTree` رکھتا ہے جس میں `next_leaf_index` ہوتا ہے؛ bloquea compromisos orden determinista میں agregar کرتے ہیں: bloquear orden میں transacciones iterar کریں؛ ہر transacción کے اندر serializado `output_idx` ascendente میں salidas blindadas iterar کریں۔
- `note_position` compensaciones de árbol سے derivar ہوتا ہے مگر anulador کا حصہ **نہیں**؛ یہ صرف testigo de prueba میں rutas de membresía کے لئے استعمال ہوتا ہے۔
- Reorgs کے تحت anulador estabilidad PRF diseño سے garantía ہوتی ہے؛ Entrada PRF `{ nk, note_preimage_hash, asset_id, chain_id, params_id }` enlace کرتا ہے، اور anclajes تاریخی Merkle raíces کو referencia کرتے ہیں جو `max_anchor_age_blocks` سے محدود ہیں۔## Flujo del libro mayor
1. **MintConfidential {id_activo, monto, sugerencia_destinatario}**
   - `Convertible` یا `ShieldedOnly` política de activos درکار؛ admisión control de autoridad de activos کرتا ہے، actual `params_id` recuperar کرتا ہے، `rho` muestra کرتا ہے، compromiso emitir کرتا ہے، Actualización del árbol Merkle کرتا ہے۔
   - `ConfidentialEvent::Shielded` emite کرتا ہے جس میں nuevo compromiso, Merkle root delta اور audit trail کیلئے transacción llamada hash ہوتا ہے۔
2. **TransferConfidential {id_activo, prueba, id_circuito, versión, anuladores, nuevos_compromisos, enc_payloads, raíz_ancla, memo}**
   - Entrada de registro de llamada al sistema de VM سے verificación de prueba کرتا ہے؛ host یقینی بناتا ہے کہ anuladores no utilizados ہوں، compromisos deterministas طور پر anexar ہوں، اور ancla reciente ہو۔
   - Registro de entradas del libro mayor `NullifierSet`, destinatarios/auditores, almacenamiento de cargas útiles cifradas, anulación de salidas ordenadas, hash de prueba, اور Las raíces de Merkle resumen کرتا ہے۔
3. **RevelarConfidencial {id_activo, prueba, id_circuito, versión, anulador, monto, cuenta_destinatario, raíz_ancla}**
   - Activos `Convertible` کیلئے؛ prueba validar کرتا ہے کہ valor de la nota cantidad revelada کے برابر ہے، libro mayor saldo transparente crédito کرتا ہے، اور anulador marca gastada کر کے quema de nota protegida کرتا ہے۔- `ConfidentialEvent::Unshielded` emite کرتا ہے جس میں monto público, anuladores consumidos, identificadores de prueba y hash de llamada de transacción شامل ہیں۔## Adiciones al modelo de datos
- `ConfidentialConfig` (nueva sección de configuración) con indicador de habilitación, `assume_valid`, perillas de límite/gas, ventana de anclaje, backend del verificador.
- `ConfidentialNote`, `ConfidentialTransfer`, y `ConfidentialMint` Norito byte de versión explícita de esquemas (`CONFIDENTIAL_ASSET_V1 = 0x01`) کے ساتھ۔
- `ConfidentialEncryptedPayload` AEAD bytes de notas کو `{ version, ephemeral_pubkey, nonce, ciphertext }` میں wrap کرتا ہے، default `version = CONFIDENTIAL_ENCRYPTED_PAYLOAD_V1` XChaCha20-Poly1305 diseño کیلئے۔
- Vectores canónicos de derivación de claves `docs/source/confidential_key_vectors.json` میں ہیں؛ CLI اور Torii endpoint ان accesorios کے خلاف regresión کرتے ہیں۔
- `asset::AssetDefinition` کو `confidential_policy: AssetConfidentialPolicy { mode, vk_set_hash, poseidon_params_id, pedersen_params_id, pending_transition }` ملتا ہے۔
- Verificadores de transferencia/desprotección `ZkAssetState` کیلئے `(backend, name, commitment)` persisten el enlace کرتا ہے؛ ejecución ان pruebas کو rechazar کرتا ہے جن کا referenciado یا clave de verificación en línea compromiso registrado سے coincidencia نہ کرے۔
- `CommitmentTree` (por activo con puntos de control fronterizos), `NullifierSet` con clave de `(chain_id, asset_id, nullifier)`, `ZkVerifierEntry`, `PedersenParams`, `PoseidonParams` tienda de software del estado mundial ہوتے ہیں۔
- Detección temprana de duplicados de Mempool اور controles de antigüedad del anclaje کیلئے transitorio `NullifierIndex` اور `AnchorIndex` estructuras mantienen کرتا ہے۔
- Actualizaciones de esquema Norito میں entradas públicas کی ordenamiento canónico شامل ہے؛ Las pruebas de ida y vuelta que codifican el determinismo garantizan کرتے ہیں۔- Pruebas unitarias de ida y vuelta de carga útil cifradas (`crates/iroha_data_model/src/confidential.rs`) کے ذریعے lock ہیں۔ Seguimiento de los auditores de vectores de billetera کیلئے transcripciones AEAD canónicas adjuntas کریں گے۔ `norito.md` sobre کیلئے documento de encabezado en línea کرتا ہے۔

## IVM Integración y llamada al sistema
- La llamada al sistema `VERIFY_CONFIDENTIAL_PROOF` introduce کریں جو قبول کرتا ہے:
  - `circuit_id`, `version`, `scheme`, `public_inputs`, `proof` y resultante `ConfidentialStateDelta { asset_id, nullifiers, commitments, enc_payloads }`.
  - Registro Syscall سے carga de metadatos del verificador کرتا ہے، límites de tamaño/tiempo aplicar کرتا ہے، carga de gas determinista کرتا ہے، اور prueba de éxito پر ہی aplicación delta کرتا ہے۔
- Exposición del rasgo `ConfidentialLedger` de solo lectura del host کرتا ہے جو Instantáneas de raíz de Merkle اور recuperación del estado del anulador کرتا ہے؛ Ayudantes de ensamblaje de testigos de biblioteca Kotodama para validación de esquemas فراہم کرتی ہے۔
- Diseño de búfer de prueba de documentos Pointer-ABI y identificadores de registro y actualización de ہوئے ہیں۔## Negociación de capacidad de nodo
- Apretón de manos `feature_bits.confidential` کے ساتھ `ConfidentialFeatureDigest { vk_set_hash, poseidon_params_id, pedersen_params_id, conf_rules_version }` publicidad کرتا ہے۔ Participación del validador کیلئے `confidential.enabled=true`, `assume_valid=false`, identificadores de backend del verificador idénticos y resúmenes coincidentes ضروری ہیں؛ no coincide `HandshakeConfidentialMismatch` کے ساتھ el protocolo de enlace falla کرتے ہیں۔
- Configuración de nodos observadores کیلئے `assume_valid` soporte کرتا ہے: deshabilitado ہونے پر instrucciones confidenciales پر determinista `UnsupportedInstruction` آتا ہے بغیر pánico؛ habilitado ہونے پر pruebas de observadores verificar کئے بغیر se aplican deltas de estado کرتے ہیں۔
- Las transacciones confidenciales de Mempool rechazan کرتا ہے اگر capacidad local deshabilitada ہو۔ Gossip filtra a pares sin capacidades de comparación کو transacciones protegidas بھیجنے سے گریز کرتے ہیں جبکہ ID de verificador desconocidos کو límites de tamaño کے اندر ciego hacia adelante کرتے ہیں۔

### Revelar la política de poda y retención de anuladores

Libros de contabilidad confidenciales کو nota frescura ثابت کرنے اور auditorías impulsadas por la gobernanza repetición کرنے کیلئے کافی historial retener کرنی ہوتی ہے۔ `ConfidentialLedger` کی política predeterminada یہ ہے:- **Retención de anulador:** anuladores gastados کم از کم `730` دن (24 مہینے) altura de gasto کے بعد رکھیں، یا اگر regulador زیادہ مدت مانگے تو وہ۔ Operadores `confidential.retention.nullifier_days` کے ذریعے ventana extender کر سکتے ہیں۔ ventana de retención کے اندر anuladores Torii کے ذریعے consultable رہیں تاکہ auditores ausencia de doble gasto ثابت کر سکیں۔
- **Revelar poda:** revelaciones transparentes (`RevealConfidential`) متعلقہ compromisos de notas کو finalización de bloque کے فوراً بعد podar کرتے ہیں، مگر anulador consumido اوپر والی regla de retención کے تابع رہتا ہے۔ Eventos relacionados con revelación (`ConfidentialEvent::Unshielded`) cantidad pública, destinatario اور registro de hash de prueba کرتے ہیں تاکہ revelaciones históricas reconstruir کرنے کیلئے texto cifrado podado کی ضرورت نہ ہو۔
- **Puntos de control fronterizos:** fronteras de compromiso puntos de control rodantes رکھتے ہیں جو `max_anchor_age_blocks` اور ventana de retención میں سے بڑے کو cover کرتے ہیں۔ Nodos پرانے puntos de control صرف تب compacto کرتے ہیں جب intervalo کے تمام anuladores caducan ہو جائیں۔
- **Remediación de resumen obsoleto:** Deriva del resumen inactivo `HandshakeConfidentialMismatch` Operadores y (1) clúster Ventanas de retención de anulador alineadas (2) `iroha_cli app confidential verify-ledger` چلا کر conjunto de anulador retenido کے خلاف resumen regenerar کرنا چاہئے، اور (3) redistribución del manifiesto actualizado کرنا چاہئے۔ Anuladores eliminados prematuramente کو unión de red سے پہلے almacenamiento en frío سے restauración کرنا ہوگا۔Anulaciones locales del runbook de operaciones del documento کریں؛ ventana de retención Políticas de gobernanza Configuración de nodos Planes de almacenamiento de archivos Bloqueo Actualización Actualización

### Flujo de desalojo y recuperación

1. Marque کے دوران `IrohaNetwork` las capacidades anunciadas comparan کرتا ہے۔ no coincide `HandshakeConfidentialMismatch` aumentar کرتا ہے؛ conexión بند ہوتی ہے اور cola de descubrimiento de pares میں رہتا ہے بغیر `Ready` بنے۔
2. Registro de servicio de red fallido میں superficie ہوتی ہے (resumen remoto اور backend سمیت)، اور Sumeragi peer کو propuesta یا votación کیلئے cronograma نہیں کرتا۔
3. Los registros del verificador de operadores y los conjuntos de parámetros (`vk_set_hash`, `pedersen_params_id`, `poseidon_params_id`) se alinean con la etapa `activation_height` acordada con la etapa `next_conf_features`. کر کے remediación کرتے ہیں۔ Resumen de coincidencia ہوتے ہی اگلا apretón de manos خودکار طور پر tener éxito کرتا ہے۔
4. Par obsoleto, transmisión en bloque, (repetición de archivo, repetición), validadores, `BlockRejectionReason::ConfidentialFeatureDigestMismatch`, determinista, rechazo, rechazo. ہیں، جس سے red میں estado del libro mayor consistente رہتا ہے۔

### Flujo de protocolo de enlace seguro para reproducción1. ہر intento de salida nuevo Ruido/material clave X25519 asignar کرتا ہے۔ Carga útil de protocolo de enlace firmado (`handshake_signature_payload`) claves públicas efímeras locales y remotas, dirección de socket anunciada codificada con Norito, y `handshake_chain_id` para compilar y concatenar identificador de cadena ہے۔ Nodo de mensaje سے نکلنے سے پہلے Cifrado AEAD ہوتی ہے۔
2. Orden de clave local/par del respondedor کو inversa کر کے recálculo de carga útil کرتا ہے اور `HandshakeHelloV1` میں verificación de firma Ed25519 integrada کرتا ہے۔ چونکہ دونوں claves efímeras اور dirección anunciada firma dominio میں ہیں، mensaje capturado کو کسی دوسرے peer کے خلاف repetición کرنا یا conexión obsoleta recuperar کرنا determinista طور پر falla ہوتا ہے۔
3. Indicadores de capacidad confidencial اور `ConfidentialFeatureDigest` `HandshakeConfidentialMeta` کے اندر travel کرتے ہیں۔ Receptor `{ enabled, assume_valid, verifier_backend, digest }` tupla کو local `ConfidentialHandshakeCaps` کے ساتھ comparar کرتا ہے؛ falta de coincidencia ہونے پر `HandshakeConfidentialMismatch` کے ساتھ salida anticipada ہوتا ہے۔
4. Los operadores کو digest (`compute_confidential_feature_digest` کے ذریعے) vuelven a calcular کرنا اور registros/políticas actualizados کے ساتھ nodos reinician کرنا DEBE ہے۔ Los resúmenes antiguos anuncian un error en el apretón de manos de los pares کرتے رہیں گے اور estado obsoleto کو conjunto de validadores میں واپس آنے سے روکیں گے۔5. Actualización de los contadores `iroha_p2p::peer` estándar de éxitos/fallos del protocolo de enlace (`handshake_failure_count` وغیرہ) کرتے ہیں اور las entradas de registro estructuradas emiten کرتے ہیں جن میں ID de par remoto اور digerir etiquetas de huellas digitales ہوتے ہیں۔ ان indicadores کو monitor کریں تاکہ implementación کے دوران intentos de repetición یا configuraciones erróneas پکڑی جا سکیں۔

## Gestión de claves y cargas útiles
- Jerarquía de derivación de claves por cuenta:
  - `sk_spend` → `nk` (clave anuladora), `ivk` (clave de visualización entrante), `ovk` (clave de visualización saliente), `fvk`.
- Cargas útiles de notas cifradas AEAD استعمال کرتے ہیں جو ECDH-derived Shared Keys سے بنے ہوتے ہیں؛ opcional auditor ver claves política de activos کے مطابق salidas کے ساتھ adjuntar کئے جا سکتے ہیں۔
- Adiciones de CLI: `confidential create-keys`, `confidential send`, `confidential export-view-key`, descifrado de notas, herramientas de auditor, ayudante `iroha app zk envelope` y sobres de notas Norito fuera de línea. producir/inspeccionar کرتا ہے۔
- Horario de gas determinista:
  - Halo2 (Plonkish): gas base `250_000` + gas `2_000` por entrada pública.
  - Gas `5` por byte de prueba, más cargos por anulador (`300`) y por compromiso (`500`).
  - Operadores یہ configuración de nodo de constantes (`confidential.gas.{proof_base, per_public_input, per_proof_byte, per_nullifier, per_commitment}`) کے ذریعے anulación کر سکتے ہیں؛ cambios inicio یا configuración recarga en caliente پر propagar ہو کر cluster میں determinista طور پر aplicar ہوتے ہیں۔
- Límites estrictos (valores predeterminados configurables):
- `max_proof_size_bytes = 262_144`.
- `max_nullifiers_per_tx = 8`, `max_commitments_per_tx = 8`, `max_confidential_ops_per_block = 256`.
- `verify_timeout_ms = 750`, `max_anchor_age_blocks = 10_000`. `verify_timeout_ms` سے زیادہ instrucciones de prueba کو determinista طور پر abort کرتے ہیں (las boletas de gobernanza `proof verification exceeded timeout` emiten کرتے ہیں، `VerifyProof` retorno de error کرتا ہے)۔
- Cuotas adicionales de vida aseguran کرتے ہیں: `max_proof_bytes_block`, `max_verify_calls_per_tx`, `max_verify_calls_per_block`, اور `max_public_inputs` constructores de bloques کو enlazados کرتے ہیں؛ `reorg_depth_bound` (≥ `max_anchor_age_blocks`) la retención del punto de control fronterizo rige کرتا ہے۔
- Los límites de tiempo de ejecución por transacción y por bloque superan las transacciones y rechazan las transacciones deterministas `InvalidParameter` emiten errores en el estado del libro mayor sin cambios
- Mempool `vk_id`, longitud de la prueba, edad del anclaje, transacciones confidenciales, prefiltro, invocación del verificador, uso de recursos limitado.- Verificación determinista طور پر tiempo de espera یا violación limitada پر detener ہوتی ہے؛ errores explícitos de transacciones کے ساتھ fallan ہوتی ہیں۔ Backends SIMD opcionales ہیں مگر contabilidad de gas alterar نہیں کرتے۔

### Líneas base de calibración y puertas de aceptación
- **Plataformas de referencia.** Ejecuciones de calibración کو نیچے دیئے گئے تین perfiles de hardware cubren کرنا DEBE ہے۔ اگر سب perfiles capturan نہ ہوں تو revisión rechazar کر دے۔

  | Perfil | Arquitectura | CPU/instancia | Banderas del compilador | Propósito |
  | --- | --- | --- | --- | --- |
  | `baseline-simd-neutral` | `x86_64` | AMD EPYC 7B12 (32c) e Intel Xeon Gold 6430 (24c) | `RUSTFLAGS="-C target-feature=-avx,-avx2,-fma"` | vectores intrínsecos کے بغیر valores mínimos establecen کرتا ہے؛ tablas de costos de respaldo sintonizar کرنے کیلئے استعمال ہوتا ہے۔ |
  | `baseline-avx2` | `x86_64` | Intel Xeon Gold 6430 (24c) | versión predeterminada | Validación de ruta AVX2 کرتا ہے؛ چیک کرتا ہے کہ SIMD acelera la tolerancia al gas neutro کے اندر رہیں۔ |
  | `baseline-neon` | `aarch64` | AWS Graviton3 (c7g.4xlarge) | versión predeterminada | NEON backend کو determinista اور x86 کے ساتھ alineado رکھتا ہے۔ |

- **Arnés de referencia.** Los informes de calibración de gas DEBEN contener:
  - `CRITERION_HOME=target/criterion cargo bench -p iroha_core isi_gas_calibration -- --sample-size 200 --warm-up-time 5 --save-baseline <profile-label>`
  - Confirmación determinista del accesorio `cargo test -p iroha_core bench_repro -- --ignored` کرنے کیلئے۔
  - `CRITERION_HOME=target/criterion cargo bench -p ivm gas_calibration -- --sample-size 200 --warm-up-time 5 --save-baseline <profile-label>` جب بھی VM opcode cuesta بدلیں۔- **Aleatoriedad fija.** bancos چلانے سے پہلے `IROHA_CONF_GAS_SEED=conf-gas-seed-2026Q1` exportar کریں تاکہ `iroha_test_samples::gen_account_in` determinista `KeyPair::from_seed` ruta پر cambiar ہو۔ Arnés ایک بار `IROHA_CONF_GAS_SEED_ACTIVE=…` print کرتا ہے؛ falta variable ہو تو la revisión DEBE fallar ہو۔ نئی utilidades de calibración بھی aleatoriedad auxiliar introducir کرتے وقت اس env var کو honor کریں۔

- **Captura de resultados.**
  - Resúmenes de criterios (`target/criterion/**/raw.csv`) ہر perfil کیلئے lanzamiento de artefacto میں carga کریں۔
  - Métricas derivadas (`ns/op`, `gas/op`, `ns/gas`) کو [Libro mayor de calibración de gas confidencial](./confidential-gas-calibration) میں git commit اور versión del compilador کے ساتھ store کریں۔
  - ہر perfil کیلئے آخری دو líneas de base برقرار رکھیں؛ نئی informe validar ہونے کے بعد پرانی instantáneas eliminar کریں۔

- **Tolerancias de aceptación.**
  - `baseline-simd-neutral` اور `baseline-avx2` کے درمیان deltas de gas ≤ ±1,5% رہیں۔
  - `baseline-simd-neutral` اور `baseline-neon` کے درمیان deltas de gas ≤ ±2.0% رہیں۔
  - Umbrales, propuestas de calibración, ajustes de cronograma, discrepancia/mitigación, solicitudes de calibración y solicitudes de RFC.- **Revisar lista de verificación.** Los remitentes ذمہ دار ہیں:
  - Extractos de `uname -a`, `/proc/cpuinfo` (modelo, paso a paso) ، اور Registro de calibración `rustc -Vv` میں شامل کریں۔
  - salida de banco میں `IROHA_CONF_GAS_SEED` کے echo ہونے کی تصدیق کریں (bancos de impresión de semillas activas کرتے ہیں)۔
  - marcapasos اور verificador confidencial producción de indicadores de funciones سے coincidencia ہوں (`--features confidential,telemetry` جب Telemetría کے ساتھ bancos چلائیں)۔

## Configuración y operaciones
- `iroha_config` Pantalla `[confidential]` Pantalla táctil:
  ```toml
  [confidential]
  enabled = true
  assume_valid = false
  verifier_backend = "ark_bls12_381"
  max_proof_size_bytes = 262144
  max_nullifiers_per_tx = 8
  max_commitments_per_tx = 8
  max_confidential_ops_per_block = 256
  verify_timeout_ms = 750
  max_anchor_age_blocks = 10000
  max_proof_bytes_block = 1048576
  max_verify_calls_per_tx = 4
  max_verify_calls_per_block = 128
  max_public_inputs = 32
  reorg_depth_bound = 10000
  policy_transition_delay_blocks = 100
  policy_transition_window_blocks = 200
  tree_roots_history_len = 10000
  tree_frontier_checkpoint_interval = 100
  registry_max_vk_entries = 64
  registry_max_params_entries = 32
  registry_max_delta_per_block = 4
  ```
- Las métricas agregadas de telemetría emiten کرتا ہے: `confidential_proof_verified`, `confidential_verifier_latency_ms`, `confidential_proof_bytes_total`, `confidential_nullifier_spent`, `confidential_commitments_appended`, `confidential_mempool_rejected_total{reason}`, اور `confidential_policy_transitions_total`, los datos de texto sin formato exponen کئے بغیر۔
- Superficies RPC:
  - `GET /confidential/capabilities`
  -`GET /confidential/zk_registry`
  - `GET /confidential/params`## Estrategia de prueba
- Determinismo: bloques de transacciones aleatorias que barajan raíces Merkle idénticas y conjuntos de anuladores producen کرتا ہے۔
- Resiliencia de reorganización: los anclajes کے ساتھ reorganizaciones multibloque simulan کریں؛ anuladores estables رہتے ہیں اور anclas obsoletas rechazan ہوتے ہیں۔
- Invariantes de gas: aceleración SIMD کے ساتھ اور بغیر nodos پر uso idéntico de gas verificar کریں۔
- Pruebas de límites: límites de tamaño/gas, pruebas, recuentos máximos de entrada/salida, cumplimiento del tiempo de espera.
- Ciclo de vida: verificador, activación/desaprobación de parámetros, operaciones de gobernanza, pruebas de gasto de rotación.
- Política FSM: transiciones permitidas/no permitidas, retrasos de transición pendientes, alturas efectivas کے گرد rechazo de mempool۔
- Emergencias registrales: retiro de emergencia `withdraw_height` پر congelación de activos afectados کرتا ہے اور اس کے بعد pruebas rechazadas کرتا ہے۔
- Control de capacidad: los bloques de validadores `conf_features` y validadores no coincidentes rechazan کرتے ہیں؛ `assume_valid=true` Consenso de observadores کو متاثر کئے بغیر sincronización رہتے ہیں۔
- Equivalencia de estado: cadena canónica de nodos validador/completo/observador y raíces de estado idénticas producen کرتے ہیں۔
- Fuzzing negativo: pruebas mal formadas, cargas útiles sobredimensionadas, colisiones anuladoras deterministas, rechazo y rechazo.## Trabajo excepcional
- Conjuntos de parámetros de Halo2 (tamaño del circuito, estrategia de búsqueda) Punto de referencia کریں اور نتائج manual de calibración میں ریکارڈ کریں تاکہ اگلی `confidential_assets_calibration.md` actualizar کے ساتھ actualización de valores predeterminados de gas/tiempo de espera ہوں۔
- Las políticas de divulgación del auditor اور متعلقہ API de visualización selectiva finalizan کریں، اور aprobación del borrador de gobernanza کے بعد flujo de trabajo aprobado کو Torii میں wire کریں۔
- Esquema de cifrado testigo, salidas de múltiples destinatarios, memorandos por lotes, extensión, implementadores de SDK, documento en formato de sobre, formato de sobre.
- Circuitos, registros, procedimientos de rotación de parámetros, comisión de revisión de seguridad externa, hallazgos, informes de auditoría interna, archivo
- Las API de conciliación del gasto del auditor especifican la guía del alcance de la clave de visualización, publican la publicación de los proveedores de billeteras, implementan la semántica de atestación## Fases de implementación
1. **Fase M0 — Endurecimiento de parada de envío**
   - ✅ Derivación del anulador اب Diseño Poseidon PRF (`nk`, `rho`, `asset_id`, `chain_id`) siga کرتا ہے اور actualizaciones del libro mayor میں compromiso determinista ordenar cumplir ہوتی ہے۔
   - ✅ Límites de tamaño de prueba de ejecución اور cuotas confidenciales por transacción/por bloque imponen کرتا ہے، transacciones por encima del presupuesto کو errores deterministas کے ساتھ rechazar کرتا ہے۔
   - ✅ El protocolo de enlace P2P `ConfidentialFeatureDigest` (resumen de backend + huellas digitales de registro) anuncia errores de coincidencia y `HandshakeConfidentialMismatch` un error determinista.
   - ✅ Rutas de ejecución confidenciales میں pánicos eliminar کئے گئے اور nodos no compatibles کیلئے activación de roles agregar کیا گیا۔
   - ⚪ Presupuestos de tiempo de espera del verificador اور puntos de control fronterizos کیلئے reorganizar los límites de profundidad hacer cumplir کرنا۔
     - ✅ Se aplican presupuestos de tiempo de espera de verificación ہوئے؛ `verify_timeout_ms` سے تجاوز کرنے والی pruebas اب fallo determinista ہوتی ہیں۔
     - ✅ Puntos de control fronterizos اب `reorg_depth_bound` respeto کرتے ہیں، ventana configurada سے پرانے puntos de control podar کرتے ہوئے instantáneas deterministas برقرار رکھتے ہیں۔
   - `AssetConfidentialPolicy`, política FSM, اور mint/transfer/revelar instrucciones کیلئے puertas de cumplimiento introducen کریں۔
   - Encabezados de bloque میں `conf_features` commit کریں اور registro/resúmenes de parámetros divergen ہونے پر participación del validador rechazar کریں۔
2. **Fase M1: Registros y parámetros**- `ZkVerifierEntry`, `PedersenParams`, `PoseidonParams` operaciones de gobernanza de registros, anclaje de génesis, gestión de caché, کے ساتھ land کریں۔
   - Syscall کو búsquedas de registro, ID de programación de gas, hash de esquema, اور verificaciones de tamaño requieren کرنے کیلئے cable کریں۔
   - Formato de carga útil cifrada v1, vectores de derivación de claves de billetera, gestión de claves confidenciales, soporte CLI para naves
3. **Fase M2: Gas y rendimiento**
   - Programación de gas determinista, contadores por bloque, telemetría, arneses de referencia implementan (verificar latencia, tamaños de prueba, rechazos de mempool) ۔
   - Puntos de control de CommitmentTree, carga de LRU, índices anulados y cargas de trabajo de múltiples activos y endurecimiento del sistema
4. **Fase M3: Rotación y herramientas de billetera**
   - La aceptación de prueba de múltiples parámetros y múltiples versiones habilita کریں؛ activación/obsolescencia impulsada por la gobernanza runbooks de transición soporte کریں۔
   - Flujos de migración de Wallet SDK/CLI, flujos de trabajo de escaneo del auditor, herramientas de conciliación de gastos que brindan کریں۔
5. **Fase M4: Auditoría y operaciones**
   - Auditor de flujos de trabajo clave, API de divulgación selectiva, runbooks operativos فراہم کریں۔
   - Calendario de revisión de seguridad/criptografía externa کریں اور hallazgos `status.md` میں publicar کریں۔

ہر hitos de la hoja de ruta de la fase اور متعلقہ actualización de pruebas کرتی ہے تاکہ red blockchain کیلئے garantías de ejecución deterministas برقرار رہیں۔