---
lang: fr
direction: ltr
source: docs/portal/docs/nexus/nexus-spec.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : nexus-spec
titre : المواصفات التقنية لسورا نيكسس
description: مرآة كاملة لـ `docs/source/nexus.md` تغطي معمارية وقيود التصميم لدفتر Iroha 3 (Sora Nexus).
---

:::note المصدر الرسمي
Il s'agit de la référence `docs/source/nexus.md`. حافظ على النسختين متطابقتين حتى يصل تراكم الترجمات إلى البوابة.
:::

#! Iroha 3 - Sora Nexus Ledger : Informations supplémentaires

يقترح هذا المستند معمارية Sora Nexus Ledger pour Iroha 3, مع تطوير Iroha 2 إلى عالمي Vous pouvez également utiliser les espaces de données (DS). توفر Data Spaces نطاقات خصوصية قوية ("espaces de données privés") et ومشاركة مفتوحة ("espaces de données publics"). يحافظ التصميم على composability عبر الدفتر العالمي مع ضمان عزل صارم وسرية بيانات private-DS، ويقدم توسيع توفر البيانات عبر Il s'agit de Kura (stockage en bloc) et WSV (World State View).

Il s'agit de Iroha 2 (Iroha 2) et Iroha 3 (SORA Nexus). يعتمد التنفيذ على Iroha Virtual Machine (IVM) المشتركة وسلسلة ادوات Kotodama, لذا تبقى العقود وقطع الـ bytecode قابلة للنقل بين النشر الذاتي والدفتر العالمي لنكسس.الاهداف
- دفتر منطقي عالمي واحد مكون من العديد من المدققين المتعاونين et espaces de données.
- Les espaces de données sont utilisés pour la CBDC (pour CBDC) avec les DS.
- Data Spaces est disponible pour vous et pour Ethereum.
- Vous pouvez utiliser les espaces de données et les espaces de données privés-DS.
- عزل الاداء بحيث لا تؤثر الحركة العامة على معاملات private-DS الداخلية.
- توفر بيانات على نطاق واسع: Kura et WSV بترميز محو لدعم بيانات شبه غير محدودة مع بقاء بيانات private-DS Oui.

غير الاهداف (المرحلة الاولى)
- تعريف اقتصاديات التوكن او حوافز المدققين؛ Il y a la planification et le jalonnement.
- إدخال اصدار ABI جديد؛ La version ABI v1 est compatible avec les appels système et le pointeur-ABI et IVM.المصطلحات
- Nexus Ledger : Gestion de l'espace de données (DS) dans la gestion de l'espace de données (DS) et de l'espace de données.
- Espace de données (DS) : Espace de données et espace de données avec DA et DA رسوم. Il s'agit de : DS public et DS privé.
- Espace de données privé : مدققون باذونات وتحكم وصول؛ بيانات المعاملة والحالة لا تغادر DS. يتم تثبيت التزامات/البيانات الوصفية فقط عالميا.
- Espace de données publiques : مشاركة بدون اذن؛ البيانات الكاملة والحالة متاحة للجميع.
- Data Space Manifest (DS Manifest) : manifeste avec Norito pour DS (validateurs/clés QC), ou ISI, DA, الاحتفاظ، الحصص، سياسة ZK، الرسوم). Il s'agit d'un hash الـ manifest et d'un lien. Le quorum de DS est basé sur ML-DSA-87 (فئة Dilithium5) post-quantique.
- Répertoire spatial : عقد دليل عالمي على السلسلة يتتبع manifeste DS والنسخ واحداث الحوكمة/الدوران لاغراض الاستدلال والتدقيق.
- DSID : correspond à Data Space. Il s'agit de l'espace de noms et de l'espace de noms.
- Ancre : التزام تشفيري من كتلة/راس DS يضم الى سلسلة nexus لربط تاريخ DS بالدفتر العالمي.
- Kura : تخزين كتل Iroha. Il s'agit de blobs qui sont également des blobs.
- WSV : Iroha Vue de l'état mondial. ممتد هنا بمقاطع حالة ذات نسخ ولقطات ومرمزة بالمحو.
- IVM : machine virtuelle Iroha pour la machine virtuelle (bytecode Kotodama `.to`).- AIR : Représentation Algébrique Intermédiaire. تمثيل جبري للحساب من اجل ادلة نمط STARK يصف التنفيذ كسلاسل تعتمد على الحقول مع قيود انتقال وحدود.

