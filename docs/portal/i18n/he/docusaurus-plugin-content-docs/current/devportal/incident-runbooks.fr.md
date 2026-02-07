---
lang: he
direction: rtl
source: docs/portal/docs/devportal/incident-runbooks.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# ספרי תקריות ותרגילים לחזרה

## Objectif

L'item de roadmap **DOCS-9** demande des Playbooks actionnables plus un plan de repetition pour que
les operators du portail puissent recuperer des echecs de livraison sans deviner. הערה Cette
couvre trois incidents a fort signal - שיעורי פריסה, השפלה דה שכפול וכו'
pannes d'analytics - et documente les drills trimestriels qui prouvent que le rollback d'alias
et la validation synthetique fonctionnent toujours de bout en bout.

### חיבור חומר

- [`devportal/deploy-guide`](./deploy-guide) - זרימת עבודה של אריזה, חתימה וקידום מכירות.
- [`devportal/observability`](./observability) - תגיות שחרור, ניתוחים ובדיקות מפנים ל-ci-dessous.
- `docs/source/sorafs_node_client_protocol.md`
  et [`sorafs/pin-registry-ops`](../sorafs/pin-registry-ops)
  - telemetrie du registre et seuils d'escalade.
- `docs/portal/scripts/sorafs-pin-release.sh` et helpers `npm run probe:*`
  הפניות לרשימות הבדיקה.

### Telemetrie and tooling partages

| אות / Outil | Objectif |
| ------------- | ------- |
| `torii_sorafs_replication_sla_total` (נפגש/הוחמצה/בהמתנה) | זיהוי בלוקים של שכפול ו-les הפרות של SLA. |
| `torii_sorafs_replication_backlog_total`, `torii_sorafs_replication_completion_latency_epochs` | Quantifie la profondeur du backlog et la latece de completion pour le triage. |
| `torii_sorafs_gateway_refusals_total`, `torii_sorafs_manifest_submit_total{status="error"}` | Montre les echecs cote gateway שמכיר את קצב הפריסה. |
| `npm run probe:portal` / `npm run probe:tryit-proxy` | בדיקות סינתטיות qui gate les releases and valides rollbacks. |
| `npm run check:links` | מארזי שער דה שעבודים; השתמש בהפחתת אפרה צ'אק. |
| `sorafs_cli manifest submit ... --alias-*` (עטוף par `scripts/sorafs-pin-release.sh`) | Mecanisme de promotion/reversion d'alias. |
| לוח `Docs Portal Publishing` Grafana (`dashboards/grafana/docs_portal.json`) | מסכים לסירובים/כינוי/טלמטרי/TLS/שכפול. התראות PagerDuty מופיעות לפני כן. |

## Runbook - שיעור פריסה או פגום חפץ

### תנאים דה דחיה

- תצוגה מקדימה/הד הפקה של Les probes (`npm run probe:portal -- --expect-release=...`).
- התראות Grafana sur `torii_sorafs_gateway_refusals_total` ou
  `torii_sorafs_manifest_submit_total{status="error"}` לפני ההשקה.
- QA manuel remarque des routes casses ou des pannes du proxy נסה זאת מייד לפני כן
  la promotion de l'alias.

### כליאה מיידית

1. **Ger les deploiements:** marquer le pipeline CI avec `DEPLOY_FREEZE=1` (קלט בזרימת עבודה
   GitHub) ou mettre en pause le job Jenkins pour qu'aucun artefact ne parte.
2. **Capturer les artefacts:** מטען טלפון `build/checksums.sha256`,
   `portal.manifest*.{json,to,bundle,sig}`, et la sortie des probes du build en echec afin que
   le rollback reference les digests exacts.
3. **Notifier les parties prenantes:** storage SRE, lead Docs/DevRel, et l'officier de garde
   ממשל לשפוך מודעות (surtout si `docs.sora` est impacte).

