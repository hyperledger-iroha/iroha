---
lang: he
direction: rtl
source: docs/portal/docs/nexus/confidential-assets.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
כותרת: Ativos confidenciais e transferencias ZK
תיאור: שרטוט שלב C עבור מחזור, רישומים ובקרות מפעיל.
slug: /nexus/confidential-assets
---
<!--
SPDX-License-Identifier: Apache-2.0
-->
# Design de ativos confidenciais e transferencias ZK

## מוטיבקאו
- Entregar fluxos de ativos blindados opt-in para que dominios preservem privacyde transacional sem alterar a circulacao transparente.
- Manter execucao determinista em heterogeneo hardware de validadores and preservar Norito/Kotodama ABI v1.
- Fornecer a auditores e operadores controls de ciclo de vida (tivacao, rotacao, revogacao) para circuitos e parametros criptograficos.

## מודל איום
- Validadores sao כנה-אך-סקרנית: executam consenso fielmente mas tentam inspecionar Book/State.
- Observadores de rede veem dados de bloco e transacoes ריכלו; nenhuma suposicao de canais privados de רכילות.
- Fora de escopo: analise de trafego off-book, adversarios quanticos (acompanhado no map PQ), ataques de disponibilidade do book.

## Visao geral do design
- נכסים podem declarar um *בריכה מוגנת* alem dos balances transparentes existentes; a circulacao blindada e representada באמצעות התחייבויות criptograficos.
- הערות encapsulam `(asset_id, amount, recipient_view_key, blinding, rho)` com:
  - התחייבות: `Comm = Pedersen(params_id || asset_id || amount || recipient_view_key || blinding)`.
  - מבטל: `Null = Poseidon(domain_sep || nk || rho || asset_id || chain_id)`, עצמאי של הערות.
  - מטען מטען: `enc_payload = AEAD_XChaCha20Poly1305(ephemeral_shared_key, note_plaintext)`.
- Transacoes מטענים `ConfidentialTransfer` קודים עם Norito טוענים:
  - כניסות ציבוריות: עוגן מרקל, ביטולים, התחייבויות חדשות, מזהה נכס, versao de circuito.
  - כתובות מטענים עבור נמענים e Auditores Opcionais.
  - הוכחה אפס ידע que atesta conservacao de valor, בעלות e autorizacao.
- אימות מפתחות e conjuntos de parametros sao controlados באמצעות registries on-ledger com janelas de ativacao; nodes recusam validar הוכחות que referenciam entradas desconhecidas ou revogadas.
- כותרות קונצנזו או תקציר סודיות של תקצירים, כך שהפרמטרים והפרמטרים עולים בקנה אחד.
- Construcao de proofs usa um stack Halo2 (Plonkish) סם התקנה מהימנה; Groth16 או וריאציות שונות של SNARK לאחר כוונה תומכת ב-v1.

### מתקנים דטרמיניסטים

מעטפות תזכיר סודיות agora enviam um fixture canonico em `fixtures/confidential/encrypted_payload_v1.json`. מערך הנתונים של המעטפה v1 חיובי יותר שליליות שליליות לפורמטים של SDKs possam afirmar paridade de parsing. מערכת האשכים עושה דגם נתונים עם Rust (`crates/iroha_data_model/tests/confidential_encrypted_payload_vectors.rs`) וחבילה של Swift (`IrohaSwift/Tests/IrohaSwiftTests/ConfidentialEncryptedPayloadTests.swift`) קאררגאם או מתקן ניהול, garantindo que o קידוד Norito, as superficies de erro e a copertura de enquanhaccam evolutioni.SDKs Swift agora podem emitir instrucos shield sem glue JSON בהתאמה אישית: construa um
`ShieldRequest` com o התחייבות של 32 בתים, או מטען מטען ומטא נתונים של חיוב,
e entao chame `IrohaSDK.submit(shield:keypair:)` (ou `submitAndWait`) עבור assinar e encaminhar a
transacao דרך `/v1/pipeline/transactions`. הו עוזר תוקף המחויבות,
הכנס `ConfidentialEncryptedPayload` ללא מקודד Norito, espelha o layout `zk::Shield`
Descrito abaixo para que ארנקים fiquem alinhadas com Rust.