Espaces de données
- Nom : `DataSpaceId (DSID)` pour DS et espace de noms pour vous. يمكن انشاء DS على درجتين:
  - Domaine-DS : `ds::domain::<domain_name>` - تنفيذ وحالة ضمن نطاق domaine.
  - Asset-DS : `ds::asset::<domain_name>::<asset_name>` - تنفيذ وحالة ضمن تعريف اصل واحد.
  الشكلان يتعايشان؛ Vous êtes en contact avec DSID.
- دورة حياة manifeste: تسجيل إنشاء DS والتحديثات (تدوير المفاتيح، تغييرات السياسة) et dans Space Directory. Il y a un hachage pour le manifeste DS pour le slot.
- Types : DS public (مشاركة مفتوحة، DA عامة) et DS privé (مقيد، DA سرية). Il y a des drapeaux dans le manifeste.
- السياسات لكل DS: صلاحيات ISI,، معلمات DA `(k,m)`, تشفير، احتفاظ، حصص (حصة tx الدنيا/العليا لكل كتلة)، سياسة اثبات ZK/تفاؤلي، رسوم.
- Fonctionnalité : Définition de la connexion DS avec le manifeste multisig et la connexion multisig خارجية مثبتة بمعاملات lien et attestations).Manifestes القدرات et UAID
- Paramètres de l'interface : vous pouvez utiliser l'UAID pour les espaces de données (`UniversalAccountId` et `crates/iroha_data_model/src/nexus/manifest.rs`). Afficher les manifestes (`AssetPermissionManifest`) pour l'UAID dans l'espace de données et autoriser/refuser `ManifestEntry` est également `dataspace` et `program_id` et `method` et `asset` et AMX. قواعد nier تفوز دائما؛ ويصدر المقيّم `ManifestVerdict::Denied` مع سبب تدقيق او منح `Allowed` مع بيانات المطابقة.
- Allocations : يحمل كل Allow bucket حتمي `AllowanceWindow` (`PerSlot`, `PerMinute`, `PerDay`) ou `max_amount` اختياري. Les hôtes et les SDK sont basés sur Norito, et vous pouvez utiliser les SDK et les hôtes.
- Télémétrie تدقيق: يبث Space Directory حدث `SpaceDirectoryEvent::{ManifestActivated, ManifestExpired, ManifestRevoked}` (`crates/iroha_data_model/src/events/data/space_directory.rs`) عند تغير حالة manifeste. Le fichier `SpaceDirectoryEventFilter` est associé à Torii/data-event pour le manifeste UAID et le deny-wins. اعداد مخصص.

Pour plus d'informations sur le SDK et les manifestes, consultez le Guide de compte universel (`docs/source/universal_accounts_guide.md`). ابق الوثيقتين متطابقتين عند تغير سياسة UAID او الادوات.المعمارية عالية المستوى
1) Chaîne de transmission (chaîne Nexus)
- تحافظ على ترتيب قانوني واحد لكتل Nexus كل 1 ثانية التثبيت معاملات ذرية تمتد عبر Data Spaces (DS). كل معاملة ملتزمة تحدث world state الموحد (متجه racines لكل DS).
- تحتوي بيانات وصفية دنيا مع proofs/QCs مجمعة لضمان composability والنهائية وكشف الاحتيال (DSIDs المتأثرة، racines لكل DS قبل/بعد، التزامات DA, preuves صلاحية لكل DS, et quorum للـ DS byاستخدام ML-DSA-87). لا تتضمن بيانات خاصة.
- Niveau : BFT par Pipeline à 22 heures (3f+1 à f=7) avec VRF/mise en jeu pour un montant de ~200 000 مدقق محتمل. لجنة Nexus ترتب المعاملات وتؤكد الكتلة خلال 1s.

