---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/provider-admission-policy.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

> Adapté de [`docs/source/sorafs/provider_admission_policy.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_admission_policy.md).

# Politique d'admission et d'identité des providers SoraFS (Brouillon SF-2b)

Cette note capture les livrables actionnables pour **SF-2b** : définir et
appliquer le workflow d'admission, les exigences d'identité et les payloads
d'attestation pour les providers de stockage SoraFS. Elle étend le processus
haut niveau décrit dans le RFC d'architecture SoraFS et découpe le travail
restant en tâches d'ingénierie traçables.

## Objectifs de la politique

- Garantir que seuls des opérateurs vérifiés peuvent publier des enregistrements
  `ProviderAdvertV1` acceptés par le réseau.
- Lier chaque clé d'annonce à un document d'identité approuvé par la gouvernance,
  des endpoints attestés et une contribution minimale de stake.
- Fournir un outillage de vérification déterministe afin que Torii, les gateways
  et `sorafs-node` appliquent les mêmes contrôles.
- Supporter le renouvellement et la révocation d'urgence sans casser le
  déterminisme ni l'ergonomie des outils.

## Exigences d'identité et de stake

| Exigence | Description | Livrable |
|----------|-------------|----------|
| Provenance de la clé d'annonce | Les providers doivent enregistrer une paire de clés Ed25519 qui signe chaque advert. Le bundle d'admission stocke la clé publique avec une signature de gouvernance. | Étendre le schéma `ProviderAdmissionProposalV1` avec `advert_key` (32 bytes) et le référencer depuis le registre (`sorafs_manifest::provider_admission`). |
| Pointeur de stake | L'admission requiert un `StakePointer` non nul pointant vers un pool de staking actif. | Ajouter la validation dans `sorafs_manifest::provider_advert::StakePointer::validate()` et remonter les erreurs dans CLI/tests. |
| Tags de juridiction | Les providers déclarent la juridiction + le contact légal. | Étendre le schéma de proposition avec `jurisdiction_code` (ISO 3166-1 alpha-2) et `contact_uri` optionnel. |
| Attestation d'endpoint | Chaque endpoint annoncé doit être soutenu par un rapport de certificat mTLS ou QUIC. | Définir le payload Norito `EndpointAttestationV1` et le stocker par endpoint dans le bundle d'admission. |

## Workflow d'admission

1. **Création de proposition**
   - CLI : ajouter `cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission proposal ...`
     produisant `ProviderAdmissionProposalV1` + bundle d'attestation.
   - Validation : s'assurer des champs requis, stake > 0, handle chunker canonique dans `profile_id`.
2. **Endossement de gouvernance**
   - Le conseil signe `blake3("sorafs-provider-admission-v1" || canonical_bytes)` via l'outillage
     d'envelope existant (module `sorafs_manifest::governance`).
   - L'envelope est persisté dans `governance/providers/<provider_id>/admission.json`.
3. **Ingestion du registre**
   - Implémenter un vérificateur partagé (`sorafs_manifest::provider_admission::validate_envelope`)
     réutilisé par Torii/gateways/CLI.
   - Mettre à jour le chemin d'admission Torii pour rejeter les adverts dont le digest ou l'expiration
     diffèrent de l'envelope.
4. **Renouvellement et révocation**
   - Ajouter `ProviderAdmissionRenewalV1` avec mises à jour optionnelles d'endpoint/stake.
   - Exposer un chemin CLI `--revoke` qui enregistre la raison de révocation et pousse un événement de gouvernance.

## Tâches d'implémentation

| Domaine | Tâche | Owner(s) | Statut |
|--------|------|----------|--------|
| Schéma | Définir `ProviderAdmissionProposalV1`, `ProviderAdmissionEnvelopeV1`, `EndpointAttestationV1` (Norito) sous `crates/sorafs_manifest/src/provider_admission.rs`. Implémenté dans `sorafs_manifest::provider_admission` avec helpers de validation.【F:crates/sorafs_manifest/src/provider_admission.rs#L1】 | Storage / Governance | ✅ Terminé |
| Outillage CLI | Étendre `sorafs_manifest_stub` avec les sous-commandes : `provider-admission proposal`, `provider-admission sign`, `provider-admission verify`. | Tooling WG | ✅ |

Le flux CLI accepte désormais les bundles de certificats intermédiaires (`--endpoint-attestation-intermediate`), émet des bytes canoniques proposal/envelope et valide les signatures du conseil pendant `sign`/`verify`. Les opérateurs peuvent fournir des corps d'advert directement, ou réutiliser des adverts signés, et des fichiers de signature peuvent être fournis en combinant `--council-signature-public-key` avec `--council-signature-file` pour faciliter l'automatisation.

### Référence CLI

Exécutez chaque commande via `cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission ...`.

- `proposal`
  - Flags requis : `--provider-id=<hex32>`, `--chunker-profile=<namespace.name@semver>`,
    `--stake-pool-id=<hex32>`, `--stake-amount=<amount>`, `--advert-key=<hex32>`,
    `--jurisdiction-code=<ISO3166-1>`, et au moins un `--endpoint=<kind:host>`.
  - L'attestation par endpoint attend `--endpoint-attestation-attested-at=<secs>`,
    `--endpoint-attestation-expires-at=<secs>`, un certificat via
    `--endpoint-attestation-leaf=<path>` (plus `--endpoint-attestation-intermediate=<path>`
    optionnel pour chaque élément de chaîne) et tout ID ALPN négocié
    (`--endpoint-attestation-alpn=<token>`). Les endpoints QUIC peuvent fournir des rapports de transport via
    `--endpoint-attestation-report[-hex]=...`.
  - Sortie : bytes canoniques de proposition Norito (`--proposal-out`) et un résumé JSON
    (stdout par défaut ou `--json-out`).
- `sign`
  - Entrées : une proposition (`--proposal`), un advert signé (`--advert`), un corps d'advert optionnel
    (`--advert-body`), retention epoch et au moins une signature du conseil. Les signatures peuvent être
    fournies inline (`--council-signature=<signer_hex:signature_hex>`) ou via fichiers en combinant
    `--council-signature-public-key` avec `--council-signature-file=<path>`.
  - Produit un envelope validé (`--envelope-out`) et un rapport JSON indiquant les liaisons de digest,
    le nombre de signataires et les chemins d'entrée.
- `verify`
  - Valide un envelope existant (`--envelope`), avec vérification optionnelle de la proposition,
    de l'advert ou du corps d'advert correspondant. Le rapport JSON met en avant les valeurs de digest,
    l'état de vérification de signature et les artefacts optionnels correspondants.
- `renewal`
  - Lie un nouvel envelope approuvé au digest précédemment ratifié. Requiert
    `--previous-envelope=<path>` et le successeur `--envelope=<path>` (deux payloads Norito).
    Le CLI vérifie que les aliases de profil, les capacités et les clés d'advert restent inchangés,
    tout en autorisant les mises à jour de stake, d'endpoints et de metadata. Émet les bytes canoniques
    `ProviderAdmissionRenewalV1` (`--renewal-out`) ainsi qu'un résumé JSON.
- `revoke`
  - Émet un bundle d'urgence `ProviderAdmissionRevocationV1` pour un provider dont l'envelope doit
    être retiré. Requiert `--envelope=<path>`, `--reason=<text>`, au moins une
    `--council-signature`, et optionnellement `--revoked-at`/`--notes`. Le CLI signe et valide le
    digest de révocation, écrit le payload Norito via `--revocation-out`, et imprime un rapport JSON
    avec le digest et le nombre de signatures.
| Vérification | Implémenter un vérificateur partagé utilisé par Torii, gateways et `sorafs-node`. Fournir des tests unitaires + d'intégration CLI.【F:crates/sorafs_manifest/src/provider_admission.rs#L1】【F:crates/iroha_torii/src/sorafs/admission.rs#L1】 | Networking TL / Storage | ✅ Terminé |
| Intégration Torii | Injecter le vérificateur dans l'ingestion des adverts Torii, rejeter les adverts hors politique, émettre la télémétrie. | Networking TL | ✅ Terminé | Torii charge désormais les envelopes de gouvernance (`torii.sorafs.admission_envelopes_dir`), vérifie les correspondances digest/signature lors de l'ingestion et expose la télémétrie d'admission.【F:crates/iroha_torii/src/sorafs/admission.rs#L1】【F:crates/iroha_torii/src/sorafs/discovery.rs#L1】【F:crates/iroha_torii/src/sorafs/api.rs#L1】 |
| Renouvellement | Ajouter le schéma de renouvellement/révocation + helpers CLI, publier un guide de cycle de vie dans les docs (voir runbook ci-dessous et commandes CLI `provider-admission renewal`/`revoke`).【crates/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#L477】【docs/source/sorafs/provider_admission_policy.md:120】 | Storage / Governance | ✅ Terminé |
| Télémétrie | Définir dashboards/alertes `provider_admission` (renouvellement manquant, expiration d'envelope). | Observability | 🟠 En cours | Le compteur `torii_sorafs_admission_total{result,reason}` existe ; dashboards/alertes en attente.【F:crates/iroha_telemetry/src/metrics.rs#L3798】【F:docs/source/telemetry.md#L614】 |

### Runbook de renouvellement et révocation

#### Renouvellement programmé (mises à jour de stake/topologie)
1. Construisez la paire proposal/advert successeur avec `provider-admission proposal` et `provider-admission sign`, en augmentant `--retention-epoch` et en mettant à jour stake/endpoints si besoin.
2. Exécutez
   ```bash
   cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission \
     renewal \
     --previous-envelope=governance/providers/<id>/envelope.to \
     --envelope=governance/providers/<id>/envelope_next.to \
     --renewal-out=governance/providers/<id>/renewal.to \
     --json-out=governance/providers/<id>/renewal.json \
     --notes="stake top-up 2025-03"
   ```
   La commande valide les champs de capacité/profil inchangés via
   `AdmissionRecord::apply_renewal`, émet `ProviderAdmissionRenewalV1`, et imprime les digests pour
   le journal de gouvernance.【crates/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#L477】【F:crates/sorafs_manifest/src/provider_admission.rs#L422】
3. Remplacez l'envelope précédent dans `torii.sorafs.admission_envelopes_dir`, commitez le Norito/JSON de renouvellement dans le dépôt de gouvernance, et ajoutez le hash de renouvellement + retention epoch à `docs/source/sorafs/migration_ledger.md`.
4. Notifiez les opérateurs que le nouvel envelope est actif et surveillez `torii_sorafs_admission_total{result="accepted",reason="stored"}` pour confirmer l'ingestion.
5. Régénérez et commitez les fixtures canoniques via `cargo run -p sorafs_car --bin provider_admission_fixtures --features cli` ; CI (`ci/check_sorafs_fixtures.sh`) valide que les sorties Norito restent stables.

#### Révocation d'urgence
1. Identifiez l'envelope compromis et émettez une révocation :
   ```bash
   cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission \
     revoke \
     --envelope=governance/providers/<id>/envelope.to \
     --reason="endpoint compromise" \
     --revoked-at=$(date +%s) \
     --notes="incident-456" \
     --council-signature=<signer_hex:signature_hex> \
     --revocation-out=governance/providers/<id>/revocation.to \
     --json-out=governance/providers/<id>/revocation.json
   ```
   Le CLI signe `ProviderAdmissionRevocationV1`, vérifie l'ensemble des signatures via
   `verify_revocation_signatures`, et rapporte le digest de révocation.【crates/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#L593】【F:crates/sorafs_manifest/src/provider_admission.rs#L486】
2. Supprimez l'envelope de `torii.sorafs.admission_envelopes_dir`, distribuez le Norito/JSON de révocation aux caches d'admission, et enregistrez le hash de la raison dans les minutes de gouvernance.
3. Surveillez `torii_sorafs_admission_total{result="rejected",reason="admission_missing"}` pour confirmer que les caches abandonnent l'advert révoqué ; conservez les artefacts de révocation dans les rétrospectives d'incident.

## Tests et télémétrie

- Ajouter des fixtures golden pour les proposals et envelopes d'admission sous
  `fixtures/sorafs_manifest/provider_admission/`.
- Étendre le CI (`ci/check_sorafs_fixtures.sh`) pour régénérer les proposals et vérifier les envelopes.
- Les fixtures générés incluent `metadata.json` avec des digests canoniques ; les tests downstream valident
  `proposal_digest_hex` == `ca8e73a1f319ae83d7bd958ccb143f9b790c7e4d9c8dfe1f6ad37fa29facf936`.
- Fournir des tests d'intégration :
  - Torii rejette les adverts avec des envelopes d'admission manquants ou expirés.
  - Le CLI fait un aller-retour proposal → envelope → verification.
  - Le renouvellement de gouvernance fait pivoter l'attestation d'endpoint sans changer l'ID du provider.
- Exigences de télémétrie :
  - Émettre les compteurs `provider_admission_envelope_{accepted,rejected}` dans Torii. ✅ `torii_sorafs_admission_total{result,reason}` expose désormais les résultats acceptés/rejetés.
  - Ajouter des alertes d'expiration dans les dashboards d'observabilité (renouvellement dû dans les 7 jours).

## Prochaines étapes

1. ✅ Finalisé les modifications du schéma Norito et intégré les helpers de validation dans
   `sorafs_manifest::provider_admission`. Aucun feature flag requis.
2. ✅ Les workflows CLI (`proposal`, `sign`, `verify`, `renewal`, `revoke`) sont documentés et exercés via tests d'intégration ; gardez les scripts de gouvernance synchronisés avec le runbook.
3. ✅ L'admission/discovery Torii ingère les envelopes et expose les compteurs de télémétrie d'acceptation/rejet.
4. Focus observabilité : terminer les dashboards/alertes d'admission pour que les renouvellements dus sous sept jours déclenchent des warnings (`torii_sorafs_admission_total`, expiry gauges).
