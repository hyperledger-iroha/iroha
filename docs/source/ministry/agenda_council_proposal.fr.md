---
lang: fr
direction: ltr
source: docs/source/ministry/agenda_council_proposal.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: d2a7a47fdf0c80d189c912baafa5d6ce81a17a4c90f2b1797e532989a56f5060
source_last_modified: "2026-01-03T18:07:57.726224+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Schéma de proposition du Conseil d'ordre du jour (MINFO-2a)

Référence de la feuille de route : **MINFO-2a — Validateur de format de proposition.**

Le flux de travail de l'Agenda Council regroupe les listes noires soumises par les citoyens et les changements de politique.
propositions avant que les comités de gouvernance ne les examinent. Ce document définit le
schéma de charge utile canonique, exigences en matière de preuves et règles de détection de duplication
consommé par le nouveau validateur (`cargo xtask ministry-agenda validate`) donc
les proposants peuvent linter les soumissions JSON localement avant de les télécharger sur le portail.

## Aperçu de la charge utile

Les propositions d'ordre du jour utilisent le schéma `AgendaProposalV1` Norito
(`iroha_data_model::ministry::AgendaProposalV1`). Les champs sont codés au format JSON lorsque
soumission via les surfaces CLI/portail.

| Champ | Tapez | Exigences |
|-------|------|--------------|
| `version` | `1` (u16) | Doit être égal à `AGENDA_PROPOSAL_VERSION_V1`. |
| `proposal_id` | chaîne (`AC-YYYY-###`) | Identifiant stable ; appliquée lors de la validation. |
| `submitted_at_unix_ms` | u64 | Millisecondes depuis l'époque Unix. |
| `language` | chaîne | Balise BCP‑47 (`"en"`, `"ja-JP"`, etc.). |
| `action` | énumération (`add-to-denylist`, `remove-from-denylist`, `amend-policy`) | Action demandée au ministère. |
| `summary.title` | chaîne | ≤256 caractères recommandés. |
| `summary.motivation` | chaîne | Pourquoi l’action est requise. |
| `summary.expected_impact` | chaîne | Résultats si l’action est acceptée. |
| `tags[]` | chaînes minuscules | Étiquettes de tri facultatives. Valeurs autorisées : `csam`, `malware`, `fraud`, `harassment`, `impersonation`, `policy-escalation`, `terrorism`, `spam`. |
| `targets[]` | objets | Une ou plusieurs entrées de famille de hachage (voir ci-dessous). |
| `evidence[]` | objets | Une ou plusieurs pièces jointes de preuves (voir ci-dessous). |
| `submitter.name` | chaîne | Nom d’affichage ou organisation. |
| `submitter.contact` | chaîne | E-mail, identifiant Matrix ou téléphone ; supprimé des tableaux de bord publics. |
| `submitter.organization` | chaîne (facultatif) | Visible dans l’interface utilisateur du réviseur. |
| `submitter.pgp_fingerprint` | chaîne (facultatif) | Empreinte digitale majuscule de 40 hex. |
| `duplicates[]` | cordes | Références facultatives aux identifiants de proposition précédemment soumis. |

### Entrées cibles (`targets[]`)

Chaque cible représente un résumé de famille de hachage référencé par la proposition.

| Champ | Descriptif | Validation |
|-------|-------------|------------|
| `label` | Nom convivial pour le contexte du réviseur. | Non vide. |
| `hash_family` | Identifiant de hachage (`blake3-256`, `sha256`, etc.). | Lettres/chiffres ASCII/`-_.`, ≤48 caractères. |
| `hash_hex` | Digest codé en hexadécimal minuscule. | ≥16 octets (32 caractères hexadécimaux) et doit être un hexadécimal valide. |
| `reason` | Brève description des raisons pour lesquelles le résumé doit être exécuté. | Non vide. |

Le validateur rejette les paires `hash_family:hash_hex` en double au sein du même
proposition et signale les conflits lorsque la même empreinte digitale existe déjà dans le
registre en double (voir ci-dessous).

### Pièces jointes aux preuves (`evidence[]`)

Document d'entrée de preuves dans lequel les réviseurs peuvent récupérer le contexte à l'appui.| Champ | Tapez | Remarques |
|-------|------|-------|
| `kind` | énumération (`url`, `torii-case`, `sorafs-cid`, `attachment`) | Détermine les exigences du résumé. |
| `uri` | chaîne | URL HTTP(S), ID de dossier Torii ou URI SoraFS. |
| `digest_blake3_hex` | chaîne | Requis pour les types `sorafs-cid` et `attachment` ; facultatif pour les autres. |
| `description` | chaîne | Texte libre facultatif pour les réviseurs. |

