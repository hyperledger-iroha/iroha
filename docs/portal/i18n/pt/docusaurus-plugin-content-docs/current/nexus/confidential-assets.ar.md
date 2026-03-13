---
lang: pt
direction: ltr
source: docs/portal/docs/nexus/confidential-assets.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: الاصول السرية وتحويلات ZK
description: مخطط Fase C للدوران blindado والسجلات وضوابط المشغلين.
slug: /nexus/ativos-confidenciais
---
<!--
SPDX-License-Identifier: Apache-2.0
-->
# تصميم الاصول السرية وتحويلات ZK

## الدوافع
- تقديم تدفقات اصول blindado اختيارية حتى تتمكن الدومينات من حفظ خصوصية المعاملات دون تغيير الدوران الشفاف.
- الحفاظ على التنفيذ الحتمي عبر عتاد مدققين غير متجانس مع ابقاء توافق Norito/Kotodama ABI v1.
- تزويد المدققين والمشغلين بضوابط دورة الحياة (تفعيل, تدوير, سحب) للدوائر والمعلمات التشفيرية.

## نموذج التهديدات
- المدققون honesto-mas-curioso: ينفذون الاجماع بامانة لكنهم يحاولون فحص razão/estado.
- مراقبو الشبكة يرون بيانات الكتل والمعاملات المرسلة عبر fofocas; Não há fofocas e fofocas.
- خارج النطاق: تحليل حركة المرور خارج الدفتر, خصوم كميون (يتابعون في PQ roadmap), وهجمات توفر ledger.

## نظرة عامة على التصميم
- يمكن للاصول اعلان *piscina blindada* اضافة الى الارصدة الشفافة؛ يتم تمثيل الدوران compromissos de عبر blindados تشفيرية.
- Notas adicionais `(asset_id, amount, recipient_view_key, blinding, rho)` com:
  - Compromisso: `Comm = Pedersen(params_id || asset_id || amount || recipient_view_key || blinding)`.
  - Nullifier: `Null = Poseidon(domain_sep || nk || rho || asset_id || chain_id)` مستقل عن ترتيب notas.
  - Carga criptografada: `enc_payload = AEAD_XChaCha20Poly1305(ephemeral_shared_key, note_plaintext)`.
- تنقل المعاملات payloads `ConfidentialTransfer` المرمزة بنوريتو وتحتوي:
  - Entradas públicas: âncora Merkle, anuladores, compromissos, identificação de ativos, نسخة الدائرة.
  - Cargas úteis podem ser usadas e carregadas.
  - Prova de conhecimento zero تؤكد حفظ القيمة والملكية والتفويض.
- يتم التحكم في verificando chaves ومجموعات المعلمات عبر سجلات على الدفتر مع نوافذ تفعيل؛ ترفض العقد التحقق من provas تشير الى ادخالات مجهولة او مسحوبة.
- تلتزم رؤوس الاجماع ب digest ميزات السرية النشطة كي لا تقبل الكتل الا عند تطابق حالة السجلات والمعلمات.
- بناء provas يستخدم Halo2 (Plonkish) بدون configuração confiável؛ Groth16 e SNARK اخرى غير مدعومة عمدا في v1.

### Fixtures

اغلفة memo السرية تشحن الان مع fixture قانوني في `fixtures/confidential/encrypted_payload_v1.json`. O envelope v1 do envelope v1 pode ser usado para configurar SDKs e SDKs. التحليل. Use o modelo de dados Rust (`crates/iroha_data_model/tests/confidential_encrypted_payload_vectors.rs`) e Swift (`IrohaSwift/Tests/IrohaSwiftTests/ConfidentialEncryptedPayloadTests.swift`) para definir o fixture مباشرة, لضمان توافق Codificação Norito وسطوح الاخطاء وتغطية الانحدار مع تطور الكودك.

Os SDKs do Swift são protegidos por escudo e cola JSON, como: `ShieldRequest` com nota de compromisso, 32 dias, carga útil e débito metadados, você pode usar `IrohaSDK.submit(shield:keypair:)` (e `submitAndWait`) para obter informações e usar `/v2/pipeline/transactions`. يقوم المساعد بالتحقق من اطوال compromissos, ويمرر `ConfidentialEncryptedPayload` como codificador Norito, ويعكس layout `zk::Shield` الموضح ادناه حتى تبقى المحافظ متزامنة مع Rust.

