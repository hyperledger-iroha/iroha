---
lang: fr
direction: ltr
source: docs/examples/da_manifest_review_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c5c959bd6654d095d2b3785a02e9c2ec162e699ad985b342760b952e38766a66
source_last_modified: "2025-11-12T19:46:29.811940+00:00"
translation_last_reviewed: 2026-01-01
---

# Paquet de gouvernance pour manifeste de disponibilite des donnees (Modele)

Utilisez ce modele lorsque les panels du Parlement examinent des manifestes DA pour des
subventions, des takedowns ou des changements de retention (roadmap DA-10). Copiez le Markdown
dans le ticket de gouvernance, remplissez les placeholders, et joignez le fichier complete
avec les payloads Norito signes et les artefacts CI references ci-dessous.

```markdown
## Metadonnees du manifeste
- Nom / version du manifeste: <string>
- Classe de blob et tag de gouvernance: <taikai_segment / da.taikai.live>
- Digest BLAKE3 (hex): `<digest>`
- Hash du payload Norito (optionnel): `<digest>`
- Envelope source / URL: <https://.../manifest_signatures.json>
- ID du snapshot de politique Torii: `<unix timestamp or git sha>`

## Verification des signatures
- Source de recuperation du manifeste / ticket de stockage: `<hex>`
- Commande/resultat de verification: `cargo run -p sorafs_car --bin sorafs-manifest-stub -- manifest.json --manifest-signatures-in=manifest_signatures.json` (extrait de log joint?)
- `manifest_blake3` rapporte par l'outil: `<digest>`
- `chunk_digest_sha3_256` rapporte par l'outil: `<digest>`
- Multihashes des signataires du conseil:
  - `<did:...>` / `<ed25519 multihash>`
- Horodatage de verification (UTC): `<2026-02-20T11:04:33Z>`

## Verification de retention
| Champ | Attendu (politique) | Observe (manifeste) | Preuve |
|-------|---------------------|---------------------|--------|
| Retention hot (secondes) | <p. ex., 86400> | <valeur> | `<torii.da_ingest.replication_policy dump | CI link>` |
| Retention cold (secondes) | <p. ex., 1209600> | <valeur> |  |
| Repliques requises | <valeur> | <valeur> |  |
| Classe de stockage | <hot / warm / cold> | <valeur> |  |
| Tag de gouvernance | <da.taikai.live> | <valeur> |  |

## Contexte
- Type de demande: <Subvention | Takedown | Rotation de manifeste | Gel d'urgence>
- Ticket d'origine / reference compliance: <lien ou ID>
- Impact subvention / loyer: <changement XOR attendu ou "n/a">
- Lien d'appel de moderation (si applicable): <case_id ou lien>

## Resume de decision
- Panel: <Infrastructure | Moderation | Tresorerie>
- Resultat du vote: `<for>/<against>/<abstain>` (quorum `<threshold>` atteint?)
- Hauteur ou fenetre d'activation / rollback: `<block/slot range>`
- Actions de suivi:
  - [ ] Notifier la Tresorerie / ops loyer
  - [ ] Mettre a jour le rapport de transparence (`TransparencyReportV1`)
  - [ ] Planifier un audit de buffer

## Escalade et reporting
- Piste d'escalade: <Subvention | Compliance | Gel d'urgence>
- Lien / ID du rapport de transparence (si mis a jour): <`TransparencyReportV1` CID>
- Bundle proof-token ou reference ComplianceUpdate: <chemin ou ticket ID>
- Delta du ledger loyer / reserve (si applicable): <`ReserveSummaryV1` snapshot link>
- URL(s) de snapshot de telemetrie: <Grafana permalink ou artefact ID>
- Notes pour le compte rendu du Parlement: <resume des delais / obligations>

## Pieces jointes
- [ ] Manifeste Norito signe (`.to`)
- [ ] Resume JSON / artefact CI prouvant les valeurs de retention
- [ ] Proof token ou paquet compliance (pour les takedowns)
- [ ] Snapshot de telemetrie buffer (`iroha_settlement_buffer_xor`)
```

Archivez chaque paquet complete sous l'entree du DAG de gouvernance pour le vote afin que les
revues suivantes puissent referencer le digest du manifeste sans repeter la ceremonie complete.
