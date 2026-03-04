---
lang: pt
direction: ltr
source: docs/portal/docs/nexus/nexus-spec.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: especificação do nexo
título: المواصفات التقنية لسورا نيكسس
description: مرآة كاملة لـ `docs/source/nexus.md` تغطي معمارية وقيود التصميم لدفتر Iroha 3 (Sora Nexus).
---

:::note المصدر الرسمي
Verifique o valor `docs/source/nexus.md`. Verifique se o dispositivo está funcionando corretamente.
:::

#! Iroha 3 - Sora Nexus Ledger: Código de registro

Você pode usar o Sora Nexus Ledger para Iroha 3, um livro de registro Iroha 2 إلى دفتر Não há espaço de dados (DS). توفر Espaços de Dados نطاقات خصوصية قوية ("espaços de dados privados") ومشاركة مفتوحة ("espaços de dados públicos"). يحافظ التصميم على composability عبر الدفتر العالمي مع ضمان عزل صارم وسرية بيانات private-DS, ويقدم توسيع توفر Os recursos são baseados em Kura (armazenamento em bloco) e WSV (World State View).

Você pode usar o Iroha 2 (um conjunto de ferramentas) e Iroha 3 (SORA Nexus). يعتمد التنفيذ على Iroha Virtual Machine (IVM) المشتركة وسلسلة ادوات Kotodama, لذا تبقى O bytecode é definido como um bytecode que está disponível para download.

الاهداف
- دفتر منطقي عالمي واحد مكون من العديد من المدققين المتعاونين e Espaços de Dados.
- Data Spaces é um espaço de dados (como CBDC) que pode ser usado no DS الخاص.
- Espaços de Dados عامة بمشاركة مفتوحة ووصول دون اذن على غرار Ethereum.
- عقود ذكية قابلة للتركيب عبر Data Spaces, مع صلاحيات صريحة للوصول إلى اصول private-DS.
- عزل الاداء بحيث لا تؤثر الحركة العامة على معاملات private-DS الداخلية.
- توفر بيانات على نطاق واسع: Kura e WSV بترميز محو لدعم بيانات شبه غير محدودة مع بقاء بيانات private-DS خاصة.

غير الاهداف (المرحلة الاولى)
- تعريف اقتصاديات التوكن او حوافز المدققين؛ Agendamento e piquetagem são necessários.
- إدخال اصدار ABI جديد; A solução ABI v1 é baseada em syscalls e pointer-ABI e IVM.

المصطلحات
- Nexus Ledger: O espaço de dados (DS) está localizado no espaço de dados (DS) no espaço de dados e no espaço de dados.
- Espaço de Dados (DS): نطاق تنفيذ وتخزين محدود بمدققين خاصين وحوكمة وفئة خصوصية وسياسة DA وحصص وسياسة رسوم. Exemplo: DS público e DS privado.
- Espaço de dados privados: مدققون باذونات وتحكم وصول؛ بيانات المعاملة والحالة لا تغادر DS. Verifique se o produto/serviço está funcionando corretamente.
- Espaço de dados públicos: مشاركة بدون اذن؛ Você pode fazer isso com antecedência.
- Manifesto de Espaço de Dados (Manifesto DS): manifesto مشفّر عبر Norito يعلن معلمات DS (validadores/chaves QC), فئة الخصوصية, سياسة ISI, معلمات DA, الاحتفاظ, الحصص, سياسة ZK, الرسوم). Você usa hash no manifesto do nexo. Para obter mais informações sobre o quorum do DS, use o ML-DSA-87 (como Dilithium5) pós-quântico.
- Diretório de espaço: عقد دليل عالمي على السلسلة يتتبع manifestos DS والنسخ واحداث الحوكمة/الدوران لاغراض الاستدلال والتدقيق.
- DSID: معرف فريد عالميا لـ Espaço de Dados. Use o namespace para o site e o site.
- Anchor: التزام تشفيري من كتلة/راس DS يضم الى سلسلة nexus لربط تاريخ DS بالدفتر العالمي.
- Kura: تخزين كتل Iroha. Você pode fazer com que os blobs sejam usados ​​​​em vez de blobs.
- WSV: Iroha Visão do Estado Mundial. Isso pode ser feito por meio de uma mensagem de texto e uma mensagem de erro.
- IVM: Máquina Virtual Iroha لتنفيذ العقود الذكية (bytecode Kotodama `.to`).
  - AIR: Representação Algébrica Intermediária. تمثيل جبري للحساب من اجل ادلة نمط STARK يصف التنفيذ كسلاسل تعتمد على الحقول مع قيود انتقال Bem.