### Registre en double

Les opérateurs peuvent maintenir un registre des empreintes digitales existantes pour éviter les duplications
cas. Le validateur accepte un fichier JSON sous la forme :

```json
{
  "entries": [
    {
      "hash_family": "blake3-256",
      "hash_hex": "0d714bed4b7c63c23a2cf8ee9ce6c3cde1007907c427b4a0754e8ad31c91338d",
      "proposal_id": "AC-2025-014",
      "note": "Already handled in 2025-08 incident"
    }
  ]
}
```

Lorsqu'une cible de proposition correspond à une entrée, le validateur abandonne à moins que
`--allow-registry-conflicts` est spécifié (des avertissements sont toujours émis).
Utilisez [`cargo xtask ministry-agenda impact`](impact_assessment_tooling.md) pour
générer le résumé prêt pour le référendum qui fait référence au duplicata
instantanés du registre et des politiques.

## Utilisation de la CLI

Lint une seule proposition et comparez-la à un registre en double :

```bash
cargo xtask ministry-agenda validate \
  --proposal docs/examples/ministry/agenda_proposal_example.json \
  --registry docs/examples/ministry/agenda_duplicate_registry.json
```

Transmettez `--allow-registry-conflicts` pour rétrograder les appels en double vers les avertissements lorsque
effectuer des audits historiques.

La CLI s'appuie sur le même schéma Norito et les mêmes aides à la validation fournies avec
`iroha_data_model`, afin que les SDK/portails puissent réutiliser le `AgendaProposalV1::validate`
méthode pour un comportement cohérent.

## CLI de tri (MINFO-2b)

Référence de la feuille de route : **MINFO-2b — Tri multi-slot et journal d'audit.**

La liste du Conseil de l'Agenda est désormais gérée via un tri déterministe afin que les citoyens
peut auditer indépendamment chaque tirage. Utilisez la nouvelle commande :

```bash
cargo xtask ministry-agenda sortition \
  --roster docs/examples/ministry/agenda_council_roster.json \
  --slots 3 \
  --seed 0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef \
  --out artifacts/ministry/agenda_sortition_2026Q1.json
```

- `--roster` — Fichier JSON décrivant chaque membre éligible :

  ```json
  {
    "format_version": 1,
    "members": [
      {
        "member_id": "citizen:ada",
        "weight": 2,
        "role": "citizen",
        "organization": "Artemis Cooperative"
      },
      {
        "member_id": "citizen:erin",
        "weight": 1,
        "role": "citizen",
        "eligible": false
      }
    ]
  }
  ```

  Le fichier d'exemple se trouve à
  `docs/examples/ministry/agenda_council_roster.json`. Champs optionnels (rôle,
  organisation, contact, métadonnées) sont capturés dans la feuille Merkle afin que les auditeurs
  peut prouver la liste qui a alimenté le tirage au sort.

- `--slots` — nombre de sièges au conseil à pourvoir.
- `--seed` — Graine BLAKE3 de 32 octets (64 caractères hexadécimaux minuscules) enregistrée dans le
  procès-verbal de gouvernance pour le tirage.
- `--out` — chemin de sortie facultatif. Lorsqu'il est omis, le résumé JSON est imprimé dans
  sortie standard.

### Résumé du résultat

La commande émet un blob JSON `SortitionSummary`. L'échantillon de sortie est stocké dans
`docs/examples/ministry/agenda_sortition_summary_example.json`. Champs clés :

| Champ | Descriptif |
|-------|-------------|
| `algorithm` | Etiquette de tri (`agenda-sortition-blake3-v1`). |
| `roster_digest` | Résumés BLAKE3 + SHA-256 du fichier de liste (utilisés pour confirmer que les audits fonctionnent sur la même liste de membres). |
| `seed_hex` / `slots` | Faites écho aux entrées CLI afin que les auditeurs puissent reproduire le tirage. |
| `merkle_root_hex` | Racine de l'arborescence Merkle de la liste (assistants `hash_node`/`hash_leaf` dans `xtask/src/ministry_agenda.rs`). |
| `selected[]` | Entrées pour chaque emplacement, y compris les métadonnées canoniques des membres, l'index éligible, l'index de la liste d'origine, l'entropie de tirage déterministe, le hachage de feuille et les frères et sœurs à l'épreuve de Merkle. |

### Vérifier un tirage au sort1. Récupérez la liste référencée par `roster_path` et vérifiez son BLAKE3/SHA-256
   les résumés correspondent au résumé.
2. Réexécutez la CLI avec les mêmes graines/emplacements/liste ; le `selected[].member_id` résultant
   l’ordre doit correspondre au résumé publié.
