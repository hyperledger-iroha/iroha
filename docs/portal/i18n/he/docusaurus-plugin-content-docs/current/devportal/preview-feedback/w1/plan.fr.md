---
lang: he
direction: rtl
source: docs/portal/docs/devportal/preview-feedback/w1/plan.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: preview-feedback-w1-plan
כותרת: Plan de preflight partenaires W1
sidebar_label: תוכנית W1
תיאור: Taches, Responsables & Checklist de preuve pour la cohorte de preview partenaires.
---

| אלמנט | פרטים |
| --- | --- |
| מעורפל | W1 - Partenaires et integrates Torii |
| Fenetre cible | Q2 2025 semaine 3 |
| Tag d'artefact (planifie) | `preview-2025-04-12` |
| גשש נושאים | `DOCS-SORA-Preview-W1` |

## אובייקטים

1. קבלת אישורים משפטיים וממשל לתנאים מקדימים של שותפים.
2. מכין את ה-proxy נסה את זה ותצלומי מצב של טלמטריה מנצלים את חבילת ההזמנה.
3. Rafraichir l'artefact de preview contrôle par checksum et les resultats de probes.
4. Finaliser le roster des partenaires et les templates de demande avant l'envoi des הזמנות.

## מגזרת דקואז'

| תעודת זהות | טאצ'ה | אחראי | Echeance | סטטוט | הערות |
| --- | --- | --- | --- | --- | --- |
| W1-P1 | Obtenir l'approbation legale pour l'addendum des termes de preview | Docs/DevRel lead -> משפטי | 2025-04-05 | טרמין | כרטיס חוקי `DOCS-SORA-Preview-W1-Legal` valide le 2025-04-05; PDF attache au tracker. |
| W1-P2 | Capturer la fenetre de staging du proxy נסה את זה (2025-04-10) et valider la sante du proxy | Docs/DevRel + Ops | 2025-04-06 | טרמין | `npm run manage:tryit-proxy -- --stage preview-w1 --expires-in=21d --target https://tryit-preprod.sora` לבצע le 2025-04-06; תעתיק CLI + `.env.tryit-proxy.bak` ארכיונים. |
| W1-P3 | Construire l'artefact de preview (`preview-2025-04-12`), מבצע `scripts/preview_verify.sh` + `npm run probe:portal`, מתאר/סיכומי ביקורת של ארכיון | פורטל TL | 2025-04-08 | טרמין | Artefact + Logs de Verification Stockes sous `artifacts/docs_preview/W1/preview-2025-04-12/`; sortie de probe attachee au tracker. |
| W1-P4 | Revoir les formulaires d'intake partenaires (`DOCS-SORA-Preview-REQ-P01...P08`), אנשי קשר לאישור + NDAs | קשר ממשל | 2025-04-07 | טרמין | Les huit demandes approuvees (les deux dernieres le 2025-04-11); הסכמות lies dans le tracker. |
| W1-P5 | Rediger le texte d'invitation (בסיס על `docs/examples/docs_preview_invite_template.md`), definir `<preview_tag>` et `<request_ticket>` pour chaque partenaire | Docs/DevRel lead | 2025-04-08 | טרמין | Brouillon d'invitation envoye le 2025-04-12 15:00 UTC avec les liens d'artefact. |

## רשימת בדיקה מוקדמת

> Astuce: lancez `scripts/preview_wave_preflight.sh --tag preview-2025-04-12 --base-url https://preview.staging.sora --descriptor artifacts/preview-2025-04-12/descriptor.json --archive artifacts/preview-2025-04-12/docs-portal-preview.tar.zst --tryit-target https://tryit-proxy.staging.sora --output-json artifacts/preview-2025-04-12/preflight-summary.json` pour executer automatiquement les etapes 1-5 (בנייה, בדיקת סכום אימות, בדיקה של פורטל, בודק קישורים, et mise a jour du proxy נסה זאת). הסקריפט רשם את JSON והצטרף למעקב אחר.

1. `npm run build` (avec `DOCS_RELEASE_TAG=preview-2025-04-12`) יוצקים מחדש `build/checksums.sha256` et `build/release.json`.
2. `docs/portal/scripts/preview_verify.sh --build-dir docs/portal/build --descriptor artifacts/<tag>/descriptor.json --archive artifacts/<tag>/docs-portal-preview.tar.zst`.
3. `PORTAL_BASE_URL=https://preview.staging.sora DOCS_RELEASE_TAG=preview-2025-04-12 npm run probe:portal -- --expect-release=preview-2025-04-12`.
4. `DOCS_RELEASE_TAG=preview-2025-04-12 npm run check:links` et Archiver `build/link-report.json` a cote du descriptor.
5. `npm run manage:tryit-proxy -- update --target https://tryit-proxy.staging.sora` (ou fournir la cible appropriee via `--tryit-target`); committer le `.env.tryit-proxy` mis a jour et conserver la `.bak` pour rollback.
6. מטה את גיליון W1 עם רישומי כימין (בדיקת סכום של תיאור, בדיקה מיון, שינוי ב-proxy נסה את זה ותמונות Snapshot Grafana).

## רשימת רשימת קודים- [x] חתם אישור משפטי (PDF ou lien du ticket) attachee a `DOCS-SORA-Preview-W1`.
- [x] צילומי מסך Grafana pour `docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals`.
- [x] Descriptor et log de checksum `preview-2025-04-12` stocks sous `artifacts/docs_preview/W1/`.
- [x] Tableau de roster d'invitations avec `invite_sent_at` renseignes (voir le log W1 du tracker).
- [x] Artefacts de feedback repris dans [`preview-feedback/w1/log.md`](./log.md) avec une ligne parenaire (ביום 2025-04-26 עם סגל/טלמטריה/בעיות).

Mettre a jour ce plan a mesure de l'avancement; le tracker s'y refere pour garder מפת הדרכים ניתנת לביקורת.

## Flux de feedback

1. יוצקים צ'אקה מבקר, dupliquer le template dans
   [`docs/examples/docs_preview_feedback_form.md`](../../../../../examples/docs_preview_feedback_form.md),
   remplir les metadonnees et stocker la copie complete sous
   `artifacts/docs_preview/W1/preview-2025-04-12/feedback/<partner-id>/`.
2. הזמנות לקורות חיים, מחסומים טלמטריים ובעיות אוברטיות
   [`preview-feedback/w1/log.md`](./log.md) pour que les reviewers governance puissent rejouer la vague
   sans quitter le depot.
3. Quand les exports de know-check ou de sondages arrivent, les joindre dans le chemin d'artefact note dans le log
   et lier l'issue du tracker.