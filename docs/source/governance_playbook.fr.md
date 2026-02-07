---
lang: fr
direction: ltr
source: docs/source/governance_playbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9201c0027f05b1ab2c83fa6b3e1a1e6dad3ff9660a8ed23bac7667408d421ada
source_last_modified: "2026-01-22T15:38:30.665014+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Manuel de gouvernance

Ce manuel de jeu capture les rituels quotidiens qui maintiennent le réseau Sora
conseil de gouvernance aligné. Il regroupe les références faisant autorité du
référentiel afin que les cérémonies individuelles puissent rester concises, tandis que les opérateurs toujours
avoir un point d’entrée unique pour le processus plus large.

## Cérémonies du Conseil

- **Gouvernance des luminaires** – Voir [Approbation des luminaires du Parlement Sora](sorafs/signing_ceremony.md)
  pour le flux d’approbation en chaîne que le panel sur les infrastructures du Parlement
  suit lors de l’examen des mises à jour du chunker SoraFS.
- **Publication du décompte des votes** – Se référer à
  [Governance Vote Tally](governance_vote_tally.md) pour la CLI étape par étape
  modèle de workflow et de reporting.

## Runbooks opérationnels

- **Intégrations API** – [Référence API de gouvernance](governance_api.md) répertorie les
  Surfaces REST/gRPC exposées par les services municipaux, y compris l'authentification
  exigences et règles de pagination.
- **Tableaux de bord de télémétrie** – Les définitions JSON Grafana sous
  `docs/source/grafana_*` définissent les « Contraintes de gouvernance » et les « Planificateurs
  Cartes TEU ». Exportez le JSON dans Grafana après chaque version pour rester aligné
  avec la disposition canonique.

## Surveillance de la disponibilité des données

### Classes de rétention

Les commissions parlementaires approuvant les manifestes du DA doivent faire référence à la conservation forcée
politique avant de voter. Le tableau ci-dessous reflète les valeurs par défaut appliquées via
`torii.da_ingest.replication_policy` afin que les réviseurs puissent détecter les incohérences sans
recherche de la source TOML.【docs/source/da/replication_policy.md:1】

| Étiquette de gouvernance | Classe Blob | Rétention à chaud | Rétention au froid | Répliques requises | Classe de stockage |
|----------------|------------|---------------|----------------|-------------------|---------------|
| `da.taikai.live` | `taikai_segment` | 24h | 14j | 5 | `hot` |
| `da.sidecar` | `nexus_lane_sidecar` | 6h | 7j | 4 | `warm` |
| `da.governance` | `governance_artifact` | 12h | 180j | 3 | `cold` |
| `da.default` | _toutes les autres classes_ | 6h | 30j | 3 | `warm` |

Le panel d'infrastructure doit joindre le modèle rempli de
`docs/examples/da_manifest_review_template.md` à chaque scrutin afin que le manifeste
le résumé, la balise de rétention et les artefacts Norito restent liés dans la gouvernance
enregistrer.

### Piste d'audit du manifeste signé