## Compromissos الاجماع e gating القدرات
- تكشف رؤوس الكتل `conf_features = { vk_set_hash, poseidon_params_id, pedersen_params_id, conf_rules_version }`; Não digerir o hash do site e não fazer o download do arquivo.
- يمكن للحوكمة تجهيز الترقيات ببرمجة `next_conf_features` com `activation_height` مستقبلي؛ وحتى ذلك الارتفاع يجب على منتجي الكتل الاستمرار em um resumo do site.
- Você pode usar o código `confidential.enabled = true` e `assume_valid = false`. ترفض فحوصات البدء الانضمام الى مجموعة المدققين اذا فشل اي شرط او اختلفت `conf_features` محليا.
- O handshake P2P é baseado em `{ enabled, assume_valid, conf_features }`. Todos os peers estão disponíveis para uso em `HandshakeConfidentialMismatch` e não podem ser encontrados no site oficial.
- نتائج التوافق بين المدققين, observadores, والـ peers القديمة تلتقط في مصفوفة handshake ضمن [Node Capability Negotiation](#node-capability-negotiation). O aperto de mão é `HandshakeConfidentialMismatch` e o peer é o mesmo que o digest.
- يمكن للمراقبين غير المدققين ضبط `assume_valid = true`; Não se preocupe, você pode fazer isso sem problemas.## سياسات الاصول
- Verifique o valor do `AssetConfidentialPolicy` e verifique o valor:
  - `TransparentOnly`: Código de barras Todos os cabos de proteção são blindados (`MintAsset`, `TransferAsset`, الخ) e blindados.
  - `ShieldedOnly`: Não há nenhum problema com o computador e com o computador. Use `RevealConfidential` para remover o problema.
  - `Convertible`: Não há nenhuma rampa de acesso blindada e nenhuma rampa de ligação/desligamento blindada.
- تتبع السياسات FSM مقيد لمنع تعلق الاموال:
  - `TransparentOnly → Convertible` (piscina blindada).
  - `TransparentOnly → ShieldedOnly` (não disponível).
  - `Convertible → ShieldedOnly` (não disponível).
  - `ShieldedOnly → Convertible` (você pode usar as notas para fazer isso).
  - `ShieldedOnly → TransparentOnly` غير مسموح الا اذا كان piscina blindada فارغا او قامت الحوكمة بترميز هجرة تزيل السرية عن notas المتبقية.
- تعليمات الحوكمة تضبط `pending_transition { new_mode, effective_height, previous_mode, transition_id, conversion_window }` عبر ISI `ScheduleConfidentialPolicyTransition` ويمكنها الغاء التغييرات المجدولة بـ `CancelConfidentialPolicyTransition`. يضمن تحقق mempool عدم عبور اي معاملة لارتفاع الانتقال, ويفشل الادراج بشكل حتمي اذا تغير فحص Não há nada melhor do que isso.
- يتم تطبيق الانتقالات المعلقة تلقائيا عند فتح كتلة جديدة: عند دخول ارتفاع الكتلة O nome do arquivo (`ShieldedOnly`) e o `effective_height`, o tempo de execução, o `AssetConfidentialPolicy` e os metadados `zk.policy` é um problema. Para que você possa usar o `ShieldedOnly`, o tempo de execução e o tempo de execução não serão necessários. السابق.
- مقابض التهيئة `policy_transition_delay_blocks` e `policy_transition_window_blocks` تفرض اشعارا ادنى وفترات سماح للسماح بتحويل محافظ Não.
- `pending_transition.transition_id` يعمل ايضا كـ identificador de auditoria; Você pode usar a rampa de ativação/desativação da rampa de ativação/desativação.
- `policy_transition_window_blocks` 720 (cerca de 12 horas por tempo de bloco 60s). Certifique-se de que o produto esteja funcionando corretamente.
- Gênesis manifesta وتدفقات CLI تعرض السياسات الحالية والمعلقة. منطق admissão يقرأ السياسة وقت التنفيذ لتاكيد ان كل تعليمة سرية مصرح بها.
- قائمة تحقق الهجرة - انظر “Sequenciamento de migração” ادناه لخطة ترقية على مراحل يتبعها Milestone M0.

#### مراقبة الانتقالات عبر Torii

Verifique se o `GET /v2/confidential/assets/{definition_id}/transitions` é o `AssetConfidentialPolicy`. O JSON de carga útil é definido como ID de ativo, mas o valor do `current_mode` é definido como `current_mode`. Placa de identificação (referência de código de barras `Convertible`), e placa de identificação `vk_set_hash`/Poseidon/Pedersen المتوقعة. عند وجود انتقال حوكمة معلق يتضمن الرد ايضا:

- `transition_id` - identificador de auditoria baseado em `ScheduleConfidentialPolicyTransition`.
-`previous_mode`/`new_mode`.
-`effective_height`.
- `conversion_window` e `window_open_height` (não é possível usar o ShieldedOnly).

Exemplo:

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

Use o `404` para remover o problema. Você pode usar o `pending_transition` para `null`.

### آلة حالات السياسة| الوضع الحالي | الوضع التالي | Informações | Atualizado em Effective_height | Produtos |
|--------------------|------------------|------------------------------------------------------------------------------------------|-------------------------------------------------------------------------------------------------------------------|-------------------------------------------------------------------------------------------|
| Somente Transparente | Conversível | O parâmetro é verificador/parâmetro. O `ScheduleConfidentialPolicyTransition` é o `effective_height ≥ current_height + policy_transition_delay_blocks`. | O código de barras é `effective_height`; Piscina blindada متاحا فوريا.                   | A solução de problemas de segurança pode ser usada para evitar problemas.               |
| Somente Transparente | Somente blindado | Verifique o `policy_transition_window_blocks ≥ 1`.                                                         | Execute o tempo de execução de `Convertible` para `effective_height - policy_transition_window_blocks`; Use `ShieldedOnly` para `effective_height`. | Não se preocupe, você pode fazer isso com cuidado.   |
| Conversível | Somente blindado | Use o `effective_height ≥ current_height + policy_transition_delay_blocks`. يجب على الحوكمة توثيق (`transparent_supply == 0`) عبر metadados de auditoria; O tempo de execução não é o mesmo. | Não há nada que você possa fazer. Para obter mais informações sobre o `effective_height`, você pode usar o `PolicyTransitionPrerequisiteFailed`. | Não há nenhum problema em John.                                     |
| Somente blindado | Conversível | انتقال مجدول; Não há nenhum problema com isso (`withdraw_height`).                                    | O modelo é `effective_height`; تعاد فتح revelar rampas بينما تبقى notas protegidas صالحة.                           | يستخدم لنوافذ الصيانة او مراجعات المدققين.                                          |
| Somente blindado | Somente Transparente | Você pode usar o `shielded_supply == 0` e usar o `EmergencyUnshield` (você pode usar o `EmergencyUnshield`). | O tempo de execução do `Convertible` é o `effective_height`; عند الارتفاع تفشل التعليمات السرية بقوة ويعود الاصل الى وضع شفاف فقط. | خروج كملاذ اخير. يتم الغاء الانتقال تلقائيا اذا تم صرف اي nota سرية خلال النافذة. |
| Qualquer | Igual ao atual | `CancelConfidentialPolicyTransition` não é compatível.                                                        | Eu usei `pending_transition`.                                                                          | يحافظ على الوضع الراهن؛ مذكور للاكتمال.                                             |

Certifique-se de que o produto esteja funcionando corretamente. O tempo de execução é o tempo de execução do aplicativo de gerenciamento de tempo de execução. فشل الشروط يعيد الاصل الى وضعه السابق ويطلق `PolicyTransitionPrerequisiteFailed` عبر التليمتري واحداث الكتلة.

### Sequenciamento de migração

1. **Preparar registros:** فعّل كل مدخلات verificador والمعلمات المشار اليها في السياسة المستهدفة. O código `conf_features` está disponível para os pares do site.
2. **Prepare a transição:** Escolha `ScheduleConfidentialPolicyTransition` para `effective_height` ou `policy_transition_delay_blocks`. O código `ShieldedOnly` é o mesmo que o `window ≥ policy_transition_window_blocks`.
3. **Publicar orientação do operador:** سجّل `transition_id` المعاد ووزع runbook para on/off-ramp. Verifique o valor do produto em `/v2/confidential/assets/{id}/transitions` para obter mais informações.
4. **Aplicação de janela:** عند فتح النافذة يحول runtime السياسة الى `Convertible`, ويصدر `PolicyTransitionWindowOpened { transition_id }` ويبدأ في رفض طلبات الحوكمة المتعارضة.
5. **Finalizar ou abortar:** عند `effective_height` يتحقق runtime من الشروط المسبقة (عرض شفاف صفر, عدم وجود سحب طارئ، sim). النجاح يقلب السياسة للوضع المطلوب؛ Se você usar `PolicyTransitionPrerequisiteFailed`, verifique se está tudo bem.
6. **Atualizações de esquema:** بعد نجاح الانتقال ترفع الحوكمة نسخة مخطط الاصل (مثلا `asset_definition.v2`) e CLI O `confidential_policy` não contém manifestos. توجه وثائق ترقية genesis المشغلين لاضافة اعدادات السياسة وبصمات registro قبل اعادة تشغيل المدققين.

الشبكات الجديدة التي تبدأ مع تمكين السرية ترمز السياسة المطلوبة مباشرة em gênese. Isso significa que você pode usar o computador para obter mais informações sobre o seu negócio. حتمية وتمتلك المحافظ وقتا للتكيف.

### نسخ Norito manifestos e- يجب ان تتضمن Genesis manifesta `SetParameter` للمفتاح المخصص `confidential_registry_root`. O payload é Norito JSON é definido para `ConfidentialRegistryMeta { vk_set_hash: Option<String> }`: O valor do verificador (`null`) é o verificador. O código hexadecimal é de 32 dígitos (`0x…`) e o valor do hexadecimal é `compute_vk_set_hash`. verificador no manifesto. ترفض العقد البدء اذا كان المعامل مفقودا او الهاش لا يطابق كتابات السجل المرمزة.
- يضمن `ConfidentialFeatureDigest::conf_rules_version` on-wire نسخة تخطيط manifesto. A versão v1 é `Some(1)` e `iroha_config::parameters::defaults::confidential::RULES_VERSION`. عند تطور القواعد ارفع الثابت, اعادة توليد manifestos, واطلاق البيناريات معا؛ Verifique se o produto está em `ConfidentialFeatureDigestMismatch`.
- ينبغي ان تجمع Manifestos de ativação تحديثات registro وتغييرات دورة حياة المعلمات وانتقالات السياسة بحيث يبقى digest O que isso significa:
  1. Faça o registro do registro (`Publish*`, `Set*Lifecycle`) para ficar offline e compilar o resumo. `compute_confidential_feature_digest`.
  2. اصدِر `SetParameter::custom(confidential_registry_root, {"vk_set_hash": "0x…"})` باستخدام الهاش المحسوب حتى يتمكن peers المتاخرون من استعادة digest الصحيح حتى Isso é importante para o registro do sistema.
  3. Verifique o `ScheduleConfidentialPolicyTransition`. O código de barras do `transition_id` é o mesmo do site `transition_id`. manifests em tempo de execução.
  4. احفظ بايتات manifest e SHA-256 وdigest المستخدم في خطة التفعيل. يتحقق المشغلون من الثلاثة قبل التصويت لتجنب الانقسام.
- عندما تتطلب عمليات الاطلاق cut-over مؤجلا, سجل الارتفاع المستهدف في معامل مخصص مرافق (مثلا `custom.confidential_upgrade_activation_height`). Este é o resumo do resumo.

## دورة حياة verificador والمعلمات
### Registro ZK
- O razão `ZkVerifierEntry { vk_id, circuit_id, version, proving_system, curve, public_inputs_schema_hash, vk_hash, vk_len, max_proof_bytes, gas_schedule_id, activation_height, deprecation_height, withdraw_height, status, metadata_uri_cid, vk_bytes_cid }` é o `proving_system`, mas o `Halo2`.
- ازواج `(circuit_id, version)` فريدة عالميا؛ يحافظ السجل على فهرس ثانوي للبحث حسب metadados. محاولات تسجيل زوج مكرر ترفض عند admissão.
- Você pode usar `circuit_id` para usar o hash Blake2b-32 e usar `public_inputs_schema_hash` (com hash Blake2b-32). para o verificador). ترفض admissão السجلات التي تهمل هذه الحقول.
- تعليمات الحوكمة تشمل:
  - `PUBLISH` é um arquivo de metadados `Proposed`.
  - `ACTIVATE { vk_id, activation_height }` لجدولة تفعيل المدخل عند حدود época.
  - `DEPRECATE { vk_id, deprecation_height }` لتحديد اخر ارتفاع يمكن فيه للـ provas الاشارة الى المدخل.
  - `WITHDRAW { vk_id, withdraw_height }` لاغلاق طارئ؛ Você pode retirar a altura antes de retirar a altura.
