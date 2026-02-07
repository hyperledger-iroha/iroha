---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/reports/sf6-security-review.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
titre : مراجعة أمان SF-6
résumé: Les manifestes sont liés à des manifestations.
---

# مراجعة أمان SF-6

**نافذة التقييم:** 2026-02-10 → 2026-02-18  
**قادة المراجعة :** Security Engineering Guild (`@sec-eng`), groupe de travail sur les outils (`@tooling-wg`)  
**النطاق:** SoraFS CLI/SDK (`sorafs_cli`, `sorafs_car`, `sorafs_manifest`), pour le streaming de preuves, pour les manifestes pour Torii, Téléchargez la version Sigstore/OIDC pour CI.  
**القطع الفنية:**  
- مصدر CLI والاختبارات (`crates/sorafs_car/src/bin/sorafs_cli.rs`)  
- معالجات manifeste/preuve pour Torii (`crates/iroha_torii/src/sorafs/api.rs`)  
- Libération de version (`ci/check_sorafs_cli_release.sh`, `scripts/release_sorafs_cli.sh`)  
- Harnais تساوي حتمي (`crates/sorafs_car/tests/sorafs_cli.rs`, [تقرير تكافؤ GA لمشغل SoraFS](./orchestrator-ga-parity.md))

## المنهجية

1. **Modélisation des menaces** Il s'agit d'une approche basée sur la CI et Torii.  
2. **مراجعة الشيفرة** ركزت على أسطح الاعتمادات (تبادل رموز OIDC, signature sans clé) et manifestes avec Norito, وcontre-pression pour le streaming de preuve.  
3. **اختبارات ديناميكية** Les manifestes d'appareils et les flux de preuve (relecture de jetons, falsification manifeste, flux inviolables) avec harnais de parité et fuzz lecteurs مخصصة.  
4. **فحص الإعدادات** Les paramètres par défaut sont `iroha_config`, pour la CLI et la version pour les versions ultérieures. وقابلة للتدقيق.  
5. **مقابلة عملية** أكدت تدفق ومسارات التصعيد وجمع preuves التدقيق مع propriétaires الإطلاق في Tooling WG.

## ملخص النتائج

| ID | الشدة | المجال | النتيجة | المعالجة |
|----|----------|------|---------|------------|
| SF6-SR-01 | عالي | Signature sans clé | كانت audience الافتراضية لرموز OIDC ضمنية في قوالب CI, ما يعرض لخطر replay بين المستأجرين. | Vous devez utiliser `--identity-token-audience` pour les hooks de CI ([processus de publication](../developer-releases.md), `docs/examples/sorafs_ci.md`). أصبح CI يفشل عند غياب public. |
| SF6-SR-02 | متوسط ​​| Preuve en streaming | Les tampons de contre-pression sont également des tampons de contre-pression. | يفرض `sorafs_cli proof stream` أحجام قنوات محدودة مع troncature حتمي، ويسجل Norito résumés ويجهض التدفق؛ J'utilise Torii pour mettre en miroir les fragments de réponse (`crates/iroha_torii/src/sorafs/api.rs`). |
| SF6-SR-03 | متوسط ​​| إرسال manifeste | La CLI manifeste les plans de fragments comme `--plan`. | Vous trouverez `sorafs_cli manifest submit` dans les résumés CAR et dans la résolution `--expect-plan-digest` pour les discordances et les mesures correctives. تغطي الاختبارات حالات النجاح/الفشل (`crates/sorafs_car/tests/sorafs_cli.rs`). |
| SF6-SR-04 | منخفض | Piste d'audit | كانت قائمة التحقق للإطلاق تفتقر إلى سجل موافقة موقع لمراجعة الأمان. | Il s'agit d'un [processus de publication](../developer-releases.md) qui utilise les hachages pour la signature de GA. |

Il s'agit d'un harnais de parité élevé/moyen. لا توجد مشكلات حرجة كامنة.

## التحقق من الضوابط- **نطاق الاعتمادات:** تفرض قوالب CI الآن audience وémetteur صريحين؛ La CLI et release helper sont disponibles pour `--identity-token-audience` et `--identity-token-provider`.  
- **إعادة التشغيل الحتمية :** تغطي الاختبارات المحدثة تدفقات إرسال manifestes الإيجابية والسلبية، وتضمن أن résumés incompatibles تبقى أخطاء غير حتمية وتُكتشف قبل لمس الشبكة.  
- **Contre-pression pour le streaming de preuve :** Torii pour la latence PoR/PoTR avec CLI et latence + Vous pouvez consulter les résumés des résumés.  
- **الرصد:** تلتقط عدادات proof streaming (`torii_sorafs_proof_stream_*`) et les résumés CLI sont inclus dans le fil d'Ariane et le fil d'Ariane تدقيق للمشغلين.  
- **التوثيق :** تشير أدلة المطورين ([index du développeur](../developer-index.md) et [référence CLI](../developer-cli.md)) إلى الأعلام الحساسة للأمان ومسارات التصعيد.

## إضافات إلى قائمة التحقق للإطلاق

**يجب** على مديري الإطلاق إرفاق الأدلة التالية عند ترقية مرشح GA :

1. Hash لآخر مذكرة مراجعة أمان (هذا المستند).  
2. رابط لتذكرة assainissement المتابعة (مثل `governance/tickets/SF6-SR-2026.md`).  
3. مخرجات `scripts/release_sorafs_cli.sh --manifest ... --bundle-out ... --signature-out ...` التي تظهر معاملات public/émetteur صريحة.  
4. Utilisez le faisceau de parité (`cargo test -p sorafs_car -- --nocapture sorafs_cli::proof_stream::bounded_channels`).  
5. Utilisez les compteurs Torii pour les compteurs et les compteurs.

J'ai des artefacts à signer pour GA.

**Hashes مرجعية للقطع الفنية (signature le 20/02/2026) :**

-`sf6_security_review.md` — `66001d0b53d8e7ed5951a07453121c075dea931ca44c11f1fcd1571ed827342a`

## متابعات معلقة

- **تحديث نموذج التهديد:** إعادة هذه المراجعة كل ربع أو قبل إضافات كبيرة لأعلام CLI.  
- **Fuzzing :** Le fuzzing est un streaming de preuve par `fuzz/proof_stream_transport`, pour les charges utiles : identité, gzip, deflate, zstd.  
- **تمرين الحوادث:** جدولة تمرين للمشغلين لمحاكاة اختراق token وrollback للـ manifest، مع ضمان أن التوثيق يعكس الإجراءات المطبقة.

## الاعتماد

- ممثل Guilde d'ingénierie de sécurité : @sec-eng (2026-02-20)  
- ممثل Groupe de travail sur l'outillage : @tooling-wg (2026-02-20)

احفظ الموافقات الموقعة بجانب bundle القطع الفنية للإطلاق.