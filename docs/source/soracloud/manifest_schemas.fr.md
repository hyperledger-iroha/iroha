<!-- Auto-generated stub for French (fr) translation. Replace this content with the full translation. -->

---
lang: fr
direction: ltr
source: docs/source/soracloud/manifest_schemas.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a0724ed92da90d8d78a7095b5fc75523ea5dd4a1c885059e28c1bbb8118d1f8c
source_last_modified: "2026-03-26T06:12:11.480497+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

# Schémas du manifeste SoraCloud V1

Cette page définit les premiers schémas déterministes Norito pour SoraCloud
déploiement sur Iroha 3 :

-`SoraContainerManifestV1`
-`SoraServiceManifestV1`
-`SoraStateBindingV1`
-`SoraDeploymentBundleV1`
-`AgentApartmentManifestV1`
-`FheParamSetV1`
-`FheExecutionPolicyV1`
-`FheGovernanceBundleV1`
-`FheJobSpecV1`
-`DecryptionAuthorityPolicyV1`
-`DecryptionRequestV1`
-`CiphertextQuerySpecV1`
-`CiphertextQueryResponseV1`
-`SecretEnvelopeV1`
-`CiphertextStateRecordV1`

Les définitions de Rust se trouvent dans `crates/iroha_data_model/src/soracloud.rs`.

Les enregistrements d'exécution privée du modèle téléchargé constituent intentionnellement une couche distincte de
ces manifestes de déploiement SCR. Ils devraient étendre le plan modèle Soracloud
et réutilisez `SecretEnvelopeV1` / `CiphertextStateRecordV1` pour les octets chiffrés
et état natif du texte chiffré, plutôt que d'être codé en tant que nouveau service/conteneur
se manifeste. Voir `uploaded_private_models.md`.

## Portée

Ces manifestes sont conçus pour le `IVM` + Sora Container Runtime personnalisé.
(SCR) (pas de WASM, pas de dépendance Docker dans l'admission à l'exécution).- `SoraContainerManifestV1` capture l'identité du bundle exécutable, le type d'exécution,
  stratégie de capacité, ressources, paramètres de sonde de cycle de vie et informations explicites
  exportations de configuration requise vers l'environnement d'exécution ou la révision montée
  arbre.
- `SoraServiceManifestV1` capture l'intention de déploiement : identité du service,
  Hachage/version du manifeste du conteneur référencé, routage, stratégie de déploiement et
  liaisons d’État.
- `SoraStateBindingV1` capture la portée et les limites déterministes de l'écriture d'état
  (préfixe d'espace de noms, mode de mutabilité, mode de cryptage, quotas d'articles/total).
- `SoraDeploymentBundleV1` couple conteneur + service manifeste et applique
  contrôles d'admission déterministes (liaison manifeste-hachage, alignement de schéma et
  capacité/cohérence de liaison).
- `AgentApartmentManifestV1` capture la stratégie d'exécution de l'agent persistant :
  plafonds d'outils, plafonds de politique, limites de dépenses, quota d'état, sortie du réseau et
  comportement de mise à niveau.
- `FheParamSetV1` capture les ensembles de paramètres FHE gérés par la gouvernance :
  identifiants déterministes du backend/schéma, profil de module, sécurité/profondeur
  limites et hauteurs du cycle de vie (`activation`/`deprecation`/`withdraw`).
- `FheExecutionPolicyV1` capture les limites d'exécution du texte chiffré déterministe :
  tailles de charge utile admises, ventilateur d'entrée/sortie, plafonds de profondeur/rotation/amorçage,
  et mode d'arrondi canonique.
- `FheGovernanceBundleV1` couple un jeu de paramètres et une politique pour le déterminisme
  validation d'admission.- `FheJobSpecV1` capture l'admission/l'exécution de tâches de texte chiffré déterministe
  requêtes : classe d'opération, engagements d'entrée ordonnés, clé de sortie et limites
  demande de profondeur/rotation/bootstrap liée à un ensemble de politiques + paramètres.
