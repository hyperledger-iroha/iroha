---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/provider-admission-policy.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

> Adapté de [`docs/source/sorafs/provider_admission_policy.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_admission_policy.md).

# Politique d'admission et d'identité des fournisseurs SoraFS (Borrador SF-2b)

Cette note capture les éléments accionables pour **SF-2b** : définir
appliquer le flux d'admission, les conditions d'identité et les charges utiles de
attestado para provenedores de almacenamiento SoraFS. Augmenter le processus de haut
Niveau décrit dans le RFC de Arquitectura de SoraFS et diviser le travail restant
en tareas de ingeniería trazables.

## Objets de la politique

- Garantizar que solo operadores verificados puedan publicar registros
  `ProviderAdvertV1` que le rouge acceptera.
- Vinculaire cada clave de anuncio a un documento de identidad approuvé por la
  gouvernance, points finaux attestés et une contribution minimale de mise.
- Outil de vérification déterminant pour Torii, les passerelles et
  `sorafs-node` appliquer les mêmes commandes.
- Soutenir la rénovation et la révocation de l'émergence sans rompre le déterminisme ni
  l'ergonomie de l'outillage.

## Conditions requises pour l'identité et l'enjeu

| Requis | Description | Entregable |
|---------------|-------------|------------|
| Procédure de la clé d'annonce | Les fournisseurs doivent enregistrer un par de claves Ed25519 qui ferme cada advert. Le paquet d'admission héberge la classe publique avec une entreprise d'État. | Extender l'esquema `ProviderAdmissionProposalV1` avec `advert_key` (32 octets) et référence à partir du registre (`sorafs_manifest::provider_admission`). |
| Porte-enjeu | L'admission nécessite un `StakePointer` sans aucun dépôt pour un pool de jalonnement actif. | Ajouter la validation en `sorafs_manifest::provider_advert::StakePointer::validate()` et les erreurs d'exponeur en CLI/tests. |
| Étiquettes de juridiction | Les fournisseurs déclarent juridiction + contact légal. | Extension de l'image proposée avec `jurisdiction_code` (ISO 3166-1 alpha-2) et `contact_uri` en option. |
| Attestation du point de terminaison | Chaque point de terminaison annoncé doit être répondu par un rapport certifié mTLS ou QUIC. | Définissez la charge utile Norito `EndpointAttestationV1` et enregistrez-la pour le point de terminaison dans le bundle d'admission. |

## Flux d'admission

1. **Création de propriété**
   - CLI : ajouter `cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission proposal ...`
     producteur `ProviderAdmissionProposalV1` + bundle de attestado.
   - Validation : assurer les champs requis, mise > 0, poignée canónico de chunker en `profile_id`.
2. **Endoso de gobernanza**
   - Le conseiller de l'entreprise `blake3("sorafs-provider-admission-v1" || canonical_bytes)` utilise l'outillage
     de l'enveloppe existante (module `sorafs_manifest::governance`).
   - L'enveloppe persiste en `governance/providers/<provider_id>/admission.json`.
3. **Insérer le registre**
   - Mettre en œuvre un vérificateur compartimenté (`sorafs_manifest::provider_admission::validate_envelope`)
     que Torii/gateways/CLI est réutilisé.
   - Actualiser l'itinéraire d'admission en Torii pour récupérer des publicités afin de digérer ou d'expiration difiera l'enveloppe.
4. **Rénovation et révocation**
   - Ajouter `ProviderAdmissionRenewalV1` avec les mises à jour optionnelles du point de terminaison/enjeu.
   - Exposer une route CLI `--revoke` pour enregistrer le motif de révocation et envoyer un événement d'administration.

## Zones de mise en œuvre| Zone | Tarée | Propriétaire(s) | État |
|------|-------|----------|--------|
| Esquema | Définir `ProviderAdmissionProposalV1`, `ProviderAdmissionEnvelopeV1`, `EndpointAttestationV1` (Norito) sous `crates/sorafs_manifest/src/provider_admission.rs`. Implémenté en `sorafs_manifest::provider_admission` avec des aides de validation.【F:crates/sorafs_manifest/src/provider_admission.rs#L1】 | Stockage / Gouvernance | ✅ Terminé |
| CLI d'outillage | Extender `sorafs_manifest_stub` avec sous-commandes : `provider-admission proposal`, `provider-admission sign`, `provider-admission verify`. | GT Outillage | ✅ |

Le flux de CLI accepte désormais les bundles de certificats intermédiaires (`--endpoint-attestation-intermediate`), émet des octets canoniques de propriété/enveloppe et valide les entreprises du contrat pendant `sign`/`verify`. Les opérateurs peuvent fournir des corps de publicité directement ou réutiliser des publicités d'entreprise, et les archives d'entreprise peuvent combiner `--council-signature-public-key` avec `--council-signature-file` pour faciliter l'automatisation.

### Référence de CLI

Exécutez chaque commande via `cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission ...`.-`proposal`
  - Indicateurs requis : `--provider-id=<hex32>`, `--chunker-profile=<namespace.name@semver>`,
    `--stake-pool-id=<hex32>`, `--stake-amount=<amount>`, `--advert-key=<hex32>`,
    `--jurisdiction-code=<ISO3166-1>`, et au moins un `--endpoint=<kind:host>`.
  - L'attestation du point de terminaison espera `--endpoint-attestation-attested-at=<secs>`,
    `--endpoint-attestation-expires-at=<secs>`, un certificat via
    `--endpoint-attestation-leaf=<path>` (plus `--endpoint-attestation-intermediate=<path>`
    en option pour chaque élément de la chaîne) et tout ID ALPN négocié
    (`--endpoint-attestation-alpn=<token>`). Endpoints QUIC peut signaler des rapports de transport avec
    `--endpoint-attestation-report[-hex]=...`.
  - Sortie : octets canoniques de propriété Norito (`--proposal-out`) et un résumé JSON
    (sortie standard par défaut sur `--json-out`).
-`sign`
  - Entrées : une proposition (`--proposal`), une annonce confirmée (`--advert`), un corps d'annonce facultatif
    (`--advert-body`), époque de rétention et au moins une entreprise du conseil. Les entreprises peuvent
    gérer en ligne (`--council-signature=<signer_hex:signature_hex>`) ou via les archives combinées
    `--council-signature-public-key` avec `--council-signature-file=<path>`.
  - Produire une enveloppe validée (`--envelope-out`) et un rapport JSON indiquant les liaisons de résumé,
    conteo de firmantes y rutas de entrada.
-`verify`
  - Valider une enveloppe existante (`--envelope`), avec vérification facultative de la propriété,
    annonce ou corps de correspondant publicitaire. Le rapport JSON sur les valeurs du résumé, estado
    de verificación de firmas y qué artefactos opcionales coïncideron.
-`renewal`
  - Vincula une enveloppe reçue approuvée par le digest préalablement ratifié. Exiger
    `--previous-envelope=<path>` et le successeur `--envelope=<path>` (charges utiles ambos Norito).
    La CLI vérifie que les alias de profil, les capacités et les clés de publicité peuvent être permanents sans changement,
    Les moments permettent d'actualiser les enjeux, les points finaux et les métadonnées. Émettre les octets canoniques
    `ProviderAdmissionRenewalV1` (`--renewal-out`) plus une reprise JSON.
-`revoke`
  - Émettez un bundle d'urgence `ProviderAdmissionRevocationV1` pour un fournisseur qui reçoit l'enveloppe nécessaire
    prendre sa retraite. Nécessite `--envelope=<path>`, `--reason=<text>`, au moins un
    `--council-signature`, et facultativement `--revoked-at`/`--notes`. La CLI est ferme et valide la
    résumé de révocation, écrivez la charge utile Norito via `--revocation-out` et imprimez un rapport JSON
    avec le digest et le conteo de firmas.
| Vérification | Implémenter le vérificateur partagé utilisé par Torii, les passerelles et `sorafs-node`. Prouvez les essais unitaires + l'intégration de CLI.【F:crates/sorafs_manifest/src/provider_admission.rs#L1】【F:crates/iroha_torii/src/sorafs/admission.rs#L1】 | Mise en réseau TL / Stockage | ✅ Terminé || Intégration Torii | Câbler le vérificateur lors de l'ingestion de publicités en Torii, rechercher des publicités à des fins politiques et émettre des télémétries. | Réseautage TL | ✅ Terminé | Torii maintenant chargé des enveloppes de gouvernement (`torii.sorafs.admission_envelopes_dir`), vérifier les coïncidences de digestion/entreprise pendant l'ingestion et exposer la télémétrie de admission.【F:crates/iroha_torii/src/sorafs/admission.rs#L1】【F:crates/iroha_torii/src/sorafs/discovery.rs#L1】【F:crates/iroha_torii/src/sorafs/api.rs#L1】 |
| Rénovation | Ajouter un exemple de rénovation/révocation + helpers de CLI, publier le guide du cycle de vie dans la documentation (voir runbook ci-dessous et commandes CLI en `provider-admission renewal`/`revoke`).【crates/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#L477】【docs/source/sorafs/provider_admission_policy.md:120】 | Stockage / Gouvernance | ✅ Terminé |
| Télémétrie | Définir les tableaux de bord/alertes `provider_admission` (rénovation incorrecte, expiration de l'enveloppe). | Observabilité | 🟠 En cours | Le contacteur `torii_sorafs_admission_total{result,reason}` existe ; tableaux de bord/alertes pendantes.【F:crates/iroha_telemetry/src/metrics.rs#L3798】【F:docs/source/telemetry.md#L614】 |

### Runbook de rénovation et de révocation

#### Rénovation programmée (actualisation de l'enjeu/topologie)
1. Construisez le par propuesta/annonce successeur avec `provider-admission proposal` et `provider-admission sign`, incrémentez `--retention-epoch` et actualisez la mise/les points finaux selon vos besoins.
2. Exécution
   ```bash
   cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission \
     renewal \
     --previous-envelope=governance/providers/<id>/envelope.to \
     --envelope=governance/providers/<id>/envelope_next.to \
     --renewal-out=governance/providers/<id>/renewal.to \
     --json-out=governance/providers/<id>/renewal.json \
     --notes="stake top-up 2025-03"
   ```
   La commande valide les champs de capacité/perfil sans changement via
   `AdmissionRecord::apply_renewal`, émet `ProviderAdmissionRenewalV1` et imprime des résumés pour le
   log de gobernanza.【crates/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#L477】【F:crates/sorafs_manifest/src/provider_admission.rs#L422】
3. Remplacez l'enveloppe précédente en `torii.sorafs.admission_envelopes_dir`, confirmez le Norito/JSON de rénovation dans le référentiel d'administration et agrégez le hachage de rénovation + l'époque de rétention à `docs/source/sorafs/migration_ledger.md`.
4. Avertir les opérateurs que la nouvelle enveloppe est activée et surveillée `torii_sorafs_admission_total{result="accepted",reason="stored"}` pour confirmer l'ingestion.
5. Régénérer et confirmer les appareils canoniques via `cargo run -p sorafs_car --bin provider_admission_fixtures --features cli` ; CI (`ci/check_sorafs_fixtures.sh`) valide les sorties Norito permanentes.

#### Révocation d'urgence
1. Identifiez l'enveloppe compromise et émettez une révocation :
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
   L'entreprise CLI `ProviderAdmissionRevocationV1`, vérifiez l'ensemble des entreprises via
   `verify_revocation_signatures`, et rapporte le résumé de la révocation.
2. Retirez l'enveloppe de `torii.sorafs.admission_envelopes_dir`, distribuez le Norito/JSON de révocation aux caches d'admission et enregistrez le hachage du motif dans les actes de gouvernement.
3. Observez `torii_sorafs_admission_total{result="rejected",reason="admission_missing"}` pour confirmer que les éléments cachés suppriment l'annonce supprimée ; conserver les artefacts de révocation dans les rétrospectives d'incidents.

## Tests et télémétrie- Ajouter des luminaires dorés pour les propositions et les enveloppes d'admission basse
  `fixtures/sorafs_manifest/provider_admission/`.
- Extender CI (`ci/check_sorafs_fixtures.sh`) pour régénérer les propriétés et vérifier les enveloppes.
- Les luminaires générés incluent `metadata.json` avec des résumés canoniques ; pruebas en aval afirman
  `proposal_digest_hex` == `ca8e73a1f319ae83d7bd958ccb143f9b790c7e4d9c8dfe1f6ad37fa29facf936`.
- Prouvez les essais d'intégration :
  - Torii rechaza adverts con enveloppes de admission faltantes ou expirados.
  - El CLI fait un aller-retour de propuesta → enveloppe → vérification.
  - La rénovation de la gouvernance tourne autour de l'attestation du point final sans modifier l'ID du fournisseur.
- Conditions requises pour la télémétrie :
  - Émettre des contadores `provider_admission_envelope_{accepted,rejected}` et Torii. ✅ `torii_sorafs_admission_total{result,reason}` ahora expone resultados acceptés/rejetés.
  - Ajouter des alertes d'expiration aux tableaux de bord d'observabilité (rénovation prévue dans 7 jours).

## Nous passons bientôt

1. ✅ Finalisez les modifications du schéma Norito et incorporez les aides de validation dans
   `sorafs_manifest::provider_admission`. Aucun indicateur de fonctionnalité n'est requis.
2. ✅ Les flux CLI (`proposal`, `sign`, `verify`, `renewal`, `revoke`) sont documentés et exécutés via des essais d'intégration ; gardez les scripts de gestion synchronisés avec le runbook.
3. ✅ Torii admission/découverte ingère les enveloppes et expose les contadores de télémétrie pour acceptation/rechazo.
4. Place en observabilité : Terminer les tableaux de bord/alertes d'admission pour les rénovations nécessaires à l'intérieur de chaque jour disparen avisos (`torii_sorafs_admission_total`, jauges d'expiration).