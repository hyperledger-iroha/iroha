---
lang: he
direction: rtl
source: docs/portal/docs/devportal/preview-invite-tracker.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# תצוגה מקדימה של Tracker des הזמנות

Ce Tracker רשם תצוגה מקדימה מעורפלת של מסמכים מעורפלים עם הבעלים DOCS-SORA ו-les relecteurs governance voient quelle cohorte est active, that aproveless les invitations and quels artefacts rested a traiter. Mettez-le a jour chaque fois que des invitations sont invoyees, revoquees ou reportees pour que la piste d'audit reste dans le depot.

## חוק המעורפל

| מעורפל | קוהורט | גיליון דה suivi | מאשר(ים) | סטטוט | Fenetre cible | הערות |
| --- | --- | --- | --- | --- | --- | --- |
| **W0 - מתחזקי ליבה** | מסמכי תחזוקה + SDK תקפים לבדיקת השטף | `DOCS-SORA-Preview-W0` (מעקב GitHub/אופס) | Lead Docs/DevRel + Portal TL | טרמין | Q2 2025 semaines 1-2 | שליחי הזמנות 2025-03-25, telemetrie restee verte, resume de sortie publie 2025-04-08. |
| **W1 - שותפים** | מפעילים SoraFS, אינטגרטורים Torii sous NDA | `DOCS-SORA-Preview-W1` | Lead Docs/DevRel + ניהול קשר | טרמין | Q2 2025 semaine 3 | הזמנות 2025-04-12 -> 2025-04-26 avec les huit partners מאשר; תפסו ראיות ב-[`preview-feedback/w1/log.md`](./preview-feedback/w1/log.md) וקורות חיים ב-[`preview-feedback/w1/summary.md`](./preview-feedback/w1/summary.md). |
| **W2 - Communaute** | רשימת המתנה תקשורת טריה (<=25 a la fois) | `DOCS-SORA-Preview-W2` | Lead Docs/DevRel + מנהל קהילה | טרמין | Q3 2025 semaine 1 (tentatif) | הזמנות 2025-06-15 -> 2025-06-29 avec telemetrie verte tout du long; ראיות + constats dans [`preview-feedback/w2/summary.md`](./preview-feedback/w2/summary.md). |
| **W3 - Cohortes בטא** | בטא פיננסים/צפיות + SDK של שותף + מערכת אקולוגית של advocate | `DOCS-SORA-Preview-W3` | Lead Docs/DevRel + ניהול קשר | טרמין | Q1 2026 semaine 8 | הזמנות 2026-02-18 -> 2026-02-28; קורות חיים + donnees portail generees דרך la vague `preview-20260218` (voir [`preview-feedback/w3/summary.md`](./preview-feedback/w3/summary.md)). |

> הערה: יש לך בעיה עם Tracker aux Tickets de demande תצוגה מקדימה ו-archivez-les dans le projet `docs-portal-preview` pour que les approbations restent decouvrables.

## Taches actives (W0)- Artefacts de preflight rafraichis (ביצוע GitHub Actions `docs-portal-preview` 2025-03-24, אימות מתאר באמצעות `scripts/preview_verify.sh` avec le tag `preview-2025-03-24`).
- תפיסות טלמטריה בסיסיות (`docs.preview.integrity`, תמונת מצב של לוחות המחוונים `TryItProxyErrors` ב-W0).
- Texte d'outreach fige avec [`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md) ותצוגה מקדימה של תג `preview-2025-03-24`.
- Demandes d'entree enregistrees pour les cinq premiers maintenanceers (כרטיסים `DOCS-SORA-Preview-REQ-01` ... `-05`).
- Cinq מציג בבכורה הזמנות שליחים 2025-03-25 10:00-10:20 UTC לפני ספטמבר 2025 יורו עוקבים de telemetrie verte; accus stockes dans `DOCS-SORA-Preview-W0`.
- Suivi telemetrie + שעות המשרד du host (צ'ק-אין quotidiens jusqu'au 2025-03-31; log des checkpoints ci-dessous).
- משוב לא מעורפל / בעיות שנאספו ותקשורות `docs-preview/w0` (לפי [W0 digest](./preview-feedback/w0/summary.md)).
- Resume de vague publie + confirmations de sortie (תאריך חבילה של מיון 2025-04-08; voir [W0 digest](./preview-feedback/w0/summary.md)).
- בטא מעורפל W3 suivie; עתידיים מעורפלים planifiees selon revue governance.

