---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/reports/sf6-security-review.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: مراجعة أمان SF-6
resumo: النتائج وعناصر المتابعة من التقييم المستقل للتوقيع بدون مفاتيح, وبث الإثباتات, وخطوط إرسال الـ se manifesta.
---

# مراجعة أمان SF-6

**نافذة التقييم:** 10/02/2026 → 18/02/2026  
**قادة المراجعة:** Guilda de Engenharia de Segurança (`@sec-eng`), Grupo de Trabalho de Ferramentas (`@tooling-wg`)  
**Edição:** SoraFS CLI/SDK (`sorafs_cli`, `sorafs_car`, `sorafs_manifest`), e streaming de prova, manifestos de manifestos aqui Torii, é necessário liberar Sigstore/OIDC e liberar no CI.  
**القطع الفنية:**  
- Dispositivo CLI de instalação (`crates/sorafs_car/src/bin/sorafs_cli.rs`)  
- Manifesto/prova de teste em Torii (`crates/iroha_torii/src/sorafs/api.rs`)  
- Versão de lançamento (`ci/check_sorafs_cli_release.sh`, `scripts/release_sorafs_cli.sh`)  
- Arnês de segurança (`crates/sorafs_car/tests/sorafs_cli.rs`, [SoraFS, [SoraFS](./orchestrator-ga-parity.md))

## المنهجية

1. **modelagem de ameaças** قامت بمواءمة قدرات المهاجمين لمحطات عمل المطورين وأنظمة CI وعقد Torii.  
2. **مراجعة الشيفرة** ركزت على أسطح الاعتمادات (تبادل رموز OIDC, assinatura sem chave), تحقق manifestas من Norito, e contrapressão para streaming à prova.  
3. **اختبارات ديناميكية** أعادت تشغيل manifestos de fixture e ومحاكت أوضاع فشل (token replay, adulteração de manifesto, fluxos de prova مقطوعة) Verifique o chicote de paridade e as unidades fuzz.  
4. **فحص الإعدادات** تحقق من defaults em `iroha_config`, معالجة أعلام CLI, وسكربتات release لضمان تشغيلات حتمية وقابلة للتدقيق.  
5. **مقابلة عملية** أكدت تدفق remediação ومسارات التصعيد وجمع evidências التدقيق مع proprietários الإطلاق في Tooling WG.

## ملخص النتائج

| ID | الشدة | المجال | النتيجة | المعالجة |
|----|----------|------|---------|------------|
| SF6-SR-01 | sim | Assinatura sem chave | O público-alvo do OIDC é o mesmo do CI, mas o replay é reproduzido novamente. | Você pode usar o `--identity-token-audience` para usar hooks no CI ([processo de liberação](../developer-releases.md), `docs/examples/sorafs_ci.md`). أصبح CI يفشل عند غياب audiência. |
| SF6-SR-02 | Medição | Transmissão de prova | Os buffers de contrapressão podem ser usados ​​para evitar problemas. | يفرض `sorafs_cli proof stream` أحجام قنوات محدودة مع truncamento حتمي, ويسجل Norito resumos ويجهض التدفق؛ É necessário espelhar Torii para blocos de resposta (`crates/iroha_torii/src/sorafs/api.rs`). |
| SF6-SR-03 | Medição | Manifestos إرسال | Os manifestos CLI são os planos de bloco do `--plan`. | أصبح `sorafs_cli manifest submit` يعيد حساب ويقارن CAR digests ما لم يتم تقديم `--expect-plan-digest`, ويرفض incompatibilidades ويعرض تلميحات remediação. Verifique o valor da chave/rede (`crates/sorafs_car/tests/sorafs_cli.rs`). |
| SF6-SR-04 | منخفض | Trilha de auditoria | كانت قائمة التحقق للإطلاق تفتقر إلى سجل موافقة موقع لمراجعة الأمان. | تمت إضافة قسم في [processo de liberação](../developer-releases.md) يطلب إرفاق hashes لمذكرة المراجعة ورابط تذكرة sign-off قبل GA. |

Portanto, você deve usar um alto/médio para usar o arnês de paridade. Não há nada que você possa fazer.

## التحقق من الضوابط- **نطاق الاعتمادات:** تفرض قوالب CI الآن audiência e emissor صريحين؛ O CLI e o auxiliar de liberação são usados ​​​​para `--identity-token-audience` e `--identity-token-provider`.  
- **إعادة التشغيل الحتمية:** تغطي الاختبارات المحدثة تدفقات إرسال manifests الإيجابية والسلبية، وتضمن أن resumos incompatíveis تبقى أخطاء غير حتمية وتُكتشف قبل لمس الشبكة.  
- **Pressão de retorno em streaming de prova:** يقوم Torii ببث عناصر PoR/PoTR عبر قنوات محدودة, ويحتفظ CLI فقط بعينات latência مقطوعة + خمسة أمثلة فشل, مانعا نمو المشتركين غير المحدود مع الحفاظ على resumos حتمية.  
- **الرصد:** تلتقط عدادات streaming de prova (`torii_sorafs_proof_stream_*`) e resumos CLI أسباب الإجهاض, مما يوفر breadcrumbs تدقيق للمشغلين.  
- **التوثيق:** تشير أدلة المطورين ([índice do desenvolvedor](../developer-index.md), [referência CLI](../developer-cli.md)) إلى الأعلام الحساسة للأمان ومسارات التصعيد.

## إضافات إلى قائمة التحقق للإطلاق

**يجب** على مديري الإطلاق إرفاق الأدلة التالية عند ترقية مرشح GA:

1. Hash é um valor de hash (هذا المستند).  
2. Execute o procedimento de remediação (exemplo `governance/tickets/SF6-SR-2026.md`).  
3. Selecione `scripts/release_sorafs_cli.sh --manifest ... --bundle-out ... --signature-out ...` para definir o público-alvo/emissor.  
4. Instale o chicote de paridade (`cargo test -p sorafs_car -- --nocapture sorafs_cli::proof_stream::bounded_channels`).  
5. Verifique se o contador Torii está instalado para remover os contadores.

عدم جمع artefatos أعلاه يمنع sign-off لـ GA.

**Hashes مرجعية للقطع الفنية (assinatura em 20/02/2026):**

- `sf6_security_review.md` — `66001d0b53d8e7ed5951a07453121c075dea931ca44c11f1fcd1571ed827342a`

## متابعات معلقة

- **تحديث نموذج التهديد:** إعادة هذه المراجعة كل ربع, أو قبل إضافات كبيرة لأعلام CLI.  
- **تغطية fuzzing:** يتم fuzzing لتشفيرات نقل streaming de prova عبر `fuzz/proof_stream_transport`, بما يشمل cargas úteis: identidade, gzip, deflate, zstd.  
- **تمرين الحوادث:** جدولة تمرين للمشغلين لمحاكاة اختراق token e rollback para o manifesto, مع ضمان أن التوثيق يعكس الإجراءات المطبقة.

## الاعتماد

- Associação de Engenharia de Segurança: @sec-eng (2026-02-20)  
- Grupo de Trabalho de Ferramentas: @tooling-wg (2026-02-20)

احفظ الموافقات الموقعة بجانب bundle القطع الفنية للإطلاق.