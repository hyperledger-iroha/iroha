---
lang: he
direction: rtl
source: docs/portal/docs/nexus/confidential-assets.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
כותרת: Activos confidenciales y transferencias ZK
תיאור: Plano de Phase C para circulacion blindada, registros y controls de operador.
slug: /nexus/confidential-assets
---
<!--
SPDX-License-Identifier: Apache-2.0
-->
# Diseno de activos confidenciales y transferencias ZK

## מוטיבציה
- Entregar flujos de activos blindados opt-in para que los dominios preserven privacidad transaccional sin alterar la circulacion transparente.
- Mantener eecucion determinista and heterogeneo hardware de validadores and conservar Norito/Kotodama ABI v1.
- הוכחה של אודיטורים ומבצעים בקרה של סיקלו דה וידה (הפעלה, סיבוב, ביטול) עבור מעגלים ופרמטרים קריפטוגרפיים.

## Modelo de amenazas
- Los validadores בן ישר-אך-סקרן: ejecutan consenso fielmente pero intentan inspeccionar ספר/מדינה.
- Observadores de red ven datos de bloque y transacciones ריכלו; no se asumen canales de gossip privados.
- Fuera de alcance: אנליסיס דה טראפיק מחוץ לפנקס, adversarios cuanticos (seguido en el מפת הדרכים PQ), אטאקס דה ספונטני של ספר חשבונות.

## קורות חיים דה דיסנו
- Los activos pueden declarar un *בריכה מוגנת* ademas de los balances transparentes existentes; la circulacion blindada se representa via התחייבויות criptograficos.
- Las notes encapsulan `(asset_id, amount, recipient_view_key, blinding, rho)` con:
  - התחייבות: `Comm = Pedersen(params_id || asset_id || amount || recipient_view_key || blinding)`.
  - מבטל: `Null = Poseidon(domain_sep || nk || rho || asset_id || chain_id)`, independiente del orden de las notes.
  - מטען מטען: `enc_payload = AEAD_XChaCha20Poly1305(ephemeral_shared_key, note_plaintext)`.
- Las transacciones transportan lasten `ConfidentialTransfer` codificados con Norito que contienen:
  - כניסות ציבוריות: עוגן מרקל, ביטולים, התחייבויות חדשות, מזהה נכס, גרסה דה מעגל.
  - כתובות מטענים עבור נמענים ו אודיטורים אופציונליים.
  - Prueba zero-knowledge que atesta conservacion de valor, בעלות y autorizacion.
- אימות מפתחות y conjuntos de parametros son controlados mediante registros on-ledger conventanas de activecion; los nodos rechazan validar הוכחות que referencian entradas desconocidas o revocadas.
- Headers de consenso comprometen el digest activo de capacidades confidenciales para que los bloques solo se acepten cuando el estado de registros y parametros coincida.
- La construccion de proofs usa un stack Halo2 (Plonkish) התקנה מהימנה; Groth16 ו-Otras Variations SNARK שוקל את הכוונה ללא תוכנית ב-v1.

### מתקנים דטרמיניסטים

תזכיר סודי כולל את התקן הקנוני של `fixtures/confidential/encrypted_payload_v1.json`. מערך הנתונים מתעדכן ב-v1 חיובי ותוצאות שליליות שליליות עבור SDKs קודמים לניתוח. בדיקות דגמי הנתונים ב-Rust (`crates/iroha_data_model/tests/confidential_encrypted_payload_vectors.rs`) ו-La suite de Swift (`IrohaSwift/Tests/IrohaSwiftTests/ConfidentialEncryptedPayloadTests.swift`) מטען אל מתקן כיוון, מבטיח את הקידוד Norito, שגיאה שטחית y la coberturaciana evolution de regresion code.Los SDKs de Swift ahora pueden emitir instrucciones shield sin דבק JSON בהזמנה אישית: construye un
`ShieldRequest` עם התחייבות של 32 בתים, מטען מטען ומטא נתונים של חיוב,
luego llama `IrohaSDK.submit(shield:keypair:)` (o `submitAndWait`) para firmar y enviar la
transaccion sobre `/v1/pipeline/transactions`. העוזר תוקף את קוי האורך של המחויבות,
enhebra `ConfidentialEncryptedPayload` en el מקודד Norito, y refleja el layout `zk::Shield`
descrito abajo para que los wallets se mantengan sincronizados con Rust.

