---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/provider-admission-policy.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

> Adapté de [`docs/source/sorafs/provider_admission_policy.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_admission_policy.md).

# Politique d'admission et d'identité des prestataires SoraFS (Brouillon SF-2b)

Cette note capture les livrables actionnables pour **SF-2b** : définir et
appliquer le workflow d'admission, les exigences d'identité et les charges utiles
d'attestation pour les fournisseurs de stockage SoraFS. Elle étend le processus
haut niveau décrit dans le RFC d'architecture SoraFS et découpe le travail
restant en tâches d'ingénierie traçables.

## Objectifs de la politique

- Garantir que seuls les opérateurs vérifiés peuvent publier des enregistrements
  `ProviderAdvertV1` acceptés par le réseau.
- Lier chaque clé d'annonce à un document d'identité approuvé par la gouvernance,
  des endpoints attestés et une contribution minimale d’enjeu.
- Fournir un outillage de vérification déterministe afin que Torii, les passerelles
  et `sorafs-node` appliquent les mêmes contrôles.
- Supporter le renouvellement et la révocation d'urgence sans casser le
  déterminisme ni l'ergonomie des outils.

## Exigences d'identité et d'enjeu

| Exigence | Descriptif | Habitable |
|--------------|-------------|--------------|
| Provenance de la clé d'annonce | Les fournisseurs doivent enregistrer une paire de clés Ed25519 qui signe chaque annonce. Le forfait d'admission stocke la clé publique avec une signature de gouvernance. | Étendre le schéma `ProviderAdmissionProposalV1` avec `advert_key` (32 octets) et le référencer depuis le registre (`sorafs_manifest::provider_admission`). |
| Pointeur de piquet | L'admission nécessite un `StakePointer` non nul pointant vers un pool de staking actif. | Ajoutez la validation dans `sorafs_manifest::provider_advert::StakePointer::validate()` et remonter les erreurs dans CLI/tests. |
| Tags de juridiction | Les prestataires déclarent la juridiction + le contact légal. | Étendre le schéma de proposition avec `jurisdiction_code` (ISO 3166-1 alpha-2) et `contact_uri` optionnel. |
| Attestation de point final | Chaque point final annoncé doit être soutenu par un rapport de certificat mTLS ou QUIC. | Définissez le payload Norito `EndpointAttestationV1` et le stocker par endpoint dans le bundle d'admission. |

## Workflow d'admission1. **Création de proposition**
   - CLI : ajouter `cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission proposal ...`
     produisant `ProviderAdmissionProposalV1` + bundle d'attestation.
   - Validation : s'assurer des champs requis, enjeux > 0, handle chunker canonique dans `profile_id`.
2. **Endossement de gouvernance**
   - Le conseil signe `blake3("sorafs-provider-admission-v1" || canonical_bytes)` via l'outillage
     d'enveloppe existante (module `sorafs_manifest::governance`).
   - L'enveloppe est persistante dans `governance/providers/<provider_id>/admission.json`.
3. **Ingestion du registre**
   - Implémenter un vérificateur partagé (`sorafs_manifest::provider_admission::validate_envelope`)
     réutilisation par Torii/gateways/CLI.
   - Mettre à jour le chemin d'admission Torii pour rejeter les annonces dont le digest ou l'expiration
     différent de l'enveloppe.
4. **Renouvellement et révocation**
   - Ajouter `ProviderAdmissionRenewalV1` avec mises à jour optionnelles d'endpoint/stake.
   - Exposer un chemin CLI `--revoke` qui enregistre la raison de révocation et pousse un événement de gouvernance.

## Tâches d'exécution

| Domaine | Tâche | Propriétaire(s) | Statuts |
|--------|------|----------|--------|
| Schéma | Définir `ProviderAdmissionProposalV1`, `ProviderAdmissionEnvelopeV1`, `EndpointAttestationV1` (Norito) sous `crates/sorafs_manifest/src/provider_admission.rs`. Implémenté dans `sorafs_manifest::provider_admission` avec helpers de validation.【F:crates/sorafs_manifest/src/provider_admission.rs#L1】 | Stockage / Gouvernance | ✅Terminé |
| Outillage CLI | Étendre `sorafs_manifest_stub` avec les sous-commandes : `provider-admission proposal`, `provider-admission sign`, `provider-admission verify`. | GT Outillage | ✅ |

Le flux CLI accepte désormais les bundles de certificats intermédiaires (`--endpoint-attestation-intermediate`), émet des octets canoniques proposition/enveloppe et valide les signatures du conseil pendant `sign`/`verify`. Les opérateurs peuvent fournir des corps d'annonce directement, ou réutiliser des annonces signées, et des fichiers de signature peuvent être fournis en combinant `--council-signature-public-key` avec `--council-signature-file` pour faciliter l'automatisation.

### Référence CLI

Exécutez chaque commande via `cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission ...`.-`proposal`
  - Drapeaux requis : `--provider-id=<hex32>`, `--chunker-profile=<namespace.name@semver>`,
    `--stake-pool-id=<hex32>`, `--stake-amount=<amount>`, `--advert-key=<hex32>`,
    `--jurisdiction-code=<ISO3166-1>`, et au moins un `--endpoint=<kind:host>`.
  - L'attestation par endpoint attend `--endpoint-attestation-attested-at=<secs>`,
    `--endpoint-attestation-expires-at=<secs>`, un certificat via
    `--endpoint-attestation-leaf=<path>` (plus `--endpoint-attestation-intermediate=<path>`
    optionnel pour chaque élément de chaîne) et tout ID ALPN négocié
    (`--endpoint-attestation-alpn=<token>`). Les endpoints QUIC peuvent fournir des rapports de transport via
    `--endpoint-attestation-report[-hex]=...`.
  - Sortie : bytes canoniques de proposition Norito (`--proposal-out`) et un résumé JSON
    (sortie standard par défaut ou `--json-out`).
-`sign`
  - Entrées : une proposition (`--proposal`), un advert signé (`--advert`), un corps d'annonce optionnel
    (`--advert-body`), époque de rétention et au moins une signature du conseil. Les signatures peuvent être
    fourni en ligne (`--council-signature=<signer_hex:signature_hex>`) ou via fichiers en combinant
    `--council-signature-public-key` avec `--council-signature-file=<path>`.
  - Produit une enveloppe validée (`--envelope-out`) et un rapport JSON indiquant les liaisons de digest,
    le nombre de signataires et les chemins d'entrée.
-`verify`
  - Valider une enveloppe existante (`--envelope`), avec vérification optionnelle de la proposition,
    de l'annonce ou du corps d'annonce correspondant. Le rapport JSON avec en avant les valeurs de digest,
    l'état de vérification de signature et les artefacts optionnels correspondants.
-`renewal`
  - Lie une nouvelle enveloppe approuvée au digest précédemment ratifié. Demander
    `--previous-envelope=<path>` et le successeur `--envelope=<path>` (deux charges utiles Norito).
    Le CLI vérifie que les alias de profil, les capacités et les clés d'annonce restent inchangées,
    tout en autorisant les mises à jour de enjeux, d'endpoints et de métadonnées. Émet les octets canoniques
    `ProviderAdmissionRenewalV1` (`--renewal-out`) ainsi qu'un résumé JSON.
-`revoke`
  - Émet un bundle d'urgence `ProviderAdmissionRevocationV1` pour un fournisseur dont l'enveloppe doit
    être retiré. Requiert `--envelope=<path>`, `--reason=<text>`, au moins une
    `--council-signature`, et optionnellement `--revoked-at`/`--notes`. La CLI signe et valide le
    digest de révocation, écrit le payload Norito via `--revocation-out`, et imprime un rapport JSON
    avec le digest et le nombre de signatures.
| Vérification | Implémenter un vérificateur partagé utilisé par Torii, gateways et `sorafs-node`. Fournir des tests unitaires + d'intégration CLI.【F:crates/sorafs_manifest/src/provider_admission.rs#L1】【F:crates/iroha_torii/src/sorafs/admission.rs#L1】 | Mise en réseau TL / Stockage | ✅Terminé || Intégration Torii | Injecter le vérificateur dans l'ingestion des publicités Torii, rejeter les publicités hors politique, émettre la télémétrie. | Réseautage TL | ✅Terminé | Torii charge désormais les enveloppes de gouvernance (`torii.sorafs.admission_envelopes_dir`), vérifiez les correspondances digest/signature lors de l'ingestion et exposez la télémétrie d'admission.【F:crates/iroha_torii/src/sorafs/admission.rs#L1】【F:crates/iroha_torii/src/sorafs/discovery.rs#L1】【F:crates/iroha_torii/src/sorafs/api.rs#L1】 |
| Renouvellement | Ajouter le schéma de renouvellement/révocation + helpers CLI, publier un guide de cycle de vie dans les docs (voir runbook ci-dessous et commandes CLI `provider-admission renewal`/`revoke`).【crates/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#L477】【docs/source/sorafs/provider_admission_policy.md:120】 | Stockage / Gouvernance | ✅Terminé |
| Télémétrie | définir les tableaux de bord/alertes `provider_admission` (renouvellement manquant, expiration d'enveloppe). | Observabilité | 🟠 En cours | Le compteur `torii_sorafs_admission_total{result,reason}` existe ; tableaux de bord/alertes en attente.【F:crates/iroha_telemetry/src/metrics.rs#L3798】【F:docs/source/telemetry.md#L614】 |

### Runbook de renouvellement et de révocation

#### Renouvellement programmé (mises à jour de enjeux/topologie)
1. Construisez la paire proposition/annonce successeur avec `provider-admission proposal` et `provider-admission sign`, en continuant `--retention-epoch` et en mettant à jour l'enjeu/endpoints si besoin.
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
   La commande valide les champs de capacité/profil définis via
   `AdmissionRecord::apply_renewal`, émet `ProviderAdmissionRenewalV1`, et imprimer les digests pour
   le journal de gouvernance.【crates/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#L477】【F:crates/sorafs_manifest/src/provider_admission.rs#L422】
3. Remplacez l'enveloppe précédente dans `torii.sorafs.admission_envelopes_dir`, commettez le Norito/JSON de renouvellement dans le dépôt de gouvernance, et ajoutez le hash de renouvellement + retention epoch à `docs/source/sorafs/migration_ledger.md`.
4. Notifiez les opérateurs que la nouvelle enveloppe est active et surveillez `torii_sorafs_admission_total{result="accepted",reason="stored"}` pour confirmer l'ingestion.
5. Régénérez et commitez les luminaires canoniques via `cargo run -p sorafs_car --bin provider_admission_fixtures --features cli` ; CI (`ci/check_sorafs_fixtures.sh`) valide que les sorties Norito restent stables.

#### Révocation d'urgence
1. Identifiez l'enveloppe compromise et émettez une révocation :
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
   Le CLI signe `ProviderAdmissionRevocationV1`, vérifiez l'ensemble des signatures via
   `verify_revocation_signatures`, et rapporte le digest de révocation.
2. Supprimez l'enveloppe de `torii.sorafs.admission_envelopes_dir`, distribuez le Norito/JSON de révocation aux caches d'admission, et enregistrez le hash de la raison dans les minutes de gouvernance.
3. Surveillez `torii_sorafs_admission_total{result="rejected",reason="admission_missing"}` pour confirmer que les caches abandonnent l'annonce révoqué ; conserver les artefacts de révocation dans les rétrospectives d'incident.

## Tests et télémétrie- Ajouter des luminaires dorés pour les propositions et enveloppes d'admission sous
  `fixtures/sorafs_manifest/provider_admission/`.
- Étendre le CI (`ci/check_sorafs_fixtures.sh`) pour régénérer les propositions et vérifier les enveloppes.
- Les luminaires générés incluent `metadata.json` avec des résumés canoniques ; les tests en aval valident
  `proposal_digest_hex` == `ca8e73a1f319ae83d7bd958ccb143f9b790c7e4d9c8dfe1f6ad37fa29facf936`.
- Fournir des tests d'intégration :
  - Torii rejette les publicités avec des enveloppes d'admission manquantes ou expirées.
  - Le CLI fait un aller-retour proposition → enveloppe → vérification.
  - Le renouvellement de gouvernance fait pivoter l'attestation d'endpoint sans changer l'ID du fournisseur.
- Exigences de télémétrie :
  - Émettre les compteurs `provider_admission_envelope_{accepted,rejected}` dans Torii. ✅ `torii_sorafs_admission_total{result,reason}` expose désormais les résultats acceptés/rejetés.
  - Ajout des alertes d'expiration dans les tableaux de bord d'observabilité (renouvellement dû dans les 7 jours).

## Prochaines étapes

1. ✅ Finalisé les modifications du schéma Norito et intégré les aides de validation dans
   `sorafs_manifest::provider_admission`. Aucun indicateur de fonctionnalité requis.
2. ✅ Les workflows CLI (`proposal`, `sign`, `verify`, `renewal`, `revoke`) sont documentés et exercés via des tests d'intégration ; gardez les scripts de gouvernance synchronisés avec le runbook.
3. ✅ L'admission/découverte Torii ingère les enveloppes et expose les compteurs de télémétrie d'acceptation/rejet.
4. Focus observabilité : terminer les tableaux de bord/alertes d'admission pour que les renouvellements dus sous sept jours déclenchent des avertissements (`torii_sorafs_admission_total`, jauges d'expiration).