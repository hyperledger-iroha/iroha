---
lang: ar
direction: rtl
source: docs/source/nexus.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c8da33b0abb8a6d46dbaaed657c8338a9d723a97f6f28ff29a62caf84c0dbfd6
source_last_modified: "2025-12-27T07:56:34.355655+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- الترجمة العربية لـ docs/source/nexus.md -->

#! Iroha 3 - Sora Nexus Ledger: مواصفات التصميم الفنية

يقترح هذا المستند بنية Sora Nexus Ledger لـ Iroha 3، مع تطوير Iroha 2 نحو دفتر عالمي موحد منطقيا مبني حول Data Spaces ‏(DS). توفر Data Spaces مجالات خصوصية قوية ("private data spaces") ومشاركة مفتوحة ("public data spaces"). يحافظ التصميم على القابلية للتركيب عبر الدفتر العالمي مع ضمان عزل صارم وسرية لبيانات DS الخاصة، ويقدم توسيع توافر البيانات عبر erasure coding في Kura (تخزين البلوك) وWSV (World State View).

يبني نفس المستودع Iroha 2 (شبكات مستضافة ذاتيا) وIroha 3 (SORA Nexus). يتم التنفيذ عبر Iroha Virtual Machine ‏(IVM) وسلسلة أدوات Kotodama المشتركة، لذا تبقى العقود واثار البايت كود قابلة للنقل بين النشر الذاتي والدفتر العالمي لـ Nexus.

الاهداف
- دفتر منطقي عالمي واحد مكون من العديد من المدققين و Data Spaces المتعاونة.
- Data Spaces خاصة للتشغيل المصرح (مثل CBDC) بحيث لا تغادر البيانات DS الخاص.
- Data Spaces عامة بمشاركة مفتوحة ووصول بدون صلاحيات على نمط Ethereum.
- عقود ذكية قابلة للتركيب بين Data Spaces مع صلاحيات صريحة للوصول الى اصول DS الخاصة.
- عزل الاداء حتى لا تؤثر النشاطات العامة على معاملات DS الخاصة الداخلية.
- توافر بيانات على نطاق واسع: Kura وWSV مع erasure coding لدعم بيانات غير محدودة عمليا مع الحفاظ على خصوصية DS الخاصة.

غير الاهداف (المرحلة الاولية)
- تعريف اقتصاديات الرموز او حوافز المدققين؛ الجدولة والستيكينغ قابلة للتركيب.
- تقديم نسخة ABI جديدة او توسيع اسطح syscalls/pointer-ABI؛ ABI v1 ثابت وترقيات runtime لا تغير ABI المضيف.

المصطلحات
- Nexus Ledger: الدفتر المنطقي العالمي الناتج من تركيب كتل Data Space (DS) الى تاريخ واحد مرتب والتزام حالة.
- Data Space (DS): مجال تنفيذ وتخزين مقيد بمدققين وحوكمة وفئة خصوصية وسياسة DA وحصص وسياسة fees خاصة به. هناك فئتان: DS عام وDS خاص.
- Private Data Space: مدققون مصرحون وتحكم وصول؛ بيانات المعاملات والحالة لا تخرج من DS. يتم تثبيت commitments/metadata فقط عالميا.
- Public Data Space: مشاركة بدون صلاحيات؛ البيانات الكاملة والحالة متاحة علنا.
- Data Space Manifest (DS Manifest): بيان Norito يعلن معلمات DS (validators/QC keys, privacy class, ISI policy, DA params, retention, quotas, ZK policy, fees). يتم تثبيت hash البيان على سلسلة nexus. ما لم يذكر خلاف ذلك تستخدم شهادات quorum لـ DS توقيع ML-DSA-87 (فئة Dilithium5) كافتراضي ما بعد كمي.
- Space Directory: عقد دليل عالمي on-chain يتتبع بيانات DS والنسخ واحداث الحوكمة/الدوران من اجل الحل والمراجعة.
- DSID: معرف عالمي فريد لـ Data Space، يستخدم لعمل namespace لكل الاشياء والمراجع.
- Anchor: التزام تشفيري من بلوك/راس DS مضمّن في سلسلة nexus لربط تاريخ DS بالدفتر العالمي.
- Kura: تخزين كتل Iroha. ممدد هنا بتخزين blobs مع erasure coding والتزامات.
- WSV: World State View في Iroha. ممدد هنا بمقاطع حالة متسلسلة مع snapshots وerasure coding.
- IVM: Iroha Virtual Machine لتنفيذ العقود الذكية (Kotodama bytecode `.to`).
 - AIR: Algebraic Intermediate Representation. عرض جبري للحساب من اجل ادلة STARK، يصف التنفيذ كمسارات على الحقول مع قيود انتقال وحدود.