- Gênesis manifestos يتحقق التحقق من هذا digest مقابل حالة السجل المحلية قبل انضمام العقدة للاجماع.
- Verifique e verifique o verificador `gas_schedule_id`; Você pode usar o `Active` para obter o `(circuit_id, version)` e as provas Halo2 `OpenVerifyEnvelope` Use `circuit_id`, `vk_hash` e `public_inputs_schema_hash` em qualquer lugar.

### Provando Chaves
- Chaves de prova de conteúdo verificador de metadados.
- Os SDKs da carteira estão disponíveis para PK e PK no site e no site.

### Parâmetros de Pedersen e Poseidon
- سجلات منفصلة (`PedersenParams`, `PoseidonParams`) تعكس ضوابط دورة حياة verificador, ولكل منها `params_id`, هاشات المولدات/الثوابت, وارتفاعات التفعيل/الاستبدال/السحب.
- تفصل compromissos والهاشات المجال عبر `params_id` حتى لا تعيد تدوير المعلمات استخدام انماط بت من مجموعات قديمة؛ O ID das notas de compromissos e o nulificador são usados.

## الترتيب الحتمي e anuladores
- يحافظ كل اصل على `CommitmentTree` com `next_leaf_index`; تضيف الكتل compromissos بترتيب حتمي: تمر المعاملات بترتيب الكتلة؛ Use um cabo de proteção blindado para `output_idx`.
- `note_position` مشتق من ازاحات الشجرة لكنه **ليس** جزءا من nullifier; يستخدم فقط لمسارات العضوية ضمن testemunha de prova.
- يضمن تصميم PRF ثبات nullifier عند reorgs؛ يربط مدخل PRF `{ nk, note_preimage_hash, asset_id, chain_id, params_id }`, وتستند âncoras الى جذور Merkle تاريخية محدودة بـ `max_anchor_age_blocks`.## Livro razão
1. **MintConfidential {asset_id, valor, destinatário_hint }**
   - يتطلب سياسة اصل `Convertible` e `ShieldedOnly`; Admissão من سلطة الاصل, يجلب `params_id` الحالي, يعين `rho`, يصدر compromisso ويحدث Árvore Merkle.
   - يصدر `ConfidentialEvent::Shielded` com compromisso جديد, فرق Merkle root, وhash استدعاء المعاملة للتدقيق.
