---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/chunker-profile-authoring.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : création de profil chunker
titre : Recherche pour le profil de l'auteur chunker SoraFS
sidebar_label : chunker automatique
la description: Tableau pour la prévisualisation du nouveau chunker de profil SoraFS et luminaires.
---

:::note Канонический источник
Cette page correspond à `docs/source/sorafs/chunker_profile_authoring.md`. Vous avez la possibilité de copier des copies de synchronisation, si vous êtes un star du Sphinx, vous ne pourrez pas vous en servir.
:::

# Recherche pour le profil de l'auteur chunker SoraFS

Ceci est utile pour préparer et publier un nouveau chunker de profil pour SoraFS.
Nous avons ajouté la RFC architecturale (SF-1) et la bibliothèque matérielle (SF-2a)
Les travaux concrets de l'auteur, la validation et la préparation des projets.
Dans le cas canonique, le premier est le même.
`docs/source/sorafs/proposals/sorafs_sf1_profile_v1.json`
et le journal de marche à sec approprié
`docs/source/sorafs/reports/sf1_determinism.md`.

## Обзор

Le profil du client est disponible pour le restaurant, actuellement :

- Définir les paramètres CDC et les paramètres multihash, définis pour nous
  architectes;
- publier des luminaires (JSON Rust/Go/TS + fuzz corpora + PoR Witness), которые
  Les SDK en aval peuvent être testés sans outils spécialisés ;
- inclure les métadonnées, les éléments de gouvernance (espace de noms, nom, semestre), ainsi que les recommandations
  по миграции и окна совместимости; je
- проходить детерминированную diff-suite до ревью совета.

N'hésitez pas à vous rendre à l'hôtel pour obtenir un aperçu, et vous serez prêt à le faire.## Снимок чартеров реестра

Avant la date prévue, il s'agirait d'un restaurant de la carte,
Le code `sorafs_manifest::chunker_registry::ensure_charter_compliance()` :

- Profil d'identification — положительные целые числа, монотонно возрастающие без пропусков.
- La poignée canonique (`namespace.name@semver`) est désormais prise en compte dans l'alias et
- L'alias d'Odin n'est pas nécessairement configuré avec la poignée canonique et la mise en service.
- Alias ​​должны быть непустыми и без пробелов по краям.

Informations CLI suivantes :

```bash
# JSON список всех зарегистрированных дескрипторов (ids, handles, aliases, multihash)
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles

# Эмитить метаданные для кандидата на профиль по умолчанию (канонический handle + aliases)
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
  --promote-profile=sorafs.sf1@1.0.0 --json-out=-
```

Ces commandes visent à prévenir les conflits avec la carte de la restauration et les données canoniques
métadonnées pour la gouvernance.

## Требуемые метаданные| Pôle | Description | Exemple (`sorafs.sf1@1.0.0`) |
|------|----------|--------------------------------------------|
| `namespace` | Le profil du groupe logique. | `sorafs` |
| `name` | Читаемая человеком метка. | `sf1` |
| `semver` | Sélectionnez la version sémantique pour les paramètres. | `1.0.0` |
| `profile_id` | L'identifiant de montre monotone est disponible après le profil de la personne. Réservez votre identifiant, mais vous ne devez pas utiliser le numéro correspondant. | `1` |
| `profile.min_size` | Минимальная длина чанка в octets. | `65536` |
| `profile.target_size` | Il y a une seule tranche en octets. | `262144` |
| `profile.max_size` | Le nombre maximal de canaux est en octets. | `524288` |
| `profile.break_mask` | Masque adapté pour le hachage roulant (hex). | `0x0000ffff` |
| `profile.polynomial` | Константа engrenage полинома (hex). | `0x3da3358b4dc173` |
| `gear_seed` | Seed для вычисления 64 KiB gear таблицы. | `sorafs-v1-gear` |
| `chunk_multihash.code` | Code Multihash pour le résumé du fichier. | `0x1f` (BLAKE3-256) |
| `chunk_multihash.digest` | Digest канонического regroupe les appareils. | `13fa...c482` |
| `fixtures_root` | Catalogue disponible pour les luminaires régénérés. | `fixtures/sorafs_chunker/sorafs.sf1@1.0.0/` |
| `por_seed` | Semences pour la détermination du PoR (`splitmix64`). | `0xfeedbeefcafebabe` (par exemple) |Les métadonnées doivent être prises en compte dans le document de prévisualisation, ainsi que les informations générales
les luminaires, les outils CLI et la gouvernance automatique peuvent améliorer la situation sans
ручных сверок. Si vous le souhaitez, installez le magasin de blocs et les CLI manifestes avec `--json-out=-`,
чтобы стримить вычисленные метаданные в заметки ревью.

### Options d'utilisation de la CLI et de restauration

- `sorafs_manifest_chunk_store --profile=<handle>` — повторно запускает метаданные чанка,
  manifeste digest et PoR проверки с предлагаемыми параметрами.
- `sorafs_manifest_chunk_store --json-out=-` — vous pouvez ouvrir un magasin de morceaux sur la sortie standard pour
  автоматизированных сравнений.
