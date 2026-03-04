---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/chunker-profile-authoring.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : création de profil chunker
titre : Guide d'administration des performances de chunker par SoraFS
sidebar_label : Guide automatique du chunker
description : Liste de contrôle pour les nouveaux performances et les appareils du chunker da SoraFS.
---

:::note Fonte canonica
Cette page espelha `docs/source/sorafs/chunker_profile_authoring.md`. Mantenha ambas comme copies synchronisées.
:::

# Guide automatique des performances du chunker par SoraFS

Ce guide explique comment publier de nouveaux résultats de chunker pour le SoraFS.
Le complément au RFC d'architecture (SF-1) et à la référence du registre (SF-2a)
avec les exigences concrètes de l'autorité, les étapes de validation et les modèles de proposition.
Pour un exemple canonique, je vois
`docs/source/sorafs/proposals/sorafs_sf1_profile_v1.json`
e o log de dry-run associé
`docs/source/sorafs/reports/sf1_determinism.md`.

## Visa général

Chaque profil qui entre dans le registre ne doit pas :

- Annoncer les paramètres CDC déterminants et configurer les identités multihash entre
  architectures;
- introduire des reproductions de luminaires (JSON Rust/Go/TS + corpus fuzz + testemunhas PoR) que
  Les SDK du système d'exploitation en aval peuvent vérifier les outils sem à moyen terme ;
- inclure des métadonnées immédiates pour la gouvernance (espace de noms, nom, semestre) avec l'orientation
  de rollout et Janelas Operacionais ; e
- passer par la suite de diff déterministe avant la révision du conseil.

Siga a checklist abaixo para preparar une proposition qui attendra ces regras.

## Résumé de la charte du registreAvant de modifier une proposition, confirmez qu'elle attend la carte d'enregistrement appliquée par
`sorafs_manifest::chunker_registry::ensure_charter_compliance()` :

- ID de profil à l'intérieur positif qui augmente la forme monotone sans lacunes.
- Le handle canonico (`namespace.name@semver`) doit apparaître dans la liste des alias et
  **deve** être la première entrée. Alias ​​alternativos (ex., `sorafs.sf1@1.0.0`) vem depois.
- Nenhum alias peut collaborer avec une autre poignée canonique ou apparaître plus d'une fois.
- Les alias doivent être des vazios et des appareils d'espace en blanc.

Aides de CLI :

```bash
# Listagem JSON de todos os descritores registrados (ids, handles, aliases, multihash)
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles

# Emitir metadados para um perfil default candidato (handle canonico + aliases)
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
  --promote-profile=sorafs.sf1@1.0.0 --json-out=-
```

Esses comandos mantem as propositions alinhadas com a carta do registro e fornecem os
métadados canonicos necessarios nas discussionsoes de gouvernance.

## métadonnées requises| Champ | Description | Exemple (`sorafs.sf1@1.0.0`) |
|-------|-----------|------------------------------|
| `namespace` | Agrupamento logique para perfis relacionados. | `sorafs` |
| `name` | Rotulo legivel para humanos. | `sf1` |
| `semver` | Cadémie de versatilité sémantique pour le ensemble de paramètres. | `1.0.0` |
| `profile_id` | Identificateur numérique monotone attribué lorsque le profil entre. Réservez à proximité pour pouvoir réutiliser de nombreux existants. | `1` |
| `profile_aliases` | Gère les options supplémentaires (nomes alternativos, abreviacoes) exposées aux clients au cours d'une négociation. Inclut toujours la poignée canonique comme première entrée. | `["sorafs.sf1@1.0.0"]` |
| `profile.min_size` | Comprimento minimum pour fragmenter les octets. | `65536` |
| `profile.target_size` | Achetez également des fragments d'octets. | `262144` |
| `profile.max_size` | Comprimento maximo pour fragmenter les octets. | `524288` |
| `profile.break_mask` | Mascara adaptatif utilisé pour rouler le hash (hex). | `0x0000ffff` |
| `profile.polynomial` | Constante de l'engrenage polinomio (hex). | `0x3da3358b4dc173` |
| `gear_seed` | Seed utilisée pour dériver un tableau Gear de 64 KiB. | `sorafs-v1-gear` |
| `chunk_multihash.code` | Codigo multihash pour digérer un morceau. | `0x1f` (BLAKE3-256) |
| `chunk_multihash.digest` | Digest do bundle canonico de luminaires. | `13fa...c482` || `fixtures_root` | Diretorio relatif aux matchs régénérés. | `fixtures/sorafs_chunker/sorafs.sf1@1.0.0/` |
| `por_seed` | Graine para amostragem PoR deterministica (`splitmix64`). | `0xfeedbeefcafebabe` (exemple) |

Les métadonnés doivent apparaître tanto no documento de proposition quanto dentro dos luminaires gerados
pour que le registre, les outils CLI et la gouvernance automatique confirment les valeurs selon
manuels de croix. Dans le cas contraire, exécutez les CLI du chunk-store et du manifeste avec
`--json-out=-` pour transmettre les métadonnées calculées pour les notes de révision.

### Ponts de contact de CLI et d'enregistrement

- `sorafs_manifest_chunk_store --profile=<handle>` - réexécuter les métadonnées du chunk,
  digest do manifest e vérifie PoR avec les paramètres proposés.