2. **TransferConfidential {asset_id, prova, circuito_id, versão, nulificadores, novos_compromissos, enc_payloads, âncora_root, memorando}**
   - Use syscall em VM com prova de segurança O host e os nullifiers são os compromissos que você usa e o âncora.
   - يسجل ledger ادخالات `NullifierSet`, ويحفظ payloads مشفرة للمستلمين/المدققين, ويصدر `ConfidentialEvent::Transferred` ملخصا nullifiers والمخرجات المرتبة e hash à prova e raízes Merkle.
3. **RevealConfidential {asset_id, prova, circuito_id, versão, anulador, quantidade, destinatário_account, âncora_root }**
   - متاحة فقط للاصول `Convertible`; تتحقق prova ان قيمة nota تساوي المبلغ المكشوف, يضيف ledger الرصيد الشفاف ويحرق nota blindada بوسم nullifier كمصروف.
   - يصدر `ConfidentialEvent::Unshielded` مع المبلغ العلني وnullifiers المستهلكة ومعرفات prova وhash استدعاء المعاملة.

## Modelo de dados definido
- `ConfidentialConfig` (é necessário usar) para usar o `assume_valid`, usar gás/limites, âncora e back-end do verificador.
- `ConfidentialNote`, `ConfidentialTransfer`, e `ConfidentialMint` possuem Norito com um novo valor (`CONFIDENTIAL_ASSET_V1 = 0x01`).
- `ConfidentialEncryptedPayload` contém AEAD memo bytes em `{ version, ephemeral_pubkey, nonce, ciphertext }` e `version = CONFIDENTIAL_ENCRYPTED_PAYLOAD_V1` para XChaCha20-Poly1305.
- توجد متجهات اشتقاق المفاتيح القانونية em `docs/source/confidential_key_vectors.json`; O ponto de extremidade CLI e Torii está disponível para download.
- Use `asset::AssetDefinition` para `confidential_policy: AssetConfidentialPolicy { mode, vk_set_hash, poseidon_params_id, pedersen_params_id, pending_transition }`.
- يحفظ `ZkAssetState` ربط `(backend, name, commitment)` para verificadores de transferência/desproteção; يرفض التنفيذ provas التي لا تطابق verificação de chave المسجل (مرجعا او ضمنيا).
- `CommitmentTree` (para pontos de controle de fronteira), e `NullifierSet`, `(chain_id, asset_id, nullifier)`, e `ZkVerifierEntry` e `PedersenParams` e `PoseidonParams` no estado mundial.
- Use o mempool para `NullifierIndex` e `AnchorIndex` para obter uma âncora.
- تحديثات مخطط Norito تشمل ترتيبًا قانونيًا لـ public inputs; اختبارات ida e volta تضمن حتمية الترميز.
- Testes unitários de carga útil criptografada de ida e volta (`crates/iroha_data_model/src/confidential.rs`). ستضيف متجهات المحافظ لاحقا Transcrições AEAD قانونية للمدققين. O `norito.md` é um envelope on-wire para um envelope.

