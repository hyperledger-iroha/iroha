---
lang: fr
direction: ltr
source: docs/source/runbooks/address_manifest_ops.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: cb5d84c6939c186ebb4cd1b622e5ab66872349f5c177191c940a9e9fd63d1a17
source_last_modified: "2025-12-14T09:53:36.233782+00:00"
translation_last_reviewed: 2025-12-28
---

# Runbook des opérations du manifeste d'adresses (ADDR-7c)

Ce runbook opérationnalise l'élément de roadmap **ADDR-7c** en décrivant comment
vérifier, publier et retirer des entrées dans le manifeste de comptes/alias Sora
Nexus. Il complète le contrat technique dans
[`docs/account_structure.md`](../../account_structure.md) §4 et les attentes
métriques consignées dans `dashboards/grafana/address_ingest.json`.

## 1. Périmètre et entrées

| Entrée | Source | Notes |
|-------|--------|-------|
| Bundle de manifeste signé (`manifest.json`, `manifest.sigstore`, `checksums.sha256`, `notes.md`) | Pin SoraFS (`sorafs://address-manifests/<CID>/`) et miroir HTTPS | Les bundles sont émis par l'automatisation de release ; conservez la structure de répertoires lors du mirroring. |
| Digest + séquence du manifeste précédent | Bundle précédent (même motif de chemin) | Requis pour prouver la monotonie/l'immutabilité. |
| Accès télémetrie | Dashboard Grafana `address_ingest` + Alertmanager | Nécessaire pour surveiller le retrait Local‑8 et les pics d'adresses invalides. |
| Outillage | `cosign`, `shasum`, `b3sum` (ou `python3 -m blake3`), `jq`, CLI `iroha`, `scripts/account_fixture_helper.py` | Installer avant d'exécuter la checklist. |

## 2. Structure des artefacts

Chaque bundle suit la structure ci‑dessous ; ne renommez pas les fichiers lors
des copies entre environnements.

```
address-manifest-<REVISION>/
├── manifest.json              # canonical JSON (UTF-8, newline-terminated)
├── manifest.sigstore          # Sigstore bundle from `cosign sign-blob`
├── checksums.sha256           # one-line SHA-256 sum for each artifact
└── notes.md                   # change log (reason codes, tickets, owners)
```

Champs d'en‑tête de `manifest.json` :