Espaços de dados
- Nome: `DataSpaceId (DSID)` DS e namespace para o DS. يمكن انشاء DS على درجتين:
  - Domínio-DS: `ds::domain::<domain_name>` - تنفيذ وحالة ضمن نطاق domínio.
  - Asset-DS: `ds::asset::<domain_name>::<asset_name>` - Ativos e dados de terceiros.
  الشكلان يتعايشان; Você pode usar o DSID para fazer isso.
- دورة حياة manifest: تسجيل إنشاء DS والتحديثات (تدوير المفاتيح, تغييرات السياسة) والإنهاء في Space Directory. O slot do DS não possui hash no manifesto.
- Nomes: DS público (مشاركة مفتوحة, DA عامة) e DS privado (مقيد, DA سرية). Você pode usar flags no manifesto.
- Nome do DS: صلاحيات ISI, معلمات DA `(k,m)`, تشفير, احتفاظ, حصص (حصة tx الدنيا/العليا لكل كتلة), سياسة اثبات ZK/تفاؤلي, رسوم.
- الحوكمة: العضوية والدوران no DS يحددان في قسم الحوكمة بالـ manifesto (مقترحات على السلسلة, multisig, او حوكمة خارجية مثبتة بمعاملات nexo e atestados).Manifestos القدرات e UAID
- Nome de usuário: Você pode usar o UAID حتمي (`UniversalAccountId` ou `crates/iroha_data_model/src/nexus/manifest.rs`) em espaços de dados. تربط manifests القدرات (`AssetPermissionManifest`) الـ UAID بـ dataspace محدد وفترات تفعيل/انتهاء وقائمة مرتبة من قواعد permitir/negar `ManifestEntry` como `dataspace` e `program_id` e `method` e `asset` e AMX. Negar تفوز دائما؛ ويصدر المقيّم `ManifestVerdict::Denied` مع سبب تدقيق او منح `Allowed` مع بيانات subsídio المطابقة.
- Subsídios: يحمل كل permitir balde حتمي `AllowanceWindow` (`PerSlot`, `PerMinute`, `PerDay`) مع `max_amount` اختياري. Verifique hosts e SDKs como Norito, sem problemas de segurança e SDKs.
- Telemetria تدقيق: يبث Space Directory حدث `SpaceDirectoryEvent::{ManifestActivated, ManifestExpired, ManifestRevoked}` (`crates/iroha_data_model/src/events/data/space_directory.rs`) عند تغير حالة manifest. O `SpaceDirectoryEventFilter` é o nome do Torii/data-event por meio do manifesto UAID e do manifesto UAID. negar-vitórias دون اعداد مخصص.

Para que você possa usar o SDK e os manifestos do SDK, não se preocupe com isso. No Guia de conta universal (`docs/source/universal_accounts_guide.md`). ابق الوثيقتين متطابقتين عند تغير سياسة UAID او الادوات.

المعمارية عالية المستوى
1) Corrente de corrente (Cadeia Nexus)
- Verifique se o Nexus é de 1 polegada e se você deseja usar o Nexus para obter mais informações Espaços de dados (DS). كل معاملة ملتزمة تحدث estado mundial الموحد (متجه raízes لكل DS).
- تحتوي بيانات وصفية دنيا مع provas/QCs مجمعة لضمان composability والنهائية وكشف الاحتيال (DSIDs المتأثرة, raízes para DS قبل/بعد, التزامات DA, provas صلاحية لكل DS, وشهادة quorum para DS باستخدام ML-DSA-87). Não há nada que você possa fazer.
- A opção: BFT tem um Pipeline de 22 dias (3f+1 de f=7) que usa VRF/stake para ganhar dinheiro. ~200k por mês. لجنة nexus ترتب المعاملات وتؤكد الكتلة خلال 1s.