2) Espace de données ouvert (public/privé)
- تنفذ مقاطع المعاملات لكل كتلة (preuves مجمعة لكل DS والتزامات DA) Le Nexus est mis en 1s.
- Private DS تشفر البيانات في السكون والحركة بين المدققين المصرح لهم؛ ولا يغادر DS سوى الالتزامات و preuves صلاحية PQ.
- Public DS تصدر اجسام البيانات الكاملة (via DA) et preuves صلاحية PQ.3) Utiliser les espaces de données (AMX)
- Élément : il s'agit d'un élément de votre DS (pour le domaine DS et l'actif DS). تلتزم ذرّيا في كتلة Nexus واحدة او تلغى؛ لا آثار جزئية.
- Prepare-Commit خلال 1s : لكل معاملة مرشحة، تنفذ DS المعنية بالتوازي على نفس snapshot (roots DS by slot) et proofs صلاحية PQ Pour DS (FASTPQ-ISI) et DA. يلتزم comite nexus فقط اذا تحققت كل proofs المطلوبة ووصلت شهادات DA في الوقت (هدف <=300 ms)؛ والا تعاد جدولة المعاملة للـ slot التالي.
- الاتساق: مجموعات القراءة/الكتابة مصرح بها؛ Vous avez la possibilité de télécharger la machine à sous Roots بداية. التنفيذ المتفائل بدون اقفال لكل DS يتجنب التعطيل العالمي؛ Il s'agit d'un lien vers le lien (الكل او لا شيء عبر DS).
- الخصوصية : تصدر DS privé فقط preuves/engagements مرتبطة بـ racines قبل/بعد. لا تخرج بيانات خاصة خام.

4) توفر البيانات (DA) بترميز محو
- يخزن Kura اجسام الكتل ولقطات WSV كـ blobs byترميز محو. يتم توزيع blobs العامة على نطاق واسع؛ Il s'agit d'un blobs contenant des fragments de private-DS et des morceaux.
- Vous pouvez utiliser DA pour les artefacts de DS et Nexus pour l'échantillonnage et l'échantillonnage. المحتوى الخاص.هيكل الكتل والالتزام
- Artefact اثبات Data Space (pour slot 1s et DS)
  - Paramètres : dsid, slot, pre_state_root, post_state_root, ds_tx_set_hash, kura_da_commitment, wsv_da_commitment, manifest_hash, ds_qc (ML-DSA-87), ds_validity_proof (FASTPQ-ISI).
  - Utiliser les artefacts private-DS pour plus de détails وpublic DS تسمح باسترجاع الاجسام عبر DA.

- Bloc Nexus (par 1s)
  - Paramètres : block_number, parent_hash, slot_time, tx_list (pour cross-DS et DSID), ds_artifacts[], nexus_qc.
  - Objets : objets d'art et objets d'art تحديث متجه racines العالمي لكل DS في خطوة واحدة.الاجماع et planification
- Chaîne Nexus : BFT prend en charge (Sumeragi) en 22 secondes (3f+1 ou f=7) en 1s ونهائية 1s. يتم اختيار اللجنة عبر VRF/mise pour لكل حقبة من ~200k مرشح؛ الدوران يحافظ على اللامركزية ومقاومة الرقابة.
- اجماع Data Space: يشغل كل DS BFT خاص به لانتاج artefacts لكل slot (preuves, تزامات DA, DS QC). يتم تحجيم لجان lane-relay الى `3f+1` byاستخدام `fault_tolerance` للـ dataspace ويتم اختيارها بشكل حتمي لكل حقبة من مجموعة المدققين باستخدام VRF seed مرتبط بـ `(dataspace_id, lane_id)`. Privé DS مقيدة؛ Public DS est un anti-Sybil. اللجنة العالمية Nexus ثابتة.
- Planification des tâches : يرسل المستخدمون معاملات ذرية مع DSID ومجموعات القراءة/الكتابة. تنفذ DS بالتوازي داخل slot؛ La durée du lien nexus est de 1 s et les artefacts et les artefacts sont pris en charge pendant 1 s (<= 300 ms).
- عزل الاداء: لكل DS mempools وتنفيذ مستقل. Les quotas pour DS sont également pour le blocage de tête de ligne et les DS privés.Questions relatives à l'espacement des noms et des noms
- ID qualifiés DS : pour les domaines (domaines, comptes, actifs, rôles) selon `dsid`. Source : `ds::<domain>::account`, `ds::<domain>::asset#precision`.
- Références mondiales : Le tuple `(dsid, object_id, version_hint)` est un tuple en chaîne pour nexus et AMX pour cross-DS.
- Sérialisation Norito : pour les codecs cross-DS (preuves AMX) et Norito. لا استخدام لـ serde في مسارات الانتاج.