نموذج Data Spaces
- الهوية: `DataSpaceId (DSID)` يعرّف DS ويؤسس namespace. يمكن إنشاء DS على مستويين:
  - Domain-DS: `ds::domain::<domain_name>` - تنفيذ وحالة محصورة بدومين.
  - Asset-DS: `ds::asset::<domain_name>::<asset_name>` - تنفيذ وحالة لتعريف اصل واحد.
  كلا النموذجين يتعايشان؛ المعاملات يمكن ان تلمس عدة DSID بشكل ذري.
- دورة حياة البيان: انشاء DS وتحديثات (دوران المفاتيح، تغيير السياسات) وتقاعدها تسجل في Space Directory. كل اثر لكل slot يشير الى hash البيان الاحدث.
- الفئات: DS عام (مشاركة مفتوحة، DA عام) وDS خاص (مصرح، DA سري). السياسات الهجينة ممكنة عبر flags في البيان.
- سياسات لكل DS: صلاحيات ISI، معلمات DA `(k,m)`، تشفير، retention، حصص (حد ادنى/اقصى من المعاملات لكل بلوك)، سياسة ZK/optimistic proofs، fees.
- الحوكمة: عضوية DS ودوران المدققين محدد في قسم الحوكمة بالبيان (مقترحات on-chain، multisig، او حوكمة خارجية مثبتة عبر معاملات nexus وattestations).

Gossip واعٍ لـ dataspace
- دفعات gossip للمعاملات تحمل tag للplane (public vs restricted) مشتقة من catalog للanes؛ الدفعات المقيدة ترسل unicast الى نظراء commit topology الحالية المتصلين (وفق `transaction_gossip_restricted_target_cap`) بينما الدفعات العامة تستخدم `transaction_gossip_public_target_cap` (اضبط `null` للبث). يتم اعادة خلط اختيار الاهداف حسب `transaction_gossip_public_target_reshuffle_ms` و`transaction_gossip_restricted_target_reshuffle_ms` (افتراضي: `transaction_gossip_period_ms`). عندما لا يوجد نظراء متصلون في commit topology يمكن للمشغلين رفض او تمرير payloads المقيدة الى overlay العام عبر `transaction_gossip_restricted_public_payload` (افتراضي `refuse`). تكشف التليمترية محاولات fallback وعدادات forward/drop والسياسة المختارة مع اختيار الاهداف حسب dataspace.
- dataspaces غير المعروفة يعاد وضعها في الصف اذا كان `transaction_gossip_drop_unknown_dataspace` مفعلا؛否则 تُوجه الى targeting مقيد لمنع التسريب.
- التحقق في جهة الاستقبال يسقط الادخالات التي لا تطابق lanes/dataspaces الكاتالوج المحلي، او التي لا يتطابق وسم plane فيها مع رؤية dataspace المشتقة، او التي لا تتطابق فيها route المعلنة مع قرار التوجيه المعاد اشتقاقه محليا.

Manifests القدرات وUAID
- الحسابات العالمية: كل مشارك يحصل على UAID حتمي (`UniversalAccountId` في `crates/iroha_data_model/src/nexus/manifest.rs`) يغطي كل dataspaces. manifests القدرات (`AssetPermissionManifest`) تربط UAID بdataspace محدد وepochs للتفعيل/الانتهاء وقائمة مرتبة من قواعد allow/deny `ManifestEntry` تحدد `dataspace`, `program_id`, `method`, `asset` وادوار AMX اختيارية. قواعد deny تفوز دائما؛ المقيّم يخرج `ManifestVerdict::Denied` مع سبب تدقيق او grant `Allowed` مع metadata السماح المطابق.
- snapshots لمحافظ UAID مكشوفة عبر `GET /v1/accounts/{uaid}/portfolio` (راجع `docs/source/torii/portfolio_api.md`) ومدعومة بالمجمع الحتمي في `iroha_core::nexus::portfolio`.
- Allowances: كل allow يحمل buckets `AllowanceWindow` حتمية (`PerSlot`, `PerMinute`, `PerDay`) مع `max_amount` اختياري. Hosts وSDKs تستهلك نفس payload Norito لضمان تماثل التنفيذ.
- تليمترية التدقيق: Space Directory يبث `SpaceDirectoryEvent::{ManifestActivated, ManifestExpired, ManifestRevoked}` (`crates/iroha_data_model/src/events/data/space_directory.rs`) عند تغير حالة البيان. السطح `SpaceDirectoryEventFilter` يسمح لمشتركي Torii/data-event بمراقبة التحديثات والالغاءات وقرارات deny-wins بدون plumbing مخصص.

