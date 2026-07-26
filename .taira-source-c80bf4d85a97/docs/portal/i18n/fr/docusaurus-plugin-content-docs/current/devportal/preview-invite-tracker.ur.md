---
lang: fr
direction: ltr
source: docs/portal/docs/devportal/preview-invite-tracker.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# پریویو دعوت ٹریکر

یہ ٹریکر docs پورٹل کی ہر پریویو ویو ریکارڈ کرتا ہے تاکہ DOCS-SORA کے مالکان اور examinateurs de gouvernance Il y a une cohorte d'artisans et des artefacts. چاہتے ہیں۔ Vous avez besoin d'une piste d'audit pour vérifier la piste d'audit ریپوزٹری کے اندر رہے۔

## ویو اسٹیٹس| et | cohorte | ٹریکر ایشو | approbateur(s) | اسٹیٹس | ہدف ونڈو | نوٹس |
| --- | --- | --- | --- | --- | --- | --- |
| **W0 - Mainteneurs principaux** | Docs + responsables du SDK avec somme de contrôle et validation | `DOCS-SORA-Preview-W0` (suivi GitHub/ops) | Responsable Docs/DevRel + Portal TL | مکمل | T2 2025 1-2 | دعوتیں 2025-03-25 کو بھیجی گئیں، ٹیلی میٹری سبز رہی، résumé de sortie 2025-04-08 کو شائع ہوا۔ |
| **W1 - Partenaires** | SoraFS intégrateurs Torii et NDA | `DOCS-SORA-Preview-W1` | Responsable Docs/DevRel + liaison gouvernance | مکمل | T2 2025 partie 3 | دعوتیں 2025-04-12 -> 2025-04-26, تمام آٹھ پارٹنرز کی تصدیق؛ preuve [`preview-feedback/w1/log.md`](./preview-feedback/w1/log.md) میں اور exit digest [`preview-feedback/w1/summary.md`](./preview-feedback/w1/summary.md) میں۔ |
| **W2 - Communauté** | Liste d'attente de la communauté منتخب ( 2025-06-29، پوری مدت میں ٹیلی میٹری سبز؛ preuves + conclusions [`preview-feedback/w2/summary.md`](./preview-feedback/w2/summary.md) میں۔ |
| **W3 - Cohortes bêta** | finance/observabilité bêta + partenaire SDK + défenseur de l'écosystème | `DOCS-SORA-Preview-W3` | Responsable Docs/DevRel + liaison gouvernance | مکمل | T1 2026 8 | Du 2026-02-18 -> 2026-02-28؛ résumé + données du portail `preview-20260218` ویو سے تیار (دیکھیں [`preview-feedback/w3/summary.md`](./preview-feedback/w3/summary.md)). |> نوٹ: ہر ٹریکر ایشو کو متعلقہ preview request tickets سے لنک کریں اور انہیں `docs-portal-preview` پروجیکٹ میں archive کریں تاکہ approbations قابلِ دریافت رہیں۔

## فعال کام (W0)

