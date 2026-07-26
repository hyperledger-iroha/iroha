---
lang: fr
direction: ltr
source: docs/source/compliance/android/eu/gdpr_dpia_summary.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8ef338a20104dc5d15094e28a1332a604b68bdcfef1ff82fea784d43fdbd10b5
source_last_modified: "2026-01-03T18:07:59.202230+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Résumé GDPR DPIA - Télémétrie du SDK Android (AND7)

| Champ | Valeur |
|-------|-------|
| Date d'évaluation | 2026-02-12 |
| Activité de traitement | Exportation de télémétrie du SDK Android vers des backends OTLP partagés |
| Contrôleurs / Processeurs | SORA Nexus Ops (contrôleur), opérateurs partenaires (contrôleurs conjoints), Hyperledger Iroha Contributeurs (processeur) |
| Documents de référence | `docs/source/sdk/android/telemetry_redaction.md`, `docs/source/android_support_playbook.md`, `docs/source/android_runbook.md` |

## 1. Description du traitement

- **Objectif :** Fournir la télémétrie opérationnelle nécessaire pour prendre en charge l'observabilité AND7 (latence, tentatives, santé de l'attestation) tout en reflétant le schéma du nœud Rust (§2 de `telemetry_redaction.md`).
- **Systèmes :** Instrumentation du SDK Android -> Exportateur OTLP -> Collecteur de télémétrie partagé géré par SRE (voir Support Playbook §8).
- ** Personnes concernées : ** personnel opérateur utilisant des applications basées sur le SDK Android ; points de terminaison Torii en aval (les chaînes d'autorité sont hachées selon la stratégie de télémétrie).

## 2. Inventaire des données et atténuations

| Chaîne | Champs | Risque PII | Atténuation | Rétention |
|---------|--------|----------|------------|---------------|
| Traces (`android.torii.http.request`, `android.torii.http.retry`) | Itinéraire, statut, latence | Faible (pas de données personnelles) | Autorité hachée avec Blake2b + sel rotatif ; aucun corps de charge utile exporté. | 7 à 30 jours (par document de télémétrie). |
| Événements (`android.keystore.attestation.result`) | Libellé d'alias, niveau de sécurité, résumé d'attestation | Moyen (données opérationnelles) | Alias ​​haché (`alias_label`), `actor_role_masked` enregistré pour les remplacements avec les jetons d'audit Norito. | 90 jours pour le succès, 365 jours pour les remplacements/échecs. |
| Métriques (`android.pending_queue.depth`, `android.telemetry.export.status`) | Nombre de files d'attente, statut d'exportateur | Faible | Comptes agrégés uniquement. | 90 jours. |
| Jauges de profil d'appareil (`android.telemetry.device_profile`) | SDK majeur, niveau matériel | Moyen | Bucketing (émulateur/consommateur/entreprise), pas de numéros OEM ou de série. | 30 jours. |
| Événements de contexte réseau (`android.telemetry.network_context`) | Type de réseau, drapeau d'itinérance | Moyen | Le nom du transporteur a été entièrement supprimé ; prend en charge les exigences de conformité pour éviter les données des abonnés. | 7 jours. |

## 3. Base légale et droits

- **Base légale :** Intérêt légitime (Art. 6(1)(f)) — assurer un fonctionnement fiable des clients du grand livre réglementé.
- **Test de nécessité :** Métriques limitées à la santé opérationnelle (pas de contenu utilisateur) ; L'autorité hachée garantit la parité avec les nœuds Rust grâce à un mappage réversible disponible uniquement pour le personnel d'assistance autorisé (via un workflow de remplacement).
- **Test d'équilibrage :** La télémétrie s'applique aux appareils contrôlés par l'opérateur, et non aux données de l'utilisateur final. Les remplacements nécessitent des artefacts Norito signés examinés par Support + Compliance (Support Playbook §3 + §9).
- **Droits des personnes concernées :** Les opérateurs contactent Support Engineering (playbook §2) pour demander l'exportation/suppression de la télémétrie. Les remplacements de rédaction et les journaux (document de télémétrie §Inventaire des signaux) permettent une exécution dans les 30 jours.

## 4. Évaluation des risques| Risque | Probabilité | Impact | Atténuation résiduelle |
|------|------------|--------|-------------------------|
| Ré-identification via des autorités hachées | Faible | Moyen | Rotation du sel enregistrée via `android.telemetry.redaction.salt_version` ; sels stockés dans un coffre-fort sécurisé ; les dérogations sont auditées trimestriellement. |
| Empreinte digitale de l'appareil via des compartiments de profil | Faible | Moyen | Seul le niveau + SDK majeur est exporté ; Support Playbook interdit les demandes d’escalade pour les données OEM/série. |
| Remplacer l'utilisation abusive des fuites de données personnelles | Très faible | Élevé | Les demandes de remplacement Norito enregistrées, expirent dans les 24 heures, nécessitent l'approbation SRE (`docs/source/android_runbook.md` §3). |
| Stockage de télémétrie en dehors de l'UE | Moyen | Moyen | Collecteurs déployés dans les régions UE + JP ; politique de rétention appliquée via la configuration du backend OTLP (documentée dans Support Playbook §8). |

Le risque résiduel est jugé acceptable compte tenu des contrôles ci-dessus et de la surveillance continue.

## 5. Actions et suivis

1. **Révision trimestrielle :** Validez les schémas de télémétrie, les rotations de sel et les journaux de remplacement ; document `docs/source/sdk/android/telemetry_redaction_minutes_YYYYMMDD.md`.
2. **Alignement inter-SDK :** Coordonnez-vous avec les responsables Swift/JS pour maintenir des règles de hachage/compartiment cohérentes (suivies dans la feuille de route AND7).
3. **Communications avec les partenaires :** Incluez le résumé DPIA dans les kits d'intégration des partenaires (Support Playbook §9) et créez un lien vers ce document depuis `status.md`.