Avant qu'un scrutin soit inscrit à l'ordre du jour, le personnel du conseil doit prouver que le manifeste
les octets examinés correspondent à l’enveloppe du Parlement et à l’artefact SoraFS. Utiliser
les outils existants pour collecter ces preuves :1. Récupérez le bundle de manifeste à partir de Torii (`iroha app da get-blob --storage-ticket <hex>`
   ou l'assistant SDK équivalent) afin que tout le monde hache les mêmes octets qui ont atteint
   les passerelles.
2. Exécutez le vérificateur de stub de manifeste avec l'enveloppe signée :
   ```
   cargo run -p sorafs_car --bin sorafs-manifest-stub -- manifest.json \
     --manifest-signatures-in=fixtures/sorafs_chunker/manifest_signatures.json \
     --json-out=/tmp/manifest_report.json
   ```
   Cela recalcule le résumé du manifeste BLAKE3, valide le
   `chunk_digest_sha3_256` et vérifie chaque signature Ed25519 intégrée dans
   `manifest_signatures.json`. Voir `docs/source/sorafs/manifest_pipeline.md`
   pour des options CLI supplémentaires.
3. Copiez le résumé, `chunk_digest_sha3_256`, le descripteur de profil et la liste des signataires dans
   le modèle de révision. REMARQUE : si le vérificateur signale une « incompatibilité de profil » ou un
   signature manquante, interrompre le vote et demander une enveloppe corrigée.
4. Stockez la sortie du vérificateur (ou l'artefact CI de
   `ci/check_sorafs_fixtures.sh`) aux côtés de la charge utile Norito `.to` afin que les auditeurs
   peut relire les preuves sans accéder aux passerelles internes.

Le pack d'audit qui en résulte devrait permettre au Parlement de recréer chaque hachage et signature
vérifiez même après que le manifeste soit sorti du stockage à chaud.

### Réviser la liste de contrôle

1. Retirez l'enveloppe du manifeste approuvée par le Parlement (voir
   `docs/source/sorafs/signing_ceremony.md`) et enregistrez le résumé BLAKE3.
2. Vérifiez que le bloc `RetentionPolicy` du manifeste correspond à la balise dans le tableau.
   ci-dessus ; Torii rejettera les disparités, mais le conseil doit capturer le
   preuves pour les auditeurs.【docs/source/da/replication_policy.md:32】
3. Confirmez que la charge utile Norito soumise fait référence à la même balise de rétention.
   et la classe blob qui apparaît dans le ticket d'admission.
4. Joignez une preuve de la vérification de la stratégie (sortie CLI, `torii.da_ingest.replication_policy`
   dump, ou artefact CI) au paquet de révision afin que SRE puisse rejouer la décision.
5. Enregistrez les subventions prévues ou les ajustements de loyer lorsque la proposition dépend de
   `docs/source/sorafs_reserve_rent_plan.md`.

### Matrice d'escalade

| Type de demande | Panneau propriétaire | Preuves à joindre | Délais & télémétrie | Références |
|--------------|--------------|--------------|-----------------------|------------|
| Subvention / adaptation du loyer | Infrastructure + Trésorerie | Paquet DA rempli, delta de loyer à partir de `reserve_rentd`, projection de réserve mise à jour CSV, procès-verbal du vote du conseil | Notez l’impact des loyers avant de soumettre la mise à jour de la Trésorerie ; inclure une télémétrie de tampon glissant de 30 jours afin que Finance puisse effectuer un rapprochement dans la prochaine fenêtre de règlement | `docs/source/sorafs_reserve_rent_plan.md`, `docs/examples/da_manifest_review_template.md` |
| Retrait de modération/action de conformité | Modération + Conformité | Ticket de conformité (`ComplianceUpdateV1`), jetons de preuve, résumé du manifeste signé, statut d'appel | Suivez le SLA de conformité de la passerelle (accusé de réception dans les 24h, suppression complète ≤72h). Joindre l'extrait `TransparencyReportV1` montrant l'action. | `docs/source/sorafs_gateway_compliance_plan.md`, `docs/source/sorafs_moderation_panel_plan.md` |
| Gel/restauration d'urgence | Panel de modération du Parlement | Dossier d'approbation préalable, nouvel ordre de gel, résumé du manifeste de restauration, journal des incidents | Publier immédiatement un avis de gel et planifier le référendum d'annulation dans le prochain créneau de gouvernance ; inclure la saturation du tampon + la télémétrie de réplication DA pour justifier l’urgence. | `docs/source/sorafs/signing_ceremony.md`, `docs/source/sorafs_moderation_panel_plan.md` |Utilisez le tableau lors du tri des tickets d'admission afin que chaque panel reçoive le nombre exact
les objets nécessaires à l’exécution de son mandat.

