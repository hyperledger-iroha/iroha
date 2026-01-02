---
id: kpi-dashboard
lang: fr
direction: ltr
source: docs/portal/docs/sns/kpi-dashboard.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---
# Tableau de bord KPI du Service de noms Sora

Le tableau de bord KPI donne aux stewards, guardians et regulateurs un seul endroit pour revoir les signaux d'adoption, d'erreur et de revenu avant la cadence mensuelle de l'annexe (SN-8a). La definition Grafana est livree dans le depot a `dashboards/grafana/sns_suffix_analytics.json` et le portail reproduit les memes panneaux via un iframe integre afin que l'experience corresponde a l'instance Grafana interne.

## Filtres et sources de donnees

- **Filtre de suffixe** - alimente les requetes `sns_registrar_status_total{suffix}` pour que `.sora`, `.nexus` et `.dao` soient inspectes independamment.
- **Filtre de release en masse** - borne les metriques `sns_bulk_release_payment_*` pour que la finance rapproche un manifest de registrar specifique.
- **Metriques** - tire des donnees de Torii (`sns_registrar_status_total`, `torii_request_duration_seconds`), de la CLI guardian (`guardian_freeze_active`), `sns_governance_activation_total`, et des metriques du helper de bulk-onboarding.

## Panneaux

1. **Inscriptions (24h)** - nombre d'evenements de registrar reussis pour le suffixe selectionne.
2. **Activations de gouvernance (30d)** - motions de charte/addendum enregistrees par la CLI.
3. **Debit du registrar** - taux par suffixe des actions reussies du registrar.
4. **Modes d'erreur du registrar** - taux sur 5 minutes des compteurs `sns_registrar_status_total` etiquetes erreur.
5. **Fenetre de gel guardian** - selecteurs en direct ou `guardian_freeze_active` signale un ticket de gel ouvert.
6. **Unites nettes de paiement par actif** - totaux rapportes par `sns_bulk_release_payment_net_units` par actif.
7. **Requetes bulk par suffixe** - volumes de manifest par id de suffixe.
8. **Unites nettes par requete** - calcul type ARPU derive des metriques de release.

## Checklist mensuelle de revue KPI

Le responsable finance anime une revue recurrente le premier mardi de chaque mois:

1. Ouvrir la page du portail **Analytics -> SNS KPI** (ou le tableau Grafana `sns-kpis`).
2. Capturer un export PDF/CSV des tableaux de debit du registrar et de revenu.
3. Comparer les suffixes pour les breaches SLA (pics de taux d'erreur, selecteurs geles >72 h, ecarts ARPU >10 %).
4. Consigner les resumes + actions dans l'annexe correspondante sous `docs/source/sns/regulatory/<suffix>/YYYY-MM.md`.
5. Joindre les artefacts exportes du tableau de bord au commit d'annexe et les lier dans l'agenda du conseil.

Si la revue detecte des breaches SLA, ouvrir un incident PagerDuty pour le proprietaire concerne (registrar duty manager, guardian on-call, ou steward program lead) et suivre la remediation dans le journal de l'annexe.