3. Pour un membre spécifique, calculez la feuille Merkle à l'aide du membre sérialisé JSON
   (`norito::json::to_vec(&sortition_member)`) et pliez chaque hachage de preuve. La finale
   le résumé doit être égal à `merkle_root_hex`. L'assistant dans l'exemple de résumé montre
   comment combiner `eligible_index`, `leaf_hash_hex` et `merkle_proof[]`.

Ces artefacts satisfont à l'exigence du MINFO-2b concernant le caractère aléatoire vérifiable,
sélection k-of-m et ajout uniquement des journaux d'audit jusqu'à ce que l'API en chaîne soit câblée.

## Référence d'erreur de validation

`AgendaProposalV1::validate` émet des variantes `AgendaProposalValidationError`
chaque fois qu'une charge utile échoue au peluchage. Le tableau ci-dessous résume les plus courants
erreurs afin que les réviseurs du portail puissent traduire les résultats de la CLI en conseils exploitables.| Erreur | Signification | Assainissement |
|-------|---------|-------------|
| `UnsupportedVersion { expected, found }` | La charge utile `version` diffère du schéma pris en charge par le validateur. | Régénérez le JSON à l'aide du dernier ensemble de schémas afin que la version corresponde à `expected`. |
| `MissingProposalId` / `InvalidProposalIdFormat { value }` | `proposal_id` est vide ou non sous la forme `AC-YYYY-###`. | Remplissez un identifiant unique en suivant le format documenté avant de soumettre à nouveau. |
| `MissingSubmissionTimestamp` | `submitted_at_unix_ms` est nul ou manquant. | Enregistrez l'horodatage de soumission en millisecondes Unix. |
| `InvalidLanguageTag { value }` | `language` n'est pas une balise BCP‑47 valide. | Utilisez une balise standard telle que `en`, `ja-JP` ou un autre paramètre régional reconnu par BCP‑47. |
| `MissingSummaryField { field }` | L’un des `summary.title`, `.motivation` ou `.expected_impact` est vide. | Fournissez un texte non vide pour le champ récapitulatif indiqué. |
| `MissingSubmitterField { field }` | `submitter.name` ou `submitter.contact` manquant. | Fournissez les métadonnées manquantes du demandeur afin que les évaluateurs puissent contacter le proposant. |
| `InvalidTag { value }` | L'entrée `tags[]` ne figure pas sur la liste verte. | Supprimez ou renommez la balise avec l'une des valeurs documentées (`csam`, `malware`, etc.). |
| `MissingTargets` | Le tableau `targets[]` est vide. | Fournissez au moins une entrée de famille de hachage cible. |
| `MissingTargetLabel { index }` / `MissingTargetReason { index }` | Entrée cible manquant les champs `label` ou `reason`. | Remplissez le champ requis pour l’entrée indexée avant de soumettre à nouveau. |
| `InvalidHashFamily { index, value }` | Étiquette `hash_family` non prise en charge. | Limitez les noms de famille de hachage aux caractères alphanumériques ASCII plus `-_`. |
| `InvalidHashHex { index, value }` / `TargetDigestTooShort { index }` | Le résumé n'est pas un hexadécimal valide ou est inférieur à 16 octets. | Fournissez un résumé hexadécimal minuscule (≥32 caractères hexadécimaux) pour la cible indexée. |
| `DuplicateTarget { index, fingerprint }` | Le résumé cible duplique une entrée antérieure ou une empreinte de registre. | Supprimez les doublons ou fusionnez les preuves à l’appui en une seule cible. |
| `MissingEvidence` | Aucune pièce jointe de preuve n’a été fournie. | Joignez au moins un dossier de preuve lié au matériel de reproduction. |
| `MissingEvidenceUri { index }` | Entrée de preuve manquant le champ `uri`. | Fournissez l’URI récupérable ou l’identifiant de cas pour l’entrée de preuve indexée. |
| `MissingEvidenceDigest { index }` / `InvalidEvidenceDigest { index, value }` | L'entrée de preuve qui nécessite un résumé (ID CID ou pièce jointe SoraFS) est manquante ou comporte un `digest_blake3_hex` non valide. | Fournissez un résumé BLAKE3 minuscule de 64 caractères pour l'entrée indexée. |

## Exemples

- `docs/examples/ministry/agenda_proposal_example.json` — canonique,
  Charge utile de proposition non pelucheuse avec deux pièces jointes de preuves.
- `docs/examples/ministry/agenda_duplicate_registry.json` — registre de démarrage
  contenant une seule empreinte digitale BLAKE3 et une justification.

Réutilisez ces fichiers comme modèles lors de l'intégration des outils de portail ou de l'écriture de CI
vérifie les soumissions automatisées.