2) Espaço de dados طبقة (público/privado)
- تنفذ مقاطع المعاملات لكل DS, وتحدث WSV المحلي, وتنتج artefatos صلاحية لكل كتلة (provas مجمعة لكل DS والتزامات DA) تتجمع في كتلة Nexus 1s.
- DS privado تشفر البيانات في السكون والحركة بين المدققين المصرح لهم؛ ولا يغادر DS سوى الالتزامات e provas صلاحية PQ.
- Public DS تصدر اجسام البيانات الكاملة (via DA) e provas صلاحية PQ.

3) Como configurar espaços de dados (AMX)
- النموذج: كل معاملة مستخدم قد تمس عدة DS (como DS de domínio e DS de ativo). Verifique o valor do Nexus e instale-o Não é isso.
- Prepare-Commit 1s: لكل معاملة مرشحة, تنفذ DS المعنية بالتوازي على نفس snapshot (roots DS ببداية slot) وتنتج provas Você pode usar PQ para DS (FASTPQ-ISI) e DA. يلتزم comite nexus فقط اذا تحققت كل provas المطلوبة ووصلت شهادات DA في الوقت (هدف <=300 ms),؛ والا تعاد جدولة المعاملة للـ slot التالي.
- الاتساق: مجموعات القراءة/الكتابة مصرح بها؛ يتم كشف التعارض عند الالتزام مقابل raízes بداية slot. Você pode usar o DS para obter mais informações. O nexo é definido como um nexo (الكل او لا شيء عبر DS).
- الخصوصية: تصدر DS privado فقط provas/compromissos مرتبطة بـ raízes قبل/بعد. Não se preocupe.

4) توفر البيانات (DA) بترميز محو
- يخزن Kura اجسام الكتل ولقطات WSV كـ blobs بترميز محو. يتم توزيع blobs العامة على نطاق واسع؛ Os blobs são usados ​​​​para usar o private-DS como chunks.
- يتم تسجيل التزامات DA في artefatos الخاصة بـ DS وفي كتل Nexus, مما يمكّن من ضمانات amostragem والاسترجاع دون كشف المحتوى الخاص.

هيكل الكتل والالتزام
- Artefato no espaço de dados (para slot 1s e DS)
  - Códigos: dsid, slot, pre_state_root, post_state_root, ds_tx_set_hash, kura_da_commitment, wsv_da_commitment, manifest_hash, ds_qc (ML-DSA-87), ds_validity_proof (FASTPQ-ISI).
  - تصدر artefatos DS privados بدون اجسام بيانات؛ وpublic DS تسمح باسترجاع الاجسام عبر DA.

- Bloco Nexus (1s)
  - Números: block_number, parent_hash, slot_time, tx_list (não entre cross-DS ou DSIDs), ds_artifacts[], nexus_qc.
  - الوظيفة: تثبيت جميع المعاملات الذرية التي تتحقق artefatos المطلوبة لها؛ تحديث متجه Roots العالمي لكل DS é um nome e um nome.Agendamento e agendamento
- Cadeia Nexus: BFT عالمي متسلسل (فئة Sumeragi) com 22 segundos (3f+1 com f=7) É 1s e 1s. Você pode usar VRF/stake por cerca de ~200k por mês الدوران يحافظ على اللامركزية ومقاومة الرقابة.
- اجماع Espaço de dados: يشغل كل DS BFT خاص به لانتاج artefatos لكل slot (provas, التزامات DA, DS QC). O lane-relay é usado para `3f+1` e `fault_tolerance` para o espaço de dados e para o espaço de dados Você pode obter a semente VRF de acordo com `(dataspace_id, lane_id)`. DS Privado مقيدة؛ DS público é anti-Sybil. Nexus ثابتة.
- Agendamento de tarefas: يرسل المستخدمون معاملات ذرية مع DSIDs e ومجموعات القراءة/الكتابة. O DS não possui slot, وتضم لجنة nexus المعاملة في كتلة 1s اذا تحقق جميع artefatos ووصلت شهادات DA في الوقت (<=300 ms).
- عزل الاداء: Não use mempools do DS. As cotas de DS são definidas para o bloqueio de linha principal e para o DS privado.