### عمليات مانيفست UAID

عمليات Space Directory تقدم بطريقتين: CLI مدمج (للتشغيل المؤتمت بالسكربت) او ارسال مباشر عبر Torii (لـ CI/CD). كلا المسارين يفرضان صلاحية `CanPublishSpaceDirectoryManifest{dataspace}` في المنفذ (`crates/iroha_core/src/smartcontracts/isi/space_directory.rs`) ويسجلان احداث دورة الحياة في world state (`iroha_core::state::space_directory_manifests`).

#### مسار CLI (`iroha app space-directory manifest ...`)

1. **ترميز JSON للبيان** — تحويل المسودات الى Norito bytes مع انتاج hash قابل لاعادة الانتاج قبل المراجعة:

   ```bash
   iroha app space-directory manifest encode \
     --json dataspace/capability.json \
     --out artifacts/capability.manifest.to \
     --hash-out artifacts/capability.manifest.hash
   ```

   يقبل helper اما `--json` (JSON خام) او `--manifest` (payload `.to`) ويطابق منطق `crates/iroha_cli/src/space_directory.rs::ManifestEncodeArgs`.

2. **نشر/استبدال المانيفست** — ادراج تعليمات `PublishSpaceDirectoryManifest` من مصادر Norito او JSON:

   ```bash
   iroha app space-directory manifest publish \
     --manifest artifacts/capability.manifest.to \
     --reason "Retail wave 4 on-boarding"
   ```

   `--reason` يملأ `entries[*].notes` للسجلات التي لم تذكر ملاحظات المشغل.

3. **انتهاء** المانيفستات او **الغاء** UAIDs عند الطلب. كلا الامرين يقبل `--uaid uaid:<hex>` او digest hex مكون من 64 خانة (LSB=1) وid عددي للـ dataspace:

   ```bash
   iroha app space-directory manifest expire \
     --uaid uaid:0f4d86b20839a8ddbe8a1a3d21cf1c502d49f3f79f0fa1cd88d5f24c56c0ab11 \
     --dataspace 11 \
     --expired-epoch 4600

   iroha app space-directory manifest revoke \
     --uaid uaid:0f4d86b20839a8ddbe8a1a3d21cf1c502d49f3f79f0fa1cd88d5f24c56c0ab11 \
     --dataspace 11 \
     --revoked-epoch 9216 \
     --reason "Fraud investigation NX-16-R05"
   ```

4. **انشاء حزمة تدقيق** — `manifest audit-bundle` يكتب JSON والـ `.to` والـ hash وبروفايل dataspace وmetadata مقروءة آليا الى مجلد مخرجات:

   ```bash
   iroha app space-directory manifest audit-bundle \
     --manifest-json dataspace/capability.json \
     --profile dataspace/profiles/cbdc_profile.json \
     --out-dir artifacts/capability_bundle
   ```

   تضمن الحزمة hooks من `SpaceDirectoryEvent` لاثبات ان dataspace يعرض webhooks تدقيق إلزامية؛ راجع `docs/space-directory.md` للتفاصيل.

#### Torii APIs

يمكن للمشغلين وSDKs تنفيذ نفس العمليات عبر HTTPS. Torii يطبق نفس التحقق من الصلاحيات ويوقع المعاملات لصالح السلطة المقدمة (المفاتيح الخاصة تبقى في الذاكرة داخل معالج Torii الامن):