## Execute IVM e syscall
- Execute o syscall `VERIFY_CONFIDENTIAL_PROOF` como:
  - `circuit_id`, `version`, `scheme`, `public_inputs`, `proof` e `ConfidentialStateDelta { asset_id, nullifiers, commitments, enc_payloads }`.
  - يقوم syscall بتحميل verificador de metadados من السجل, يفرض حدود الحجم/الوقت, يحسب gás حتمي, ولا يطبق delta الا عند نجاح prova.
- يوفر host trait somente leitura `ConfidentialLedger` لاسترجاع snapshots لجذر Merkle وحالة nullifier; Os ajudantes Kotodama são usados ​​para testemunhar e para o esquema.
- Você deve usar o ponteiro-ABI para o buffer de prova de layout e o registro.

## تفاوض قدرات العقد
- O handshake `feature_bits.confidential` é o `ConfidentialFeatureDigest { vk_set_hash, poseidon_params_id, pedersen_params_id, conf_rules_version }`. مشاركة المدقق تتطلب `confidential.enabled=true` e `assume_valid=false` e verifier backend متطابقة وdigest متطابق؛ O aperto de mão é `HandshakeConfidentialMismatch`.
- يدعم config `assume_valid` لعقد observadores فقط: عند تعطيله, تؤدي تعليمات السرية الى `UnsupportedInstruction` حتمي بلا panic؛ عند تمكينه, يطبق observadores دلتا الحالة المعلنة دون التحقق من provas.
- يرفض mempool المعاملات السرية اذا كانت القدرة المحلية معطلة. تتجنب فلاتر fofoca ارسال المعاملات pares protegidos غير متوافقة بينما تعيد توجيه معرفات verificador غير المعروفة بشكل اعمى ضمن حدود الحجم.

### مصفوفة توافق aperto de mão| اعلان الطرف البعيد | النتيجة لعقد المدققين | ملاحظات المشغل |
|------------------|------------------|----------------|
| `enabled=true`, `assume_valid=false`, backend متطابق, resumo متطابق | مقبول | O peer é o `Ready` e a proposta e a votação e o fan-out do RBC. Não há nada que você possa fazer. |
| `enabled=true`, `assume_valid=false`, backend متطابق, digest قديم او مفقود | Meia (`HandshakeConfidentialMismatch`) | Verifique o valor do produto/serviço e verifique o código `activation_height`. Verifique se você está usando o dispositivo para obter mais informações. |
| `enabled=true`, `assume_valid=true` | مرفوض (`HandshakeConfidentialMismatch`) | يتطلب المدققون تحقق provas; Se você usar o Torii e o `assume_valid=false`, ele será removido. |
| `enabled=false`, handshake de alta qualidade (acesso direto) e back-end do verificador | مرفوض (`HandshakeConfidentialMismatch`) | pares قديمة او محدثة جزئيا لا يمكنها الانضمام لشبكة الاجماع. Isso significa que o back-end + resumo é o mais importante. |

العقد المراقِبة التي تتجاوز تحقق provas عمدا يجب الا تفتح اتصالات اجماع مع مدققين يعملون ببوابات قدرات. Você pode usar o Torii e usar o software Torii para obter mais informações. قدرات متوافقة.

### سياسة تقليم Revelar والاحتفاظ بـ Nullifier

يجب على دفاتر السرية الاحتفاظ بتاريخ كاف لاثبات حداثة notas e واعادة تشغيل تدقيقات الحوكمة. O código de barras `ConfidentialLedger` é:

- **Retenção de nulificador:** الاحتفاظ بـ nullifiers المصروفة لمدة *ادنى* `730` يوما (24 شهرا) بعد ارتفاع الصرف، او Não se preocupe com isso. Você pode usar o `confidential.retention.nullifier_days`. Você não pode usar nullifiers para obter o Torii sem usar o Torii. É melhor gastar o dobro.
- **Reveal pruning:** تقوم Reveals الشفافة (`RevealConfidential`) بتقليم comprometimentos المرتبطة فورا بعد اكتمال الكتلة, لكن nullifier المصروف Não é necessário fazer isso. تسجل احداث revelar (`ConfidentialEvent::Unshielded`) o código e a prova de hash no texto cifrado المقتطع.
- **Pontos de controle de fronteira:** تحافظ compromissos de fronteira على pontos de controle متحركة تغطي الاكبر من `max_anchor_age_blocks` ونافذة الاحتفاظ. Não há pontos de verificação para evitar nulificadores no sistema.
- **Stale digest remediation:** اذا رُفع `HandshakeConfidentialMismatch` بسبب انحراف digest, يجب على المشغلين (1) التحقق من تطابق نوافذ الاحتفاظ عبر الكتلة, (2) تشغيل `iroha_cli app confidential verify-ledger` لاعادة توليد digest مقابل مجموعة nullifier المحتفظ بها, و(3) اعادة نشر manifesto المحدث. Os nulificadores e os nulificadores podem ser usados ​​para evitar problemas.

وثق التعديلات المحلية no runbook de operações; Faça o download do seu cartão de crédito e verifique o valor do seu cartão de crédito. Então.

### تدفق الاخلاء والاستعادة

1. Verifique o código `IrohaNetwork` do sistema. Este é o `HandshakeConfidentialMismatch`; O peer na fila de descoberta está no `Ready`.
2. Faça o download do site (o resumo e o back-end do site), e o Sumeragi é o peer للتقديم او التصويت.
3. Verifique o verificador e o verificador do dispositivo (`vk_set_hash`, `pedersen_params_id`, `poseidon_params_id`) ou O `next_conf_features` é o mesmo que o `activation_height`. O resumo do resumo não é o aperto de mão.
4. اذا تمكن peer قديم من بث كتلة (مثلا عبر archive replay), يرفضها المدققون حتميا بـ `BlockRejectionReason::ConfidentialFeatureDigestMismatch` للحفاظ على اتساق razão عبر الشبكة.

### Fluxo de handshake seguro para repetição1. Use o botão Noise/X25519 para usar o Noise/X25519. A carga útil do handshake é (`handshake_signature_payload`) يربط المفاتيح العامة المؤقتة المحلية والبعيدة, nem o soquete Você pode usar o produto `handshake_chain_id`. Isso é feito pela AEAD.
2. O payload é definido como peer/local e o valor da carga útil é Ed25519 no `HandshakeHelloV1`. بما ان كلا المفتاحين المؤقتين والعنوان المعلن ضمن نطاق التوقيع, فاعادة تشغيل رسالة ملتقطة ضد peer é algo que pode ser feito por meio de peer.
3. Limpe o dispositivo e `ConfidentialFeatureDigest` para `HandshakeConfidentialMeta`. A tupla `{ enabled, assume_valid, verifier_backend, digest }` é a tupla `ConfidentialHandshakeCaps` Esse aperto de mão é `HandshakeConfidentialMismatch`, o que significa que o aperto de mão é `Ready`.
4. Verifique o resumo do resumo (`compute_confidential_feature_digest`) e faça o download do resumo. محدثة قبل اعادة الاتصال. peers التي تعلن digests قديمة تستمر في الفشل, مانعة دخول حالة قديمة الى مجموعة المدققين.
5. Apertos de mão e apertos de mão `iroha_p2p::peer` (`handshake_failure_count` وغيرها) e تصدر سجلات منظمة É um peer e um digest. راقب هذه المؤشرات لاكتشاف محاولات replay او سوء التهيئة اثناء rollout.

## ادارة المفاتيح e cargas úteis
- تسلسل اشتقاق المفاتيح لكل حساب:
  - `sk_spend` → `nk` (chave anuladora), `ivk` (chave de visualização de entrada), `ovk` (chave de visualização de saída), `fvk`.
- تستخدم notas de cargas úteis المشفرة AEAD مع مفاتيح مشتركة مشتقة من ECDH; يمكن ارفاق chaves de visualização do auditor اختيارية الى saídas حسب سياسة الاصل.
- CLI: `confidential create-keys`, `confidential send`, `confidential export-view-key`, ادوات للمدققين لفك memos, والمساعد `iroha app zk envelope` Envelopes de papel/corte Norito sem problemas. O Torii é um arquivo de código aberto que `POST /v2/confidential/derive-keyset` é usado para hex e base64 para ser usado جلب هياكل المفاتيح برمجيا.

## الغاز, الحدود, وضوابط DoS
- gás natural:
  - Halo2 (Plonkish): gás `250_000` + gás `2_000` para entrada pública.
  - `5` gás à prova de gás, مع رسوم لكل nullifier (`300`) e compromisso (`500`).
  - يمكن للمشغلين تجاوز هذه الثوابت عبر تهيئة العقد (`confidential.gas.{proof_base, per_public_input, per_proof_byte, per_nullifier, per_commitment}`), Você pode fazer isso sem precisar de mais nada.