### Livrables de reporting

Chaque décision DA-10 doit être livrée avec les artefacts suivants (joignez-les au
Entrée du DAG sur la gouvernance référencée dans le vote) :

- Le paquet Markdown complété de
  `docs/examples/da_manifest_review_template.md` (incluant désormais la signature et
  sections d'escalade).
- Le manifeste Norito signé (`.to`) plus l'enveloppe `manifest_signatures.json`
  ou les journaux du vérificateur CI qui prouvent le résumé de récupération.
- Toute mise à jour de transparence déclenchée par l'action :
  - Delta `TransparencyReportV1` pour les retraits ou les gels liés à la conformité.
  - Delta du grand livre de location/réserve ou instantané `ReserveSummaryV1` pour les subventions.
- Liens vers les instantanés de télémétrie collectés lors de la revue (profondeur de réplication,
  marge de tampon, retard de modération) afin que les observateurs puissent vérifier les conditions
  après coup.

## Modération et escalade

Les retraits de passerelles, les récupérations de subventions ou les gels de DA suivent la conformité
pipeline décrit dans `docs/source/sorafs_gateway_compliance_plan.md` et le
outillage d'appel dans `docs/source/sorafs_moderation_panel_plan.md`. Les panneaux devraient :

1. Enregistrez le ticket de conformité d'origine (`ComplianceUpdateV1` ou
   `ModerationAppealV1`) et joignez les jetons de preuve associés.【docs/source/sorafs_gateway_compliance_plan.md:20】
2. Confirmez si la demande invoque la voie d'appel de modération (panel de citoyens
   vote) ou un gel d'urgence du Parlement ; les deux flux doivent citer le manifeste
   résumé et balise de rétention capturés dans le nouveau modèle.【docs/source/sorafs_moderation_panel_plan.md:1】
3. Énumérer les délais d'escalade (fenêtres de validation/révélation d'appel, urgence
   durée du gel) et indiquer à quel conseil ou comité appartient le suivi.
4. Capturez l'instantané de télémétrie (marge de tampon, retard de modération) utilisé pour
   justifier l'action afin que les audits en aval puissent faire correspondre la décision à la réalité
   état.

Les panels de conformité et de modération doivent synchroniser leurs rapports hebdomadaires de transparence
avec les opérateurs de routeurs de règlement afin que les retraits et les subventions affectent la même chose
ensemble de manifestes.

## Modèles de rapports

Tous les avis DA-10 nécessitent désormais un paquet Markdown signé. Copier
`docs/examples/da_manifest_review_template.md`, renseigner les métadonnées du manifeste,
tableau de vérification de la rétention et résumé des votes du panel, puis épinglez le formulaire terminé.
document (plus les artefacts Norito/JSON référencés) à l'entrée du DAG de gouvernance.
Les panels doivent lier le paquet dans les minutes de gouvernance afin que les futurs retraits ou
les renouvellements de subventions peuvent citer le résumé du manifeste original sans réexécuter le
toute la cérémonie.

## Flux de travail des incidents et des révocations

Les actions d'urgence se déroulent désormais en chaîne. Lorsqu'une version de luminaire doit être
annulé, déposer un ticket de gouvernance et ouvrir une proposition de retour au Parlement
pointant vers le résumé du manifeste précédemment approuvé. Le panel des infrastructures
gère le vote, et une fois finalisé, le runtime Nexus publie la restauration
événement consommé par les clients en aval. Aucun artefact JSON local n'est requis.

## Garder le Playbook à jour- Mettez à jour ce fichier chaque fois qu'un nouveau runbook destiné à la gouvernance arrive dans le
  référentiel.
- Reliez ici les nouvelles cérémonies afin que l'index du conseil reste visible.
- Si un document référencé se déplace (par exemple, un nouveau chemin SDK), mettez à jour le lien
  dans le cadre de la même pull request pour éviter les pointeurs obsolètes.