## קורות חיים מעורפלים לשותפים W1

- אישורים משפטיים וממשל. שותפים לתוספת חותמים 2025-04-05; אישורי חיובים dans `DOCS-SORA-Preview-W1`.
- Telemetrie + נסה זאת בימוי. כרטיס שינוי `OPS-TRYIT-147` ביצוע 2025-04-06 avec צילומי מצב Grafana de `docs.preview.integrity`, `TryItProxyErrors`, et `DocsPortal/GatewayRefusals` ארכיונים.
- חפץ הכנה + סכום בדיקה. חבילה `preview-2025-04-12` לאמת; מתאר יומנים/צ'ק-סיום/ מניות בדיקה ב-`artifacts/docs_preview/W1/preview-2025-04-12/`.
- הזמנות סגל + שליחות. Huit דורשת משותפים (`DOCS-SORA-Preview-REQ-P01...P08`) אישורים; שליחי הזמנות 2025-04-12 15:00-15:21 UTC avec accus par relecteur.
- משוב על מכשור. שעות המשרד quotidiennes + מחסומים טלמטריה נרשמים; voir [`preview-feedback/w1/summary.md`](./preview-feedback/w1/summary.md) pour le digest.
- סגל סגל / יומן מיון. [`preview-feedback/w1/log.md`](./preview-feedback/w1/log.md) רשום חותמות זמן לתחזוקה d'invitation/ack, ראיות טלמטריה, חידון יצוא וחפצי אמנות au 2025-04-26 pour permettre la relecture governance.

## יומן הזמנות - מנהלי הליבה של W0| תעודת זהות | תפקיד | כרטיס דה דרישה | שליח הזמנה (UTC) | השתתפות במיון (UTC) | סטטוט | הערות |
| --- | --- | --- | --- | --- | --- | --- |
| docs-core-01 | מתחזק פורטל | `DOCS-SORA-Preview-REQ-01` | 25-03-2025 10:05 | 2025-04-08 10:00 | אקטיב | A confirme la checksum; מיקוד ניווט/סרגל צד. |
| sdk-rust-01 | עופרת SDK חלודה | `DOCS-SORA-Preview-REQ-02` | 25-03-2025 10:08 | 2025-04-08 10:00 | אקטיב | Teste les recettes SDK + התחלה מהירה Norito. |
| sdk-js-01 | מתחזק JS SDK | `DOCS-SORA-Preview-REQ-03` | 25-03-2025 10:12 | 2025-04-08 10:00 | אקטיב | תקף למסוף נסה את זה + זורם ISO. |
| sorafs-ops-01 | SoraFS קשר מפעיל | `DOCS-SORA-Preview-REQ-04` | 25-03-2025 10:15 | 2025-04-08 10:00 | אקטיב | Audite les runbooks SoraFS + תזמור מסמכים. |
| observability-01 | צפיות TL | `DOCS-SORA-Preview-REQ-05` | 25-03-2025 10:18 | 2025-04-08 10:00 | אקטיב | Revoit les annexes telemetrie/תקריות; אחראי couverture Alertmanager. |

Toutes les invitations referencent le meme artefact `docs-portal-preview` (ביצוע 2025-03-24, תג `preview-2025-03-24`) et le log de verification capture dans `DOCS-SORA-Preview-W0`. Toute ajout/pause doit etre consigne dans le tableau ci-dessus et l'issue du tracker avant de passer a la vague suivante.

## יומן מחסומים - W0