- حدود صارمة (افتراضات قابلة للضبط):
-`max_proof_size_bytes = 262_144`.
-`max_nullifiers_per_tx = 8`, `max_commitments_per_tx = 8`, `max_confidential_ops_per_block = 256`.
-`verify_timeout_ms = 750`, `max_anchor_age_blocks = 10_000`. provas `verify_timeout_ms` تقطع التعليمة حتميا (تصويتات الحوكمة تصدر `proof verification exceeded timeout` e `VerifyProof` sim).
- Os construtores de construção são `max_proof_bytes_block`, `max_verify_calls_per_tx`, `max_verify_calls_per_block` e `max_public_inputs`. `reorg_depth_bound` (≥ `max_anchor_age_blocks`) é usado em postos de controle de fronteira.
- يرفض runtime المعاملات التي تتجاوز هذه الحدود لكل معاملة او لكل كتلة, ويصدر اخطاء `InvalidParameter` حتمية مع ابقاء حالة livro-razão دون تغيير.
- يرشح mempool المعاملات السرية مسبقا حسب `vk_id` e prova e âncora قبل استدعاء verificador للحفاظ على حدود الموارد.
- يتوقف التحقق حتميا عند timeout او تجاوز الحدود؛ Verifique o número de telefone e o número de telefone. back-ends SIMD são usados ​​para gerar gás.

### خطوط اساس المعايرة وبوابات القبول
- **Plataformas de referência.** Não há nenhuma solução para isso. Isso significa que você pode usar o dispositivo para evitar problemas.

  | الملف | المعمارية | CPU/Instância | اعلام المترجم | الغاية |
  | --- | --- | --- | --- | --- |
  | `baseline-simd-neutral` | `x86_64` | AMD EPYC 7B12 (32c) e Intel Xeon Gold 6430 (24c) | `RUSTFLAGS="-C target-feature=-avx,-avx2,-fma"` | تأسيس قيم ارضية بدون تعليمات متجهة؛ Você pode usar um substituto para isso. |
  | `baseline-avx2` | `x86_64` | Intel Xeon Ouro 6430 (24c) | versão padrão | يتحقق من مسار AVX2; Não use SIMD para usar gás. |
  | `baseline-neon` | `aarch64` | AWS Graviton3 (c7g.4xlarge) | versão padrão | O backend NEON é compatível com x86. |

- **Arnês de referência.** يجب انتاج كل تقارير معايرة الغاز باستخدام:
  -`CRITERION_HOME=target/criterion cargo bench -p iroha_core isi_gas_calibration -- --sample-size 200 --warm-up-time 5 --save-baseline <profile-label>`
  - `cargo test -p iroha_core bench_repro -- --ignored` para fixação de fixação.
  - `CRITERION_HOME=target/criterion cargo bench -p ivm gas_calibration -- --sample-size 200 --warm-up-time 5 --save-baseline <profile-label>` é um código de operação da VM.

- **Aleatoriedade fixa.** O `IROHA_CONF_GAS_SEED=conf-gas-seed-2026Q1` é definido como `iroha_test_samples::gen_account_in` e `KeyPair::from_seed`. Chicote de fios `IROHA_CONF_GAS_SEED_ACTIVE=…` de comprimento total Não se preocupe, não há problema. Não há nada que você possa fazer com que o seu telefone esteja funcionando corretamente.- **Captura de resultados.**
  - ارفع Criterion summaries (`target/criterion/**/raw.csv`) لكل ملف الى artefato الاصدار.
  - خزّن المقاييس المشتقة (`ns/op`, `gas/op`, `ns/gas`) em [Ledger de calibração de gás confidencial](./confidential-gas-calibration) com git commit ونسخة المترجم المستخدمة.
  - احتفظ بآخر linha de base اثنين لكل ملف؛ Você pode usar o software para obter mais informações.

- **Tolerâncias de aceitação.**
  - يجب ان تبقى فروقات الغاز بين `baseline-simd-neutral` e `baseline-avx2` ضمن ≤ ±1,5%.
  - Não há nenhum problema em `baseline-simd-neutral` e `baseline-neon` ≤ ±2,0%.
  - O código de barras do site está disponível para download e RFC.

- **Lista de verificação de revisão.** يتحمل مقدمو الطلب مسؤولية:
  - Use `uname -a` e `/proc/cpuinfo` (modelo, passo a passo) e `rustc -Vv` em qualquer lugar.
  - O código `IROHA_CONF_GAS_SEED` está disponível na versão original (semente de semente).
  - ضمان تطابق marcapasso e verificador confidencial de produção (`--features confidential,telemetry` عند تشغيل البنش مع Telemetria).

## التهيئة والعمليات
- `iroha_config` como `[confidential]`:
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
- تصدر التليمتري مقاييس مجمعة: `confidential_proof_verified`, `confidential_verifier_latency_ms`, `confidential_proof_bytes_total`, `confidential_nullifier_spent`, `confidential_commitments_appended`, `confidential_mempool_rejected_total{reason}`, e `confidential_policy_transitions_total` são texto simples.
- RPC:
  -`GET /confidential/capabilities`
  -`GET /confidential/zk_registry`
  -`GET /confidential/params`

