---
lang: fr
direction: ltr
source: docs/i18n/japanese_backlog.md
status: complete
translator: manual
generator: scripts/sync_docs_i18n.py
source_hash: 569668c78c5fe322f602bfeb31944cde7c1f10c4e78cfae66b31cdfcca845885
source_last_modified: "2025-11-06T15:33:45.173464+00:00"
translation_last_reviewed: 2025-11-14
---

# Backlog de traduction de la documentation en japonais

Ce fichier recense la documentation japonaise encore marquée `status: needs-translation`,
regroupée par catégorie pour planifier les prochains lots. Les fichiers correspondant à
`CHANGELOG.*`, `status.*` et `roadmap.*` sont considérés comme temporaires et exclus du
backlog. Exécutez `python3 scripts/sync_docs_i18n.py --dry-run` pour rafraîchir la liste
avant de démarrer un nouveau lot.

## Vue d’ensemble

- Fichiers en attente : 0 (mis à jour le 2025-10-26)
- Exclusions de traduction : continuer à ignorer `CHANGELOG.*`, `status.*` et `roadmap.*`.
- Aucun backlog n’est actuellement ouvert. Lorsque de nouveaux documents anglais sont
  ajoutés, exécutez `scripts/sync_docs_i18n.py --dry-run` pour détecter le delta et
  planifier le prochain lot.

### Traductions japonaises récemment terminées

La liste ci‑dessous énumère les fichiers japonais déjà traduits, afin d’aider à planifier
les futurs lots et éviter les doublons de travail.

## Suggestions de lotissement

> Avec aucun élément en attente, nous conservons le plan de lots comme matériel de
> référence pour la prochaine vague de documents.

### Lot A – Documentation opérationnelle ZK / Torii
- Objectif : maintenir à jour en japonais les runbooks de rattachement et de vérification
  ZK.
- Périmètre : tout `docs/source/zk/*.ja.md` restant (les fichiers `lifecycle` et
  `prover_runbook` sont déjà complets).
- Livrable : couverture complète des opérations ZK, plus un glossaire annexé aux
  runbooks.

### Lot B – Conception cœur IVM / Kotodama / Norito
- Objectif : améliorer la découvrabilité développeur en traduisant la documentation de
  conception du VM et du codec.
- Périmètre : `docs/source/ivm_*.ja.md`, `docs/source/kotodama_*.ja.md`,
  `docs/source/norito_*.ja.md`.
- Livrable : résumés concis et index de mots‑clés pour chaque document.

### Lot C – Sumeragi / opérations réseau
- Objectif : fournir des runbooks opérationnels pour le consensus, le pipeline et la
  couche réseau.
- Périmètre : `docs/source/sumeragi*.ja.md`, `docs/source/governance_api.ja.md`,
  `docs/source/pipeline.ja.md`, `docs/source/p2p.ja.md`,
  `docs/source/state_tiering.ja.md`.
- Livrable : première édition japonaise du manuel opérateur.

### Lot D – Outils / exemples / références
- Objectif : fournir aux développeurs du contenu de référence et des exemples localisés.
- Périmètre : fichiers `docs/source/query_*.ja.md` restants (et d’éventuelles nouvelles
  références).
- Livrable : garantir que les principales références CLI et API restent alignées sur la
  source anglaise.

## Rappels de workflow de traduction

1. Choisissez les fichiers cibles et travaillez dans une branche dédiée.
2. Après avoir terminé la traduction, définissez `status: complete` dans le front matter
   et renseignez `translator` si nécessaire.
3. Exécutez `python3 scripts/sync_docs_i18n.py --dry-run` pour vérifier qu’il ne reste
   aucun stub.
4. Alignez la terminologie avec la documentation japonaise existante (par exemple
   `README.ja.md`).

## Notes

- Surveillez le formatage Markdown (indentation, tableaux, blocs de code) plutôt que
  d’exécuter `cargo fmt --all`.
- Lorsque le document source change, mettez à jour l’édition japonaise dans la même PR
  autant que possible.
- Respectez les conventions de terminologie partagées par la documentation publiée ;
  programmez des revues si le texte nécessite un consensus.
- Exécutez régulièrement `python3 scripts/sync_docs_i18n.py --dry-run` et mettez à jour ce
  backlog si de nouveaux écarts apparaissent.