العقود الذكية وامتدادات IVM
- Nom du produit : `dsid` et IVM. Utilisez Kotodama pour utiliser l'espace de données.
- primitif ذرية عبر DS :
  - `amx_begin()` / `amx_commit()` est compatible avec l'hôte multi-DS vers IVM.
  - `amx_touch(dsid, key)` prend en charge l'instantané Roots pour l'emplacement.
  - `verify_space_proof(dsid, proof, statement)` -> booléen
  - `use_asset_handle(handle, op, amount)` -> result (مسموح فقط اذا سمحت السياسة وكان handle صالحا)
- Poignées d'actifs والرسوم :
  - عمليات الاصول مصرح بها عبر سياسات ISI/role للـ DS؛ Le gaz est utilisé pour DS. Il existe des jetons de capacité et des fonctionnalités d'approbation multiple (multi-approbateurs, limites de débit, géorepérage) pour créer des liens.
- Fonctions : les appels système sont activés et les appels système AMX activés. لا تأثيرات خفية للوقت او البيئة.اثباتات الصلاحية post-quantique (ISI معممة)
- FASTPQ-ISI (PQ pour configuration de confiance) : transfert de données basé sur le hachage pour ISI avec un minimum de 20 k Il s'agit d'un GPU.
  - ملف التشغيل :
    - Utilisez le prouveur `fastpq_prover::Prover::canonical` pour le backend. تمت ازالة mock الحتمي. [crates/fastpq_prover/src/proof.rs:126]
    - `zk.fastpq.execution_mode` (config) et `irohad --fastpq-execution-mode` pour les connexions CPU/GPU avec le hook d'observateur demandé/résolu/backend للتدقيقات. [crates/iroha_config/src/parameters/user.rs:1357] [crates/irohad/src/main.rs:270] [crates/irohad/src/main.rs:2192] [crates/iroha_telemetry/src/metrics.rs:8887]
- Aritmétisation :
  - KV-Update AIR : WSV utilise la valeur-clé par rapport à Poseidon2-SMT. ISI يتوسع الى مجموعة صغيرة من صفوف read-check-write على المفاتيح (comptes, actifs, rôles, domaines, métadonnées, approvisionnement).
  - Il s'agit d'un opcode : AIR et le sélecteur de plage sont également associés à ISI (conservation, compteurs monotones, autorisations, vérifications de plage, métadonnées de gestion).
  - Arguments de recherche : جداول شفافة ملتزمة بالهاش لصلاحيات/ادوار وprecisions ومعلمات سياسة تتجنب قيود bitwise الثقيلة.
- Fonctions et fonctionnalités:
  - Preuve SMT agrégée : جميع المفاتيح المتأثرة (pre/post) تثبت مقابل `old_root`/`new_root` by frontier مضغوط مع siblings مكررة تمت ازالتها.- Invariants: يتم فرض ثوابت عالمية (مثل اجمالي supply لكل اصل) عبر مساواة multiset بين صفوف التأثير والعدادات المتعقبة.
- نظام الاثبات:
  - La date d'ouverture du vendredi (DEEP-FRI) avec arity عالية (8/16) et Blow-up 8-16؛ hache Poséidon2؛ transcription Fiat-Shamir مع SHA-2/3.
  - Récursion Fonctionnalité : Utilisation de DS pour les micro-batches et les slots pour les micro-lots.
- النطاق والامثلة:
  - Fonctions : transférer, créer, graver, enregistrer/désenregistrer les définitions d'actifs, définir la précision (مقيد), définir les métadonnées.
  - Fonctions/المجالات : créer/supprimer, définir une clé/un seuil, ajouter/supprimer des signataires (حالة فقط؛ فحص التواقيع يثبت من مدققي DS وليس داخل AIR).
  - الادوار/الصلاحيات (ISI) : accorder/révoquer des rôles et des autorisations Il s'agit d'une recherche et de contrôles monotones.
  - Fonction/AMX : début/commit pour la capacité AMX, menthe/révoquer. تثبت كتحولات حالة وعدادات سياسة.