- `GET /v1/space-directory/uaids/{uaid}` — حل bindings الحالية للـ dataspace لUAID (عناوين موحدة، ids، program bindings). اضف `address_format=compressed` لاخراج Sora Name Service (IH58 هو المفضل؛ compressed (`sora`) هو الخيار الثاني لسورا فقط).
- `GET /v1/space-directory/uaids/{uaid}/portfolio` — مجمع Norito يعكس `ToriiClient.getUaidPortfolio` لاظهار الممتلكات العامة.
- `GET /v1/space-directory/uaids/{uaid}/manifests?dataspace={id}` — جلب JSON المانيفست canonical وmetadata وhash.
- `POST /v1/space-directory/manifests` — ارسال مانيفست جديد او بديل من JSON (`authority`, `private_key`, `manifest`, `reason` اختياري). Torii يعيد `202 Accepted` عند ادخال المعاملة في الصف.
- `POST /v1/space-directory/manifests/revoke` — ادراج الغاءات طارئة مع UAID وdataspace id وepoch فعال وسبب اختياري.

SDK JS (`javascript/iroha_js/src/toriiClient.js`) يغلف هذه الواجهات عبر `ToriiClient.getUaidPortfolio` و`.getUaidBindings` و`.getUaidManifests`؛ اصدارات Swift/Python ستعيد استخدام نفس REST payloads. راجع `docs/source/torii/portfolio_api.md` و`docs/space-directory.md`.

تحديثات SDK/AMX الاخيرة
- **NX-11 (التحقق من relay بين lanes):** helpers في SDK تتحقق من envelopes المعروضة عبر `/v1/sumeragi/status`. عميل Rust يوفر `iroha::nexus` لبناء/التحقق ورفض التكرار `(lane_id, dataspace_id, height)`. ربط Python يوفر `verify_lane_relay_envelope_bytes`/`lane_settlement_hash` وSDK JS يوفر `verifyLaneRelayEnvelope`/`laneRelayEnvelopeSample`.
  【crates/iroha/src/nexus.rs:1】【python/iroha_python/iroha_python_rs/src/lib.rs:666】【crates/iroha_js_host/src/lib.rs:640】【javascript/iroha_js/src/nexus.js:1】
- **NX-17 (حواجز ميزانية AMX):** `ivm::analysis::enforce_amx_budget` يقدر تكلفة التنفيذ per-dataspace/group ويطبق ميزانيات 30 ms / 140 ms. يوفر مخرجات واضحة ومغطى باختبارات وحدة.
  【crates/ivm/src/analysis.rs:142】【crates/ivm/src/analysis.rs:241】

هندسة عالية المستوى
1) طبقة التركيب العالمية (Nexus Chain)
- تحافظ على ترتيب كتل Nexus بزمن 1 s لتثبيت معاملات ذرية عبر DS واحد او اكثر، وتحديث world state العالمي.
- تحتوي metadata دنيا مع proofs/QC مجمعة لضمان القابلية للتركيب والنهائية وكشف الاحتيال (DSIDs، جذور الحالة قبل/بعد، DA commitments، proofs صلاحية DS، وDS QC باستخدام ML-DSA-87).
- اجماع: لجنة BFT عالمية بحجم 22 (3f+1 مع f=7) تُختار VRF/stake عبر epochs، وتُنهي البلوك خلال 1 s.

2) طبقة Data Space (Public/Private)
- تنفذ اجزاء DS من المعاملة وتحدث WSV محلي وتولد artifacts صحة لكل بلوك.
- DS الخاصة تشفر البيانات وتخرج فقط commitments وPQ proofs.
- DS العامة تصدر اجسام البيانات عبر DA وPQ proofs.

3) معاملات ذرية عبر Data Spaces (AMX)
- نموذج: معاملة قد تلمس عدة DS وتُثبت ذرّيا في Nexus Block او تُلغى.
- Prepare-Commit خلال 1 s مع proofs PQ وDA commitments؛ تُقبل فقط عند تحقق جميع proofs وشهادات DA (<=300 ms).
- الاتساق: read-write sets معلنة، كشف التعارضات ضد جذور بداية slot.
- الخصوصية: DS الخاصة تصدر فقط proofs/commitments مرتبطة بالجذور.

4) توافر البيانات (DA) مع erasure coding
- Kura يخزن اجسام الكتل وsnapshots كـ blobs مع erasure coding.
- DA commitments تُسجل في artifacts وblocks لتمكين الاسترجاع دون كشف البيانات الخاصة.