| תאריך (UTC) | פעילות | הערות |
| --- | --- | --- |
| 26-03-2025 | Revue telemetrie baseline + שעות עבודה | `docs.preview.integrity` + `TryItProxyErrors` sont restes verts; שעות המשרד ont confirme la verification checksum terminee. |
| 27-03-2025 | תקציר משוב intermediaire publie | לכידת קורות חיים ב-[`preview-feedback/w0/summary.md`](./preview-feedback/w0/summary.md); deux בעיות nav mineures taguees `docs-preview/w0`, תקרית Aucun. |
| 31-03-2025 | בדוק telemetrie fin de semaine | Dernieres שעות המשרד לפני יציאה; relecteurs ont confirme les taches restantes, aucune alerte. |
| 2025-04-08 | קורות חיים + הזמנות פריטים | ביקורות שנקבעו מאושרים, גישה לשחזור זמני, קבצים לארכיונים ב-[`preview-feedback/w0/summary.md`](./preview-feedback/w0/summary.md#exit-summary-2025-04-08); tracker mis a jour avant W1. |

## יומן הזמנות - שותפי W1| תעודת זהות | תפקיד | כרטיס דה דרישה | שליח הזמנה (UTC) | השתתפות במיון (UTC) | סטטוט | הערות |
| --- | --- | --- | --- | --- | --- | --- |
| soraps-op-01 | מפעיל SoraFS (EU) | `DOCS-SORA-Preview-REQ-P01` | 2025-04-12 15:00 | 26/04/2025 15:00 | טרמין | משוב מבצעי מתזמר livre 2025-04-20; מיון ack 15:05 UTC. |
| soraps-op-02 | מפעיל SoraFS (JP) | `DOCS-SORA-Preview-REQ-P02` | 12-04-2025 15:03 | 26/04/2025 15:00 | טרמין | יומני השקת פרשנים ב-`docs-preview/w1`; ack 15:10 UTC. |
| soraps-op-03 | מפעיל SoraFS (ארה"ב) | `DOCS-SORA-Preview-REQ-P03` | 2025-04-12 15:06 | 26/04/2025 15:00 | טרמין | עורך רישום מחלוקת/רשימה שחורה; ack 15:12 UTC. |
| torii-int-01 | אינטגרטור Torii | `DOCS-SORA-Preview-REQ-P04` | 12-04-2025 15:09 | 26/04/2025 15:00 | טרמין | Walkthrough נסה את זה אישור אישור; ack 15:14 UTC. |
| torii-int-02 | אינטגרטור Torii | `DOCS-SORA-Preview-REQ-P05` | 12-04-2025 15:12 | 26/04/2025 15:00 | טרמין | מפרשים יומני RPC/OAuth; ack 15:16 UTC. |
| sdk-partner-01 | שותף SDK (Swift) | `DOCS-SORA-Preview-REQ-P06` | 12-04-2025 15:15 | 26/04/2025 15:00 | טרמין | מיזוג תצוגה מקדימה של משוב; ack 15:18 UTC. |
| sdk-partner-02 | שותף SDK (אנדרואיד) | `DOCS-SORA-Preview-REQ-P07` | 2025-04-12 15:18 | 26/04/2025 15:00 | טרמין | Revue telemetrie/redaction faite; ack 15:22 UTC. |
| gateway-ops-01 | מפעיל שער | `DOCS-SORA-Preview-REQ-P08` | 2025-04-12 15:21 | 26/04/2025 15:00 | טרמין | פרשנויות ריצות יומני שער DNS; ack 15:24 UTC. |

## יומן מחסומים - W1

| תאריך (UTC) | פעילות | הערות |
| --- | --- | --- |
| 2025-04-12 | הזמנות Envoi + חפצי אימות | דוא"ל של שותפי Huit עם תיאור/ארכיון `preview-2025-04-12`; accus stockes dans le tracker. |
| 2025-04-13 | Revue telemetrie baseline | `docs.preview.integrity`, `TryItProxyErrors`, et `DocsPortal/GatewayRefusals` ורטים; שעות המשרד ont confirme la verification checksum terminee. |
| 2025-04-18 | שעות המשרד מי מעורפלות | `docs.preview.integrity` reste vert; deux nits docs tags `docs-preview/w1` (ניסוח ניווט + צילום מסך נסה זאת). |
| 22-04-2025 | בדוק טלמטריה סופית | פרוקסי + לוחות מחוונים sains; גיליון aucune nouvelle, notee dans le tracker avant sortie. |
| 26-04-2025 | קורות חיים + הזמנות תסיסה | Tous les partners on confirme la review, revoquetes, הזמנות, archivee dans [`preview-feedback/w1/summary.md`](./preview-feedback/w1/summary.md#exit-summary-2025-04-26). |

## Recap Cohorte Beta W3

- שליחי הזמנות 2026-02-18 avec בדיקת אימות + accus le meme jour.
- משוב לאסוף sous `docs-preview/20260218` avec נושא ניהול `DOCS-SORA-Preview-20260218`; תקציר + קורות חיים דרך `npm run --prefix docs/portal preview:wave -- --wave preview-20260218`.
- Acces revoque 2026-02-28 apres le check telemetrie final; tracker + tables portail mises a jour pour marquer W3 termine.

## יומן הזמנות - קהילת W2| תעודת זהות | תפקיד | כרטיס דה דרישה | שליח הזמנה (UTC) | השתתפות במיון (UTC) | סטטוט | הערות |
| --- | --- | --- | --- | --- | --- | --- |
| comm-vol-01 | מבקר קהילה (SDK) | `DOCS-SORA-Preview-REQ-C01` | 15-06-2025 16:00 | 29/06/2025 16:00 | טרמין | אק 16:06 UTC; focus starts Quick SDK; מאושרת מיון 2025-06-29. |
| comm-vol-02 | מבקר קהילה (ממשל) | `REQ-C02` | 15/06/2025 16:03 | 29/06/2025 16:00 | טרמין | מנהלת רווי/SNS קטר; מאושרת מיון 2025-06-29. |
| comm-vol-03 | מבקר קהילה (Norito) | `REQ-C03` | 15/06/2025 16:06 | 29/06/2025 16:00 | טרמין | דרך משוב יומן Norito; ack 2025-06-29. |
| comm-vol-04 | מבקר קהילה (SoraFS) | `REQ-C04` | 15/06/2025 16:09 | 29/06/2025 16:00 | טרמין | Revue runbooks SoraFS terminee; ack 2025-06-29. |
| comm-vol-05 | מבקר קהילה (נגישות) | `REQ-C05` | 15/06/2025 16:12 | 29/06/2025 16:00 | טרמין | הערות נגיש/UX partagees; ack 2025-06-29. |
| comm-vol-06 | מבקר קהילה (לוקליזציה) | `REQ-C06` | 15/06/2025 16:15 | 29/06/2025 16:00 | טרמין | יומן לוקליזציה של משוב; ack 2025-06-29. |
| comm-vol-07 | מבקר קהילה (נייד) | `REQ-C07` | 15/06/2025 16:18 | 29/06/2025 16:00 | טרמין | בדיקות SDK למסמכים ניידים; ack 2025-06-29. |
| comm-vol-08 | מבקר קהילה (צפיות) | `REQ-C08` | 15/06/2025 16:21 | 29/06/2025 16:00 | טרמין | Revue נספח observabilite terminee; ack 2025-06-29. |

## יומן מחסומים - W2

| תאריך (UTC) | פעילות | הערות |
| --- | --- | --- |
| 2025-06-15 | הזמנות Envoi + חפצי אימות | מתאר/ארכיון `preview-2025-06-15` partage avec 8 relecteurs; accus stockes dans tracker. |
| 2025-06-16 | Revue telemetrie baseline | `docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals` ורטים; logs proxy נסה את זה montrent tokens communaute actifs. |
| 2025-06-18 | שעות עבודה ובעיות טריאז' | הצעות Deux (הסבר על ניסוח `docs-preview/w2 #1`, `#2` לוקליזציה של סרגל הצד) - מציג את ה-Docs. |
| 21-06-2025 | בדוק טלמטריה + תיקוני מסמכים | Docs a corrige `docs-preview/w2 #1/#2`; לוחות מחוונים verts, תקרית אוקון. |
| 24-06-2025 | שעות המשרד fin de semaine | רלקטורים לאשר את שאר המסעדות; התראה אקוניה. |
| 29-06-2025 | קורות חיים + הזמנות תסיסה | Acks רישום, גישה מחדש לתצוגה מקדימה, תמונות מצב + ארכיון חפצים (voir [`preview-feedback/w2/summary.md`](./preview-feedback/w2/summary.md#exit-summary-2025-06-29)). |
| 2025-04-15 | שעות עבודה ובעיות טריאז' | Deux הצעות תיעוד loges sous `docs-preview/w1`; תקרית אאוקון לא התראה. |

## Hooks de reporting- Chaque mercredi, mettre a jour le tableau ci-dessus et l'issue הזמינו פעילים עם נאום אדיבות (הזמנות שליחים, פעילים חוזרים, תקריות).
- Quand une vague se termine, ajouter le chemin du resume feedback (ex. `docs/portal/docs/devportal/preview-feedback/w0/summary.md`) et le lier depuis `status.md`.
- Si un critere de pause du [זרימת הזמנה מקדימה](./preview-invite-flow.md) est declenche, ajouter les etapes de remediation ici avant de reprendre les invitations.