- فحوصات خارج AIR للحفاظ على الكمون:
  - التواقيع والتشفير الثقيل (مثل تواقيع ML-DSA للمستخدمين) يتحقق منها مدققو DS et DS QC؛ اثبات الصلاحية يغطي فقط اتساق الحالة والامتثال للسياسات. هذا يبقي preuves PQ وسريعة.
- Fonctionnement (processeur CPU 32 pouces + GPU intégré) :
  - 20k ISI مختلطة مع لمس مفاتيح صغير (<=8 touches/ISI) : ~0,4-0,9 s اثبات، ~150-450 KB اثبات، ~5-15 ms تحقق.
  - ISI اثقل : micro-batch (مثلا 10x2k) + récursion للحفاظ على <1 s par slot.
- تهيئة DS Manifeste :
  -`zk.policy = "fastpq_isi"`-`zk.hash = "poseidon2"`, `zk.fri = { blowup: 8|16, arity: 8|16 }`
  -`state.commitment = "smt_poseidon2"`
  -`zk.recursion = { none | local }`
  - `attestation.signatures_in_proof = false` (pour DS QC)
  - `attestation.qc_signature = "ml_dsa_87"` (افتراضي؛ البدائل يجب اعلانها صراحة)
- Solutions de repli :
  - ISI المعقدة/المخصصة يمكنها استخدام STARK عام (`zk.policy = "stark_fri_general"`) مع اثبات مؤجل ونهائية 1s عبر attestation QC + slashing على ادلة غير صحيحة.
  - خيارات غير PQ (مثل Plonk مع KZG) تتطلب Trusted Setup et لم تعد مدعومة في build الافتراضي.

Apprêt AIR (لـ Nexus)
- Trace التنفيذ: مصفوفة بعرض (اعمدة سجلات) et (خطوات). كل صف هو خطوة منطقية في معالجة ISI؛ Il s'agit des sélecteurs et des drapeaux pré/post.
- القيود:
  - قيود الانتقال : تفرض علاقات صف-الى-صف (مثلا post_balance = pre_balance - montant لصف خصم عند `sel_transfer = 1`).
  - Fonctionnalités : utiliser les E/S publiques (old_root/new_root, counters) pour les entrées/sorties publiques.
  - Recherches/permutations : les autorisations et les paramètres d'actifs sont plus lourds en bits.
- التزام والتحقق :
  - يقوم prouveur pour les méthodes de hachage et les outils de hachage et les outils de hachage.
  - Il s'agit d'un vérificateur de type FRI (basé sur le hachage, post-quantique) et de Merkle. التكلفة لوغاريتمية بعدد الخطوات.
- Fonction (Transfert) : paramètres pre_balance, montant, post_balance, nonce, sélecteurs. Il s'agit d'une solution SMT multi-preuves pré/post pour les racines anciennes/nouvelles.Utiliser ABI et les appels système (ABI v1)
- Syscalls لاضافتها (اسماء توضيحية):
  - `SYS_AMX_BEGIN`, `SYS_AMX_TOUCH`, `SYS_AMX_COMMIT`, `SYS_VERIFY_SPACE_PROOF`, `SYS_USE_ASSET_HANDLE`.
- انواع pointeur-ABI لاضافتها :
  -`PointerType::DataSpaceId`, `PointerType::AmxDescriptor`, `PointerType::AssetHandle`, `PointerType::ProofBlob`.
- تحديثات مطلوبة:
  - اضافة الى `ivm::syscalls::abi_syscall_list()` (مع الحفاظ على الترتيب) et مع gate حسب السياسة.
  - ربط الارقام غير المعروفة بـ `VMError::UnknownSyscall` pour les hôtes.
  - Fonctions : liste d'appels système dorée, hachage ABI, ID de type de pointeur dorés, et fonctions de hachage.
  - Documents : `crates/ivm/docs/syscalls.md`, `status.md`, `roadmap.md`.