## התחייבויות קונצנזו y porting יכולת
- Los headers de bloque exponen `conf_features = { vk_set_hash, poseidon_params_id, pedersen_params_id, conf_rules_version }`; el digest participa en el hash de consenso y debe igualar la vista local del registry para aceptar el bloque.
- ממשל הכנת שדרוגים בתוכנית `next_conf_features` עם `activation_height` עתיד; hasta esa altura, los productores de bloques deben seguir emitiendo el digest previo.
- Los nodos validadores DEBEN operar con `confidential.enabled = true` y `assume_valid = false`. Los checks de inicio rechazan unirse al set de validadores si cualquiera falla o si el `conf_features` מקומית מתפצלת.
- מטא נתונים של לחיצת יד P2P אוורה כולל `{ enabled, assume_valid, conf_features }`. Peers que anuncien תכונות ללא גישה ל-`HandshakeConfidentialMismatch` y nunca entran en rotacion de consenso.
- Los resultados de לחיצת יד אנטר validadores, observers y peers to capturan in la matriz de לחיצת יד bajo [Node Capability Negotiation](#node-capability-negotiation). Los fallos de exponen לחיצת יד `HandshakeConfidentialMismatch` y mantienen al peer fura de la rotacion de consenso hasta que su digest coincida.
- משקיפים ללא validadores pueden fijar `assume_valid = true`; aplican deltas confidenciales a ciegas pero no influyen en la seguridad del consenso.## Politicas de assets
- Cada definicion de asset lleva un `AssetConfidentialPolicy` fijado por el creador o באמצעות ממשל:
  - `TransparentOnly`: modo por defecto; solo se permiten instrucciones transparentes (`MintAsset`, `TransferAsset` וכו') y las operaciones shielded se rechazan.
  - `ShieldedOnly`: toda emision y transferencias deben usar instrucciones confidenciales; `RevealConfidential` esta prohibido para que los balances nunca se expongan publicamente.
  - `Convertible`: מחזיקי מחזיקי גבורה גבורה נציגות שקופה y shielded usando las instrucciones de on/off-ramp de abajo.
- Las politicas siguen un FSM restringido para evitar fondos varados:
  - `TransparentOnly -> Convertible` (habilitacion inmediata del shielded pool).
  - `TransparentOnly -> ShieldedOnly` (מחייב המרת מעבר והמרה).
  - `Convertible -> ShieldedOnly` (demora minima obligatoria).
  - `ShieldedOnly -> Convertible` (דרוש תוכנית מיגרציה עבור que las notes blindadas sigan siendo gastables).
  - `ShieldedOnly -> TransparentOnly` אין אישור לבריכה ולבריכה מוגנת או קוד ניהול ממשל ולא מיגרציה que des-blinde notes pendientes.
- Instrucciones de governance fijan `pending_transition { new_mode, effective_height, previous_mode, transition_id, conversion_window }` via el ISI `ScheduleConfidentialPolicyTransition` y pueden abortar cambios programados con `CancelConfidentialPolicyTransition`. La validacion del mempool asegura que ninguna transaccion cruce la altura de transicion y la inclusion falla de forma determinista si un check de politica cambiaria a mitad de bloque.
- Las transiciones pendientes se aplican automaticamente cuando abre un nuevo bloque: una vez que la altura entra en la ventana de conversion (para upgrades `ShieldedOnly`) o alcanza `effective_height`, el runtime actualiza `ShieldedOnly` `zk.policy` y limpia la entrada pendiente. יש לספק אספקה ​​שקופה cuando madura una transicion `ShieldedOnly`, או בזמן ריצה ביטול cambio y registra una advertencia, dejando el modo prevo intacto.
- Knobs de config `policy_transition_delay_blocks` y `policy_transition_window_blocks` fuerzan aviso minimo y periodos de gracia para permitir convertes de wallets alrededor del cambio.
- `pending_transition.transition_id` tambien funciona como handle de auditoria; ממשל debe citarlo al finalizar או ביטול מעברים עבור que los operadores correlacionen reportes de on/off-ramp.
- `policy_transition_window_blocks` ברירת מחדל a 720 (כ-12 שעות חסימה של 60 שניות). לוס נודוס מוגבלים לשדלות הממשל que intenten avisos mas cortos.
- בראשית ביטוי y flujos CLI exponen politicas actuales y pendientes. La logica de admission lee la politica en tiempo de ejecucion para confirmar que cada instruccion confidencial esta autorizada.
- Checklist de migracion - וראה "רצף הגירה" עבור תוכנית השדרוג על ידי אבן דרך M0.

#### Monitoreo de transiciones דרך Toriiיועץ ארנקים y auditores `GET /v1/confidential/assets/{definition_id}/transitions` לבדיקת `AssetConfidentialPolicy` פעיל. El payload JSON siempre incluye el asset id canonico, la ultima altura de bloque observada, el `current_mode` de la politica, el modo efectivo en esa altura (las ventanas de conversion reportan temporalmente `Convertible`), `vk_set_hash`/פוסידון/פדרסן. Cuando hay una transicion pendiente la respuesta tambien כולל:

- `transition_id` - handle de auditoria devuelto por `ScheduleConfidentialPolicyTransition`.
- `previous_mode`/`new_mode`.
- `effective_height`.
- `conversion_window` y el `window_open_height` derivado (el bloque donde wallets deben comenzar conversion para cut-overs ShieldedOnly).

דוגמה לתשובה:

```json
{
  "asset_id": "rose#wonderland",
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

תשובה `404` אינדיקציה לא קיימת הגדרה של נכס מקרי. Cuando no hay transicion programada el campo `pending_transition` es `null`.

### מאקווינה דה אסטאדוס דה פוליטיקה| מודו בפועל | Modo suuiente | דרישות מוקדמות | Manejo de altura efectiva | Notas |
|--------------------|----------------|----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| שקוף בלבד | להמרה | ממשל הפעיל את רישום האימות/פרמטרים. Enviar `ScheduleConfidentialPolicyTransition` con `effective_height >= current_height + policy_transition_delay_blocks`. | La transicion se ejecuta exactamente en `effective_height`; אל הבריכה המוגנת queda disponible de inmediato.        | Ruta por defecto para habilitar confidencialidad mientras se mantienen flujos transparentes. |
| שקוף בלבד | ShieldedOnly | Igual que arriba, mas `policy_transition_window_blocks >= 1`.                                                       | El Time run entra automaticamente en `Convertible` en `effective_height - policy_transition_window_blocks`; cambia a `ShieldedOnly` en `effective_height`. | הוכח את ההמרה קבע את ההוראות השקופות. |
| להמרה | ShieldedOnly | Transicion programada con `effective_height >= current_height + policy_transition_delay_blocks`. ממשל DEBE אישור (`transparent_supply == 0`) באמצעות metadata de auditoria; el runtime lo aplica en el cut-over. | Semantica de ventana identica a la anterior. Si el supply transparente es no-cero en `effective_height`, la transicion aborta con `PolicyTransitionPrerequisiteFailed`. | Bloquea el asset en circulacion completamente confidencial.                                |
| ShieldedOnly | להמרה | Transicion programada; sin retiro de emergencia activo (`withdraw_height` ללא הגדרה).                            | El estado cambia en `effective_height`; los לחשוף רמפות se reabren mientras las notes blindadas siguen siendo validas. | Usado para ventanas de mantenimiento o revisiones de auditores.                            |
| ShieldedOnly | שקוף בלבד | ממשל debe probar `shielded_supply == 0` או הכנת תוכנית `EmergencyUnshield` firmado (firmas de auditor requeridas). | זמן ריצה אבר וונטנה `Convertible` antes de `effective_height`; en esa altura las instrucciones confidenciales fallan duro y el asset vuelve al modo שקוף בלבד. | Salida de ultimo recurso. La transicion se auto-cancela si ocurre cualquier gasto de note confidencial durante la ventana. |
| כל | כמו הנוכחי | `CancelConfidentialPolicyTransition` limpia el cambio pendiente.                                                    | `pending_transition` זה מיידית.                                                                     | מנטיין אל סטטוס קוו; מוסטרדו לשלמות.                                          |המעבר אינו זמין עבור ההגשה לממשל. כל אימות זמן ריצה לאשור התנאים המוקדמים רק לפני היישום של תוכנת מעבר; אני מת, פתח את הנכס אל מוד הקודם ו- emite `PolicyTransitionPrerequisiteFailed` דרך טלמטריה ואירועי גוש.

### Secuencia de migracion

1. **הכן את הרישום:** אקטיבר todas las entradas de verificador y parametros referenciados por la politica objetivo. Los nodos anuncian el `conf_features` resultante para que los peers verifiquen coherencia.
2. **תוכנית המעבר:** `ScheduleConfidentialPolicyTransition` עם `effective_height` וחזרה `policy_transition_delay_blocks`. Al moverse hacia `ShieldedOnly`, especificar una ventana de conversion (`window >= policy_transition_window_blocks`).
3. **מידע ציבורי עבור מפעילים:** הרשם של `transition_id` devuelto y circular un runbook de on/off-ramp. ארנקים y auditores se suscriben a `/v1/confidential/assets/{id}/transitions` para conocer la altura de apertura de ventana.
4. **Aplicar ventana:** cuando abre la ventana, el runtime cambia la politica a `Convertible`, emite `PolicyTransitionWindowOpened { transition_id }`, y empieza and rechazar requests de governance in conflicto.
5. **סיום או ביטול:** ב-`effective_height`, אימות זמן ריצה לאימות דרישות מעבר (אספקת ציוד שקוף, חירום וכו'). Si pasa, cambia la politica al modo solicitado; si falla, emite `PolicyTransitionPrerequisiteFailed`, limpia la transicion pendiente y deja la politica sin cambios.
6. **שדרוגי סכימה:** מבטל את יציאת המעבר, ממשל תת גרסת הסכימה של הנכס (באמצעות דוגמה, `asset_definition.v2`) והכלי CLI דרושים `confidential_policy` לגילויי סדרה. מסמכי שדרוג בראשית מפעילים הגדרות פוליטיות וטביעות אצבעות ברישום לפני אישורי תקינות.

Nuevas redes que inician confidencialidad habilitada codifican la politica deseada directamente en genesis. Aun asi siguen la checklist anterior al cambiar modos לאחר ההשקה para que las ventanas de conversion signan siendo deterministas y los wallets tengan tiempo de ajustar.

### גרסה והפעלה של מניפסט Norito- Genesis manifests DEBEN incluir un `SetParameter` para la key custom `confidential_registry_root`. מטען es Norito JSON que iguala `ConfidentialRegistryMeta { vk_set_hash: Option<String> }`: emitir el campo (`null`) cuando no hay entradas actives, o proer un string hex de 32 bytes (`null`) `compute_vk_set_hash` sobre las instrucciones de verificador enviadas en el manifest. Los nodos se niegan a iniciar si el parametro falta o si el hash difiere de las escrituras de registry codificadas.
- El on-wire `ConfidentialFeatureDigest::conf_rules_version` להטמיע את הגרסה של פריסת המניפסט. Para Redes v1 DEBE permanecer `Some(1)` y es igual a `iroha_config::parameters::defaults::confidential::RULES_VERSION`. Cuando el ruleset evolucione, sube la constante, regenera manifests y despliega binarios in lock-step; גרסאות mezclar hasce que los validadores rechacen bloques con `ConfidentialFeatureDigestMismatch`.
- הפעלה מתבטאת DEBERIAN agrupar actualizaciones de registry, cambios de ciclo de vida de parametros y transiciones de politica para que el digest se mantenga consistente:
  1. אפליקציית מוטציונות של רישום תכנון (`Publish*`, `Set*Lifecycle`) ב-Una Vista Offline del Estado y calcula el digest post-activacion with `compute_confidential_feature_digest`.
  2. Emite `SetParameter::custom(confidential_registry_root, {"vk_set_hash": "0x..."})` usando el hash calculado para que peers atrasados ​​puedan recuperar el digest correcto aunque se pierdan instrucciones intermedias de registry.
  3. Anexa las instrucciones `ScheduleConfidentialPolicyTransition`. Cada instruccion debe citar el `transition_id` emitido por governance; מראה que lo omiten seran rechazados por el runtime.
  4. התמידו בתים של מניפסט, ללא טביעת אצבע SHA-256 y el digest usado in el plan de activecion. Los operadores verifican los tres artefactos antes de votar el manifest para evitar particiones.
- Cuando los rollouts requieran un cut-over diferido, registra la altura objetivo en un parametro custom accompanante (por ejemplo `custom.confidential_upgrade_activation_height`). Esto da a los auditores una prueba codificada en Norito de que los validadores respetaron la ventana de aviso antes de que el cambio de digest entrara en efecto.## סיקלו דה vida de verificadores y parametros
### Registro ZK
- פנקס החשבונות של `ZkVerifierEntry { vk_id, circuit_id, version, proving_system, curve, public_inputs_schema_hash, vk_hash, vk_len, max_proof_bytes, gas_schedule_id, activation_height, deprecation_height, withdraw_height, status, metadata_uri_cid, vk_bytes_cid }` דונה `proving_system` בפועל esta fijo a `Halo2`.
- Pares `(circuit_id, version)` son globalmente unicos; אל רישום מניין ומדד שניות לייעוץ עבור מטא נתונים של מעגל. Intentos de registrar un par duplicado se rechazan durante admission.
- `circuit_id` debe ser no vacio y `public_inputs_schema_hash` debe ser provisto (tipicamente un hash Blake2b-32 del coding canonico de public input del verificador). כניסה rechaza registros que omiten estos campos.
- הוראות הממשל כוללות:
  - `PUBLISH` עבור אגריר una entrada `Proposed` סולו עם מטא נתונים.
  - `ACTIVATE { vk_id, activation_height }` עבור הפעלה מתוכנתת לתקופה מוגבלת.
  - `DEPRECATE { vk_id, deprecation_height }` para marcar la altura final donde proofs pueden referenciar la entrada.
  - `WITHDRAW { vk_id, withdraw_height }` para apagado de emergencia; נכסים אפקטאטוס Congelan Gastos Confidenciales Despues de למשוך גובה hasta que nuevas entradas se activen.
- Genesis manifests emiten automaticamente un parametro custom `confidential_registry_root` cuyo `vk_set_hash` coincide con entradas actives; la validacion cruza este digest contra el estado local del registry antes de que un nodo pueda unirse consenso.
- רשם או בפועל לא אימות דורש `gas_schedule_id`; la verificacion exige que la entrada de registry este `Active`, presente en el indice `(circuit_id, version)`, y que los proofs Halo2 provean un `OpenVerifyEnvelope` cuyo Sumeragi,08NI60y, `public_inputs_schema_hash` במקביל לרישום הרישום.

### מפתחות הוכחה
- המפתחות המוכיחים את תוצאות החשבון מחוץ לדף החשבונות.
- Los SDKs de wallet obtienen datas de PK, hashes valider, y cachean localmente.

### Parametros Pedersen y Poseidon
- Registros separados (`PedersenParams`, `PoseidonParams`) פקדים של תקליטורים, קוד אוומו של `params_id`, ערכי גיבוב/קונסטנטים, הפעלה, ביטול ביטול ושינוי.
- התחייבויות y hashes separan dominios por `params_id` para que la rotacion de parametros nunca reutilice patrones de bits de sets deprecados; el ID se embebe en התחייבויות de notes y tags de dominio de nullifier.
- Los circuitos soportan seleccion multi-parametro en tiempo de verificacion; sets de parametros deprecados siguen siendo gastables hasta su `deprecation_height`, y sets retirados se rechazan exactamente en su `withdraw_height`.## Orden determinista y nullifiers
- Cada asset mantiene un `CommitmentTree` con `next_leaf_index`; los bloques agregan commitments en orden determinista: iterar transacciones en orden de bloque; dentro de cada transaccion iterar פלטי blindados por `output_idx` serializado ascendente.
- `note_position` se deriva de offsets del arbol pero **no** es parte del nullifier; סולו alimenta rutas de membresia dentro del witness de prueba.
- La estabilidad del nullifier bajo reorgs esta garantizada por el diseno PRF; el input PRF enlaza `{ nk, note_preimage_hash, asset_id, chain_id, params_id }`, y los anchors referencian roots Merkle historicos limitados por `max_anchor_age_blocks`.

## Flujo de Ledger
1. **MintConfidential { asset_id, amount, recipient_hint }**
   - Requiere politica de asset `Convertible` o `ShieldedOnly`; אישור כניסה לנכס, להשיג `params_id` בפועל, muestrea `rho`, התחייבות עמית, מציאות ארבול מרקל.
   - Emite `ConfidentialEvent::Shielded` con el nuevo התחייבות, delta de Merkle root y hash de llamada de transaccion para auditing trails.
2. **TransferConfidential { asset_id, proof, circuit_id, version, nullifiers, new_commitments, enc_payloads, anchor_root, memo }**
   - Syscall del VM Verifica proof usando la entrada del registry; el host asegura nullifiers no usados, התחייבויות anexados de forma determinista, anchor reciente.
   - רישום פנקסי חשבונות של `NullifierSet`, קבצי עומסי רווחים עבור נמענים/אודיטורים ו-Emite `ConfidentialEvent::Transferred` מבטלים רזומה, סדרת פלטים, הוכחת hash de proof ושורשי מרקל.
3. **RevealConfidential { asset_id, proof, circuit_id, version, nullifier, amount, recipient_account, anchor_root }**
   - נכסי סולו זמינים `Convertible`; la proof valida que el valor de la note iguala el monto revelado, el ledger acredita balance transparente y quema la note blindada marcando el nullifier como gastado.
   - Emite `ConfidentialEvent::Unshielded` con el monto publico, מבטל צרכנות, זיהוי הוכחה ו-hash de lamada de transaccion.## מודל מידע נוסף
- `ConfidentialConfig` (נוeva seccion de config) con flag de habilitacion, `assume_valid`, כפתורי גז/מגבלות, ventana de anchor, backend de verificador.
- סכימות `ConfidentialNote`, `ConfidentialTransfer`, y `ConfidentialMint` Norito עם byte de version explicito (`CONFIDENTIAL_ASSET_V1 = 0x01`).
- `ConfidentialEncryptedPayload` envuelve bytes de memo AEAD con `{ version, ephemeral_pubkey, nonce, ciphertext }`, con default `version = CONFIDENTIAL_ENCRYPTED_PAYLOAD_V1` para el layout XChaCha20-Poly1305.
- Vectores canonicos de key-derivation viven en `docs/source/confidential_key_vectors.json`; tanto el CLI como el endpoint Torii regresan contra estos גופי.
- `asset::AssetDefinition` agrega `confidential_policy: AssetConfidentialPolicy { mode, vk_set_hash, poseidon_params_id, pedersen_params_id, pending_transition }`.
- `ZkAssetState` מתמיד בקישור `(backend, name, commitment)` עבור מאמת העברה/מגן; la ejecucion rechaza הוכחות cuyo אימות מפתח רפרנציאדו או מוטבע אין coincida con el התחייבות registrado.
- `CommitmentTree` (פור נכס עם מחסומי גבול), `NullifierSet` con llave `(chain_id, asset_id, nullifier)`, `ZkVerifierEntry`, `PedersenParams`, `PoseidonParams` en world state.
- Mempool mantiene estructuras transitorias `NullifierIndex` y `AnchorIndex` para deteccion temprana de duplicados y checks de edad de anchor.
- Actualizaciones de schema Norito כולל הזמנת קנוניקו עבור תשומות ציבוריות; בדיקות הקידוד הלוך ושוב aseguran determinismo de.
- נסיעות הלוך ושוב של מטען מטען באמצעות בדיקות יחידה (`crates/iroha_data_model/src/confidential.rs`). Vectores de wallet de seguimiento adjuntaran תמלילים AEAD canonicos para auditores. `norito.md` תיעוד של כותרת על חוט למעטפה.

## Integracion con IVM y syscall
- היכרות עם syscall `VERIFY_CONFIDENTIAL_PROOF`:
  - `circuit_id`, `version`, `scheme`, `public_inputs`, `proof`, y el `ConfidentialStateDelta { asset_id, nullifiers, commitments, enc_payloads }` התוצאה.
  - מערכת ה-Syscall Carga Metadata Del Verificador של הרישום, אפליקציית מגבלות של Tamano/Tiempo, Cobra Gas determinista, y Solo Aplica El Delta סי la proof tiene exito.
- הצגת תכונה יחידה של `ConfidentialLedger` עבור תצלומי מצב של Merkle root y estado de nullifier; la libreria Kotodama להוכיח את עוזרי ההרכבה של עדים ותאימות סכימה.
- Los docs de pointer-ABI זמין עבור פריסה ברורה של מאגר הוכחה וידיות הרישום.## Negociacion de capacidades de nodo
- El לחיצת יד anuncia `feature_bits.confidential` junto con `ConfidentialFeatureDigest { vk_set_hash, poseidon_params_id, pedersen_params_id, conf_rules_version }`. La participacion de validadores requiere `confidential.enabled=true`, `assume_valid=false`, identificadores de backend de verificador identicos y digests coincidentes; discrepancias fallan el לחיצת יד קון `HandshakeConfidentialMismatch`.
- La config soporta `assume_valid` solo para nodos observers: cuando esta deshabilitado, encontrar instrucciones confidenciales produce `UnsupportedInstruction` determinista sin panic; cuando esta habilitado, משקיפים אפליקניים deltas declarados חטא הוכחות אימות.
- Mempool rechaza transacciones confidenciales si la capacidad local esta deshabilitada. Los filtros de gossip evitan enviar transacciones blindadas a peers sin capacidades coincidentes mientras reenvian a ciegas IDs de verificador desconocidos dentro de limites de tamano.

### לחיצת יד של Matriz de

| Anuncio remoto | Resultado para nodos validadores | Notas para Operadores |
|----------------------|--------------------------------|----------------|
| `enabled=true`, `assume_valid=false`, קצה אחורי תואם, תקציר תואם | Aceptado | El peer llega al estado `Ready` y participa en propuesta, voto y fan-out RBC. אין צורך במדריך לעזרה. |
| `enabled=true`, `assume_valid=false`, קצה אחורי תואם, עיכול מעופש או faltante | Rechazado (`HandshakeConfidentialMismatch`) | הפעלת רישום אפליקציית רישום/פרמטרים או תוכנת `activation_height`. Hasta corregir, el nodo segue visible pero nunca entra en rotacion de consenso. |
| `enabled=true`, `assume_valid=true` | Rechazado (`HandshakeConfidentialMismatch`) | Los validadores requieren verificacion de proofs; קונפיגורציה מרחוק כמו Observer עם אינגרסו סולו Torii או קאמביה `assume_valid=false` אימות מלא. |
| `enabled=false`, campos omitidos (build desactualizado), o backend de verificador distinto | Rechazado (`HandshakeConfidentialMismatch`) | Peers desactualizados o parcialmente actualizados no pueden unirse a la red de consenso. אקטואליזאלוס יש לשחרר בפועל y asegura que el tuple backend + digest coincida antes de reconectar. |

משקיפים que intencionalmente להשמיט אימות הוכחות אין deben abrir conexiones de consenso contra validadores עם שערי יכולת. Aun pueden ingerir bloques via Torii o APIs de archivo.

### פוליטיקה דה גיזום חושפת את החזקות המבטלות

ספרי החשבונות הסודיים של ספר החשבונות וההיסטוריונים מספקים לבדיקת הערות והחזרות אודיטוריאליות לממשל. La politica por defecto, aplicada por `ConfidentialLedger`, es:- **Retencion de nullifiers:** mantener nullifiers gastados por un *minimo* de `730` dias (24 שניות) despues de la altura de gasto, o la ventana regulatoria obligatoria si es mayor. Los operadores pueden extender la ventana via `confidential.retention.nullifier_days`. מבטל את תוצאות החיפוש של DEBEN seguir consultables via Torii para que auditores prueben ausencia de double-spend.
- **גיזום מגלה:** מגלה שקופה (`RevealConfidential`) התחייבויות אסייאדו אינדיבידואליות דיספואס דה que el bloque finaliza. Eventos relacionados con reveal (`ConfidentialEvent::Unshielded`) רושם אל מונטו פובליקו, נמען y hash de proof para que reconstruir מגלה היסטוריקוס ללא צורך ב- ciphertext podado.
- **מחסומי גבולות:** מחסומי מחסומי מחויבות ב-Los Frontiers de commitment en rolling que cubren el mayor de `max_anchor_age_blocks` y la ventana de retencion. Los nodos compactan checkpoints mas antiguos solo despues de que todos los nullifiers dentro del intervalo expiran.
- **Remediacion por digest stale:** si se levanta `HandshakeConfidentialMismatch` por drift de digest, los operadores deben (1) verificar que las ventanas de retencion de nullifiers coincidan en el cluster, (2) ejecutar el18NI0000 מבטל retenidos, y (3) reployar el manifest actualizado. Cualquier nullifier podado prematuramente debe restaurarse desde almacenamiento frio antes de reingresar a la red.

דוקומנטה עוקפת את המקומות ב-el runbook de operations; las politicas de governance que extienden la ventana de retencion deben actualizar configuracion de nodo y planes de almacenamiento de archivo en lockstep.

### פינוי והחלמה

1. חוגה Durante el, `IrohaNetwork` בהשוואה ל-capacidades anunciadas. Cualquier חוסר התאמה levanta `HandshakeConfidentialMismatch`; la conexion se cierra y el peer permanece en la cola de Discovery sin ser promovido a `Ready`.
2. El fallo se expone via el log del servicio de red (כולל תקליט מרחוק y backend), y Sumeragi nunca programa al peer para propuesta o voto.
3. מערכת רישימי ה-Remedian alineando מתקנת וערכי הגדרות (`vk_set_hash`, `pedersen_params_id`, `poseidon_params_id`) או תוכנת `next_conf_features` עם I018030X עם I018NI30X. אם אפשר לעכל את זה בקנה אחד, לחיצת יד הבאה תצא אוטומטית.
4. Si un peer stale logra difundir un bloque (por ejemplo, via replay de archivo), los validadores lo rechazan de forma determinista con `BlockRejectionReason::ConfidentialFeatureDigestMismatch`, manteniendo el estado del Ledger consistente en la red.

### שידור חוזר של לחיצת יד1. Cada intento saliente asigna material de clave Noise/X25519 nuevo. El payload de handshake que se firma (`handshake_signature_payload`) concatena las claves publicas efimeras local y remota, la direccion de socket anunciada codificada en Norito y, cuando se compila con Sumeragi identificador dechain,. El mensaje se encripta con AEAD antes de salir del nodo.
2. El receptor recomputa el payload con el orden de claves peer/local invertido y verifica la firma Ed25519 embebida en `HandshakeHelloV1`. Debido a que ambas claves efimeras y la direccion anunciada forman parte del dominio de firma, reproducir un mensaje capturado contra otro peer o recuperar una conexion stale falla la verificacion de forma determinista.
3. Flags de capacidad confidencial y el `ConfidentialFeatureDigest` viajan dentro de `HandshakeConfidentialMeta`. El receptor compar el tuple `{ enabled, assume_valid, verifier_backend, digest }` contra su `ConfidentialHandshakeCaps` מקומי; cualquier חוסר התאמה מכירה temprano con `HandshakeConfidentialMismatch` antes de que el transporte transicione a `Ready`.
4. מפעילי DEBEN recomputar el digest (דרך `compute_confidential_feature_digest`) y reiniciar nodos con los registries/politicas actualizados antes de reconectar. Peers que anuncian digests antiguos siguen fallando el לחיצת יד, evitando que estado stale reingrese al set de validadores.
5. יציאות ולחיצות יד אקטואליזאן los contadores estandar `iroha_p2p::peer` (`handshake_failure_count`, helpers de taxonomia de errores) y emiten logs estructurados with el peer ID remoto y el טביעת אצבע של עיכול. מעקב אחר אינדיקטורים עבור שידורים חוזרים של זיהוי או הגדרות שגויות במהלך ההשקה.

## Gestion de claves y מטענים
- Jerarquia derivacion por account:
  - `sk_spend` -> `nk` (מפתח מבטל), `ivk` (מפתח צפייה נכנס), `ovk` (מפתח צפייה יוצא), `fvk`.
- מטענים משותפים של AEAD בשימוש במפתחות משותפים של ECDH; הצג מפתחות אודיטורים אופציונליים ומפתחות תצוגות מפורטות של נכסים פוליטיים.
- Adiciones al CLI: `confidential create-keys`, `confidential send`, `confidential export-view-key`, Tooling de Auditor para decifrar memos, y el helper `iroha app zk envelope` para producer/inspectionar I01080130X offline. Torii לחשוף את מיסו flujo de derivacion דרך `POST /v1/confidential/derive-keyset`, retornando formas hex y base64 para que ארנקים אבטנגן jerarquias de claves programaticamente.## גז, מגביל את ה-DoS
- לוח זמנים לקביעת גז:
  - Halo2 (Plonkish): בסיס `250_000` גז + `2_000` גז עבור קלט ציבורי.
  - `5` בתים חסין גז, mas cargos por nullifier (`300`) y por התחייבות (`500`).
  - Los operadores pueden sobrescribir estas constantes via la configuracion del nodo (`confidential.gas.{proof_base, per_public_input, per_proof_byte, per_nullifier, per_commitment}`); los cambios se propagan al inicio o cuando la capa de config hot-reload y se aplican de forma determinista en el cluster.
- מגביל דורוס (ברירת מחדל ניתנות להגדרה):
- `max_proof_size_bytes = 262_144`.
- `max_nullifiers_per_tx = 8`, `max_commitments_per_tx = 8`, `max_confidential_ops_per_block = 256`.
- `verify_timeout_ms = 750`, `max_anchor_age_blocks = 10_000`. Proofs que exceden `verify_timeout_ms` abortan la instruccion de forma determinista (הקלפיות של ממשל emiten `proof verification exceeded timeout`, `VerifyProof` שגיאה חוזרת).
- Cuotas adicionales aseguran lifeness: `max_proof_bytes_block`, `max_verify_calls_per_tx`, `max_verify_calls_per_block`, y `max_public_inputs` בוני אקוטן; `reorg_depth_bound` (>= `max_anchor_age_blocks`) gobierna la retencion de frontier checkpoints.
- La ejecucion runtime ahora rechaza transacciones que exceden estos limites por transaccion o por bloque, emitiendo errores `InvalidParameter` deterministas y dejando el estado del ledger in cambios.
- Mempool prefiltro transacciones confidenciales por `vk_id`, אורך הוכחה y edad de anchor antes de invocar el verificador para mantener acotado el uso de recursos.
- La verificacion se detiene de forma determinista en timeout o violacion de limites; las transacciones fallan con errores explicitos. Backends SIMD בן אופציונלי אבל אין חלופה לחשבונאות של גז.

### קווי בסיס של כיול וקבלת שערים
- **Plataformas de referencia.** Las corridas de calibracion DEBEN cubrir los tres perfiles de hardware abajo. Corridas que no capturan todos los perfiles se rechazan durante la revision.

  | פרפיל | Arquitectura | CPU / Instancia | Flags de compilador | פרופוזיטו |
  | --- | --- | --- | --- | --- |
  | `baseline-simd-neutral` | `x86_64` | AMD EPYC 7B12 (32c) או Intel Xeon Gold 6430 (24c) | `RUSTFLAGS="-C target-feature=-avx,-avx2,-fma"` | Establece valores piso sin intrinsics vectoriales; se usa para ajustar tablas de costo fallback. |
  | `baseline-avx2` | `x86_64` | Intel Xeon Gold 6430 (24c) | שחרור por defecto | Valida el path AVX2; revisa que los speedups SIMD se mantengan dentro de tolerancia del gas neutral. |
  | `baseline-neon` | `aarch64` | AWS Graviton3 (c7g.4xlarge) | שחרור por defecto | Asegura que el backend NEON permanezca determinista y alineado con los לוחות זמנים x86. |

- **רתמת אמת מידה.** כל הדיווחים על כיול גז DEBEN מפיק עם:
  - `CRITERION_HOME=target/criterion cargo bench -p iroha_core isi_gas_calibration -- --sample-size 200 --warm-up-time 5 --save-baseline <profile-label>`
  - `cargo test -p iroha_core bench_repro -- --ignored` עבור מתקן קבוע.
  - `CRITERION_HOME=target/criterion cargo bench -p ivm gas_calibration -- --sample-size 200 --warm-up-time 5 --save-baseline <profile-label>` cuando cambian costos de opcode del VM.- **Randomness fija.** Exporta `IROHA_CONF_GAS_SEED=conf-gas-seed-2026Q1` ספסלי אנטס דה קורר para que `iroha_test_samples::gen_account_in` cambie a la ruta determinista `KeyPair::from_seed`. אל רתמת איפריים `IROHA_CONF_GAS_SEED_ACTIVE=...` una vez; si falta la variable, la revision DEBE fallar. Cualquier utilidad nueva de calibracion debe seguir respetando esta env var al introducir aleatoriedad auxiliar.

- **Captura de resultados.**
  - Subir resmenes de Criterion (`target/criterion/**/raw.csv`) עבור קובץ עזר לפרסום.
  - Guardar metricas derivadas (`ns/op`, `gas/op`, `ns/gas`) en [ספר כיול גז סודי](./confidential-gas-calibration) junto con el commit de git de compilador us.
  - Mantener los ultimos dos baselines por perfil; תצלומי מצב ביטול אס אנטיגואוס una vez validado el reporte mas nuevo.

- **Tolerancias de acceptacion.**
  - Deltas de gas entre `baseline-simd-neutral` y `baseline-avx2` DEBEN permanecer <= +/-1.5%.
  - Deltas de gas entre `baseline-simd-neutral` y `baseline-neon` DEBEN permanecer <= +/-2.0%.
  - כיול פרופסור que exceden estos umbrales מחייב התאמות לוח הזמנים או RFC que que la discrepancia y su mitigacion.

- **רשימת בדיקה של עדכון.** Los submitters son responsables de:
  - כולל `uname -a`, extractos de `/proc/cpuinfo` (דגם, דריכה), y `rustc -Vv` en el log de calibracion.
  - Verificar que `IROHA_CONF_GAS_SEED` se vea en la salida de bench (las benches imprimen la seed active).
  - תכונה דגלים של קוצב לב וייצור מידע סודי (`--features confidential,telemetry` על ספסלים מתקן עם טלמטריה).

## תצורת פעולות
- `iroha_config` agrega la seccion `[confidential]`:
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
- Telemetria emite metricas agregadas: `confidential_proof_verified`, `confidential_verifier_latency_ms`, `confidential_proof_bytes_total`, `confidential_nullifier_spent`, `confidential_commitments_appended`, Norito, Norito, Norito, 014X, I18NI,5 exponiendo datos en claro.
- RPC של שטחים:
  - `GET /confidential/capabilities`
  - `GET /confidential/zk_registry`
  - `GET /confidential/params`## אסטרטגיה לבדיקות
- Determinismo: el shuffle aleatorio de transacciones dentro de bloques מייצר שורשי Merkle y sets de nullifier identicos.
- Resiliencia a reorg: ריאורגים דומים מרובי בלוקים קונדיטוריים; nullifiers se mantienen estables y anchors stale se rechazan.
- Invariantes de gas: בדוק את השימוש בגז זהות גישה עם y sin acceleration SIMD.
- בדיקות שדה: הוכחות וטכנולוגיות של טמאנו/גז, ספירות מקסימליות של כניסה/יציאה, אכיפה פסק זמן.
- מחזור חיים: פעולות ממשל עבור הפעלה/ביטול של אימות ופרמטרים, בדיקות סיבוביות.
- מדיניות FSM: transiciones permitidas/no permitidas, delays de transicion pendiente y rechazo de mempool alrededor de alturas efectivas.
- Emergencias de registry: retiro de emergencia congela assets afectados en `withdraw_height` y rechaza proofs despues.
- שער יכולת: validadores con `conf_features` לא תואם בלוקס rechazan; משקיפים con `assume_valid=true` avanzan sin afectar consenso.
- Equivalencia de estado: שורשי מאמת/מלא/מתבונן שנוצרו דה estado identicos en la cadena canonica.
- שלילי מטושטש: מוכיח תקלות, מטען חומרי עזר ותקלות מבטל את ההגדרה.

## מיגרציה
- דגל תכונת השקה: שלב C3 הושלם, ברירת המחדל של `enabled` a `false`; los nodos anuncian capacidades antes de unirse al set validador.
- נכסים transparentes no se afectan; לאס הוראות סודיות דרושות רישום ורישום ניהולי.
- Nodos compilados sin soporte confidencial rechazan bloques relevantes de forma determinista; no pueden unirse al set validador pero pueden operar como observers con `assume_valid=true`.
- ביטוי בראשית כולל ראשוני רישום, ערכות פרמטרים, פוליטיקה סודית לנכסים ומפתחות אודיטור אופציונליים.
- Los operadores siuen runbooks publicados para rotacion de registry, transiciones de politica y retiro de emergencia para mantener שדרוגים דטרמיניסטים.

## Trabajo pendiente
- מדדי הפרמטרים של Halo2 (טמנו דה מעגל, אסטרטגית חיפוש) והתוצאות של הרשם ב-Playbook de calibracion עבור ברירות המחדל של גז/פסק זמן.
- סיום הגילוי הפוליטי של המבקר וממשקי ה-API של אסוציאציות של צפייה סלקטיבית.
- מרחיב הצפנה של עדים ליצירת פלטים מרובי נמענים ותזכירים באצווה, מתעד את הפורמט של המעטפה ליישום SDK.
- Encargar una revision de seguridad externa de circuitos, registries y procedimientos de rotacion de parametros y archivar hallazgos junto a los reportes internos de auditoria.
- ממשקי API ספציפיים ל-Reconciliacion de spendness para auditores y publicar guia de alcance de view-key למען יישומי ספקי ארנקים לאסמס סמנטיקה של אטסטאקיון.## Fases de implementacion
1. **שלב M0 - Endurecimiento Stop-Ship**
   - [x] La derivacion de nullifier ahora segue el diseno Poseidon PRF (`nk`, `rho`, `asset_id`, `chain_id`) con orden determinista determinista deledgerizaciones for realised determinista deledger התחייבויות.
   - [x] La ejecucion aplica limites de tamano de proof y cuotas confidenciales por transaccion/por bloque, recchazando transacciones fuera de presupuesto con errores deterministas.
   - [x] לחיצת יד P2P הודיעה `ConfidentialFeatureDigest` (עיכוב backend + טביעות אצבעות של הרישום) y falla mismatches de forma determinista via `HandshakeConfidentialMismatch`.
   - [x] Remover פאניקה בנתיבים של פליטת סודיות y agregar role gating para nodos sin soporte.
   - [ ] אפליקציית זמן קצוב לזמן קצוב של אימות וגבולות של פרופונדידאד דה ריורג למחסומי גבול.
     - [x] Presupuestos de timeout de verificacion aplicados; הוכחות que exceden `verify_timeout_ms` ahora fallan deterministamente.
     - [x] מחסומי גבול ahora respetan `reorg_depth_bound`, מחסומי פודנדו mas antiguos que la ventana configurada mientras mantienen תמונת מצב deterministas.
   - Introducir `AssetConfidentialPolicy`, מדיניות FSM y Gates de enforcement para instrucciones mint/transfer/reveal.
   - מדפסר `conf_features` ב-headers de bloque y rechazar participacion de validadores cuando los digests de registry/parametros divergen.
2. **שלב M1 - רישום ופרמטרים**
   - רישום Entregar `ZkVerifierEntry`, `PedersenParams`, y `PoseidonParams` con ops de governance, anclaje de genesis y manejo de cache.
   - Conectar syscall לדרישת חיפושי רישום, מזהי לוח זמנים של גז, גיבוש סכימה ובדיקות טמאנו.
   - מצא פורמט מטען מטען v1, וקטור גזירת מפתחות עבור ארנק, ושירות CLI עבור הדרכה של סודיות קלאבים.
3. **שלב M2 - ביצועי גז וביצועים**
   - לוח זמנים יישום גז דטרמיניסטה, contadores por bloque, y rates de benchmark con telemetria (latencia de verificacion, tamanos de proof, rechazos de mempool).
   - מחסומי Endurecer CommitmentTree, מטען LRU, מדדי מבטל עבור עומסי עבודה מרובי נכסים.
4. **Phase M3 - Rotacion y Tooling de Wallet**
   - קבלת הוכחות מרובות פרמטרים וריבוי גרסאות; המשך הפעלה/ביטול אימפולס של ממשל עם ספרי המעבר.
   - הפעל תהליכי העברת נתונים ב-SDK/CLI, זרימות עבודה של escaneo de auditor, ו-tools de reconciliacion de spendness.
5. **שלב M4 - ביקורת פעולות**
   - בדוק זרימות עבודה של מפתחות המבקר, ממשקי API לבחירת גילוי, והפעלת ספרי הפעלה.
   - גרסת תוכנה חיצונית של קריפטוגרפיה/סגורידד y publicar hallazgos en `status.md`.

אבני דרך מפת הדרכים ובדיקות אsociados para mantener garantias de ejecucion determinista in la red blockchain.