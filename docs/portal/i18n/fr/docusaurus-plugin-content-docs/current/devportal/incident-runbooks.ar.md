---
lang: fr
direction: ltr
source: docs/portal/docs/devportal/incident-runbooks.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# كتيبات الحوادث وتمارين rollback

## الغرض

بند خارطة الطريق **DOCS-9** يتطلب كتيبات اجرائية وخطة تدريب حتى يتمكن مشغلو البوابة من
التعافي من فشل النشر دون تخمين. تغطي هذه الملاحظة ثلاثة حوادث عالية الاشارة—فشل النشر،
Vous pouvez utiliser la fonction de restauration—et vous pouvez utiliser la restauration de l'alias
والتحقق الاصطناعي ما زالا يعملان de bout en bout.

### مواد ذات صلة

- [`devportal/deploy-guide`](./deploy-guide) — Pour l'emballage et la signature et l'alias.
- [`devportal/observability`](./observability) — balises de version et sondes de sortie.
-`docs/source/sorafs_node_client_protocol.md`
  et [`sorafs/pin-registry-ops`](../sorafs/pin-registry-ops)
  — télémétrie السجل وحدود التصعيد.
- Aides `docs/portal/scripts/sorafs-pin-release.sh` et `npm run probe:*`
  المشار اليها عبر قوائم التحقق.

### القياس عن بعد والادوات المشتركة| Signal / Outil | الغرض |
| ------------- | ------- |
| `torii_sorafs_replication_sla_total` (réalisé/manqué/en attente) | Il s'agit d'une question de SLA. |
| `torii_sorafs_replication_backlog_total`, `torii_sorafs_replication_completion_latency_epochs` | Il s'agit d'un arriéré et d'un triage. |
| `torii_sorafs_gateway_refusals_total`, `torii_sorafs_manifest_submit_total{status="error"}` | Vous devez utiliser la passerelle pour déployer cette application. |
| `npm run probe:portal` / `npm run probe:tryit-proxy` | les sondes sont utilisées pour la porte et les versions ainsi que les rollbacks. |
| `npm run check:links` | بوابة الروابط المكسورة؛ Il s'agit d'atténuation. |
| `sorafs_cli manifest submit ... --alias-*` (pour `scripts/sorafs-pin-release.sh`) | آلية ترقية/اعادة alias. |
| Carte `Docs Portal Publishing` Grafana (`dashboards/grafana/docs_portal.json`) | تجمع refus/alias/TLS/réplication de télémétrie. L'application PagerDuty est également disponible en ligne. |

## Runbook - فشل نشر او artefact سيئ

### شروط الاطلاق

- فشل sondes للpreview/production (`npm run probe:portal -- --expect-release=...`).
- Fonctions Grafana et `torii_sorafs_gateway_refusals_total` et
  `torii_sorafs_manifest_submit_total{status="error"}` pour le déploiement.
- QA يدوي يلاحظ مسارات مكسورة او فشل proxy Essayez-le مباشرة بعد ترقية alias.

### احتواء فوري

1. **Application :** pour `DEPLOY_FREEZE=1` pour le pipeline vers CI (entrée pour le workflow dans GitHub)
   Et le travail de Jenkins est de travailler sur les artefacts.
2. **التقاط artefacts :** حمل `build/checksums.sha256`,
   `portal.manifest*.{json,to,bundle,sig}`, les sondes sont chargées de la build avant la restauration
   الى digère الدقيقة.