- `DecryptionAuthorityPolicyV1` capture la politique de divulgation gérée par la gouvernance :
  mode d'autorité (service détenu par le client ou service à seuil), quorum/membres approbateurs,
  allocation pour bris de glace, marquage de la juridiction, exigence de preuve de consentement,
  Limites TTL et marquage d’audit canonique.
- `DecryptionRequestV1` capture les tentatives de divulgation liées à une stratégie :
  Référence de clé de texte chiffré (`binding_name` + `state_key` + engagement),
  justification, balise de juridiction, hachage facultatif des preuves de consentement, TTL,
  intention/raison de bris de verre et lien de hachage de gouvernance.
- `CiphertextQuerySpecV1` capture l'intention de requête déterministe uniquement en texte chiffré :
  portée du service/liaison, filtre de préfixe de clé, limite de résultat limitée, métadonnées
  niveau de projection et bascule d’inclusion de preuve.
- `CiphertextQueryResponseV1` capture les résultats de requête à divulgation réduite :
  références de clés orientées résumé, métadonnées de texte chiffré, preuves d'inclusion facultatives,
  et le contexte de troncature/séquence au niveau de la réponse.
- `SecretEnvelopeV1` capture lui-même le contenu chiffré de la charge utile :
  mode de chiffrement, identifiant/version de clé, nom occasionnel, octets de texte chiffré et
  engagements d’intégrité.
- `CiphertextStateRecordV1` capture les entrées d'état natives du texte chiffré quicombiner des métadonnées publiques (type de contenu, balises de stratégie, engagement, taille de la charge utile)
  avec un `SecretEnvelopeV1`.
- Les bundles de modèles privés téléchargés par les utilisateurs doivent s'appuyer sur ces textes chiffrés natifs.
  enregistrements :
  les morceaux chiffrés de poids/configuration/processeur vivent dans l'état, tandis que le registre de modèles,
  la lignée de poids, les profils de compilation, les sessions d'inférence et les points de contrôle restent
  enregistrements Soracloud de première classe.

## Gestion des versions

-`SORA_CONTAINER_MANIFEST_VERSION_V1 = 1`
-`SORA_SERVICE_MANIFEST_VERSION_V1 = 1`
-`SORA_STATE_BINDING_VERSION_V1 = 1`
-`SORA_DEPLOYMENT_BUNDLE_VERSION_V1 = 1`
-`AGENT_APARTMENT_MANIFEST_VERSION_V1 = 1`
-`FHE_PARAM_SET_VERSION_V1 = 1`
-`FHE_EXECUTION_POLICY_VERSION_V1 = 1`
-`FHE_GOVERNANCE_BUNDLE_VERSION_V1 = 1`
-`FHE_JOB_SPEC_VERSION_V1 = 1`
-`DECRYPTION_AUTHORITY_POLICY_VERSION_V1 = 1`
-`DECRYPTION_REQUEST_VERSION_V1 = 1`
-`CIPHERTEXT_QUERY_SPEC_VERSION_V1 = 1`
-`CIPHERTEXT_QUERY_RESPONSE_VERSION_V1 = 1`
-`CIPHERTEXT_QUERY_PROOF_VERSION_V1 = 1`
-`SECRET_ENVELOPE_VERSION_V1 = 1`
-`CIPHERTEXT_STATE_RECORD_VERSION_V1 = 1`

La validation rejette les versions non prises en charge avec
`SoraCloudManifestError::UnsupportedVersion`.

## Règles de validation déterministes (V1)- Manifeste du conteneur :
  - `bundle_path` et `entrypoint` doivent être non vides.
  - `healthcheck_path` (si défini) doit commencer par `/`.
  - `config_exports` ne peut référencer que les configurations déclarées dans
    `required_config_names`.
  - Les cibles d'environnement config-export doivent utiliser des noms de variables d'environnement canoniques
    (`[A-Za-z_][A-Za-z0-9_]*`).
  - les cibles des fichiers de configuration-export doivent rester relatives, utiliser les séparateurs `/` et
    ne doit pas contenir de segments vides, `.` ou `..`.
  - les exportations de configuration ne doivent pas cibler la même variable d'environnement ou le même chemin de fichier relatif.
    qu'une fois.