- `sorafs_manifest_chunk_store --json-out=-` - transmettre le rapport du chunk-store pour
  Stdout pour les comparaisons automatisées.
- `sorafs_manifest_stub --chunker-profile=<handle>` - confirmer que manifeste et plan CAR
  embutem o handle canonico more alias.
- `sorafs_manifest_stub --plan=-` - réenviar o `chunk_fetch_specs` para antérieur
  vérifier les compensations/digestes après un changement.

Enregistrez les commandes (digestes, lèves PoR, hachages de manifeste) à propos de cela
os revisores possam reproduzi-los littéralement.

## Checklist de déterminisme et de validation1. **Régénérer les luminaires**
   ```bash
   cargo run --locked -p sorafs_chunker --bin export_vectors \
     --signature-out=fixtures/sorafs_chunker/manifest_signatures.json
   ```
2. **Exécuter une suite de parité** - `cargo test -p sorafs_chunker` et le diff du faisceau
   multilingue (`crates/sorafs_chunker/tests/vectors.rs`) devem ficar verdes com os
   novos luminaires pas de lieu.
3. **Réexécuter le corps fuzz/contre-pression** - exécuter `cargo fuzz list` et le harnais de
   streaming (`fuzz/sorafs_chunker`) contre les actifs régénérés.
4. **Vérifier la preuve de récupérabilité du test** - exécuter
   `sorafs_manifest_chunk_store --por-sample=<n>` utiliser le profil proposé et confirmer
   que les augmentations correspondent au manifeste des luminaires.
5. **Dry run de CI** - exécutez `ci/check_sorafs_fixtures.sh` localement ; ou script
   deve ter successo com os novos luminaires e o `manifest_signatures.json` existant.
6. **Confirmation cross-runtime** - assurez-vous que les liaisons Go/TS utilisent JSON
   régénéré et émis des limites et des digestions identiques.

Documentez les commandes et les résumés résultants de la proposition du Tooling WG.
reexecuta-los sem adivinhacoes.

### Confirmation du manifeste / PoR

Après avoir régénéré les appareils, exécutez le pipeline complet du manifeste pour garantir que
Les métadonnées CAR et les preuves PoR continuent d'être cohérentes :

```bash
# Validar metadados de chunk + PoR com o novo perfil
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
  --profile=sorafs.sf2@1.0.0 \
  --json-out=- --por-json-out=- fixtures/sorafs_chunker/input.bin

# Gerar manifest + CAR e capturar chunk fetch specs
cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- \
  fixtures/sorafs_chunker/input.bin \
  --chunker-profile=sorafs.sf2@1.0.0 \
  --chunk-fetch-plan-out=chunk_plan.json \
  --manifest-out=sf2.manifest \
  --car-out=sf2.car \
  --json-out=sf2.report.json

# Reexecutar usando o plano de fetch salvo (evita offsets obsoletos)
cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- \
  fixtures/sorafs_chunker/input.bin \
  --chunker-profile=sorafs.sf2@1.0.0 \
  --plan=chunk_plan.json --json-out=-
```

Remplacer l'archive d'entrée par tout corpus représentant utilisé nos appareils
(ex., un flux déterministe de 1 Gio) et l'annexe des résumés résultants à propos de la proposition.

## Modèle de propositionComme proposé par sa soumission comme registré Norito `ChunkerProfileProposalV1` enregistré
`docs/source/sorafs/proposals/`. Le modèle JSON est créé pour illustrer le format attendu
(substitua seus valores conforme necessario):


Contactez un correspondant Markdown (`determinism_report`) qui capture un
Saida dos commandos, digère le morceau et quaisquer desvios rencontrés lors d'une validation.

## Flux de gouvernance

1. **Submeter PR avec proposition + luminaires.** Incluant les actifs gérés, à proposition
   Norito et actualisé dans `chunker_registry_data.rs`.
2. **Révision du Tooling WG.** Les réviseurs réexécutent la liste de contrôle de validation et de confirmation
   que a proposeta segue as regras do registro (sem reutilizacao de id, determinismo satisfeito).
3. **Enveloppe du conseil.** Une fois approuvé, les membres du conseil assimilent le résumé du
   proposition (`blake3("sorafs-chunker-profile-v1" || canonical_bytes)`) et examen
   assinaturas ao enveloppe do perfil armazenado junto aos luminaires.
4. **Publicacao do registro.** La fusion actualise le registre, les documents et les luminaires. O CLI
   par défaut, il n'y a pas de profil antérieur qui demande au gouvernement de déclarer une migration immédiate.
5. **Rastreamento de deprecacao.** Après une jeune fille de migration, actualisez le registre pour

## Dicas de l'automobile- Préférez les limites de puissance de deux pour minimiser le comportement de chunking à l'intérieur des bords.
- Évitez de changer le code multihash en coordonnant les utilisateurs du manifeste et de la passerelle ; y compris uma
  nota operacional quando fizer isso.
- Mantenha as seeds da tabela gear legiveis para humanos, mais globalement unique pour simplifier les auditoires.
- Armazene artefatos de benchmarking (ex., comparaisons de débit) em
  `docs/source/sorafs/reports/` pour référence future.

Pour les attentes opérationnelles pendant le déploiement, consultez le registre de migration
(`docs/source/sorafs/migration_ledger.md`). Pour vérifier la conformité au runtime, voyez
`docs/source/sorafs/chunker_conformance.md`.