Nomes de domínio e namespace
- IDs qualificados para DS: IDs (domínios, contas, ativos, funções) cujo nome é `dsid`. Exemplo: `ds::<domain>::account`, `ds::<domain>::asset#precision`.
- Referências Globais: O nome da tupla `(dsid, object_id, version_hint)` e a cadeia on-chain são Nexus e AMX para cross-DS.
- Serialização Norito: جميع رسائل cross-DS (provas AMX), usando codecs Norito. Não deixe de ser serde em qualquer lugar.

Nome do produto e IVM
- Nome de usuário: `dsid` ou IVM. O Kotodama é usado para armazenar espaço de dados.
- primitivo ذرية عبر DS:
  - `amx_begin()` / `amx_commit()` é um dispositivo multi-DS no host IVM.
  - `amx_touch(dsid, key)` تعلن نية القراءة/الكتابة لكشف التعارض مقابل root snapshot no slot.
  - `verify_space_proof(dsid, proof, statement)` -> bool
  - `use_asset_handle(handle, op, amount)` -> resultado (مسموح فقط اذا سمحت السياسة وكان handle صالحا)
- Identificadores de ativos والرسوم:
  - عمليات الاصول مصرح بها عبر سياسات ISI/função para DS; Verifique o gás do DS. Existem tokens de capacidade e tokens de capacidade (aprovadores múltiplos, limites de taxa, cerca geográfica).
- الحتمية: syscalls الجديدة نقية وحتمية عند المدخلات ومجموعات القراءة/الكتابة AMX المعلنة. Não se preocupe, não há problema em usá-lo.اثباتات الصلاحية pós-quântico (ISI معممة)
- FASTPQ-ISI (PQ بدون configuração confiável): transferência baseada em hash تعمم تصميم transferência لكل عائلات ISI مع استهداف اثبات دون الثانية لدفعات Mais de 20k por GPU.
  - Como fazer:
    - عقد الانتاج تنشئ prover عبر `fastpq_prover::Prover::canonical` الذي يهيئ دائما backend الانتاج؛ تمت ازالة mock الحتمي. [crates/fastpq_prover/src/proof.rs:126]
    - `zk.fastpq.execution_mode` (config) e `irohad --fastpq-execution-mode` يتيحان تثبيت تنفيذ CPU/GPU بشكل حتمي بينما يسجل observer hook ثلاثيات solicitado/resolvido/backend Não. [crates/iroha_config/src/parameters/user.rs:1357] [crates/irohad/src/main.rs:270] [crates/irohad/src/main.rs:2192] [crates/iroha_telemetry/src/metrics.rs:8887]
- Aritmetização:
  - KV-Update AIR: يعامل WSV كخريطة valor-chave نوعية ملتزمة عبر Poseidon2-SMT. O ISI possui uma função de leitura-verificação-gravação de dados (contas, ativos, funções, domínios, metadados, fornecimento).
  - قيود محكومة بالـ opcode: جدول AIR e مع اعمدة selector يفرض قواعد كل ISI (conservação, contadores monotônicos, permissões, verificações de intervalo, metadados de segurança محدودة).
  - Argumentos de pesquisa: جداول شفافة ملتزمة بالهاش لصلاحيات/ادوار وprecisions ومعلمات سياسة تتجنب قيود bitwise الثقيلة.
- التزامات وتحديثات الحالة:
  - Prova SMT agregada: جميع المفاتيح المتأثرة (pré/pós) تثبت مقابل `old_root`/`new_root` باستخدام frontier مضغوط مع irmãos Não use nada.
  - Invariantes: يتم فرض ثوابت عالمية (مثل اجمالي supply لكل اصل) عبر مساواة multiset بين صفوف التأثير والعدادات المتعقبة.
- نظام الاثبات:
  - التزامات متعددة الحدود بأسلوب FRI (DEEP-FRI) مع arity عالية (8/16) e explosão 8-16; hashes Poseidon2؛ transcrição Fiat-Shamir por SHA-2/3.
  - Recursão اختيارية: تجميع محلي داخل DS لضغط micro-lotes الى اثبات واحد لكل slot عند الحاجة.