- Manifeste de service :
  - `service_version` doit être non vide.
  - `container.expected_schema_version` doit correspondre au schéma de conteneur v1.
  - `rollout.canary_percent` doit être `0..=100`.
  - `route.path_prefix` (si défini) doit commencer par `/`.
  - Les noms de liaison d'état doivent être uniques.
- Engagement d'État :
  - `key_prefix` doit être non vide et commencer par `/`.
  -`max_item_bytes <= max_total_bytes`.
  - Les liaisons `ConfidentialState` ne peuvent pas utiliser le chiffrement de texte en clair.
- Bundle de déploiement :
  - `service.container.manifest_hash` doit correspondre à l'encodage canonique
    hachage du manifeste du conteneur.
  - `service.container.expected_schema_version` doit correspondre au schéma du conteneur.
  - Les liaisons d'état mutables nécessitent `container.capabilities.allow_state_writes=true`.
  - Les itinéraires publics nécessitent `container.lifecycle.healthcheck_path`.
- Manifeste d'appartement de l'agent :
  - `container.expected_schema_version` doit correspondre au schéma de conteneur v1.
  - les noms des fonctionnalités des outils doivent être non vides et uniques.- les noms de capacités de stratégie doivent être uniques.
  - les actifs à limite de dépenses doivent être non vides et uniques.
  - `max_per_tx_nanos <= max_per_day_nanos` pour chaque limite de dépenses.
  - La politique réseau de liste blanche doit inclure des hôtes uniques non vides.
- Jeu de paramètres FHE :
  - `backend` et `ciphertext_modulus_bits` doivent être non vides.
  - la taille en bits de chaque module de texte chiffré doit être comprise dans la plage `2..=120`.
  - L'ordre de la chaîne du module de texte chiffré doit être non croissant.
  - `plaintext_modulus_bits` doit être inférieur au plus grand module du texte chiffré.
  -`slot_count <= polynomial_modulus_degree`.
  -`max_multiplicative_depth < ciphertext_modulus_bits.len()`.
  - l'ordre des hauteurs de cycle de vie doit être strict :
    `activation < deprecation < withdraw` lorsqu'il est présent.
  - exigences relatives à l'état du cycle de vie :
    - `Proposed` interdit les hauteurs de dépréciation/retrait.
    - `Active` nécessite `activation_height`.
    - `Deprecated` nécessite `activation_height` + `deprecation_height`.
    - `Withdrawn` nécessite `activation_height` + `withdraw_height`.
- Politique d'exécution FHE :
  -`max_plaintext_bytes <= max_ciphertext_bytes`.
  -`max_output_ciphertexts <= max_input_ciphertexts`.
  - La liaison du jeu de paramètres doit correspondre à `(param_set, version)`.
  - `max_multiplication_depth` ne doit pas dépasser la profondeur paramétrée.
  - L'admission à la stratégie rejette le cycle de vie de l'ensemble de paramètres `Proposed` ou `Withdrawn`.
- Bundle de gouvernance FHE :
  - valide la compatibilité politique + ensemble de paramètres en tant que charge utile d'admission déterministe.
- Spécification du poste FHE :
  - `job_id` et `output_state_key` doivent être non vides (`output_state_key` commence par `/`).- L'ensemble d'entrées doit être non vide et les clés d'entrée doivent être des chemins canoniques uniques.
  - les contraintes spécifiques au fonctionnement sont strictes (multi-entrées `Add`/`Multiply`,
    `RotateLeft`/`Bootstrap` à entrée unique, avec boutons de profondeur/rotation/bootstrap mutuellement exclusifs).
  - l'admission liée à la politique applique :
    - Les identifiants de politique/paramètre et les versions correspondent.
    - Le nombre d'entrées/octets, la profondeur, la rotation et les limites d'amorçage respectent les plafonds de la politique.
    - Les octets de sortie projetés déterministes correspondent aux limites du texte chiffré de la politique.