- `sorafs_manifest_stub --chunker-profile=<handle>` — подтверждает, что manifestes и CAR
  планы встраивают канонический handle и alias.
- `sorafs_manifest_stub --plan=-` — avant `chunk_fetch_specs` pour les vérifications
  offsets/digests après la révision.

Téléchargez votre commande (digests, racines PoR, hachages manifestes) en amont, ce que vous obtenez maintenant
воспроизвести их буквально.

## Test de détection et de validation1. **Régénérer les luminaires**
   ```bash
   cargo run --locked -p sorafs_chunker --bin export_vectors \
     --signature-out=fixtures/sorafs_chunker/manifest_signatures.json
   ```
2. **Définir la suite paritaire** — `cargo test -p sorafs_chunker` et harnais de comparaison multilingue
   (`crates/sorafs_chunker/tests/vectors.rs`) Vous pouvez facilement installer de nouveaux luminaires.
3. **Assurez-vous des corps fuzz/contre-pression** — sélectionnez `cargo fuzz list` et le harnais de streaming
   (`fuzz/sorafs_chunker`) pour les actifs régénérés.
4. **Проверить Témoins de preuve de récupérabilité** — запустите
   `sorafs_manifest_chunk_store --por-sample=<n>` avec le profil précédent et la mise à jour,
   что racines совпадают с luminaire manifeste.
5. **CI dry run** — выполните `ci/check_sorafs_fixtures.sh` локально ; écriture du livre
   пройти с новыми luminaires и существующим `manifest_signatures.json`.
6. **Mise à jour inter-exécution** — assurez-vous que les liaisons Go/TS peuvent être régénérées
   JSON et vous permettent d'identifier les fichiers et les résumés.

Documentez les commandes et les résumés complets en avant-première, le Tooling WG peut être rédigé sans chien.

### Подтверждение Manifeste / PoR

Une fois les appareils régénérés, vous avez terminé le pipeline de manifestes, ce qui s'est produit
Les preuves CAR метаданные и PoR sont disponibles :

```bash
# Проверить метаданные чанка + PoR с новым профилем
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
  --profile=sorafs.sf2@1.0.0 \
  --json-out=- --por-json-out=- fixtures/sorafs_chunker/input.bin

# Сгенерировать manifest + CAR и сохранить chunk fetch specs
cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- \
  fixtures/sorafs_chunker/input.bin \
  --chunker-profile=sorafs.sf2@1.0.0 \
  --chunk-fetch-plan-out=chunk_plan.json \
  --manifest-out=sf2.manifest \
  --car-out=sf2.car \
  --json-out=sf2.report.json

# Повторно запустить с сохраненным планом fetch (защищает от устаревших offsets)
cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- \
  fixtures/sorafs_chunker/input.bin \
  --chunker-profile=sorafs.sf2@1.0.0 \
  --plan=chunk_plan.json --json-out=-
```

Prenez soin de votre club de football en utilisant vos luminaires
(par exemple, déterminer une valeur de 1 Gio) et ajouter des résumés complets à la prévision.

## Шаблон предложенияLa préparation du Norito concerne le `ChunkerProfileProposalV1` et la configuration du
`docs/source/sorafs/proposals/`. Le format JSON ne permet pas de créer une forme d'affichage
(подставьте свои значения по мере необходимости):


Précédez la sortie Markdown (`determinism_report`) de votre choix.
команд, digère les чанков и любые отклонения, обнаруженные при валидации.

## Workflow de gouvernance

1. **Отправить PR с предложением + luminaires.** Включите сгенерированные actifs, Norito
   pré-commande et mise à jour `chunker_registry_data.rs`.
2. **Ревью Tooling WG.** Ревьюеры повторно запускают чекlist валидации и подтверждают,
   что предложение соответствует правилам реестра (sans повторного использования id,
   детерминизм достигнут).
3. **Конверт совета.** После одобрения члены совета подписывают digest предложения
   (`blake3("sorafs-chunker-profile-v1" || canonical_bytes)`) et ajouter des modules
   Dans le profil de conversion, vous êtes en contact avec les luminaires.
4. ** Publier le restaurant. ** Fusionner les fichiers actuels, les documents et les luminaires. En utilisant la CLI
   Selon le profil précédent, la gouvernance ne vise pas la migration gouvernementale.
5. **Déclassement.** Une fois la migration terminée, vous devez restaurer les paramètres.

## Советы по авторингу- Prévoyez plusieurs étapes pour réduire la taille du morceau.
- Vous ne pouvez pas utiliser le code multihash sans coordonner le manifeste et la passerelle ; добавляйте
  заметку о совместимости.
- Ajoutez des tableaux d'engrenages pour graines, qui sont globalement uniques pour l'amélioration de l'audio.
- Vérifiez le banc d'œuvre d'art (pour déterminer le débit) dans
  `docs/source/sorafs/reports/` pour le bureau du propriétaire.

Le fonctionnement du déploiement est simple. dans le registre des migrations
(`docs/source/sorafs/migration_ledger.md`). Правила runtime соответствия см. в
`docs/source/sorafs/chunker_conformance.md`.