- النطاق والامثلة:
  - Função: transferir, cunhar, queimar, registrar/cancelar registro de definições de ativos, definir precisão (مقيد), definir metadados.
  - الحسابات/المجالات: criar/remover, definir chave/limite, adicionar/remover signatários (حالة فقط؛ فحص التواقيع يثبت من مدققي DS وليس داخل AIR).
  - ISI/Importante (ISI): conceder/revogar funções e permissões; يتم فرضها عبر جداول pesquisa e verificações سياسة monotônico.
  - العقود/AMX: علامات start/commit para AMX,capacidade mint/revoke اذا كانت مفعلة؛ Deixe-os cair e solte-os.
- فحوصات خارج AIR للحفاظ على الكمون:
  - O software de gerenciamento de dados (que pode ser usado no ML-DSA) está disponível no DS QC; Certifique-se de que o produto esteja limpo e limpo. Não há provas PQ وسريعة.
- Capacidade de CPU (CPU 32 N + GPU حديثة):
  - 20k ISI مختلطة مع لمس مفاتيح صغير (<=8 chaves/ISI): ~0.4-0.9 s اثبات, ~150-450 KB اثبات, ~5-15 ms تحقق.
  - ISI اثقل: micro-lote (como 10x2k) + recursão para o slot <1 s para o slot.
- Manifesto DS:
  -`zk.policy = "fastpq_isi"`
  -`zk.hash = "poseidon2"`, `zk.fri = { blowup: 8|16, arity: 8|16 }`
  -`state.commitment = "smt_poseidon2"`
  -`zk.recursion = { none | local }`
  - `attestation.signatures_in_proof = false` (referência ao DS QC)
  - `attestation.qc_signature = "ml_dsa_87"` (`attestation.qc_signature = "ml_dsa_87"`)
- Subsídios:
  - ISI المعقدة/المخصصة يمكنها استخدام STARK عام (`zk.policy = "stark_fri_general"`) مع اثبات مؤجل ونهائية 1s عبر atestado QC + cortando على ادلة غير صحيحة.
  - Você pode usar o PQ (como o Plonk do KZG) para configurar a configuração confiável e instalá-lo no build do site.

AIR Primer (para Nexus)
- Trace التنفيذ: مصفوفة بعرض (اعمدة سجلات) وطول (خطوات). Isso é algo que pode ser feito pelo ISI; Eles usam pré/pós, seletores e sinalizadores.
- Nome:
  - قيود الانتقال: تفرض علاقات صف-الى-صف (مثلا post_balance = pre_balance - amount لصف خصم عند `sel_transfer = 1`).
  - قيود الحدود: تربط public I/O (old_root/new_root, counters) بالصف الاول/الاخير.
  - Pesquisas/permutações: تضمن العضوية والمساواة متعددة المجموعات مقابل جداول ملتزمة (permissões, parâmetros de ativos) ou pouco pesado.
- الالتزام والتحقق:
  - يقوم prover بالالتزام بالتراسات عبر ترميزات hash ويبني متعددات حدود منخفضة الدرجة صالحة اذا تحققت القيود.
  - يتحقق verificador من الدرجة المنخفضة عبر FRI (baseado em hash, pós-quântico) مع فتحات Merkle قليلة؛ O problema é que você não pode usá-lo.
- مثال (Transferência): تتضمن السجلات pre_balance, amount, post_balance, nonce, seletores. تفرض القيود عدم السلبية/المدى, الحفظ, ورتابة nonce, بينما تربط multi-prova SMT مجمعة الاوراق pré/pós بـ raízes antigas/novas.Usando ABI e syscalls (ABI v1)
- Syscalls para usar (اسماء توضيحية):
  -`SYS_AMX_BEGIN`, `SYS_AMX_TOUCH`, `SYS_AMX_COMMIT`, `SYS_VERIFY_SPACE_PROOF`, `SYS_USE_ASSET_HANDLE`.
- O ponteiro-ABI é definido:
  -`PointerType::DataSpaceId`, `PointerType::AmxDescriptor`, `PointerType::AssetHandle`, `PointerType::ProofBlob`.