3. **اخطار اصحاب المصلحة :** stockage SRE et responsable de Docs/DevRel et de la société de stockage
   (Il s'agit de `docs.sora`).

### اجراء restauration1. تحديد manifeste الاخير المعروف انه جيد (LKG). يقوم workflow الانتاجي بتخزينه في
   `artifacts/devportal/<release>/sorafs/portal.manifest.to`.
2. Utilisez l'alias pour manifester comme assistant d'assistance :

```bash
cd docs/portal
./scripts/sorafs-pin-release.sh \
  --build-dir build \
  --artifact-dir artifacts/revert-$(date +%Y%m%d%H%M) \
  --sorafs-dir artifacts/revert-$(date +%Y%m%d%H%M)/sorafs \
  --pin-min-replicas 5 \
  --alias "docs-prod-revert" \
  --alias-namespace "${PIN_ALIAS_NAMESPACE}" \
  --alias-name "${PIN_ALIAS_NAME}" \
  --alias-proof "${PIN_ALIAS_PROOF_PATH}" \
  --torii-url "${TORII_URL}" \
  --submitted-epoch "$(date +%Y%m%d)" \
  --authority "${AUTHORITY}" \
  --private-key "${PRIVATE_KEY}" \
  --skip-submit

# swap in the LKG artefacts before submission
cp /secure/archive/lkg/portal.manifest.to artifacts/.../sorafs/portal.manifest.to
cp /secure/archive/lkg/portal.manifest.bundle.json artifacts/.../sorafs/

cargo run -p sorafs_orchestrator --bin sorafs_cli -- \
  manifest submit \
  --manifest artifacts/.../sorafs/portal.manifest.to \
  --chunk-plan artifacts/.../sorafs/portal.plan.json \
  --torii-url "${TORII_URL}" \
  --authority "${AUTHORITY}" \
  --private-key "${PRIVATE_KEY}" \
  --alias-namespace "${PIN_ALIAS_NAMESPACE}" \
  --alias-name "${PIN_ALIAS_NAME}" \
  --alias-proof "${PIN_ALIAS_PROOF_PATH}" \
  --metadata rollback_from="${FAILED_RELEASE}" \
  --summary-out artifacts/.../sorafs/rollback.submit.json
```

3. Assurez-vous de restaurer la version en arrière du manifeste LKG et du manifeste.

### التحقق

1. `npm run probe:portal -- --expect-release=${LKG_TAG}`.
2. `npm run check:links`.
3. `sorafs_cli manifest verify-signature ...` et `sorafs_cli proof verify ...`
   (انظر دليل النشر) لتاكيد ان manifeste المعاد ترقيته ما زال يطابق CAR المؤرشف.
4. `npm run probe:tryit-proxy` est un proxy Try-It pour la mise en scène.

### ما بعد الحادثة

1. Mettre en place un pipeline en fonction de votre projet.
2. Voir "Leçons apprises" dans [`devportal/deploy-guide`](./deploy-guide)
   بملاحظات جديدة عند الحاجة.
3. فتح défauts لاختبارات فشلت (sonde, vérificateur de lien, etc.).

## Runbook - تدهور النسخ

### شروط الاطلاق

- Texte : `sum(torii_sorafs_replication_sla_total{outcome="met"}) /
  clamp_min(sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}), 1) <
  0.95` pour 10 dollars.
- `torii_sorafs_replication_backlog_total > 10` pour 10 dollars (انظر
  `pin-registry-ops.md`).
- الحوكمة تبلغ عن بطء توفر alias بعد release.

### Triage

1. Tableaux de bord [`sorafs/pin-registry-ops`](../sorafs/pin-registry-ops) pour votre recherche
   backlog محصورا في فئة تخزين او اسطول fournisseurs.
2. Mettez en place le Torii pour le `sorafs_registry::submit_manifest` pour le mettre en place.
   soumissions تفشل.
3. Utilisez le `sorafs_cli manifest status --manifest ...` (fournisseur de services).

### Atténuation1. اعادة اصدار manifeste بعدد نسخ اعلى (`--pin-min-replicas 7`) عبر
   `scripts/sorafs-pin-release.sh` est un planificateur pour les fournisseurs.
   سجل digest الجديد في سجل الحادثة.
2. Ajouter le backlog au fournisseur et au planificateur de réplication
   (`pin-registry-ops.md`) Le manifeste est un fournisseur d'alias.
3. عندما تكون حداثة alias اهم من parity النسخ، اعد ربط alias الى manifest دافئ
   staged (`docs-preview`) est un manifeste pour le backlog du SRE.

### التعافي والاغلاق

1. راقب `torii_sorafs_replication_sla_total{outcome="missed"}` لضمان استقرار العد.
2. Le module `sorafs_cli manifest status` est destiné à la réplique de la réplique.
3. Analyser et analyser post-mortem l'arriéré en matière de gestion des arriérés
   (fournisseurs de services, chunker, etc.).

## Runbook - انقطاع التحليلات او القياس عن بعد

### شروط الاطلاق

- `npm run probe:portal` ينجح لكن tableaux de bord تتوقف عن ابتلاع احداث
  `AnalyticsTracker` est disponible pour 15 jours.
- Examen de la confidentialité ترصد زيادة غير متوقعة في الاحداث المسقطة.
- `npm run probe:tryit-proxy` correspond à `/probe/analytics`.

### الاستجابة1. Utilisez les entrées et les paramètres : `DOCS_ANALYTICS_ENDPOINT` et
   `DOCS_ANALYTICS_SAMPLE_RATE` est un artefact en cours (`build/release.json`).
2. Utiliser `npm run probe:portal` pour `DOCS_ANALYTICS_ENDPOINT`.
   collecteur pour la mise en scène et le traqueur pour les charges utiles.
3. Les collectionneurs utilisent le `DOCS_ANALYTICS_ENDPOINT=""` et la reconstruction
   ليقوم tracker بعمل court-circuit؛ سجل نافذة الانقطاع في timeline الحادثة.
4. Utiliser `scripts/check-links.mjs` pour utiliser l'empreinte digitale `checksums.sha256`
   (انقطاعات التحليلات يجب *الا* تمنع التحقق من plan du site).
5. Utilisez le collecteur pour `npm run test:widgets` pour les tests unitaires et l'assistant analytique.
   قبل اعادة النشر.

### ما بعد الحادثة

1. Utiliser [`devportal/observability`](./observability) pour le collecteur et
   متطلبات échantillonnage.
2. اصدر اخطار حوكمة اذا تم فقدان او تنقيح بيانات التحليلات خارج السياسة.

## تمارين المرونة الربع سنوية

شغل كلا التمرينين خلال **اول ثلاثاء من كل ربع** (Jan/Apr/Jul/Oct)
او مباشرة بعد اي تغيير كبير في البنية التحتية. خزّن artefacts تحت
`artifacts/devportal/drills/<YYYYMMDD>/`.| التمرين | الخطوات | الدليل |
| ----- | ----- | -------- |
| تمرين rollback للalias | 1. اعادة تشغيل rollback الخاص بـ "Échec du déploiement" en utilisant le manifeste انتاجي.2. اعادة الربط الى الانتاج بعد نجاح sondes.3. Utilisez les sondes `portal.manifest.submit.summary.json` pour les utiliser. | `rollback.submit.json`, sondes de détection et balise de libération. |
| تدقيق التحقق الاصطناعي | 1. Utilisez `npm run probe:portal` et `npm run probe:tryit-proxy` pour la mise en scène.2. Utilisez `npm run check:links` et `build/link-report.json`.3. Ajouter des captures d'écran/exportations avec les sondes Grafana. | سجلات sondes + `link-report.json` تشير الى empreinte digitale للmanifest. |

صعّد التمارين الفائتة الى مدير Docs/DevRel ومراجعة حوكمة SRE, لان خارطة الطريق تتطلب
Vous pouvez également restaurer les alias et les sondes en arrière-plan.

## تنسيق PagerDuty et de garde- خدمة PagerDuty **Docs Portal Publishing** تملك التنبيهات المولدة من
  `dashboards/grafana/docs_portal.json`. Titre `DocsPortal/GatewayRefusals`,
  `DocsPortal/AliasCache` et `DocsPortal/TLSExpiry` pour la pagination vers Docs/DevRel
  الرئيسي مع Storage SRE كاحتياطي.
- عند النداء، ارفق `DOCS_RELEASE_TAG`, وارفق captures d'écran للوحات Grafana المتاثرة،
  Il s'agit d'une sonde/vérification de lien pour une atténuation.
- Pour l'atténuation (restauration et redéploiement), comme `npm run probe:portal`,
  `npm run check:links`, et instantanés à partir de Grafana pour les instantanés
  ضمن العتبات. ارفق جميع الادلة بحادثة PagerDuty قبل اغلاقها.
- اذا اطلق تنبيهان في نفس الوقت (مثلا TLS expiration et backlog) et تعامل مع refus et اولا
  (Publication en cours)Il s'agit d'une restauration en arrière ou d'un pont TLS/backlog avec Storage SRE.