نموذج الخصوصية
- احتواء البيانات الخاصة: اجسام المعاملات وفروق الحالة ولقطات WSV الخاصة by private DS لا تغادر مجموعة المدققين الخاصة.
- التعرض العام: يتم تصدير en-têtes والتزامات DA et preuves صلاحية PQ فقط.
- ادلة ZK اختيارية: يمكن لـ private DS انتاج ادلة ZK (مثل كفاية الرصيد او امتثال السياسة) لتمكين عمليات cross-DS دون كشف الحالة الداخلية.
- Fonctionnalités : يتم فرض التخويل عبر سياسات ISI/rôle pour DS. jetons de capacité اختيارية ويمكن اضافتها لاحقا.

عزل الاداء et QoS
- اجماع ومجموعات انتظار وتخزين منفصلة لكل DS.
- quotas pour la planification, pour DS et pour Nexus, pour les ancres et le blocage de tête de ligne.
- ميزانيات موارد العقود لكل DS (calcul/mémoire/IO) pour l'hôte IVM. Les DS publics sont également des DS privés.
- Les cross-DS sont également compatibles avec les DS privés.توفر البيانات وتصميم التخزين
1) ترميز المحو
- استخدام Reed-Solomon منهجي (مثلا GF(2^16)) لترميز المحو على مستوى blob لكتل Kura ولقطات WSV: معلمات `(k, m)` مع Fragments `n = k + m`.
- Nombre de fragments (DS public) : `k=32, m=16` (n = 48) pour 16 fragments, avec une valeur de ~1,5x. Pour DS privé : `k=16, m=8` (n = 24) ضمن المجموعة المصرح بها. Il s'agit du manifeste DS.
- Blobs العامة : shards موزعة عبر عقد DA/مدققين عديدة مع فحوص توفر بالعينات. Les en-têtes sont des clients légers.
- Blobs الخاصة : shards مشفرة وموزعة فقط بين مدققي private-DS (او حراس معينين). Les fragments de DA (دون مواقع او المفاتيح) sont inclus.

2) Analyses et échantillonnage
- Pour le blob : la racine Merkle et les fragments sont également utilisés pour `*_da_commitment`. حافظ على PQ بتجنب الالتزامات البيضوية.
- DA Attesters : attestateurs اقليميون يتم اختيارهم عبر VRF (مثلا 64 لكل منطقة) يصدرون شهادة ML-DSA-87 تؤكد sampling الناجح. الهدف لزمن attestation <=300 ms. Le Nexus est doté de fragments de fragments.

3) تكامل Kura
- Il s'agit de blobs par rapport à Merkle.
- en-têtes تحمل التزامات blobs؛ Vous pouvez également utiliser DA pour DS public et DS privé.4) Comment WSV
- Snapshots WSV : vous pouvez utiliser le point de contrôle pour DS et les instantanés ainsi que les en-têtes. Vous trouverez également des journaux de modifications. لقطات public توزع على نطاق واسع، ولقطات private تبقى داخل مدققين خاصين.
- Accès avec preuve : يمكن للعقود تقديم (او طلب) ادلة حالة (Merkle/Verkle) مثبتة عبر التزامات instantané. يمكن لـ private DS تقديم شهادات zero-knowledge بدلا من ادلة خام.

5) الاحتفاظ et taille
- بدون élagage للـ public DS: الاحتفاظ بكل اجسام Kura ولقطات WSV عبر DA (توسع افقي). يمكن لـ private DS تحديد احتفاظ داخلي، لكن الالتزامات المصدرة تظل غير قابلة للتغيير. Nexus utilise les blocs Nexus et les artefacts pour DS.