- Politique d'autorité de décryptage :
  - `approver_ids` doit être non vide, unique et strictement trié lexicographiquement.
  - Le mode `ClientHeld` nécessite exactement un approbateur, `approver_quorum=1`,
    et `allow_break_glass=false`.
  - Le mode `ThresholdService` nécessite au moins deux approbateurs et
    `approver_quorum <= approver_ids.len()`.
  - `jurisdiction_tag` doit être non vide et ne doit pas contenir de caractères de contrôle.
  - `audit_tag` doit être non vide et ne doit pas contenir de caractères de contrôle.
- Demande de décryptage :
  - `request_id`, `state_key` et `justification` doivent être non vides
    (`state_key` commence par `/`).
  - `jurisdiction_tag` doit être non vide et ne doit pas contenir de caractères de contrôle.
  - `break_glass_reason` est requis lorsque `break_glass=true` et doit être omis lorsque
    `break_glass=false`.
  - L'admission liée à la politique applique l'égalité des noms de politique, ne demande pas de TTLdépassant `policy.max_ttl_blocks`, égalité des balises de juridiction, bris de glace
    le contrôle et les exigences en matière de preuves de consentement lorsque
    `policy.require_consent_evidence=true` pour les demandes sans bris de verre.
- Spécification de requête de texte chiffré :
  - `state_key_prefix` doit être non vide et commencer par `/`.
  - `max_results` est borné de manière déterministe (`<=256`).
  - la projection des métadonnées est explicite (`Minimal` digest uniquement vs `Standard` clé visible).
- Réponse à la requête chiffrée :
  - `result_count` doit être égal au nombre de lignes sérialisées.
  - La projection `Minimal` ne doit pas exposer `state_key` ; `Standard` doit l'exposer.
  - les lignes ne doivent jamais apparaître en mode de chiffrement en texte brut.
  - les preuves d'inclusion (lorsqu'elles sont présentes) doivent inclure des identifiants de schéma non vides et
    `anchor_sequence >= event_sequence`.
- Enveloppe secrète :
  - `key_id`, `nonce` et `ciphertext` doivent être non vides.
  - la longueur du occasionnel est limitée (`<=256` octets).
  - la longueur du texte chiffré est limitée (`<=33554432` octets).
- Enregistrement d'état du texte chiffré :
  - `state_key` doit être non vide et commencer par `/`.
  - le type de contenu des métadonnées doit être non vide ; les balises doivent être des chaînes uniques non vides.
  - `metadata.payload_bytes` doit être égal à `secret.ciphertext.len()`.
  - `metadata.commitment` doit être égal à `secret.commitment`.

## Appareils canoniques

Les appareils canoniques JSON sont stockés dans :-`fixtures/soracloud/sora_container_manifest_v1.json`
-`fixtures/soracloud/sora_service_manifest_v1.json`
-`fixtures/soracloud/sora_state_binding_v1.json`
-`fixtures/soracloud/sora_deployment_bundle_v1.json`
-`fixtures/soracloud/agent_apartment_manifest_v1.json`
-`fixtures/soracloud/fhe_param_set_v1.json`
-`fixtures/soracloud/fhe_execution_policy_v1.json`
-`fixtures/soracloud/fhe_governance_bundle_v1.json`
-`fixtures/soracloud/fhe_job_spec_v1.json`
-`fixtures/soracloud/decryption_authority_policy_v1.json`
-`fixtures/soracloud/decryption_request_v1.json`
-`fixtures/soracloud/ciphertext_query_spec_v1.json`
-`fixtures/soracloud/ciphertext_query_response_v1.json`
-`fixtures/soracloud/secret_envelope_v1.json`
-`fixtures/soracloud/ciphertext_state_record_v1.json`

Tests de montage/aller-retour :

-`crates/iroha_data_model/tests/soracloud_manifest_fixtures.rs`