### נוהל החזרה לאחור

1. מזהה le manifest last-known-good (LKG). Le workflow de production les stocke sous
   `artifacts/devportal/<release>/sorafs/portal.manifest.to`.
2. Re-lier l'alias a ce manifest avec le helper de shipping:

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

3. רשם את קורות החיים של החזרה לאחור בכרטיס לאירוע עם
   manifest LKG et du manifest en echec.

### אימות1. `npm run probe:portal -- --expect-release=${LKG_TAG}`.
2. `npm run check:links`.
3. `sorafs_cli manifest verify-signature ...` et `sorafs_cli proof verify ...`
   (voir le guide de deploiement) pour confirmer que le manifest repromu correspond
   ארכיון toujours au CAR.
4. `npm run probe:tryit-proxy` להעריך את ה-Proxy Try-It בימוי est revenue.

### לאחר התקרית

1. Reactiveer le pipeline deploiement ייחודיות לאחר ניסיון המורכבת מהגורם הגזע.
2. Mettre a jour les מנות ראשונות "לקחים שנלמדו" ב-[`devportal/deploy-guide`](./deploy-guide)
   avec de nouveaux points, si besoin.
3. Ouvrir des defects pour la suite de tests en echec (בדיקה, בודק קישורים וכו').

## Runbook - Degradation de replication

### תנאים דה דחיה

- התראה: `sum(torii_sorafs_replication_sla_total{outcome="met"}) /
  clamp_min(sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}), 1) <
  תליון 0.95` 10 דקות.
- `torii_sorafs_replication_backlog_total > 10` תליון 10 דקות (voir
  `pin-registry-ops.md`).
- ממשל סימן une disponibilite d'alias lente apres un release.

### טריאז'

1. מפקח על לוחות המחוונים [`sorafs/pin-registry-ops`](../sorafs/pin-registry-ops) לשפוך
   מאשש את הצטברות est localize sur une classe de stockage או une flotte de providers.
2. Croiser les logs Torii pour les warnings `sorafs_registry::submit_manifest` afin de
   קובע את כל ההגשות echouent.
3. Echantillonner la sante des replicas דרך `sorafs_cli manifest status --manifest ...`
   (רשימת תוצאות לפי ספק).

### הקלה

1. Reemettre le manifest avec un nombre de replicas plus eleve (`--pin-min-replicas 7`) דרך
   `scripts/sorafs-pin-release.sh` afin que le scheduler etale la charge sur plus de providers.
   רושם את ה- Nouveau digest dans le log d'incident.
2. מצב הצטברות הוא ספק ייחודי, ביטול הפעילות הזמני באמצעות מתזמן
   דה רפליקציה (מסמך ב-`pin-registry-ops.md`) et somettre un nouveau manifest
   forcant les autres ספקי a rafraichir l'alias.
3. Quand la fraicheur de l'alias est פלוס ביקורת que la parite de replication, re-lier
   l'alias a un manifest chaud deja stage (`docs-preview`), puis publier un manifest de
   ללא שם: סיווווי אחד פויס que SRE א נטויה הצטברות.

### החלמה והלבשה

1. Surveiller `torii_sorafs_replication_sla_total{outcome="missed"}` pour s'assurer que le
   compteur se לייצב.
2. Capturer la sortie `sorafs_cli manifest status` comme preuve que chaque replica est de
   retour en conformite.
4
   etapes (ספקי קנה מידה, tuning du chunker וכו').

## Runbook - Panne d'analytics ou de telemetrie

### תנאים דה דחיה

- `npm run probe:portal` reussit mais les לוחות מחוונים cessent d'ingester des evenements
  `AnalyticsTracker` תליון >15 דקות.
- סקירת פרטיות לזהות une Hausse inattendue d'evenements abandonnes.
- `npm run probe:tryit-proxy` echoue sur les paths `/probe/analytics`.

### תגובה1. Verifier les inputs de build: `DOCS_ANALYTICS_ENDPOINT` et
   `DOCS_ANALYTICS_SAMPLE_RATE` dans l'artefact de release (`build/release.json`).
2. Re-executer `npm run probe:portal` avec `DOCS_ANALYTICS_ENDPOINT` pointant vers le
   אספן דה בימוי pour confirmer que le tracker emet הדרן des payloads.
3. כל אספנים נמצאים, מגדירים `DOCS_ANALYTICS_ENDPOINT=""` ובנייה מחדש
   que le tracker court-circuite; שליח la fenetre d'outage dans la ציר הזמן.
4. Valider que `scripts/check-links.mjs` המשך טביעת אצבע `checksums.sha256`
   (les pannes d'analytics ne doivent *pas* bloquer la validation du sitemap).
5. Une fois le collector retabli, lancer `npm run test:widgets` pour executer les
   בדיקות יחידה du helper analytics avant de republish.

### לאחר התקרית

1. Mettre a jour [`devportal/observability`](./observability) avec les nouvelles limites
   du collector ou les exigences de sampling.
2. Emettre une notice governance si des donnees analytics on ete perdues ou redactees
   הורס פוליטיקה.

## מקדחות trimestriels de resilience

Lancer les deux drills durant le **premier mardi de chaque trimestre** (ינואר/אבר/יולי/אוקטובר)
או מיידית לפני השינוי הגדול בתשתית. Stocker les artefacts sous
`artifacts/devportal/drills/<YYYYMMDD>/`.

| מקדחה | אטאפס | Preuve |
| ----- | ----- | -------- |
| Repetition du rollback d'alias | 1. שחזר את החזרה לאחור של "קצב פריסה" עם ייצור המניפסט בתוספת האחרונה.<br/>2. הצג מחדש הפקה יחידה לבדיקה.<br/>3. רשום `portal.manifest.submit.summary.json` et les logs de probes dans le dossier du drill. | `rollback.submit.json`, מיון בדיקות, ותג שחרור של חזרה. |
| Audit de validation synthetic | 1. Lancer `npm run probe:portal` ו-`npm run probe:tryit-proxy` קונטרה ייצור ושלב.<br/>2. Lancer `npm run check:links` et Archiver `build/link-report.json`.<br/>3. הצטרף לצילומי מסך/ייצוא של Panneaux Grafana מאשר את ההצלחה של בדיקות. | Logs de probes + `link-report.json` רפרנס לטביעת האצבע של המניפסט. |

Escalader les drills manques au manager Docs/DevRel et a la revue governance SRE, car le
מפת דרכים exige une preuve trimestrielle deterministe que le rollback d'alias et les probes
פורטל restent sains.

## תיאום PagerDuty וכוננות- Le service PagerDuty **Docs Portal Publishing** בעל התראות גנרי
  `dashboards/grafana/docs_portal.json`. Les regles `DocsPortal/GatewayRefusals`,
  `DocsPortal/AliasCache`, et `DocsPortal/TLSExpiry` page la primaire Docs/DevRel
  avec Storage SRE בשנייה.
- Quand בדף est, כולל le `DOCS_RELEASE_TAG`, joindre des screenshots des panneaux
  Grafana impactes et lier la sortie probe/link-check in les notes d'incident avant
  de commencer la mitigation.
- הפחתת אפרה (החזרה או פריסה מחדש), מבצע מחדש `npm run probe:portal`,
  `npm run check:links`, et capturer des snapshots Grafana montrant les metriques
  הכנסות dans les seuils. מצטרפים לראיה לאירוע PagerDuty
  רזולוציה אוונטית.
- Si deux alertes se declenchent en meme temps (לדוגמה. תפוגת TLS פלוס צבר), טרייר
  סירובים en premier (arreter la publication), executer la procedure de rollback, puis
  traiter TLS/backlog avec Storage SRE sur le bridge.