الشبكات وادوار العقد
- Fonctions de connexion : connexions avec les blocs Nexus et artefacts DS, ainsi que pour DS public.
- مدققو Data Space : يشغلون اجماع DS, ينفذون العقود، يديرون Kura/WSV المحلي، ويتولون DA لـ DS الخاصة بهم.
- عقد DA (اختياري): تخزن/تنشر blobs et échantillonnage. Dans DS privé, il y a DA تكون متجاورة مع المدققين او حراس موثوقين.تحسينات واعتبارات على مستوى النظام
- Pour le séquençage/mempool : utiliser le mempool DAG (pour Narwhal) et BFT pour le lien nexus pour le débit et le débit et le débit المنطقي.
- حصص DS والعدالة: حصص لكل DS في كل كتلة وحدود وزن لتجنب head-of-line blocking وضمان كمون متوقع لـ private DS.
- Pour DS (PQ) : le quorum pour DS est pour ML-DSA-87 (pour Dilithium5) pour. Il s'agit d'une application post-quantique pour EC et d'une machine à sous QC. يمكن لـ DS اختيار ML-DSA-65/44 الاصغر او تواقيع EC اذا اعلن في DS Manifest؛ Le DS public est compatible avec ML-DSA-87.
- Attestateurs DA : للـ public DS, استخدم attestateurs اقليميين مختارين بـ VRF يصدرون شهادات DA. Nexus est une solution pour l'échantillonnage et l'échantillonnage. private DS تحتفظ بشهاداتها الداخلية.
- Récursion et récursivité : vous pouvez utiliser des micro-batches pour DS et les slots/époques et les options de récursion التحقق مستقرا تحت الحمل العالي.
- Mise à l'échelle des voies (عند الحاجة) : Les voies K sont également utilisées pour les voies K. هذا يحافظ على ترتيب عالمي واحد مع توسع افقي.
- Fonctions supplémentaires : les noyaux SIMD/CUDA sont dotés d'indicateurs de fonctionnalité pour le hachage/FFT et le processeur de secours pour les fonctionnalités de base.- Nombre de voies (اقتراح): 2-4 voies ou (a) Vitesse de déplacement p95 1,2 s ou 3 دقائق متتالية، او (b) تجاوز اشغال الكتلة 85% لاكثر من 5 دقائق، او (c) تطلب معدل المعاملات الداخل >1.2x من سعة الكتلة بشكل مستدام. Les voies sont utilisées pour le hachage DSID et le lien.

الرسوم والاقتصاد (افتراضات اولية)
- Nom du produit : jeton pour DS ou pour computing/IO؛ Il s'agit d'une commande pour DS. التحويل بين DS مسؤولية التطبيق.
- Mode de fonctionnement : round-robin pour DS et SLO 1s داخل DS يمكن للمزايدة على الرسوم ان تفك التعادل.
- مستقبلا: يمكن استكشاف سوق رسوم عالمي او سياسات تقلل MEV دون تغيير الذرية او تصميم ادلة PQ.

سير عمل cross-Data-Space (مثال)
1) يرسل مستخدم معاملة AMX تمس public DS P et private DS S : نقل الاصل X من S الى المستفيد B الذي حسابه في P.
2) Utilisez l'emplacement P et S pour prendre un instantané. تتحقق S من التفويض والتوفر وتحدث حالتها الداخلية وتنتج اثبات صلاحية PQ والتزام DA (بدون تسريب بيانات خاصة). يحضر P تحديث الحالة المقابل (مثل mint/burn/locking في P حسب السياسة) واثباته.
3) Utiliser Nexus avec DS et DA؛ Il s'agit d'un slot pour les racines du DS et de l'état mondial Nexus. العالمي.
4) اذا كان اي اثبات او شهادة DA مفقودا او غير صالح، تلغى المعاملة (بدون اثر) ويمكن للعميل اعادة ارسالها للـ slot التالي. لا تغادر بيانات S الخاصة في اي خطوة.- اعتبارات الامان
- Fonctionnalités : Utiliser les appels système pour IVM. Il s'agit d'un engagement DS avec AMX commit et d'un engagement supplémentaire.
- Fonctionnalités : ISI ISI pour DS privées et plus encore. jetons de capacité sont disponibles pour cross-DS.
- Titre : Vous pouvez utiliser les fragments private-DS pour créer un lien vers ZK اختيارية لاثباتات خارجية.
- DoS : Il s'agit d'un mempool/consensus/storage ou d'un DS public ou privé.

تغييرات على مكونات Iroha
- iroha_data_model : utilise `DataSpaceId` et les identifiants pour les descripteurs DS et AMX (مجموعات قراءة/كتابة) et les preuves/التزامات DA. Voir Norito ici.
- ivm : appels système et types pointeur-ABI pour AMX (`amx_begin`, `amx_commit`, `amx_touch`) et preuves DA تحديث اختبارات/توثيق ABI وفقا لسياسة v1.