هيكل البلوك والالتزام
- Data Space Proof Artifact لكل DS:
  - الحقول: dsid, slot, pre_state_root, post_state_root, ds_tx_set_hash, kura_da_commitment, wsv_da_commitment, manifest_hash, ds_qc (ML-DSA-87), ds_validity_proof (FASTPQ-ISI).
  - DS الخاصة تصدر artifacts بدون اجسام بيانات، وDS العامة تسمح بالاسترجاع عبر DA.

- Nexus Block:
  - الحقول: block_number, parent_hash, slot_time, tx_list, ds_artifacts[], nexus_qc.
  - يثبت المعاملات الذرية ويحدث جذور DS عالميا.

الاجماع والجدولة
- اجماع Nexus: BFT عالمي بكتلة 1 s، لجنة 22، اختيار VRF/stake.
- اجماع DS: BFT خاص بكل DS، لجان lane-relay بحجم `3f+1` حسب `fault_tolerance`، اختيار دترمينيستي عبر seed VRF المرتبط بـ `(dataspace_id, lane_id)`.
- جدولة المعاملات: ادخالها ضمن slot اذا تحققت artifacts وDA في الوقت (<=300 ms).
- عزل الاداء: mempools وتنفيذ مستقل لكل DS، مع حصص لكل DS.

نموذج البيانات وNamespacing
- IDs مؤهلة بـ dsid لكل كيان.
- مراجع عالمية `(dsid, object_id, version_hint)`.
- جميع رسائل cross-DS تستخدم Norito.

العقود الذكية وامتدادات IVM
- اضافة `dsid` لسياق التنفيذ.
- `amx_begin`/`amx_commit` و`amx_touch` و`verify_space_proof` و`use_asset_handle`.
- الرسوم تُدفع بعملة gas الخاصة بالـ DS.
- syscalls حتمية.

Post-Quantum Validity Proofs
- FASTPQ-ISI بدون trusted setup.
- AIR، قيود، Lookups، proof system، اهداف الاداء، اعدادات manifest، وfallbacks.

AIR Primer
- trace، constraints، الالتزام والتحقق، مثال Transfer.

ABI وsyscalls (ABI v1)
- سطح ABI v1 ثابت؛ لا تتم اضافة syscalls او pointer-ABI types جديدة.
- ترقيات runtime يجب ان تبقي `abi_version = 1` و`added_syscalls`/`added_pointer_types` فارغة، مع تثبيت اختبارات ABI.

نموذج الخصوصية
- احتواء بيانات DS الخاصة، تعرض البيانات العامة فقط.
- ZK proofs اختيارية.

عزل الاداء وQoS
- عزل mempool/consensus/storage لكل DS.

تصميم التخزين وDA
- Reed-Solomon، commitments، sampling، Kura وWSV، retention.

الشبكات وادوار العقد
- مدققون عالميون، مدققو DS، وعقد DA.

تحسينات النظام
- DAG mempool، quotas، attesters، recursion، lane scaling، تسريع حتمي، عتبات تفعيل lanes.

Fees والاقتصاد
- gas per-DS، اولويات الادراج، سوق fees مستقبلي.

مثال cross-DS
- ارسال AMX، تنفيذ متوازي، تحقق proofs، commit او abort.

اعتبارات الامان
- تنفيذ حتمي، تحكم وصول، خصوصية، مقاومة DoS.

تغييرات مكونات Iroha
- تحديثات في data_model وivm وiroha_core وkura وWSV وirohad.

Configuration and Determinism
- عبر `iroha_config` مع fallbacks حتمية.

### Runtime Lane Lifecycle Control

- `POST /v1/nexus/lifecycle` لاضافة/ازالة lanes بدون اعادة تشغيل، مع سلوك وتحقق واضحين.

Migration Path
- خطوات الانتقال من Iroha 2 الى Iroha 3.

Testing Strategy
- اختبارات وحدة، IVM، تكامل، وامان.

### NX-18 Telemetry & Runbook Assets

- لوحات Grafana، بوابة CI، تجميع الادلة، وrunbooks.

Open Questions
- اسئلة مفتوحة حول التوقيع والاقتصاد وDA وغيرها.

ملحق: الامتثال لسياسات المستودع
- Norito لكل wire/JSON، ABI v1 فقط وسطح syscalls/pointer-ABI ثابت، حتمية، بدون serde او متغيرات بيئية في المسارات الانتاجية.

</div>