- artefacts de contrôle en amont (GitHub Actions `docs-portal-preview` publié le 2025-03-24, descripteur `scripts/preview_verify.sh` pour vérifier `preview-2025-03-24`)
- ٹیلی میٹری lignes de base محفوظ (`docs.preview.integrity`, `TryItProxyErrors` instantané du tableau de bord, problème W0 میں محفوظ)۔
- sensibilisation avec [`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md) et verrouillage de la balise d'aperçu `preview-2025-03-24`۔
- پہلے پانچ mainteneurs کے لئے demandes d'admission لاگ (billets `DOCS-SORA-Preview-REQ-01` ... `-05`).
- پہلی پانچ دعوتیں 2025-03-25 10:00-10:20 UTC کو بھیجیں، سات دن مسلسل سبز ٹیلی میٹری کے بعد؛ remerciements `DOCS-SORA-Preview-W0` میں محفوظ۔
- ٹیلی میٹری مانیٹرنگ + heures du bureau hôte (2025-03-31 تک روزانہ check-ins ; journal des points de contrôle نیچے)۔
- commentaires / problèmes à mi-parcours اکٹھے کر کے `docs-preview/w0` ٹیگ (دیکھیں [W0 digest](./preview-feedback/w0/summary.md))۔
- ویو résumé شائع + confirmations de sortie d'invitation (lot de sortie تاریخ 2025-04-08; دیکھیں [W0 digest](./preview-feedback/w0/summary.md))۔
- Vague bêta W3 ٹریک ; مستقبل کی ویوز examen de la gouvernance کے بعد شیڈول۔

## Partenaires W1 et خلاصہ- قانونی اور approbations de gouvernance۔ Addendum partenaire 2025-04-05 ici agréments `DOCS-SORA-Preview-W1` میں اپ لوڈ۔
- Télémétrie + Essayez-le par mise en scène۔ Changer de ticket `OPS-TRYIT-147` 2025-04-06 کو اجرا؛ `docs.preview.integrity`, `TryItProxyErrors`, et `DocsPortal/GatewayRefusals` et Grafana instantanés
- Artefact + préparation de la somme de contrôle Vérification du bundle `preview-2025-04-12`؛ descripteur/somme de contrôle/journaux de sonde `artifacts/docs_preview/W1/preview-2025-04-12/` میں محفوظ۔
- Liste d'invitations + expédition۔ آٹھ demandes de partenaires (`DOCS-SORA-Preview-REQ-P01...P08`) منظور؛ Aujourd'hui 2025-04-12 15:00-15:21 UTC Je vous réponds par le critique et vous recevez un accusé de réception.
- Instrumentation de rétroaction۔ Horaires de bureau + points de contrôle télémétriques résumé کے لئے [`preview-feedback/w1/summary.md`](./preview-feedback/w1/summary.md) دیکھیں۔
- Liste finale/journal de sortie۔ [`preview-feedback/w1/log.md`](./preview-feedback/w1/log.md) Pour les horodatages d'invitation/ack, les preuves de télémétrie, les exportations de quiz, et les pointeurs d'artefacts 2025-04-26. سکے۔

## Journal d'invitation - Mainteneurs principaux de W0| ID du réviseur | Rôle | Demander un billet | Invitation envoyée (UTC) | Sortie prévue (UTC) | Statut | Remarques |
| --- | --- | --- | --- | --- | --- | --- |
| docs-core-01 | Responsable du portail | `DOCS-SORA-Preview-REQ-01` | 2025-03-25 10:05 | 2025-04-08 10:00 | Actif | vérification de la somme de contrôle nav/sidebar review پر توجہ۔ |
| sdk-rouille-01 | Responsable du SDK Rust | `DOCS-SORA-Preview-REQ-02` | 2025-03-25 10:08 | 2025-04-08 10:00 | Actif | Recettes SDK + démarrages rapides Norito ici |
| sdk-js-01 | Responsable du SDK JS | `DOCS-SORA-Preview-REQ-03` | 2025-03-25 10:12 | 2025-04-08 10:00 | Actif | Essayez-le console + flux ISO |
| sorafs-ops-01 | SoraFS liaison opérateur | `DOCS-SORA-Preview-REQ-04` | 2025-03-25 10:15 | 2025-04-08 10:00 | Actif | Runbooks SoraFS + documents d'orchestration |
| observabilité-01 | Observabilité TL | `DOCS-SORA-Preview-REQ-05` | 2025-03-25 10:18 | 2025-04-08 10:00 | Actif | annexes télémétrie/incident Couverture d'Alertmanager en anglais |

Il s'agit de l'artefact `docs-portal-preview` (exécuté le 24/03/2025, balise `preview-2025-03-24`) et de la transcription de vérification `DOCS-SORA-Preview-W0`. ریفرنس کرتی ہیں۔ کسی بھی اضافے/وقفے کو اگلی ویو پر جانے سے پہلے اوپر والی ٹیبل اور tracker issue دونوں میں لاگ کریں۔

## Journal des points de contrôle - W0| Date (UTC) | Activité | Remarques |
| --- | --- | --- |
| 2025-03-26 | Examen de base de la télémétrie + heures de bureau | `docs.preview.integrity` + `TryItProxyErrors` sont disponibles heures de bureau et vérification de la somme de contrôle |
| 2025-03-27 | Publication d'un résumé des commentaires à mi-parcours | Résumé [`preview-feedback/w0/summary.md`](./preview-feedback/w0/summary.md) میں محفوظ؛ Les problèmes de navigation `docs-preview/w0` sont liés à un incident de navigation |
| 2025-03-31 | Vérification ponctuelle de télémétrie de la dernière semaine | آخری heures de bureau؛ critiques نے باقی کام تصدیق کیا، کوئی alert نہیں۔ |
| 2025-04-08 | Résumé de sortie + fermetures d'invitations | avis مکمل، عارضی رسائی منسوخ، conclusions [`preview-feedback/w0/summary.md`](./preview-feedback/w0/summary.md#exit-summary-2025-04-08) W1 سے پہلے tracker اپ ڈیٹ۔ |

## Journal d'invitation - Partenaires W1| ID du réviseur | Rôle | Demander un billet | Invitation envoyée (UTC) | Sortie prévue (UTC) | Statut | Remarques |
| --- | --- | --- | --- | --- | --- | --- |
| sorafs-op-01 | Opérateur SoraFS (UE) | `DOCS-SORA-Preview-REQ-P01` | 2025-04-12 15:00 | 2025-04-26 15:00 | Terminé | Commentaires sur les opérations de l'orchestrateur 2025-04-20 ici accusé de réception à 15h05 UTC۔ |
| sorafs-op-02 | Opérateur SoraFS (JP) | `DOCS-SORA-Preview-REQ-P02` | 2025-04-12 15:03 | 2025-04-26 15:00 | Terminé | commentaires sur le déploiement `docs-preview/w1` میں لاگ؛ accusé de réception à 15h10 UTC۔ |
| sorafs-op-03 | Opérateur SoraFS (États-Unis) | `DOCS-SORA-Preview-REQ-P03` | 2025-04-12 15:06 | 2025-04-26 15:00 | Terminé | litige/modifications sur liste noire فائل؛ accusé de réception à 15:12 UTC۔ |
| torii-int-01 | Intégrateur Torii | `DOCS-SORA-Preview-REQ-P04` | 2025-04-12 15:09 | 2025-04-26 15:00 | Terminé | Essayez-le, procédure pas à pas d'authentification accusé de réception à 15:14 UTC۔ |
| torii-int-02 | Intégrateur Torii | `DOCS-SORA-Preview-REQ-P05` | 2025-04-12 15:12 | 2025-04-26 15:00 | Terminé | Commentaires RPC/OAuth ici accusé de réception à 15:16 UTC۔ |
| sdk-partenaire-01 | Partenaire SDK (Swift) | `DOCS-SORA-Preview-REQ-P06` | 2025-04-12 15:15 | 2025-04-26 15:00 | Terminé | aperçu de la fusion des commentaires sur l'intégrité؛ accusé de réception à 15:18 UTC۔ |
| sdk-partenaire-02 | Partenaire SDK (Android) | `DOCS-SORA-Preview-REQ-P07` | 2025-04-12 15:18 | 2025-04-26 15:00 | Terminé | revue de télémétrie/rédaction مکمل؛ accusé de réception à 15:22 UTC۔ || passerelle-ops-01 | Opérateur de passerelle | `DOCS-SORA-Preview-REQ-P08` | 2025-04-12 15:21 | 2025-04-26 15:00 | Terminé | commentaires sur le runbook DNS de la passerelle accusé de réception à 15:24 UTC۔ |

## Journal des points de contrôle - W1

| Date (UTC) | Activité | Remarques |
| --- | --- | --- |
| 2025-04-12 | Envoi d'invitations + vérification des artefacts | `preview-2025-04-12` descripteur/archive pour les partenaires partenaires suivi des remerciements میں محفوظ۔ |
| 2025-04-13 | Examen de base de la télémétrie | `docs.preview.integrity`, `TryItProxyErrors`, et `DocsPortal/GatewayRefusals` ici heures de bureau et vérification de la somme de contrôle |
| 2025-04-18 | Horaires de bureau en moyenne vague | `docs.preview.integrity` pour le client دو doc نٹس `docs-preview/w1` کے تحت لاگ (formulation de navigation + capture d'écran Essayez-le)۔ |
| 2025-04-22 | Vérification ponctuelle finale de télémétrie | proxy + tableaux de bord صحت مند؛ کوئی نئی issues نہیں، exit سے پہلے tracker میں نوٹ۔ |
| 2025-04-26 | Résumé de sortie + fermetures d'invitations | Les partenaires terminent l'examen et invitent à révoquer les preuves [`preview-feedback/w1/summary.md`](./preview-feedback/w1/summary.md#exit-summary-2025-04-26) میں آرکائیو۔ |

## Récapitulatif de la cohorte bêta W3

- 2026-02-18 کو invite گئیں، vérification de la somme de contrôle + accusés de réception اسی دن لاگ۔
- feedback `docs-preview/20260218` concernant le problème de gouvernance `DOCS-SORA-Preview-20260218`؛ résumé + résumé `npm run --prefix docs/portal preview:wave -- --wave preview-20260218` سے تیار۔
- 2026-02-28 کو contrôle télémétrique final کے بعد révocation d'accès؛ tracker + tables de portail pour W3

## Journal des invitations - Communauté W2| ID du réviseur | Rôle | Demander un billet | Invitation envoyée (UTC) | Sortie prévue (UTC) | Statut | Remarques |
| --- | --- | --- | --- | --- | --- | --- |
| comm-vol-01 | Réviseur communautaire (SDK) | `DOCS-SORA-Preview-REQ-C01` | 2025-06-15 16:00 | 2025-06-29 16:00 | Terminé | Accusé de réception à 16h06 UTC ; Démarrages rapides du SDK ici sortie 2025-06-29 کو کنفرم۔ |
| comm-vol-02 | Réviseur communautaire (Gouvernance) | `REQ-C02` | 2025-06-15 16:03 | 2025-06-29 16:00 | Terminé | examen de la gouvernance/SNS مکمل؛ sortie 2025-06-29 کو کنفرم۔ |
| comm-vol-03 | Réviseur communautaire (Norito) | `REQ-C03` | 2025-06-15 16:06 | 2025-06-29 16:00 | Terminé | Commentaires pas à pas sur la procédure Norito accusé de réception 2025-06-29۔ |
| comm-vol-04 | Réviseur communautaire (SoraFS) | `REQ-C04` | 2025-06-15 16:09 | 2025-06-29 16:00 | Terminé | Examen du runbook SoraFS Mise à jour accusé de réception 2025-06-29۔ |
| comm-vol-05 | Réviseur communautaire (Accessibilité) | `REQ-C05` | 2025-06-15 16:12 | 2025-06-29 16:00 | Terminé | Notes sur l'accessibilité/UX accusé de réception 2025-06-29۔ |
| comm-vol-06 | Réviseur communautaire (localisation) | `REQ-C06` | 2025-06-15 16:15 | 2025-06-29 16:00 | Terminé | Commentaires sur la localisation accusé de réception 2025-06-29۔ |
| comm-vol-07 | Réviseur communautaire (Mobile) | `REQ-C07` | 2025-06-15 16:18 | 2025-06-29 16:00 | Terminé | Vérifications de la documentation du SDK mobile accusé de réception 2025-06-29۔ || comm-vol-08 | Réviseur communautaire (Observabilité) | `REQ-C08` | 2025-06-15 16:21 | 2025-06-29 16:00 | Terminé | Examen de l'annexe sur l'observabilité accusé de réception 2025-06-29۔ |

## Journal des points de contrôle - W2

| Date (UTC) | Activité | Remarques |
| --- | --- | --- |
| 2025-06-15 | Envoi d'invitations + vérification des artefacts | `preview-2025-06-15` descripteur/archive 8 évaluateurs de la communauté کے ساتھ شیئر؛ suivi des remerciements میں محفوظ۔ |
| 2025-06-16 | Examen de base de la télémétrie | Tableaux de bord `docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals` Essayez-le, journaux proxy et jetons de communauté |
| 2025-06-18 | Horaires de bureau et tri des problèmes | Suggestions (formulation de l'info-bulle `docs-preview/w2 #1`, barre latérale de localisation `#2`) - Docs est routé |
| 2025-06-21 | Vérification de télémétrie + correctifs de documentation | Docs نے `docs-preview/w2 #1/#2` pour کیا؛ tableaux de bord concernant les incidents |
| 2025-06-24 | Horaires de bureau de la dernière semaine | les évaluateurs ont soumis des commentaires et soumis des commentaires کوئی alerte نہیں۔ |
| 2025-06-29 | Résumé de sortie + fermetures d'invitations | accusés de réception de la révocation de l'accès à l'aperçu, des instantanés de télémétrie + des artefacts (دیکھیں [`preview-feedback/w2/summary.md`] (./preview-feedback/w2/summary.md#exit-summary-2025-06-29)). |
| 2025-04-15 | Horaires de bureau et tri des problèmes | Voir les suggestions de documentation `docs-preview/w1` pour plus de détails. Incidents et alertes |

## Crochets de rapport- Il y a une table d'invitation et un problème d'invitation actif et une note d'état de la note d'état (invitations envoyées, réviseurs actifs, incidents).
- Il s'agit d'un chemin récapitulatif des commentaires et d'un chemin d'accès récapitulatif des commentaires (`docs/portal/docs/devportal/preview-feedback/w0/summary.md`) et `status.md`.
- اگر [prévisualiser le flux d'invitation](./preview-invite-flow.md) کے critères de pause ٹرگر ہوں تو invites دوبارہ شروع کرنے سے پہلے étapes de remédiation یہاں شامل کریں۔