| Champ | Description |
|-------|-------------|
| `version` | Version du schéma (actuellement `1`). |
| `sequence` | Numéro de révision monotone ; doit incrémenter d'exactement un. |
| `generated_ms` | Horodatage UTC de publication (millisecondes depuis l'époque). |
| `ttl_hours` | Durée maximale de cache que Torii/SDKs peuvent honorer (24 par défaut). |
| `previous_digest` | BLAKE3 du corps du manifeste précédent (hex). |
| `entries` | Tableau ordonné de registres (`global_domain`, `local_alias`, ou `tombstone`). |

## 3. Procédure de vérification

1. **Télécharger le bundle.**

   ```bash
   export REV=2025-04-12
   sorafs_cli fetch --id sorafs://address-manifests/${REV} --out artifacts/address_manifest_${REV}
   cd artifacts/address_manifest_${REV}
   ```

2. **Garde‑fou checksum.**

   ```bash
   shasum -a 256 -c checksums.sha256
   ```

   Tous les fichiers doivent signaler `OK` ; traiter tout écart comme une
   altération.

3. **Vérification Sigstore.**

   ```bash
   cosign verify-blob \
     --bundle manifest.sigstore \
     --certificate-identity-regexp 'governance\.sora\.nexus/addr-manifest' \
     --certificate-oidc-issuer https://accounts.google.com \
     manifest.json
   ```

4. **Preuve d'immutabilité.** Comparez `sequence` et `previous_digest` au
   manifeste archivé :

   ```bash
   jq '.sequence, .previous_digest' manifest.json
   b3sum -l 256 ../address-manifest_<prev>/manifest.json
   ```

   Le digest imprimé doit correspondre à `previous_digest`. Les sauts de séquence
   ne sont pas autorisés ; réémettre le manifeste si violation.

5. **Conformité TTL.** Assurez‑vous que `generated_ms + ttl_hours` couvre les
   fenêtres de déploiement prévues ; sinon la gouvernance doit republier avant
   expiration des caches.

6. **Sanité des entrées.**
   - Les entrées `global_domain` DOIVENT inclure `{ "domain": "example", "chain": "sora:nexus:global", "selector": "global" }`.
   - Les entrées `local_alias` DOIVENT embarquer le digest de 12 octets produit par
     Norm v1 (utilisez `iroha tools address convert <address-or-account_id> --format json --expect-prefix 753`
     pour confirmer ; le résumé JSON reflète le domaine fourni via `input_domain` et
     `legacy  suffix` rejoue l'encodage converti sous la forme `<i105>@<domain>` pour les manifestes).
   - Les entrées `tombstone` DOIVENT référencer exactement le sélecteur retiré,
     inclure `reason_code`, `ticket`, et `replaces_sequence`.

7. **Parité des fixtures.** Régénérez les vecteurs canoniques et assurez‑vous que
   la table des digests Local n'a pas changé de manière inattendue :

   ```bash
   cargo xtask address-vectors
   python3 scripts/account_fixture_helper.py check --quiet
   ```

8. **Garde‑fou d'automatisation.** Lancez le vérificateur de manifeste pour
   revalider le bundle de bout en bout (schéma d'en‑tête, formes des entrées,
   checksums et câblage du previous‑digest) :

   ```bash
   cargo xtask address-manifest verify \
     --bundle artifacts/address-manifest_2025-05-12 \
     --previous artifacts/address-manifest_2025-04-30
   ```

   L'option `--previous` pointe vers le bundle immédiatement précédent afin que
   l'outil confirme la monotonie de `sequence` et recalcule la preuve BLAKE3 de
   `previous_digest`. La commande échoue rapidement si un checksum dérive ou si un
   sélecteur `tombstone` omet les champs requis ; incluez la sortie dans votre
   ticket de changement avant de demander des signatures.

## 4. Flux de changement alias & tombstone

1. **Proposer le changement.** Ouvrez un ticket de gouvernance indiquant le code
   raison (`LOCAL8_RETIREMENT`, `DOMAIN_REASSIGNED`, etc.) et les sélecteurs affectés.
2. **Dériver les payloads canoniques.** Pour chaque alias mis à jour, exécutez :

   ```bash
   iroha tools address convert sora... --expect-prefix 753 --format json > /tmp/alias.json
   jq '.canonical_hex, .i105' /tmp/alias.json
   ```

3. **Rédiger l'entrée de manifeste.** Ajoutez un enregistrement JSON comme :

   ```json
   {
     "type": "tombstone",
     "selector": { "kind": "local", "digest_hex": "b18fe9c1abbac45b3e38fc5d" },
     "reason_code": "LOCAL8_RETIREMENT",
     "ticket": "ADDR-7c-2025-04-12",
     "replaces_sequence": 36
   }
   ```

   Lorsque vous remplacez un alias Local par un Global, incluez à la fois un
   enregistrement `tombstone` et l'enregistrement `global_domain` suivant portant
   le discriminant Nexus.

4. **Valider le bundle.** Rejouez les étapes de vérification ci‑dessus sur le
   manifeste brouillon avant de demander des signatures.
5. **Publier + surveiller.** Après signature par la gouvernance, suivez §3 et
   config) à sa valeur par défaut `true` sur les clusters de production une fois
   que les métriques confirment zéro usage Local‑8. Ne passez le drapeau à `false`
   que sur les clusters dev/test lorsque vous avez besoin de temps de soak.

## 5. Monitoring & rollback

- Dashboards : `dashboards/grafana/address_ingest.json` (panneaux pour
  `torii_address_invalid_total{endpoint,reason}`,
  `torii_address_local8_total{endpoint}`,
  `torii_address_collision_total{endpoint,kind="local12_digest"}`, et
  `torii_address_collision_domain_total{endpoint,domain}`) doivent rester au
  vert pendant 30 jours avant de bloquer définitivement le trafic Local‑8/Local‑12.
- Preuve de gating : exportez une requête de plage Prometheus 30 jours pour
  `torii_address_local8_total` et `torii_address_collision_total` (ex. :
  `promtool query range --output=json ...`) et exécutez
  `cargo xtask address-local8-gate --input <file> --json-out artifacts/address_gate.json` ;
  attachez le JSON + la sortie CLI aux tickets de déploiement afin que la
  gouvernance voie la fenêtre de couverture et confirme que les compteurs sont
  restés plats.
- Alertes (voir `dashboards/alerts/address_ingest_rules.yml`) :
  - `AddressLocal8Resurgence` — page dès qu'un contexte signale une nouvelle
    incrémentation Local‑8. Traitez‑la comme un bloqueur de release, stoppez les
    (en surcharge du défaut) jusqu'à remédiation du client fautif. Restaurez le
    drapeau à `true` une fois la télémétrie propre.
  - `AddressLocal12Collision` — se déclenche dès que deux labels Local‑12 hashent
    vers le même digest. Mettez en pause les promotions de manifeste, exécutez
    `scripts/address_local_toolkit.sh` pour confirmer le mapping de digest et
    coordonnez avec la gouvernance Nexus avant de réémettre l'entrée de registre
    affectée.
  - `AddressInvalidRatioSlo` — avertit lorsque les soumissions i105/compressées
    invalides (hors rejets Local‑8/strict‑mode) dépassent le SLO global de 0,1 %
    pendant dix minutes. Analysez `torii_address_invalid_total` par
    contexte/raison et coordonnez avec l'équipe SDK propriétaire avant de
    réactiver le mode strict.
- Logs : conservez les lignes de log Torii `manifest_refresh` et le numéro de
  ticket de gouvernance dans `notes.md`.
- Rollback : republiez le bundle précédent (mêmes fichiers, ticket incrémenté
  uniquement sur l'environnement affecté jusqu'à résolution, puis revenez à `true`.

## 6. Références

- [`docs/account_structure.md`](../../account_structure.md) §§4–4.1 (contrat).
- [`scripts/account_fixture_helper.py`](../../../scripts/account_fixture_helper.py) (synchronisation fixtures).
- [`fixtures/account/address_vectors.json`](../../../fixtures/account/address_vectors.json) (digests canoniques).
- [`dashboards/grafana/address_ingest.json`](../../../dashboards/grafana/address_ingest.json) (télémétrie).