## התחייבויות קונצנזוס ועריכת יכולת
- Headers de bloco expoem `conf_features = { vk_set_hash, poseidon_params_id, pedersen_params_id, conf_rules_version }`; o digest participa do hash de consenso e deve igualar a visao local do registry para aceitacao de bloco.
- עמדת ממשל מכינה שדרוגי תוכנה `next_conf_features` com um `activation_height` futuro; אכלתי essa altura, produtores de bloco devem מתמשך emitindo o digest anterior.
- צמתים מאושרים DEVEM com `confidential.enabled = true` ו `assume_valid = false`. בודקים את ההפעלה recusam entrar no set validador se qualquer condicao falhar ou se o `conf_features` local divergir.
- מטא נתונים ללחיצת יד P2P agora כולל `{ enabled, assume_valid, conf_features }`. עמיתים que anunciam תכונות שאינן תואמות עם `HandshakeConfidentialMismatch` והן נכללות בהסכמה.
- תוצאות של לחיצת יד קודמים, משקיפים ועמיתים משולבים בלחיצת יד [משא ומתן על יכולת צומת](#node-capability-negotiation). Falhas de לחיצת יד expoem `HandshakeConfidentialMismatch` e mantem o peer fora da rotacao de consenso ate que o digest coincida.
- Observers nao validadores podem definir `assume_valid = true`; aplicam deltas confidenciais Sem אימות הוכחות, mas nao influenciam a seguranca do consenso.## Politicas de assets
- Cada definicao de asset carrega um `AssetConfidentialPolicy` definido pelo criador ou באמצעות ממשל:
  - `TransparentOnly`: ברירת המחדל של modo; apenas instrucoes transparentes (`MintAsset`, `TransferAsset` וכו') sao permitidas e operacoes shielded sao rejeitadas.
  - `ShieldedOnly`: toda emisso e transferencias devem usar instrucoes confidenciais; `RevealConfidential` e proibido para que balances nunca aparecam publicamente.
  - `Convertible`: מחזיקי פודם מעביר גבורה ומייצגים שקופים והוראות שימוש מוגן על/מחוץ לרמפה.
- Politicas seguem um FSM restrito para evitar fundos encalhados:
  - `TransparentOnly -> Convertible` (habilitacao imediata do בריכה ממוגנת).
  - `TransparentOnly -> ShieldedOnly` (לבקש transicao pendente e janela de conversao).
  - `Convertible -> ShieldedOnly` (עיכוב מינימו אובריטוריו).
  - `ShieldedOnly -> Convertible` (plano de migracao requerido para que notes blindadas continum gastavis).
  - `ShieldedOnly -> TransparentOnly` e proibido a menos que o shielded pool esteja vazio או קוד ממשל אומה migracao que des-blinde notes pendentes.
- הוראות הממשל מוגדרות `pending_transition { new_mode, effective_height, previous_mode, transition_id, conversion_window }` באמצעות o ISI `ScheduleConfidentialPolicyTransition` ו-podem abortar mudancas programadas com `CancelConfidentialPolicyTransition`. A validacao do mempool garante que nenhuma transacao atravesse a altura de transicao e a inclusao falha deterministicamente se um check de politica mudaria no meio do bloco.
- Transicoes pendentes sao aplicadas automaticamente quando um novo bloco abre: quando a altura entra na janela de conversao (עבור שדרוגים `ShieldedOnly`) ou atinge `effective_height`, o runtime atualiza Norito a metualiza Norito `zk.policy` e limpa a entrada penente. ראה או אספקת עמידה שקופה quando uma transicao `ShieldedOnly` amadurece, o זמן ריצה אבורטה א מודנקה e registra um aviso, mantendo o modo anterior.
- Knobs de config `policy_transition_delay_blocks` e `policy_transition_window_blocks` impoem aviso minimo e periodos de tolerancia para permitir conversoes de wallet em torno da mudanca.
- `pending_transition.transition_id` tambem funciona como handle de auditoria; ממשל deve cita-lo ao finalizar או ביטול טרנסיקונים עבור מפעילי correlacionem relatorios de on/off-ramp.
- `policy_transition_window_blocks` ברירת מחדל a 720 (~12 שעות com זמן חסימה של 60 שניות). צמתים מגבילים את בקשות הממשל.
- Genesis manifests e fluxos CLI expoem politicas atuais e pendentes. לוגיקה של קבלה לפוליטיקה עם טמפו דה execucao para confirmar que cada instrucao confidencial esta autorisada.
- Checklist de migracao - עבור "רצף הגירה" abaixo para o plano de upgrade em etapas que o Milestone M0 acompanha.

#### Monitorando transicoes דרך Toriiארנקים וייעוץ רואי חשבון `GET /v1/confidential/assets/{definition_id}/transitions` לבחירת `AssetConfidentialPolicy`. o מטען JSON semper כולל מזהה נכס קנוניק, אולטרה נקודתית, o `current_mode` da politica, o modo efetivo nessa altura (janelas de conversao reportam temporariamente `Convertible`), e ossperadocadores de `vk_set_hash`/פוסידון/פדרסן. קונדו אומה טרנסיקאו של ממשל esta pendente a resposta tambem embute:

- `transition_id` - handle de auditoria retornado por `ScheduleConfidentialPolicyTransition`.
- `previous_mode`/`new_mode`.
- `effective_height`.
- `conversion_window` e o `window_open_height` derivado (o bloco onde wallets devem comecar conversao para cut-overs ShieldedOnly).

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

אומה תשובה `404` אינדיקציה que nenhuma definicao de asset correspondente existe. Quando nao ha transicao agendada o campo `pending_transition` e `null`.

### מאקווינה דה אסטאדוס דה פוליטיקה| Modo atual | Proximo modo | דרישות מוקדמות | Tratamento de altura efetiva | Notas |
|--------------------|----------------|----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| שקוף בלבד | להמרה | ממשל ativou entradas de registry de verificador/parametros. מד משנה `ScheduleConfidentialPolicyTransition` com `effective_height >= current_height + policy_transition_delay_blocks`. | A transicao executa exatamente em `effective_height`; o בריכה מוגנת Fica disponivel imediatamente.               | קמינהו ברירת מחדל עבור חשאי סודיות שקופה.          |
| שקוף בלבד | ShieldedOnly | Mesmo acima, mais `policy_transition_window_blocks >= 1`.                                                         | O זמן ריצה entra automaticamente em `Convertible` em `effective_height - policy_transition_window_blocks`; muda para `ShieldedOnly` em `effective_height`. | Fornece janela de conversao determinista antes de desabilitar instrucos transparentes.   |
| להמרה | ShieldedOnly | Transicao programada com `effective_height >= current_height + policy_transition_delay_blocks`. אישור ממשל DEVE (`transparent_supply == 0`) באמצעות metadata de auditoria; זמן ריצה aplica isso ללא חתך-אובר. | Semantica de Janela identica. Se o ספק שקוף עבור nao-0 em `effective_height`, a transicao aborta com `PolicyTransitionPrerequisiteFailed`. | Trava o asset em circulacao totalmente סודי.                                      |
| ShieldedOnly | להמרה | Transicao programada; sem נסיגה חירום ativo (`withdraw_height` ללא הגדרה).                              | O estado muda em `effective_height`; לחשוף רמפות reabrem enquanto הערות blindadas permanecem validas.             | Usado para janelas de manutencao או revisoes de auditores.                                |
| ShieldedOnly | שקוף בלבד | ממשל מפתחים `shielded_supply == 0` או מכינים את התוכנית `EmergencyUnshield` assinado (אסינאטורס של מבקר המבקר). | O זמן ריצה abre uma janela `Convertible` antes de `effective_height`; לחלופין, הנחיות קונפידינציאליות Falham duro e o asset retorna ao modo שקוף בלבד. | Saida de ultimo recurso. A transicao se auto-cancela se qualquer not confidencial for gasta durante a janela. |
| כל | כמו הנוכחי | `CancelConfidentialPolicyTransition` limpa a mudanca pendente.                                                    | `pending_transition` להסיר מיד.                                                                        | Mantem o סטטוס קוו; מוסטרדו לשלמות.                                             |Transicoes nao Listadas acima sao rejeitadas durante submissao de governance. או זמן ריצה בדיקת הלוגו המוקדמים לפני היישום של אומה טרנסיקאו תוכנה; falhas devolvem o asset ao modo emitem קדמי `PolicyTransitionPrerequisiteFailed` באמצעות טלמטריה ואירועי גוש.

### Sequenciamento de migracao

1. **הכנת רישומים:** ativar todas as entradas de verificador e parametros referenciadas pela politica alvo. Nodes anunciam o `conf_features` resultante para que peers verifiquem coerencia.
2. **סדר יום:** מד משנה `ScheduleConfidentialPolicyTransition` com um `effective_height` que respeite `policy_transition_delay_blocks`. Ao Mover para `ShieldedOnly`, especificar Uma Janela de Conversao (`window >= policy_transition_window_blocks`).
3. **שירות ציבורי להפעלת:** רשם של `transition_id` רטרנדו e circular um runbook on/off ramp. ארנקים e auditores assinam `/v1/confidential/assets/{id}/transitions` para aprender a altura de abertura da janela.
4. **Aplicar janela:** quando a janela abre, o runtime muda a politica para `Convertible`, emite `PolicyTransitionWindowOpened { transition_id }`, e comeca a rejeitar requests de governance conflitantes.
5. **סיום או ביטול:** ב-`effective_height`, או אימות זמן ריצה של דרישות מוקדמות (אספקת אפס שקוף, סיום חירום וכו'). Sucesso muda a politica para o modo solicitado; falha emite `PolicyTransitionPrerequisiteFailed`, limpa a transicao pendente e deixa a politica inalterada.
6. **שדרוגי סכימה:** apos uma transicao bem-sucedida, governance aumenta and versao de schema do asset (לדוגמה, `asset_definition.v2`) כלי עבודה CLI exige `confidential_policy` או מניפסטים סדרתיים. Docs de upgrade de genesis instruem מפעיל הגדרות נוספות של פוליטיקה וטביעות אצבעות ברישום לפני אימות חידושים.

Redes novas que iniciam com confidencialidade habilitada codificam a politica desejada diretamente em genesis. Ainda assim seguem a checklist acima quando mudam modos pos-launch para que janelas de conversao sejam deterministas e ארנקים tenham tempo de ajustar.

### Versionamento e ativacao de manifest Norito- Genesis manifests DEVEM כולל את `SetParameter` עבור מפתח מותאם אישית `confidential_registry_root`. מטען e Norito JSON que corresponde a `ConfidentialRegistryMeta { vk_set_hash: Option<String> }`: omitir o campo (`null`) quando nao ha entradas ativas, ou fornecer um string hex de 32 bytes (`ScheduleConfidentialPolicyTransition`) `compute_vk_set_hash` מפואר כמו הוראות אימות אינסטרומנטליות. Nodes recusam iniciar se o parametro faltar ou se o hash divergir das escritas de registry codificadas.
- O on-wire `ConfidentialFeatureDigest::conf_rules_version` להטמיע פריסה הפוך לעשות מניפסט. Para Redes v1 DEVE permanecer `Some(1)` e igual a `iroha_config::parameters::defaults::confidential::RULES_VERSION`. יצירת חוקים התפתחותית, חשובה מאוד, חידוש ביטויים וביצוע הפעלה בינוארית בשלב נעילה; misturar versoes faz validadores rejeitarem blocos com `ConfidentialFeatureDigestMismatch`.
- הפעלה מציגה עדכוני DEVEM agrupar de registry, mudancas de ciclo de vida de parametros e transicoes de politica para manter o digest consistente:
  1. אפליקציית שינויים ברישום פלילי (`Publish*`, `Set*Lifecycle`) עם אומה צפה במצב לא מקוון לעשות estado e calcular o digest pos-ativacao com `compute_confidential_feature_digest`.
  2. Emitir `SetParameter::custom(confidential_registry_root, {"vk_set_hash": "0x..."})` usando o hash calculado para que peers atrasados ​​recuperem o digest correto mesmo se perderem instrucoes intermediarias.
  3. הנחיות אנקסר `ScheduleConfidentialPolicyTransition`. Cada instrucao deve citar o `transition_id` emitido por governance; מראה que o esquecem serao rejeitados pelo runtime.
  4. Persistir os bytes do manifest, um טביעת אצבע SHA-256 e o ​​digest usado no plano de ativacao. Operadores verificam os tres artefatos antes de votar o manifest para evitar particoes.
- Quando מפעילה את הפרמטרים של החתך-אובר, רשם תוכנית מותאמת אישית של פרמטר (בדוגמה `custom.confidential_upgrade_activation_height`). Iso fornece aos auditores uma prova codificada em Norito de que validadores honraram a janela de aviso antes do digest entrar em efeito.## מידע על פרמטרים ומאמתים
### רישום ZK
- O ספר חשבונות ארמזנה `ZkVerifierEntry { vk_id, circuit_id, version, proving_system, curve, public_inputs_schema_hash, vk_hash, vk_len, max_proof_bytes, gas_schedule_id, activation_height, deprecation_height, withdraw_height, status, metadata_uri_cid, vk_bytes_cid }` onde `proving_system` אטואלמנטה e fixo em `Halo2`.
- Pares `(circuit_id, version)` sao globalmente unicos; o מנדט הרישום um מדד שניות עבור מטא נתונים של מעגל. Tentativas de registrar um par duplicado sao rejeitadas durante.
- `circuit_id` deve ser nao vazio e `public_inputs_schema_hash` deve ser fornecido (tipicamente um hash Blake2b-32 do coding canonico de public input do verificador). אישור כניסה לא רשומים que omitem esses campos.
- הוראות הממשל כוללות:
  - `PUBLISH` להוספת מטא נתונים `Proposed`.
  - `ACTIVATE { vk_id, activation_height }` עבור תוכנת אטיווקאו לתקופה מוגבלת.
  - `DEPRECATE { vk_id, deprecation_height }` para marcar a altura final onde proofs podem referenciar a entrada.
  - `WITHDRAW { vk_id, withdraw_height }` para desligamento de emergencia; נכסים afetados congelam gastos confidenciais apos למשוך גובה אכל novas entradas ativarem.
- Genesis manifests auto-emitem um parametro custom `confidential_registry_root` cujo `vk_set_hash` coincide com as entradas ativas; a validacao cruza esse digest com o estado local do registry antes que um node possa entrar no consenso.
- רשם או אטואליזר על המאמת מבקש `gas_schedule_id`; a verificacao exige que a entrada do registry esteja `Active`, presente no indice `(circuit_id, version)`, e que הוכחות Halo2 fornecam um `OpenVerifyEnvelope` cujo `ACTIVATE { vk_id, activation_height }`,08NI60X,08NI60X, `public_inputs_schema_hash` מכתבים או רישום לעשות את הרישום.

### מפתחות הוכחה
- הוכחה של מפתחות מרוכזים בכתובת תוכן (`pk_cid`, `pk_hash`, `pk_len`) מפרסמים גם את המטא נתונים.
- SDKs של ארנק buscam dados de PK, אימות hashes ו-fazem cache מקומי.

### Parametros Pedersen e Poseidon
- Registries separados (`PedersenParams`, `PoseidonParams`) espelham controls de ciclo de vida de verifiers, cada um com `params_id`, hashes de geradores/constantes, ativacao, depredracaw heights.
- התחייבויות e hashes separam dominios por `params_id` para que a rotacao de parametros nunca reutilize padroes de bits de sets deprecados; o ID e embutido em commitments de note e tags de dominio de nullifier.
- Circuitos suportam selecao multi-parametro em tempo de verificacao; sets de parametros deprecados permanecem spendable אכלו `deprecation_height`, e sets retirados sao rejeitados exatamente em `withdraw_height`.## Ordenacao determinista e nullifiers
- מנדט נכסי Cada um `CommitmentTree` com `next_leaf_index`; התחייבויות blocos acrescentam em ordem determinista: iterar transacoes na ordem do bloco; dentro de cada transacao iterar יציאות מסוככות por `output_idx` serializado ascendente.
- `note_position` e derivado dos offsets da arvore mas **nao** faz parte do nullifier; ele so alimenta paths de membership dentro do witness da proof.
- A estabilidade do nullifier sob reorgs e garantida pelo design PRF; o קלט PRF vincula `{ nk, note_preimage_hash, asset_id, chain_id, params_id }`, e anchors referenciam roots Merkle historicos limitados por `max_anchor_age_blocks`.

## Fluxo do ספר חשבונות
1. **MintConfidential { asset_id, amount, recipient_hint }**
   - Requer politica de asset `Convertible` או `ShieldedOnly`; כניסה checa autoridade do asset, recupera `params_id` atual, amostra `rho`, emite התחייבות, atualiza a arvore Merkle.
   - Emite `ConfidentialEvent::Shielded` com o novo commitment, delta de Merkle root e hash de chamada da transacao para auditing trails.
2. **TransferConfidential { asset_id, proof, circuit_id, version, nullifiers, new_commitments, enc_payloads, anchor_root, memo }**
   - Syscall VM אימות הוכחה לשימוש ברישום; o מארח ערבות מבטלים נאו ארה"ב, התחייבויות anexados deterministicamente ו anchor recente.
   - O Ledger registra entradas `NullifierSet`, Armazena payloads encriptados para recipients/auditores e emite `ConfidentialEvent::Transferred` resumindo nullifiers, outputs orderados, hash de proof ו- Merkle roots.
3. **RevealConfidential { asset_id, proof, circuit_id, version, nullifier, amount, recipient_account, anchor_root }**
   - Disponivel apenas para assets `Convertible`; א הוכחה valida que o valor da note iguala o montante revelado, o פנקס קרדיט מאזן שקוף e queima פתק מוגן מרcando o nullifier como gasto.
   - Emite `ConfidentialEvent::Unshielded` com o montante publico, nullifiers consumidos, identificadores de proof e hash de chamada da transacao.## מודל הנתונים של Adicoes ao
- `ConfidentialConfig` (nova secao de config) com flag de habilitacao, `assume_valid`, כפתורי גז/מגבלות, janela de anchor, backend de verifier.
- סכימות `ConfidentialNote`, `ConfidentialTransfer`, ו-`ConfidentialMint` Norito com byte de versao explicito (`CONFIDENTIAL_ASSET_V1 = 0x01`).
- `ConfidentialEncryptedPayload` כולל בתים של תזכיר AEAD com `{ version, ephemeral_pubkey, nonce, ciphertext }`, com ברירת מחדל `version = CONFIDENTIAL_ENCRYPTED_PAYLOAD_V1` עבור פריסת XChaCha20-Poly1305.
- Vectores canonicos de key-derivative vivem em `docs/source/confidential_key_vectors.json`; tanto o CLI quanto o נקודת קצה Torii רגרסאם נגד גופים.
- `asset::AssetDefinition` ganha `confidential_policy: AssetConfidentialPolicy { mode, vk_set_hash, poseidon_params_id, pedersen_params_id, pending_transition }`.
- `ZkAssetState` מתמיד או מחייב `(backend, name, commitment)` עבור מאמת העברה/ביטול מגן; א execucao rejeita הוכחות cujo אימות מפתח התייחסות או מקושר נאו מתכתב או התחייבות registrado.
- `CommitmentTree` (por asset com נקודות הגבול), `NullifierSet` com chave `(chain_id, asset_id, nullifier)`, `ZkVerifierEntry`, `PedersenParams`, Norito em.
- Mempool mantem estruturas transitorias `NullifierIndex` e `AnchorIndex` para deteccao precoce de duplicados e checks de idade de anchor.
- עדכוני סכימה Norito כוללים הזמנת קנוניקו עבור תשומות ציבוריות; בדיקות של קידוד הלוך ושוב.
- נסיעות הלוך ושוב של מטען מוצפן מוצפן באמצעות בדיקות יחידה (`crates/iroha_data_model/src/confidential.rs`). Vectores de wallet de acompanhamento vao anexar תמלילים AEAD canonicos para auditores. `norito.md` תיעוד או כותרת על חוט עבור מעטפה.

## Integracao IVM e syscall
- Introduzir syscall `VERIFY_CONFIDENTIAL_PROOF` aceitando:
  - `circuit_id`, `version`, `scheme`, `public_inputs`, `proof`, או `ConfidentialStateDelta { asset_id, nullifiers, commitments, enc_payloads }` התוצאה.
  - O syscall carrega metadata לעשות מאמת לעשות את הרישום, Aplica limites de tamanho/tempo, cobra gas determinista, e so aplica o delta se a proof tiver sucesso.
- חשיפה מארח או תכונה לקריאה בלבד `ConfidentialLedger` לצילומי מצב של Merkle root ו-estado de nullifier; a biblioteca Kotodama fornece helpers de assembly de witness e validacao de schema.
- Docs de pointer-ABI פורם אטualizados para esclarecer layout do buffer de proof e handles de registry.

## Negociacao de capacidades de node
- O לחיצת יד anuncia `feature_bits.confidential` junto com `ConfidentialFeatureDigest { vk_set_hash, poseidon_params_id, pedersen_params_id, conf_rules_version }`. משתתפי validadores Requer `confidential.enabled=true`, `assume_valid=false`, זיהויים של backend לעשות אימות זהות e digests que correspondem; לא תואם falham o handshake com `HandshakeConfidentialMismatch`.
- Config supporta `assume_valid` apenas para observers: quando desabilitado, encontrar instrucoes confidenciais gera `UnsupportedInstruction` determinista sem panic; quando habilitado, observers aplicam deltas declarados sem verificar הוכחות.
- Mempool rejeita transacoes confidenciais se a capacidade local estiver desabilitada. Filtros de gossip evitam enviar transacoes shielded para peers incompativeis enquanto encaminham cegamente IDs de Verifier desconhecidos dentro de limites de tamanho.

### לחיצת יד של Matriz de| Anuncio remoto | Resultado para nodes validadores | Notas de Operador |
|----------------------|--------------------------------|----------------|
| `enabled=true`, `assume_valid=false`, התאמה עורפית, התאמה לעיכול | Aceito | O peer chega ao estado `Ready` e participa de proposta, voto e RBC fan-out. Nenhuma acao manual requerida. |
| `enabled=true`, `assume_valid=false`, התאמה עורפית, עיכול מעופש או ausente | Rejeitado (`HandshakeConfidentialMismatch`) | O remoto deve aplicar ativacoes pendentes de registry/parametros ou aguardar o `activation_height` programado. אכלתי קוריגיר, o node segue descobrivel mas nunca entra na rotacao de consenso. |
| `enabled=true`, `assume_valid=true` | Rejeitado (`HandshakeConfidentialMismatch`) | Validadores requerem verificacao de proofs; הגדר o remoto como observer com Torii-רק כניסה או עמום `assume_valid=false` apos habilitar verificacao completa. |
| `enabled=false`, campos omitidos (build desatualizado), או Backend de Verifier diferente | Rejeitado (`HandshakeConfidentialMismatch`) | Peers desatualizados ou parcialmente atualizados nao podem entrar na rede de consenso. אטואלי עבור שחרור אטואלי e garanta que o tuple backend + digest corresponde antes de reconectar. |

משקיפים que intencionalmente pulam verificacao de proofs נאו דעוום abrir conexoes de consenso contra validadores com שערי יכולת. Eles ainda podem ingerir blocos via Torii או APIs de arquivo, mas a rede de consenso os rejeita ate anunciarem capacidades compativeis.

### Politica de pruning de reveal e retencao de nullifier

ספרי החשבונות פיתחו מחדש את ההיסטוריות המספיקות לבדיקת הערות ותעודות אודיטוריות של ממשל. ברירת מחדל של פוליטיקה, aplicada por `ConfidentialLedger`, e:- **Retencao de nullifiers:** manter nullifiers gastos por um *minimo* de `730` dias (24 שניות) apos a altura de gasto, ou a janela regulatoria obrigatoria se for maior. Operadores podem estender a janela via `confidential.retention.nullifier_days`. מבטלים mais novos que a janela DEVEM permanecer consultavis via Torii para que auditores provem ausencia de double-spend.
- **גיזום מגלה:** מגלה שקיפות (`RevealConfidential`) התחייבויות פודאם associados imediatamente apos o bloco finalizar, mas o nullifier consumido continua sujeito a regra de retencao acima. Eventos `ConfidentialEvent::Unshielded` רישום או מונטנטה פומבי, נמען e hash de proof para que reconstruir מגלה היסטוריות נאו אקסיג'ה או ciphertext podado.
- **מחסומי גבולות:** מחסומי גבולות מתגלגלים cobrindo o maior entre `max_anchor_age_blocks` e a janela de retencao. Nodes compactam checkpoints antigos apenas depois que todos os nullifiers no intervalo expiram.
- **Remediacao de digest stale:** se `HandshakeConfidentialMismatch` ocorrer por drift de digest, operadores devem (1) verificar que as janelas de retencao de nullifiers estao alinhadas no cluster, (2) rodar Norito rodar Sumeragi00024 retidos, e (3) reployar o manifest atualizado. Nullifiers podados prematuramente devem ser restaurados לעשות אחסון קר antes de reingressar na red.

Documente עוקף את locais no runbook de operacoes; politicas de governance que estendem a Janela de retencao devem atualizar configuracao de node e planos de storage de arquivo em lockstep.

### Fluxo de eviction e recovery

1. Durante o dial, `IrohaNetwork` בהשוואה ל-capacidades anunciadas. Qualquer mismatch levanta `HandshakeConfidentialMismatch`; a conexao e fechada e o peer permanece na fila de discovery sem ser promovido a `Ready`.
2. A falha aparece no log do servico de rede (כולל עיכוב מרחוק ו-backend), e Sumeragi נונקה סדר היום o peer para proposta ou voto.
3. מפעילים מתקינים את הרישום המאמתים והקשרים של הפרמטרים (`vk_set_hash`, `pedersen_params_id`, `poseidon_params_id`) או תוכנת `next_conf_features` עם `next_conf_features` com0002030X על I18030X. אומה vez que o לעכל עולה בקנה אחד, o proximo לחיצת יד טם sucesso automaticamente.
4. ראה עמיתים מעופש דיפונדir um bloco (לדוגמה, דרך משחק חוזר של arquivo), validadores o rejeitam deterministicamente com `BlockRejectionReason::ConfidentialFeatureDigestMismatch`, mantendo o estado do dodger consequent na red.

### Fluxo de Handshake Seguro Contra שידור חוזר1. Cada tentativa outbound aloca material de chave Noise/X25519 novo. O payload de handshake assinado (`handshake_signature_payload`) concatena as chaves publicas efemeras local e remota, o endereco de socket anunciado codificado em Norito e, Quando compilado com I18NIcador de dechain, o22NIcador de dechain, o22. A mensagem e encriptada com AEAD antes de sair do node.
2. O responder recomputa o payload com a orderem de chaves peer/local invertida e verifica a assinatura Ed25519 embutida em `HandshakeHelloV1`. Como ambas as chaves efemeras e o endereco anunciado fazem parte do dominio de assinatura, שידור חוזר של אומה מנסאגם capturada contra outro peer או recuperar uma conexao stale falha deterministicamente.
3. Flags de capacidade confidencial e o `ConfidentialFeatureDigest` viajam dentro de `HandshakeConfidentialMeta`. O receptor compar o tuple `{ enabled, assume_valid, verifier_backend, digest }` com seu `ConfidentialHandshakeCaps` local; qualquer mismatch sai cedo com `HandshakeConfidentialMismatch` antes de o transporte transitar para `Ready`.
4. מפעילים DEVEM מחדש או לעכל (באמצעות `compute_confidential_feature_digest`) ויצירת צמתים מחדש באמצעות רישום/פוליטיקה מותאמת לפני ההתקדמות. עמיתים anunciando digests antigos continuam falhando o לחיצת יד, evitando que estado stale reentre no set validador.
5. Sucessos e falhas de לחיצת יד atualizam contadores padrao `iroha_p2p::peer` (`handshake_failure_count`, helpers de taxonomia de erros) emitem logs estruturados com o peer ID remoto e o o טביעת אצבע לעכל. מעקב אחר אינדיקציות עבור שידורים חוזרים של זיהוי או הגדרות שגויות במהלך ההפצה.

## מטענים וניהול מפתחות
- Hierarquia de derivacao por account:
  - `sk_spend` -> `nk` (מפתח מבטל), `ivk` (מפתח צפייה נכנס), `ovk` (מפתח צפייה יוצא), `fvk`.
- מטענים משותפים באמצעות AEAD com מפתחות משותפים נגזרות של ECDH; הצג מפתחות של אודיטור אופציונאי פודם סר אנקסאדות פלטים תואמים נכס פוליטי.
- Adicoes ao CLI: `confidential create-keys`, `confidential send`, `confidential export-view-key`, tooling de auditor para descriptografar מזכרים, e o helper `iroha app zk envelope` para produzir/inspecionar envelopes I008NT030X offline. Torii expoe o mesmo fluxo de derivacao via `POST /v1/confidential/derive-keyset`, retornando formas hex e base64 para que wallets busquem hierarquias de chave programaticamente.## גז, מגביל את ה-DoS
- לוח זמנים לקביעת גז:
  - Halo2 (Plonkish): בסיס `250_000` גז + `2_000` גז עבור קלט ציבורי.
  - `5` בתים חסין גז, מטען מבטל (`300`) ומחויבות (`500`).
  - Operadores podem sobrescrever essas constantes דרך configuracao do node (`confidential.gas.{proof_base, per_public_input, per_proof_byte, per_nullifier, per_commitment}`); תפוצה של mudancas ללא הפעלה או ללא טעינה חמה מחדש עם קבצי תצורה או אפליקציית דטרמיניסטיקה ללא אשכול.
- מגבלת duros (ברירת מחדל תצורות):
- `max_proof_size_bytes = 262_144`.
- `max_nullifiers_per_tx = 8`, `max_commitments_per_tx = 8`, `max_confidential_ops_per_block = 256`.
- `verify_timeout_ms = 750`, `max_anchor_age_blocks = 10_000`. Proofs que excedem `verify_timeout_ms` abortam a instrucao deterministicamente (הקלפיות הממשל emitem `proof verification exceeded timeout`, `VerifyProof` retorna erro).
- מכסות מובטחות לחיות: `max_proof_bytes_block`, `max_verify_calls_per_tx`, `max_verify_calls_per_block`, ו-`max_public_inputs` בוני בלוקים מוגבלים; `reorg_depth_bound` (>= `max_anchor_age_blocks`) שולט במחסומי הגבול.
- A execucao runtime agora rejeita transacoes que excedem esses limites por transacao ou por bloco, emitindo erros `InvalidParameter` deterministas e mantendo o estado do do book inalterado.
- Mempool prefiltra transacoes confidenciais por `vk_id`, tamanho de proof e idade de anchor antes de invocar o Verifier para manter uso de recursos limitado.
- A verificacao para deterministicamente em timeout ou violacao de limite; transacoes falham com erros explicitos. Backends SIMD סאו אופציונאי mas nao alteram o Accounting de Gas.

### Baselines de calibracao e gates de aceitacao
- **Plataformas de referencia.** Rodadas de calibracao DEVEM cobrir os tres perfis abaixo. Rodadas sem todos os perfis sao rejeitadas אין סקירה.

  | פרפיל | ארקוויטטורה | CPU / Instancia | Flags de compilador | פרופוזיטו |
  | --- | --- | --- | --- | --- |
  | `baseline-simd-neutral` | `x86_64` | AMD EPYC 7B12 (32c) או Intel Xeon Gold 6430 (24c) | `RUSTFLAGS="-C target-feature=-avx,-avx2,-fma"` | Estabelece valores piso sem intrinsics vetoriais; usado para ajustar טבלאות דה custo fallback. |
  | `baseline-avx2` | `x86_64` | Intel Xeon Gold 6430 (24c) | שחרור ברירת מחדל | Valida o path AVX2; checa se os ganhos SIMD ficam dentro da tolerancia do gas neutral. |
  | `baseline-neon` | `aarch64` | AWS Graviton3 (c7g.4xlarge) | שחרור ברירת מחדל | Garante que o backend NEON קביעות קבועות ו-alinhado aos לוחות זמנים x86. |

- **רתמת אמת מידה.** Todos os relatorios de calibracao de gas DEVEM ser produzidos com:
  - `CRITERION_HOME=target/criterion cargo bench -p iroha_core isi_gas_calibration -- --sample-size 200 --warm-up-time 5 --save-baseline <profile-label>`
  - `cargo test -p iroha_core bench_repro -- --ignored` לאישור מתקן קבוע.
  - `CRITERION_HOME=target/criterion cargo bench -p ivm gas_calibration -- --sample-size 200 --warm-up-time 5 --save-baseline <profile-label>` semper que custos de opcode do VM mudarem.

- **רנדומנס fixa.** Exporte `IROHA_CONF_GAS_SEED=conf-gas-seed-2026Q1` antes de rodar ספסלים para que `iroha_test_samples::gen_account_in` mude para o caminho determinista `KeyPair::from_seed`. O רתום imprime `IROHA_CONF_GAS_SEED_ACTIVE=...` uma vez; se a variavel faltar, סקירה DEVE falhar. Qualquer utilidade nova de calibracao deve continuar honrando esta env var ao introduzir אקראיות auxiliar.- **Captura de resultados.**
  - טען את קריטריון הסיכומים (`target/criterion/**/raw.csv`) עבור קובץ פרטי ללא פרסום.
  - Armazenar metricas derivadas (`ns/op`, `gas/op`, `ns/gas`) לא [ספר חשבונות סודיות של כיול גז](./confidential-gas-calibration)
  - Manter os dois ultimos baselines por perfil; תמונות מצב של apagar mais antigos uma vez validado o relatorio mais novo.

- **Tolerancias de aceitacao.**
  - Deltas de gas entre `baseline-simd-neutral` e `baseline-avx2` DEVEM permanecer <= +/-1.5%.
  - Deltas de gas entre `baseline-simd-neutral` e `baseline-neon` DEVEM permanecer <= +/-2.0%.
  - הצעות ה-calibracao que excedem esses esses esses requerem ajustes de לוח הזמנים או אום RFC explicando a discrepancia e a mitigacao.

- **רשימת בדיקה לביקורת.** שולחים בתשובות ל:
  - כולל `uname -a`, trechos de `/proc/cpuinfo` (דגם, דריכה), ו-`rustc -Vv` ללא לוג קליברקאו.
  - Verificar que `IROHA_CONF_GAS_SEED` aparece na saida do ספסל (כמו ספסלים לקדם זרע אטיביה).
  - Garantir que תכונה דגלים לעשות קוצב e do Verifier סודי espelhem producao (`--features confidential,telemetry` או Rodar benches com Telemetry).

## תצורת אופרות
- `iroha_config` adiciona a secao `[confidential]`:
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
- Telemetria emite metricas agregadas: `confidential_proof_verified`, `confidential_verifier_latency_ms`, `confidential_proof_bytes_total`, `confidential_nullifier_spent`, `confidential_commitments_appended`, `IROHA_CONF_GAS_SEED_ACTIVE=...` 0,5 expor, `IROHA_CONF_GAS_SEED_ACTIVE=...`,5 dados em claro.
- RPC של שטחים:
  - `GET /confidential/capabilities`
  - `GET /confidential/zk_registry`
  - `GET /confidential/params`

## אסטרטגיה של אשכים
- Determinismo: randomizacao de transacoes dentro de blocos gera Merkle roots e sets de nullifier identicos.
- Resiliencia a reorg: reorgs סימולרי multi-bloco com anchors; מבטלים permanecem estaveis e anchors stale sao rejeitados.
- Invariantes de gas: בדוק את השימוש של גז זהות צמתים com e sem aceleracao SIMD.
- בדיקת גבול: הוכחה ללא טטו דה טמנהו/גז, ספירת כניסה/יציאה מקסימלית, פסק זמן של אכיפה.
- מחזור חיים: אופרות הממשל למען אטיביקאו/הפרקאקאו דה מאמת ופרמטרים, בדיקות גסטו אפוס רוטאקאאו.
- מדיניות FSM: transicoes permitidas/negadas, delays de transicao pendente e rejeicao de mempool perto de alturas efetivas.
- חירום רישום: משיכת חירום congela assets afetados em `withdraw_height` e rejeita proofs depois.
- שער יכולת: validadores com `conf_features` divergentes rejeitam blocos; observers com `assume_valid=true` acompanham sem afetar consenso.
- Equivalencia de estado: מאמת צמתים/מלא/תצפיתן שורשי שורשי ארץ זהים בקדמת הקאנוניקה.
- שלילי מטושטש: מוכיח פגמים, מטען רב-ממדי ומבטל את ההגדרה.## מיגרסאו
- דגל התכונה של השקת com: אכלה שלב C3 טרמינר, `enabled` ברירת מחדל a `false`; צמתים anunciam capacidades antes de entrar אין סט validador.
- נכסים transparentes nao sao afetados; instrucoes confidenciais requerem entradas de registry e negociacao de capacidades.
- Nodes compilados sem suporte confidencial rejeitam blocos relevantes deterministicamente; nao podem entrar no set validador mas podem operar como observers com `assume_valid=true`.
- בראשית מופיעה הרשמת רישום, ערכות פרמטרים, פוליטיקה סודית לנכסים ומפתחות אודיטור.
- Operadores seguem runbooks publicados para rotacao de registry, transicoes de politica e נסיגת חירום עבור שדרוגים דטרמיניסטים.

## Trabalho pendente
- בנצ'מרק של parametros Halo2 (תמונה מעגל, אסטרטגיה של חיפוש) ורשם תוצאות ללא פנקס קליברקאו עבור ברירות מחדל של גז/זמן קצוב ו-refresh proximo proximo de `confidential_assets_calibration.md`.
- סיום הגילוי הפוליטי של המבקר וממשקי ה-API של אסוציאציות של צפייה סלקטיבית, חיבור או אישור זרימת עבודה עם Torii אסיימו של טיוטת הממשל עבור Assinado.
- Estender או esquema de Witness הצפנת עבור פלטי ריבוי נמענים ותזכירים באצווה, מסמכים או פורמטים לעשות מעטפה ליישום SDK.
- נציבות אומה revisao de seguranca externa de circuitos, registries e procedimentos de rotacao de parametros e arquivar os achados ao lado dos relatorios internos de auditoria.
- ממשקי API ספציפיים ל-Reconciliacao de spendness ל-auditors ו-Publicar Guia de escopo de view-key עבור ספקי ארנק ליישם אותם כמו סמנטיקה של אטסטקאו.## Phasing de implementacao
1. **שלב M0 - עצור התקשות הספינה**
   - [x] Derivacao de nullifier segue o design Poseidon PRF (`nk`, `rho`, `asset_id`, `chain_id`) com הזמנה קביעת התחייבויות aplicado nas ledgerualizacado nas.
   - [x] Execucao aplica limites de tamanho de proof e quotas confidenciais por transacao/por bloco, rejeitando transacoes fora de budget com erros deterministas.
   - [x] לחיצת יד P2P anuncia `ConfidentialFeatureDigest` (תקציר אחורי + טביעות אצבעות של הרישום) ו-falha dismatches deterministicamente via `HandshakeConfidentialMismatch`.
   - [x] Remover נכנס לפאניקה בנתיבי הביצוע סודיים ותפקידים נוספים שערים עבור צמתים לא תואמים.
   - [ ] תקציבי זמן קצוב אפליקציוניים עושים אימות ומגבלות של פרופונדידה דה ריורג למחסומי גבול.
     - [x] Budgets de timeout de verificacao aplicados; הוכחות que excedem `verify_timeout_ms` agora falham deterministicamente.
     - [x] מחסומי גבול agora respeitam `reorg_depth_bound`, מחסומי פונדו mais antigos que a janela configurada e mantendo snapshots deterministas.
   - Introduzir `AssetConfidentialPolicy`, מדיניות FSM ו-gates de enforcement para instrucos mint/transfer/reveal.
   - Commit `conf_features` ללא כותרות de bloco e recusar participacao de validadores quando digests de registry/parametros divergem.
2. **שלב M1 - רישום ופרמטרים**
   - Entregar registries `ZkVerifierEntry`, `PedersenParams`, e `PoseidonParams` com ops de governance, ancoragem de genesis e gestao de cache.
   - Conectar syscall עבור חיפושי רישום, מזהי לוח זמנים של גז, גיבוב סכימה, בדיקות ובדיקות.
   - מצא פורמט מטען מטען גרסה 1, ותמיכת מפתחות לארנק, ותומכים ב-CLI לשמירה על סודיות.
3. **שלב M2 - ביצועי גז e**
   - לוח זמנים יישום גז דטרמיניסטה, contadores por bloco e רתמות de benchmark com telemetria (latencia de verificacao, tamanhos de proof, rejeicoes de mempool).
   - נקודות ביקורת של Endurecer CommitmentTree, מטען LRU ומדדים מבטלים עבור עומסי עבודה מרובי נכסים.
4. **Phase M3 - Rotacao e tooling de wallet**
   - Habilitar aceitacao de proofs multi-parametro e multi-versao; suportar ativacao/deprecacao guiada por governance com runbooks de transicao.
   - הפעל SDK/CLI זרימות, זרימות עבודה של סריקת אודיטור וכלי התאמה להוצאות.
5. **שלב M4 - ביקורת פעולות**
   - זרימות עבודה של Fornecer של מפתחות המבקר, ממשקי API של חשיפה סלקטיבית, ספרי הפעלה אלקטרוניים.
   - Agendar revisao externa de criptografia/seguranca e publicar achados em `status.md`.

אבני דרך אטואליזה בשלב הבא לעשות מפת דרכים e בדיקות associados para manter garantias de execucao determinista on red blockchain.