- تحديثات مطلوبة:
  - اضافة الى `ivm::syscalls::abi_syscall_list()` (مع الحفاظ على الترتيب), مع gating حسب السياسة.
  - ربط الارقام غير المعروفة بـ `VMError::UnknownSyscall` em hosts.
  - تحديث الاختبارات: syscall list golden, ABI hash, ponteiro tipo ID goldens, واختبارات السياسة.
  - Documentos: `crates/ivm/docs/syscalls.md`, `status.md`, `roadmap.md`.

نموذج الخصوصية
- احتواء البيانات الخاصة: اجسام المعاملات وفروق الحالة ولقطات WSV الخاصة بـ private DS para تغادر مجموعة المدققين الخاصة.
- التعرض العام: يتم تصدير cabeçalhos والتزامات DA e provas صلاحية PQ فقط.
- تمكين عمليات cross-DS Eu não sei.
- التحكم بالوصول: يتم فرض التخويل عبر سياسات ISI/role داخل DS. tokens de capacidade

Qualidade de serviço e QoS
- اجماع ومجموعات انتظار وتخزين منفصلة لكل DS.
- cotas para agendamento على مستوى DS no nexus لتقييد زمن ادراج âncoras e bloqueio head-of-line.
- ميزانيات موارد العقود لكل DS (computação/memória/IO) no host IVM. O DS público não é o DS privado.
- استدعاءات cross-DS غير متزامنة تتجنب انتظار متزامن طويل داخل تنفيذ private-DS.

توفر البيانات وتصميم التخزين
1) ترميز المحو
- استخدام Reed-Solomon منهجي (مثلا GF(2^16)) لترميز المحو على مستوى blob لكتل Kura ولقطات WSV: معلمات `(k, m)` com fragmentos `n = k + m`.
- Número de bits (DS público): `k=32, m=16` (n=48) O tamanho é de 16 fragmentos com tamanho de ~1,5x. O DS privado: `k=16, m=8` (n=24) está disponível. Você pode usar o DS Manifest.
- Blobs العامة: shards موزعة عبر عقد DA/مدققين عديدة مع فحوص توفر بالعينات. Os DA são cabeçalhos para clientes leves.
- Blobs الخاصة: shards مشفرة وموزعة فقط بين مدققي private-DS (او حراس معينين). السلسلة العالمية تحمل فقط التزامات DA (shards او المفاتيح).

2) amostragem e amostragem
- Para blob: احسب Merkle root e shards وادرجه في `*_da_commitment`. Você não pode usar PQ para obter mais informações.
- DA Attesters: atestadores اقليميون يتم اختيارهم عبر VRF (مثلا 64 لكل منطقة) يصدرون شهادة ML-DSA-87 تؤكد sampling الناجح. O atestado é <=300 ms. O nexus deve ser usado em shards.

3) تكامل Kura
- الكتل تخزن اجسام المعاملات كـ blobs بترميز محو مع التزامات Merkle.
- cabeçalhos تحمل التزامات blobs; Você pode usar o DS público e o DS privado.

4) Baixar WSV
- Snapshots WSV: O ponto de verificação do DS é usado para instantâneos e cabeçalhos. Não há logs de alterações. O público não pode ser usado nem o privado, nem o privado.
- Acesso de transporte de prova: يمكن للعقود تقديم (او طلب) ادلة حالة (Merkle/Verkle) مثبتة عبر التزامات snapshot. O DS privado não possui conhecimento zero para você.

5) Corte e poda
- بدون pruning للـ public DS: الاحتفاظ بكل اجسام Kura ولقطات WSV عبر DA (توسع افقي). O DS privado não permite que você use o DS privado. طبقة nexus تحتفظ بكل Nexus Blocos e artefatos no DS.

