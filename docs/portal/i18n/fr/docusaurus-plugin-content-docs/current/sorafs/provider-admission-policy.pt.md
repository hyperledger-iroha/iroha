---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/provider-admission-policy.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

> Adapté de [`docs/source/sorafs/provider_admission_policy.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_admission_policy.md).

# Politique d'admission et d'identité des fournisseurs SoraFS (Rascunho SF-2b)

Cette note capture les actions entreprises pour **SF-2b** : définir
appliquer le flux d'admission, les exigences d'identité et les charges utiles de
attestation pour les fournisseurs d'armement SoraFS. L'amplificateur ou le processus de haut niveau
Niveau décrit dans la RFC d'architecture de SoraFS et division du travail restant
tarefas de engenharia rastreaveis.

## Objets de la politique

- Garantir que apenas operadores verificados possam publicar registros `ProviderAdvertV1` que a rede aceita.
- Vinculaire chaque fois qu'il annonce un document d'identité approuvé par le gouvernement, les points finaux attestés et la contribution minimale de mise.
- Fornecer ferramentas de verificacao deterministica para que Torii, gateways et `sorafs-node` apliquem as mesmas verificacoes.
- Soutenir la rénovation et la rénovation de l'émergence sans pour autant négliger le déterminisme ou l'ergonomie des ferramentas.

## Conditions requises pour l'identité et l'enjeu

| Requis | Description | Entregavel |
|-----------|-----------|------------|
| Proveniencia da chave de anuncio | Les fournisseurs doivent enregistrer un par de chaves Ed25519 qui assina cada advert. Le paquet d'admissions armazena a chave publica junto com una assinatura de goveranca. | Écrivez le schéma `ProviderAdmissionProposalV1` avec `advert_key` (32 octets) et la référence sans enregistrement (`sorafs_manifest::provider_admission`). |
| Pont de pieu | L'admission demande un `StakePointer` sans aucun dépôt pour un pool de jalonnement actif. | Ajouter la validation dans `sorafs_manifest::provider_advert::StakePointer::validate()` et exporter les erreurs dans CLI/tests. |
| Tags de juridiction | Osprovorees declaram jurisdicao + contato legal. | Estender o esquema de proposition com `jurisdiction_code` (ISO 3166-1 alpha-2) et `contact_uri` facultatif. |
| Test du point final | Chaque point de terminaison annoncé doit être répondu par un rapport certifié mTLS ou QUIC. | Définissez la charge utile Norito `EndpointAttestationV1` et installez-la pour le point de terminaison dans le bundle d'admission. |

## Flux d'admission

1. **Criaçao de la proposition**
   - CLI : ajouter `cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission proposal ...`
     produit `ProviderAdmissionProposalV1` + bundle de testacao.
   - Validation : garantir les champs requis, mise > 0, poignée canonique de chunker em `profile_id`.
2. **Endosso de gouvernance**
   - Le conseil assina `blake3("sorafs-provider-admission-v1" || canonical_bytes)` en utilisant l'outillage de l'enveloppe existante
     (module `sorafs_manifest::governance`).
   - L'enveloppe est persistante dans `governance/providers/<provider_id>/admission.json`.
3. **Ingestao no registro**
   - Mettre en œuvre un vérificateur partagé (`sorafs_manifest::provider_admission::validate_envelope`)
     que Torii/gateways/CLI est réutilisé.
   - Actualiser le chemin d'admission du Torii pour rejeter les publicités lorsque le résumé ou l'expiration est différente de l'enveloppe.
4. **Renovacao e revogacao**
   - Ajouter `ProviderAdmissionRenewalV1` pour actualiser les options de point de terminaison/enjeu.
   - Afficher un chemin CLI `--revoke` qui enregistre le motif de revogacao et envoie un événement de gouvernance.

## Tarifs de mise en œuvre| Zone | Taréfa | Propriétaire(s) | Statut |
|------|--------|----------|--------|
| Esquema | Définissez `ProviderAdmissionProposalV1`, `ProviderAdmissionEnvelopeV1`, `EndpointAttestationV1` (Norito) dans `crates/sorafs_manifest/src/provider_admission.rs`. Implémenté dans `sorafs_manifest::provider_admission` avec les aides de validation.[F:crates/sorafs_manifest/src/provider_admission.rs#L1] | Stockage / Gouvernance | Concluido |
| Ferramentas CLI | Estender `sorafs_manifest_stub` avec sous-commandes : `provider-admission proposal`, `provider-admission sign`, `provider-admission verify`. | GT Outillage | Concluido |

Le flux de CLI agora aceita bundles de certificados intermediarios (`--endpoint-attestation-intermediate`), émet
octets canoniques de la proposition/enveloppe et validation des attributions du conseil pendant `sign`/`verify`. Podème des opérateurs
fornecer corps de advert directement ou réutiliser les publicités assassinées, et les archivistes de assassinat peuvent être
nous vous proposons de combiner `--council-signature-public-key` avec `--council-signature-file` pour faciliter l'automatisation.

### Référence de CLI

Exécutez cada commando via `cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission ...`.-`proposal`
  - Drapeaux requis : `--provider-id=<hex32>`, `--chunker-profile=<namespace.name@semver>`,
    `--stake-pool-id=<hex32>`, `--stake-amount=<amount>`, `--advert-key=<hex32>`,
    `--jurisdiction-code=<ISO3166-1>`, et pour moins un `--endpoint=<kind:host>`.
  - Une attestation du point de terminaison espera `--endpoint-attestation-attested-at=<secs>`,
    `--endpoint-attestation-expires-at=<secs>`, certifié via
    `--endpoint-attestation-leaf=<path>` (mais `--endpoint-attestation-intermediate=<path>`
    optionnel pour chaque élément de la chaîne) et tous les identifiants ALPN négociés
    (`--endpoint-attestation-alpn=<token>`). Endpoints QUIC peut fournir des informations sur les transports avec
    `--endpoint-attestation-report[-hex]=...`.
  - Saida : octets canoniques de la proposition Norito (`--proposal-out`) et un résumé JSON
    (sortie standard padrao ou `--json-out`).
-`sign`
  - Entrées : une proposition (`--proposal`), une annonce assassinée (`--advert`), un corps d'annonce facultatif
    (`--advert-body`), époque de rétention et pour moins une assinatura du conseil. Comme assinaturas podem
    être fourni en ligne (`--council-signature=<signer_hex:signature_hex>`) ou via des archives vers une combinaison
    `--council-signature-public-key` avec `--council-signature-file=<path>`.
  - Produisez une enveloppe validée (`--envelope-out`) et un rapport JSON indiquant les vins du résumé,
    Contagem des assassins et des chemins d'entrée.
-`verify`
  - Valider une enveloppe existante (`--envelope`), avec vérification facultative de la proposition, de l'annonce ou
    corps de correspondant publicitaire. Le rapport JSON sur les valeurs de résumé, l'état de vérification
    de assinaturas e quais artefatos opcionais correspondanteram.
-`renewal`
  - Vincula um enveloppe recem aprovado ao digest previamente ratificado. Demander
    `--previous-envelope=<path>` et le successeur `--envelope=<path>` (charges utiles ambos Norito).
    O CLI vérifie que les alias de profil, les capacités et les chaves de publicité permanents sont modifiés,
    en quanto permet d'actualiser les enjeux, les points finaux et les métadonnées. Émettre les octets canoniques
    `ProviderAdmissionRenewalV1` (`--renewal-out`) mais un résumé JSON.
-`revoke`
  - Émettez un bundle d'émergence `ProviderAdmissionRevocationV1` pour un fournisseur de développement d'enveloppe
    je suis retraité. Demander `--envelope=<path>`, `--reason=<text>`, pour moins que `--council-signature`,
    et les options `--revoked-at`/`--notes`. La CLI assina et valide le résumé de revogacao, écris la charge utile
    Norito via `--revocation-out` et imprimer un résumé JSON avec le résumé et le nombre d'Assinaturas.
| Vérification | Implémenter le vérificateur partagé utilisé par Torii, les passerelles et `sorafs-node`. Prover testes unitarios + de integracao de CLI.[F:crates/sorafs_manifest/src/provider_admission.rs#L1][F:crates/iroha_torii/src/sorafs/admission.rs#L1] | Mise en réseau TL / Stockage | Concluido |
| Intégration Torii | Passer le vérificateur de l'acquisition d'annonces n° Torii, refuser les annonces sur les forums politiques et émettre des télémétries. | Réseautage TL | Concluido | Torii agora carrega enveloppes de gouvernance (`torii.sorafs.admission_envelopes_dir`), vérification des correspondances de digest/assinatura durante a ingestao e expoe telemetria de admissao.[F:crates/iroha_torii/src/sorafs/admission.rs#L1][F:crates/iroha_torii/src/sorafs/discovery.rs#L1][F:crates/iroha_torii/src/sorafs/api.rs#L1] || Rénovation | Ajouter un exemple de rénovation/révocation + helpers de CLI, publier le guide du cycle de vie dans nos documents (voir le runbook abaixo et les commandes CLI dans `provider-admission renewal`/`revoke`).[crates/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#L477][docs/source/sorafs/provider_admission_policy.md:120] | Stockage / Gouvernance | Concluido |
| Télémétrie | Définir les tableaux de bord/alertes `provider_admission` (renovacao ausente, expiracao de enveloppe). | Observabilité | En progrès | Le contacteur `torii_sorafs_admission_total{result,reason}` existe ; tableaux de bord/alertes pendantes.[F:crates/iroha_telemetry/src/metrics.rs#L3798][F:docs/source/telemetry.md#L614] |

### Runbook de rénovation et de rénovation

#### Renovacao agendada (actualisation des enjeux/topologie)
1. Construit ou proposé par le successeur de l'annonce avec `provider-admission proposal` et `provider-admission sign`,
   en augmentant `--retention-epoch` et en ajustant les enjeux/points de terminaison conformément aux exigences.
2. Exécuter
   ```bash
   cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission \
     renewal \
     --previous-envelope=governance/providers/<id>/envelope.to \
     --envelope=governance/providers/<id>/envelope_next.to \
     --renewal-out=governance/providers/<id>/renewal.to \
     --json-out=governance/providers/<id>/renewal.json \
     --notes="stake top-up 2025-03"
   ```
   Le commandant valide les champs de capacité/profils modifiés via `AdmissionRecord::apply_renewal`,
   Émettez `ProviderAdmissionRenewalV1` et imprimez des résumés pour le journal de gouvernance.
3. Remplacer l'enveloppe antérieure par `torii.sorafs.admission_envelopes_dir`, confirmer le Norito/JSON de rénovation
   pas de référentiel de gouvernance et d'ajout de hachage de rénovation + époque de rétention à `docs/source/sorafs/migration_ledger.md`.
4. Notifier les opérateurs que la nouvelle enveloppe est active et surveillée
   `torii_sorafs_admission_total{result="accepted",reason="stored"}` pour confirmer l'ingestion.
5. Régénérez et confirmez les appareils canoniques via `cargo run -p sorafs_car --bin provider_admission_fixtures --features cli` ;
   CI (`ci/check_sorafs_fixtures.sh`) valide comme indiqué Norito en permanence.

#### Revogação de émergence
1. Identifier l'enveloppe compromise et émettre une modification :
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
   O CLI assina o `ProviderAdmissionRevocationV1`, vérifiez le conjunto de assinaturas via
   `verify_revocation_signatures` et relatif au résumé de revogacao.[crates/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#L593][F:crates/sorafs_manifest/src/provider_admission.rs#L486]
2. Supprimez l'enveloppe de `torii.sorafs.admission_envelopes_dir`, distribuez le Norito/JSON de revogacao pour les caches
   de admission e registre o hash do motivation nas atas de gouvernance.
3. Observez `torii_sorafs_admission_total{result="rejected",reason="admission_missing"}` pour confirmer que le système
   caches descartaram ou advert revogado; mantenha os artefatos de revogacao nas retrospectivas de incidents.

## Testicules et télémétrie- Ajouter des luminaires dorés pour les propositions et les enveloppes d'admission
  `fixtures/sorafs_manifest/provider_admission/`.
- Estender CI (`ci/check_sorafs_fixtures.sh`) pour régénérer les propositions et vérifier les enveloppes.
- Les appareils d'exploitation gerados incluent `metadata.json` avec les résumés canonicos ; testicules en aval confirmer
  `proposal_digest_hex` == `ca8e73a1f319ae83d7bd958ccb143f9b790c7e4d9c8dfe1f6ad37fa29facf936`.
- Prouvez les tests d'intégration :
  - Torii rejeita adverts com enveloppes de admissao ausentes ou expirados.
  - O CLI faz aller-retour de proposition -> enveloppe -> vérification.
  - Une rotation de gouvernance renouvelée de l'attestation de point final sans modification de l'ID du fournisseur.
- Conditions requises pour la télémétrie :
  - Émettre des contadores `provider_admission_envelope_{accepted,rejected}` dans Torii. `torii_sorafs_admission_total{result,reason}` il y a quelques jours, les résultats ont été acceptés/rejetés.
  - Ajout d'alertes d'expiration aux tableaux de bord d'observation (rénovation prévue dans 7 jours).

## Passons à proximité

1. En remplacement du formulaire Norito finalisé et des assistants de validation incorporés au forum `sorafs_manifest::provider_admission`. Nao ha présente des drapeaux.
2. Les flux CLI (`proposal`, `sign`, `verify`, `renewal`, `revoke`) sont documentés et exercés via des tests d'intégration ; Les scripts de gouvernance doivent être synchronisés avec le runbook.
3. Torii admission/découverte contient les enveloppes et l'exposition des contadores de télémétrie pour l'acitacao/rejeicao.
4. Foco em observabilidade: conclure des tableaux de bord/alertes d'admission pour que les rénovations soient effectuées à l'intérieur de six jours d'avis disparem (`torii_sorafs_admission_total`, jauges d'expiration).