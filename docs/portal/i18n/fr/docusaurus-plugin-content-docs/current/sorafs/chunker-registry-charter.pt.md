---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/chunker-registry-charter.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : chunker-registry-charte
titre : Carta do registro de chunker da SoraFS
sidebar_label : Carte d'enregistrement du chunker
description : Carta de gouvernance pour la soumission et l'approbation des perfis de chunker.
---

:::note Fonte canonica
Cette page espelha `docs/source/sorafs/chunker_registry_charter.md`. Mantenha ambas comme copies synchronisées.
:::

# Carte de gouvernance du registre du chunker du SoraFS

> **Ratificado:** 2025-10-29 par le Panel sur les infrastructures du Parlement de Sora (voir
> `docs/source/sorafs/council_minutes_2025-10-29.md`). Qu'importe quoi emenda requer um
> vote formel de gouvernance ; les équipes de mise en œuvre développent ce document comme
> normativo ate que uma carta substituta seja aprovada.

Cette carte définit le processus et les papiers pour évoluer le registre de chunker da SoraFS.
Elle complète le [Guide automatique des performances du chunker](./chunker-profile-authoring.md) pour le découvrir comme nouveau
perfis sao propositions, revisados, ratificados e eventualmente descontinuados.

## Escopo

La carte est appliquée à chaque entrée dans le `sorafs_manifest::chunker_registry` e
un outil quelconque qui consomme ou s'enregistre (CLI manifeste, CLI d'annonce de fournisseur,
SDK). Cela impose des invariants d'alias et de handle vérifiés par
`chunker_registry::ensure_charter_compliance()` :- ID de profil à l'intérieur positif qui augmente la forme monotone.
- La poignée canonico `namespace.name@semver` **deve** apparaît comme la première
  entrée dans `profile_aliases`. Les alias alternatifs viennent en suite.
- As strings de alias sao aparadas, unicas e nao colidem com handles canonicos
  de outras entradas.

## Papeis

- **Autor(es)** - préparer une proposition, régénérer les luminaires et les assembler
  preuve de déterminisme.
- **Groupe de travail sur l'outillage (TWG)** - valide la proposition d'utilisation comme listes de contrôle
  publicadas e assure que les invariants du registre sont toujours présents.
- **Conseil de gouvernance (GC)** - révision du rapport du TWG, assina ou enveloppe de la proposition
  et approuve les prazos de publicacao/deprecacao.
- **Storage Team** - gère la mise en œuvre du registre et du public
  actualisations de la documentation.

## Flux du cycle de vie

1. **Soumission d'une proposition**
   - L'auteur exécute une liste de contrôle de validation du guide de l'autorité et de la cria
     euh JSON `ChunkerProfileProposalV1` sanglot
     `docs/source/sorafs/proposals/`.
   - Inclua a dita do CLI de :
     ```bash
     cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles
     cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
       --promote-profile=<handle> --json-out=-
     cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- \
       --chunker-profile=<handle> --json-out=-
     ```
   - Envie d'un match de relations publiques, proposition, rapport de déterminisme et
     atualizacoes do registro.2. **Révision de l'outillage (TWG)**
   - Répéter une liste de contrôle de validation (fixtures, fuzz, pipeline de manifest/PoR).
   - Exécuter `cargo test -p sorafs_car --chunker-registry` et garantir que
     `ensure_charter_compliance()` passe par une nouvelle entrée.
   - Vérifiez que le comportement de CLI (`--list-profiles`, `--promote-profile`, streaming
     `--json-out=-`) renvoie les alias et les poignées actualisés.
   - Produza um relatorio curto reumindo achados e status de aprovacao/reprovacao.

3. **Aprovacao do conselho (GC)**
   - Réviser le rapport du TWG et les métadonnées de la proposition.
   - Assine o digest da proposition (`blake3("sorafs-chunker-profile-v1" || bytes)`)
     e anexe assinaturas ao enveloppe do conselho mantido junto aos luminaires.
   - Registre des résultats du vote sur les actes de gouvernance.

4. **Publicacao**
   - Faca merge do PR, actualisé :
     - `sorafs_manifest::chunker_registry_data`.
     - Documentacao (`chunker_registry.md`, guias de autoria/conformidade).
     - Luminaires et rapports de déterminisme.
   - Notifier les opérateurs et les équipements SDK sur le nouveau profil et le plan de déploiement.

5. **Déprécacao / Encerramento**
   - Il est proposé de remplacer un profil existant en incluant une page de publication
     double (périodes de charge) et un plan de mise à niveau.
     pas d'enregistrement et d'actualisation du grand livre de migration.6. **Mudancas d'émergence**
   - Les suppressions ou les correctifs nécessitent le vote du conseil avec l'approbation de la majorité.
   - O TWG développe des documents sur les étapes d'atténuation des risques et l'actualisation du journal des incidents.

## Attentes de l'outillage

- Exposition `sorafs_manifest_chunk_store` et `sorafs_manifest_stub` :
  - `--list-profiles` pour inspection du registre.
  - `--promote-profile=<handle>` pour utiliser le bloc de métadonnées canonique
    ao promouvoir un profil.
  - `--json-out=-` pour transmettre des informations sur la sortie standard, permettant les journaux de révision
    reproduisant.
- `ensure_charter_compliance()` et appelé pour l'initialisation des binaires pertinents
  (`manifest_chunk_store`, `provider_advert_stub`). Os testes de CI devem falhar se
  novas entradas violarem a carta.

## Registre

- Armazene tous les rapports de déterminisme dans `docs/source/sorafs/reports/`.
- Comme le conseille le référent aux décisions de chunker, ficam em
  `docs/source/sorafs/migration_ledger.md`.
- Actualisez `roadmap.md` et `status.md` après chaque changement principal sans enregistrement.

## Références

- Guia de autoria : [Guia de autoria de perfis de chunker](./chunker-profile-authoring.md)
- Liste de contrôle de conformité : `docs/source/sorafs/chunker_conformance.md`
- Référence du registre : [Registro de perfis de chunker](./chunker-registry.md)