الشبكات وادوار العقد
- المدققون العالميون: يشاركون في اجماع nexus, يتحققون من Nexus Blocks و DS artefatos, ويجرون فحوص DA لـ public DS.
- Espaço de dados مدققو: يشغلون اجماع DS, ينفذون العقود, يديرون Kura/WSV المحلي, ويتولون DA لـ DS الخاصة بهم.
- عقد DA (اختياري): تخزن/تنشر blobs عامة وتدعم amostragem. No DS privado, você pode usar o DA تكون متجاورة مع المدققين او حراس موثوقين.تحسينات واعتبارات على مستوى النظام
- فصل sequenciamento/mempool: اعتماد mempool DAG (مثل Narwhal) يغذي BFT متسلسل في طبقة nexus لتقليل الكمون وزيادة دون تغيير النموذج المنطقي.
- حصص DS والعدالة: حصص لكل DS في كل كتلة وحدود وزن لتجنب bloqueio head-of-line وضمان كمون متوقع لـ DS privado.
- اقرار DS (PQ): quorum de شهادات para DS تستخدم ML-DSA-87 (فئة Dilithium5) افتراضيا. O pós-quântico é um sistema de controle de qualidade e um slot de QC. O DS usa ML-DSA-65/44 para o DS Manifest e o EC para o DS Manifest; O DS público é compatível com ML-DSA-87.
- Atestadores DA: para DS público, استخدم atestadores اقليميين مختارين بـ VRF يصدرون شهادات DA. O nexo é baseado em amostragem. DS privado é um problema.
- Recursão. Você pode fazer isso com antecedência.
- Escala de pista (عند الحاجة): اذا اصبحت اللجنة العالمية عنق زجاجة, قدم K pistas تسلسل متوازية مع دمج حتمي. Você não pode se preocupar com isso e não comê-lo.
- تسريع حتمي: Os kernels SIMD/CUDA têm sinalizadores de recurso para hashing/FFT com CPU de fallback.
- عتبات تفعيل pistas (اقتراح): تفعيل 2-4 pistas اذا (a) تجاوزت نهائية p95 مدة 1.2 s لاكثر من 3 دقائق متتالية, او (b) تجاوز اشغال الكتلة 85% لاكثر من 5 دقائق, او (c) تطلب معدل المعاملات الداخل >1.2x Isso é algo que você pode fazer. As pistas são usadas para usar o hash do DSID no nexo.

الرسوم والاقتصاد (افتراضات اولية)
- وحدة الغاز: token غاز لكل DS مع قياس compute/IO; Verifique se o DS está funcionando corretamente. O DS não é compatível com o DS.
- اولوية الادراج: round-robin عبر DS مع حصص لكل DS للحفاظ على العدالة وSLOs 1s; Verifique se o DS não está funcionando corretamente.
- مستقبلا: يمكن استكشاف سوق رسوم عالمي او سياسات تقلل MEV دون تغيير الذرية او تصميم ادلة PQ.

سير عمل cross-Data-Space (مثال)
1) يرسل مستخدم معاملة AMX تمس público DS P e privado DS S: نقل الاصل X من S الى المستفيد B الذي حسابه في P.
2) داخل slot, ينفذ P e S مقطعهم على snapshot. تتحقق S من التفويض والتوفر وتحدث حالتها الداخلية وتنتج اثبات صلاحية PQ والتزام DA (بدون تسريب بيانات خاصة). يحضر P تحديث الحالة المقابل (como hortelã/queimadura/travamento em P حسب السياسة) واثباته.
3) تتحقق لجنة nexus no DS e no DA; اذا تحقق كلاهما خلال slot يتم التزام المعاملة ذرياً في كتلة Nexus 1s وتحديث Roots لكل DS في متجه estado mundial العالمي.
4). اعادة ارسالها للـ slot التالي. Não há problema em S الخاصة em qualquer lugar.

- اعتبارات الامان
- A configuração: syscalls em IVM O DS não permite o commit do AMX, o commit do DS e o commit do AMX.
- O código de segurança: O ISI do DS privado é usado para configurar o serviço e o serviço. tokens de capacidade são usados ​​para cross-DS.
- السرية: تشفير من الطرف للطرف لبيانات private-DS, shards بترميز محو مخزنة فقط لدى الاعضاء المصرح لهم, وادلة ZK اختيارية لاثباتات خارجية.
- Como DoS: O nome do mempool/consensus/storage é o público que está no DS privado.

Ferramentas de reparo Iroha
- iroha_data_model: Use `DataSpaceId` e IDs para descritores DS e AMX (como قراءة/كتابة) e Proofs/التزامات DA. Instale Norito.
- ivm: contém syscalls e tipos de ponteiro-ABI para AMX (`amx_begin`, `amx_commit`, `amx_touch`) e provas DA; Baixe a versão 1.0 do ABI e a versão v1.