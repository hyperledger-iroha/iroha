---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/provider-advert-rollout.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
title: "خطة طرح وتوافق adverts لمزودي SoraFS"
---

> مقتبس من [`docs/source/sorafs/provider_advert_rollout.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_rollout.md).

# خطة طرح وتوافق annonces pour SoraFS

تنسق هذه الخطة الانتقال من adverts المسموحة لمزودي التخزين إلى سطح `ProviderAdvertV1`
Il s'agit de morceaux de morceaux. وهي تركز على
ثلاثة مخرجات رئيسية:

- **دليل المشغل.** خطوات يجب على مزودي التخزين إكمالها قبل تفعيل كل porte.
- **تغطية التليمترية.** لوحات معلومات وتنبيهات تستخدمها Observabilité et Ops
  للتأكد من أن الشبكة تقبل annonces المتوافقة فقط.
- **الجدول الزمني للتوافق.** تواريخ واضحة لرفض enveloppes القديمة حتى تتمكن
  Le SDK et les outils sont pour vous.

يتماشى الطرح مع معالم SF-2b/2c في
[خارطة طريق هجرة SoraFS](./migration-roadmap) ويفترض أن سياسة القبول في
[Politique d'admission du fournisseur](./provider-admission-policy) مطبقة بالفعل.

## الجدول الزمني للمراحل| المرحلة | النافذة (الهدف) | السلوك | إجراءات المشغل | تركيز الملاحظة |
|-------|-----------------|-----------|------------------|-------------------|
| **R0 - الملاحظة الأساسية** | Lire **2025-03-31** | Le Torii est destiné aux publicités pour les charges utiles et aux charges utiles `ProviderAdvertV1`. Vous pouvez également utiliser les publicités `chunk_range_fetch` et `profile_aliases` pour l'ingestion. | - Il y a des annonces pour le pipeline et l'annonce du fournisseur (ProviderAdvertV1 + enveloppe de gouvernance) avec `profile_id=sorafs.sf1@1.0.0` et `profile_aliases` et `signature_strict=true`. <br />- تشغيل اختبارات `sorafs_fetch` محليا؛ يجب triage تحذيرات capacités غير المعروفة. | نشر لوحات Grafana المؤقتة (انظر أدناه) وتحديد عتبات التنبيه مع إبقائها في وضع التحذير فقط. |
| **R1 - بوابة التحذير** | **2025-04-01 → 2025-05-15** | Torii pour les annonces publicitaires pour `torii_sorafs_admission_total{result="warn"}` pour la charge utile `chunk_range_fetch` et ses capacités معروفة دون `allow_unknown_capabilities=true`. La CLI d'outillage est également disponible pour gérer la poignée. | - Annonces publicitaires pour la mise en scène et la production pour les charges utiles `CapabilityType::ChunkRangeFetch` et les tests de GRAISSE pour `allow_unknown_capabilities=true`. <br />- توثيق الاستعلامات الجديدة للتليمترية في runbooks التشغيل. | ترقية tableaux de bord إلى دوران de garde؛ Le prix du produit est de 5% pour 15 dollars. |
| **R2 - Application** | **2025-05-16 → 2025-06-30** | يرفض Torii annonces pour les enveloppes et la capacité `chunk_range_fetch`. لم تعد gère la version `namespace-name` تُحلل. Les fonctionnalités disponibles sont celles de l'opt-in GREASE pour `reason="unknown_capability"`. | - Les enveloppes et les enveloppes sont `torii.sorafs.admission_envelopes_dir` et les annonces publicitaires. <br />- Les SDK gèrent les alias et les alias des SDK. | Alertes de téléavertisseur : `torii_sorafs_admission_total{result="reject"}` > 0 لمدة 5 دقائق يستدعي تدخل المشغل. تتبع نسبة القبول وهيستوغرامات أسباب القبول. |
| **R3 - إيقاف القديمة** | **اعتبارا من 2025-07-01** | Utilisez Discovery pour les publicités de `signature_strict=true` et `profile_aliases`. Le cache de découverte Torii est prévu pour la date limite avant la date limite. | - جدولة نافذة déclassement النهائية لمكدسات المزودين القديمة. <br />- التأكد من أن تشغيلات GREASE `--allow-unknown` تتم فقط خلال forets مضبوطة ويتم تسجيلها. <br />- تحديث playbooks للحوادث لاعتبار مخرجات تحذير `sorafs_fetch` مانعا قبل الإصدارات. | تشديد التنبيهات: أي نتيجة `warn` تنبه de garde. Les fonctionnalités de découverte JSON et de fonctionnalités supplémentaires sont disponibles. |

## قائمة تحقق المشغل1. **جرد adverts.** احصر كل annonce منشور وسجل:
   - مسار الـ enveloppe de gouvernance (`defaults/nexus/sorafs_admission/...` أو ما يعادله في الإنتاج).
   - `profile_id` et `profile_aliases` pour la publicité.
   - Capacités supplémentaires (تتوقع على الأقل `torii_gateway` et `chunk_range_fetch`).
   - Utilisez `allow_unknown_capabilities` (il s'agit d'un fournisseur de TLV).
2. **إعادة التوليد باستخدام outillage المزود.**
   - Utilisez la charge utile pour publier une annonce de fournisseur, ainsi que :
     -`profile_id=sorafs.sf1@1.0.0`
     - `capability=chunk_range_fetch` ou `max_span`
     - `allow_unknown_capabilities=<true|false>` pour les TLV et la GRAISSE
   - تحقق عبر `/v1/sorafs/providers` و`sorafs_fetch` ; يجب triage تحذيرات
     capacités غير المعروفة.
3. **التحقق من جاهزية multi-source.**
   - نفذ `sorafs_fetch` ou `--provider-advert=<path>`؛ يفشل CLI الآن عندما
     يغيب `chunk_range_fetch` ويطبع تحذيرات عند تجاهل غير المعروفة.
     JSON est également compatible avec les applications.
4. **تجهيز التجديدات.**
   - Enveloppes à envelopper depuis le `ProviderAdmissionRenewalV1` jusqu'au 30 janvier
     application de la passerelle (R2). يجب أن تحافظ التجديدات على الـ handle القياسي
     ومجموعة capacités؛ Il s'agit d'enjeux, de points de terminaison et de métadonnées.
5. **التواصل مع الفرق المعتمدة.**
   - Vous pouvez utiliser le SDK pour ajouter des publicités.
   - يعلن DevRel كل انتقال مرحلة؛ Il existe des tableaux de bord et des tableaux de bord.
6. **Tableaux de bord تثبيت والتنبيهات.**
   - استورد تصدير Grafana et **SoraFS / Déploiement du fournisseur** avec UID
     `sorafs-provider-admission`.
   - تأكد من أن قواعد التنبيه تشير إلى قناة `sorafs-advert-rollout` المشتركة
     Dans la mise en scène et la production.

## التليمترية ولوحات المعلومات

Lien vers l'article `iroha_telemetry` :

- `torii_sorafs_admission_total{result,reason}` — يعد القبول والرفض ونتائج التحذير.
  تشمل الأسباب `missing_envelope` et `unknown_capability` et `stale` et `policy_violation`.

Utiliser Grafana : [`docs/source/grafana_sorafs_admission.json`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/grafana_sorafs_admission.json).
قم باستيراد الملف إلى مستودع tableaux de bord المشترك (`observability/dashboards`) مع
Utilisez l'UID pour créer un lien vers l'interface utilisateur.

يُنشر اللوح تحت مجلد Grafana **SoraFS / Provider Rollout** avec UID ثابت
`sorafs-provider-admission`. قواعد التنبيه `sorafs-admission-warn` (avertissement) et
`sorafs-admission-reject` (critique) مُعدة مسبقا لاستخدام سياسة الإشعار
`sorafs-advert-rollout`؛ عدل جهة الاتصال إذا تغيّرت قائمة الوجهات بدلا من تحرير
JSON est disponible.

لوحات Grafana الموصى بها:

| اللوحة | الاستعلام | الملاحظات |
|-------|-------|-------|
| **Taux de résultats d'admission** | `sum by(result)(rate(torii_sorafs_admission_total[5m]))` | Il s'agit plutôt d'accepter, d'avertir ou de rejeter. Il s'agit d'un avertissement > 0,05 * total (avertissement) ou d'un rejet > 0 (critique). |
| **Taux d'avertissement** | `sum(rate(torii_sorafs_admission_total{result="warn"}[5m])) / sum(rate(torii_sorafs_admission_total[5m]))` | Vous pouvez utiliser un téléavertisseur (5 % à 15 jours). |
| **Motifs de refus** | `sum by(reason)(rate(torii_sorafs_admission_total{result="reject"}[5m]))` | تدعم triage في runbook؛ أرفق روابط لخطوات التخفيف. |
| **Rafraîchir la dette** | `sum(rate(torii_sorafs_admission_total{reason="stale"}[1h]))` | تشير إلى مزودين فاتتهم مهلة التجديد؛ Il s'agit du cache de découverte. |

Artefacts en CLI et tableaux de bord disponibles :- `sorafs_fetch --provider-metrics-out` et `failures` et `successes` et
  `disabled` pour moi. استوردها في tableaux de bord ad hoc لمراقبة essais à sec في
  orchestrateur قبل تبديل مزودي الإنتاج.
- Utilisez `chunk_retry_rate` et `provider_failure_rate` pour utiliser JSON.
  la limitation des charges utiles périmées est nécessaire pour les rendre obsolètes.

### تخطيط لوحة Grafana

Voir Observability لوحة مخصصة — **SoraFS Admission du fournisseur
Déploiement** (`sorafs-provider-admission`) — Par **SoraFS / Déploiement du fournisseur**
مع معرفات اللوحات القياسية التالية:

- Panel 1 — *Taux de résultat d'admission* (zone empilée, et "ops/min").
- Panneau 2 — *Taux d'avertissement* (série unique)، مع التعبير
  `sum(rate(torii_sorafs_admission_total{result="warn"}[5m])) /
   somme(taux(torii_sorafs_admission_total[5m]))`.
- Panel 3 — *Raisons de rejet* (séries chronologiques comme `reason`)
  `rate(...[5m])`.
- Panel 4 — *Actualiser la dette* (stat)
  Il s'agit du grand livre des migrations.

انسخ (أو أنشئ) JSON skeleton في مستودع لوحات البنية التحتية
`observability/dashboards/sorafs_provider_admission.json`, vous avez besoin d'un UID
البيانات؛ معرفات اللوحات وقواعد التنبيه يُرجع إليها runbooks أدناه، لذا تجنب
إعادة ترقيمها دون تحديث هذا المستند.

للتسهيل، يوفر المستودع تعريفا مرجعيا للوحة في
`docs/source/grafana_sorafs_admission.json`؛ انسخه إلى مجلد Grafana عند الحاجة
كنقطة انطلاق للاختبارات المحلية.

### قواعد تنبيه Prometheus

أضف مجموعة القواعد التالية إلى
`observability/prometheus/sorafs_admission.rules.yml` (أنشئ الملف إن كانت هذه
أول مجموعة قواعد SoraFS) et إعدادات Prometheus. Télécharger `<pagerduty>`
بعلامة التوجيه الفعلية لدوام المناوبة.

```yaml
groups:
  - name: torii_sorafs_admission
    rules:
      - alert: SorafsProviderAdvertWarnFlood
        expr: sum(rate(torii_sorafs_admission_total{result="warn"}[5m])) /
              sum(rate(torii_sorafs_admission_total[5m])) > 0.05
        for: 15m
        labels:
          severity: warning
          route: <pagerduty>
        annotations:
          summary: "SoraFS provider adverts generating warnings"
          description: |
            Warn outcomes exceeded 5% of all admissions for 15 minutes.
            Inspect panel 3 on the sorafs/provider-admission dashboard and
            coordinate advert rotation with the affected operator.
      - alert: SorafsProviderAdvertReject
        expr: increase(torii_sorafs_admission_total{result="reject"}[5m]) > 0
        for: 5m
        labels:
          severity: critical
          route: <pagerduty>
        annotations:
          summary: "SoraFS provider adverts rejected"
          description: |
            Provider adverts have been rejected for the last five minutes.
            Check panel 4 (rejection reasons) and rotate envelopes before
            the refresh deadline elapses.
```

`scripts/check_prometheus_rules.sh observability/prometheus/sorafs_admission.rules.yml`
قبل رفع التغييرات للتأكد من أن الصياغة تمر عبر `promtool check rules`.

## مصفوفة التوافق

| Annonce publicitaire | R0 | R1 | R2 | R3 |
|------------------------|----|----|----|----|
| `profile_id = sorafs.sf1@1.0.0`, `chunk_range_fetch` et alias pour `signature_strict=true` | ✅ | ✅ | ✅ | ✅ |
| Capacité `chunk_range_fetch` | ⚠️ Avertir (ingérer + télémétrie) | ⚠️ Avertir | ❌ Rejeter (`reason="missing_capability"`) | ❌ Rejeter |
| Capacité TLV pour `allow_unknown_capabilities=true` | ✅ | ⚠️ Avertir (`reason="unknown_capability"`) | ❌ Rejeter | ❌ Rejeter |
| `refresh_deadline` | ❌ Rejeter | ❌ Rejeter | ❌ Rejeter | ❌ Rejeter |
| `signature_strict=false` (appareils de diagnostic) | ✅ (للتطوير فقط) | ⚠️ Avertir | ⚠️ Avertir | ❌ Rejeter |

كل الأوقات بتوقيت UTC. تواريخ application معكوسة في migration ledger ولن تتغير
بدون تصويت المجلس؛ Il s'agit d'un grand livre pour les relations publiques.

> **ملاحظة تنفيذية:** يقدم R1 سلسلة `result="warn"` في
> `torii_sorafs_admission_total`. تتبع رقعة ingest في Torii التي تضيف هذه التسمية
> ضمن مهام تليمترية SF-2؛ وحتى ذلك الحين استخدم أخذ عينات من السجلات لمراقبة

## التواصل ومعالجة الحوادث- **رسالة حالة أسبوعية.** يرسل DevRel ملخصا موجزا لمقاييس القبول والتحذيرات
  والمواعيد القادمة.
- **استجابة للحوادث.** إذا انطلقت تنبيهات `reject`, يقوم on-call بما يلي:
  1. Publier une annonce de découverte sur Torii (`/v1/sorafs/providers`).
  2. Ajouter une publicité pour un pipeline en ligne
     `/v1/sorafs/providers` لإعادة إنتاج الخطأ.
  3. La publicité doit être actualisée pour actualiser la page.
- **Détails du schéma.** Pour les fonctionnalités du schéma et des capacités, pour R1/R2 comme pour
  يوافق عليها فريق déploiement؛ يجب جدولة تجارب GREASE ضمن نافذة الصيانة الأسبوعية
  وتسجيلها في registre des migrations.

## المراجع

- [Protocole nœud/client SoraFS] (https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/sorafs_node_client_protocol.md)
- [Politique d'admission des fournisseurs](./provider-admission-policy)
- [Feuille de route de migration](./migration-roadmap)
- [Extensions multi-sources d'annonce de fournisseur] (https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_multisource.md)