## استراتيجية الاختبار
- الحتمية: خلط المعاملات عشوائيا داخل الكتل ينتج نفس Raízes Merkle e conjuntos anuladores.
- تحمل reorg: محاكاة reorg متعدد الكتل مع âncoras; Anuladores e âncoras são usados.
- ثوابت الغاز: تاكد من نفس استخدام الغاز عبر عقد مع او بدون تسريع SIMD.
- اختبار الحدود: provas عند سقوف الحجم/الغاز, الحد الاقصى للمدخلات/المخرجات, e tempo limite.
- دورة الحياة: عمليات الحوكمة لتفعيل/استبدال verificador والمعلمات, واختبارات صرف الدوران.
- Política FSM: الانتقالات المسموح بها/المرفوضة, تأخيرات transição pendente, ورفض mempool e alturas efetivas.
- طوارئ السجل: سحب طارئ يجمد الاصول المتاثرة عند `withdraw_height` ويرفض provas بعدها.
- Controle de capacidade: المدققون مع `conf_features` غير متطابقة يرفضون الكتل؛ observadores مع `assume_valid=true` يواكبون دون التأثير على الاجماع.
- تكافؤ الحالة: عقد validador/completo/observador تنتج نفس raízes de estado على السلسلة القانونية.
- Fuzzing negativo: provas تالفة, payloads كبيرة جدا, وتصادمات nullifier ترفض حتميا.

## الهجرة والتوافق
- طرح محكوم بالميزة: حتى اكتمال Fase C3, `enabled` ou `false`; Você pode usar o aplicativo para obter mais informações.
- الاصول الشفافة غير متاثرة؛ Certifique-se de que o produto esteja funcionando corretamente e sem problemas.
- العقد المترجمة دون دعم السرية ترفض الكتل المعنية حتميا؛ Para obter mais informações sobre o produto, use `assume_valid=true`.
- Gênesis se manifesta اختيارية.
- يتبع المشغلون runbooks المنشورة لتدوير السجل وانتقالات السياسة والسحب الطارئ للحفاظ على ترقية Bem.

## اعمال متبقية
- قياس مجموعات معلمات Halo2 (حجم الدائرة, استراتيجية lookup) وتسجيل النتائج في Calibration Playbook حتى يمكن تحديث A configuração de gás/tempo limite é definida como `confidential_assets_calibration.md`.
- Você pode usar a visualização seletiva e a visualização seletiva no Torii بعد توقيع مسودة الحوكمة.
- توسيع مخطط criptografia de testemunha ليغطي مخرجات متعددة المستلمين وmemos مجمعة, وتوثيق تنسيق envelope لمطوري SDK.
- طلب مراجعة امنية خارجية للدوائر والسجلات واجراءات تدوير المعلمات وارشفة النتائج بجانب تقارير التدقيق الداخلية.
- تحديد واجهات API لتسوية gastos للمدققين ونشر ارشاد نطاق view-key حتى ينفذ مزودو المحافظ نفس دلالات Não.## مراحل التنفيذ
1. **Fase M0 — Endurecimento Stop-Ship**
   - ✅ اشتقاق nullifier يتبع الان تصميم Poseidon PRF (`nk`, `rho`, `asset_id`, `chain_id`) مع فرض ترتيب compromissos O livro é o livro-razão.
   - ✅ يفرض التنفيذ حدود حجم provas وحصص العمليات السرية لكل معاملة/كتلة, رافضا المعاملات المتجاوزة باخطاء حتمية.
   - ✅ يعلن P2P handshake `ConfidentialFeatureDigest` (backend digest + بصمات السجل) ويفشل عدم التطابق حتميا عبر `HandshakeConfidentialMismatch`.
   - ✅ ازالة panics في مسارات تنفيذ السرية واضافة role gating لللعقد غير المتوافقة.
   - ⚪ فرض ميزانيات timeout para o verificador وحدود عمق reorg para os pontos de verificação de fronteira.
     - ✅ Tempo limite de tempo limite; as provas são `verify_timeout_ms` para serem corrigidas.
     - ✅ احترام `reorg_depth_bound` e pontos de verificação الاقدم من النافذة مع الحفاظ على snapshots حتمية.
   - تقديم `AssetConfidentialPolicy` e política FSM وبوابات aplicação لتعليمات mint/transfer/reveal.
   - O arquivo `conf_features` está disponível para o registro/parâmetro do resumo.
2. **Fase M1 — Registros e Parâmetros**
   - شحن سجلات `ZkVerifierEntry` و`PedersenParams` و`PoseidonParams` مع عمليات حوكمة ومرساة genesis وادارة الكاش.
   - Faça syscall para pesquisas de registro, IDs de programação de gás, hash de esquema e outros.
   - شحن صيغة payload المشفر v1, متجهات اشتقاق مفاتيح المحافظ, ودعم CLI لادارة مفاتيح السرية.
3. **Fase M2 — Gás e Desempenho**
   - تنفيذ جدول gas حتمي, عدادات لكل كتلة, وحزم قياس مع تليمتري (verificar latência, احجام provas, رفض mempool).
   - Verifique os pontos de verificação CommitmentTree, o carregamento do LRU e os índices nulos.
4. **Fase M3 – Rotação e Ferramentas de Carteira**
   - تمكين قبول provas متعددة المعلمات والنسخ؛ A ativação/descontinuação está associada aos runbooks.
   - تقديم تدفقات هجرة لـ wallet SDK/CLI, سير عمل مسح المدققين, وادوات تسوية gastos.
5. **Fase M4 — Auditoria e Operações**
   - توفير تدفقات مفاتيح المدققين, e divulgação seletiva, e runbooks تشغيلية.
   - Verifique se o produto/serviço está disponível em `status.md`.

كل مرحلة تحدث معالم roadmap والاختبارات المرتبطة لضمان التنفيذ الحتمي لشبكة البلوكتشين.