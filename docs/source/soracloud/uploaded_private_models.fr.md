<!-- Auto-generated stub for French (fr) translation. Replace this content with the full translation. -->

---
lang: fr
direction: ltr
source: docs/source/soracloud/uploaded_private_models.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 97d6a421ce93a0e85be6cc99e828f965c9d8617d0ee27a772a2c9f2f646e77b7
source_last_modified: "2026-03-24T18:59:46.535846+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

# Modèles téléchargés par l'utilisateur Soracloud et environnement d'exécution privé

Cette note définit comment le flux de modèle téléchargé par l'utilisateur de So Ra doit atterrir sur le
plan modèle Soracloud existant sans inventer un environnement d'exécution parallèle.

## Objectif de conception

Ajoutez un système de modèles téléchargés uniquement Soracloud qui permet aux clients :

- télécharger leurs propres référentiels de modèles ;
- lier une version de modèle épinglée à un appartement d'agent ou à une équipe d'arène fermée ;
- exécuter une inférence privée avec des entrées chiffrées et un modèle/état chiffré ; et
- recevoir les engagements publics, les reçus, les prix et les pistes d'audit.

Il ne s'agit pas d'une fonctionnalité `ram_lfe`. `ram_lfe` reste la fonction cachée générique
sous-système documenté dans `../universal_accounts_guide.md`. Modèle téléchargé
l'inférence privée devrait plutôt étendre le registre de modèles existant de Soracloud,
Surfaces d’artefact, de capacité d’appartement, de FHE et de politique de décryptage.

## Surfaces Soracloud existantes à réutiliser

La pile Soracloud actuelle possède déjà les bons objets de base :-`SoraModelRegistryV1`
  - Nom du modèle par service faisant autorité et état de la version promue.
-`SoraModelWeightVersionRecordV1`
  - lignée des versions, promotion, restauration, provenance et reproductibilité
    hachages.
-`SoraModelArtifactRecordV1`
  - métadonnées d'artefact déterministes déjà liées au pipeline modèle/poids.
-`SoraCapabilityPolicyV1.allow_model_inference`
  - indicateur de capacité d'appartement/service qui devrait devenir obligatoire pour
    appartements liés aux modèles téléchargés.
-`SecretEnvelopeV1` et `CiphertextStateRecordV1`
  - porteurs déterministes d'octets chiffrés et d'état de texte chiffré.
-`FheParamSetV1`, `FheExecutionPolicyV1`, `FheGovernanceBundleV1`,
  `DecryptionAuthorityPolicyV1` et `DecryptionRequestV1`
  - la couche politique/gouvernance pour une exécution chiffrée et une sortie contrôlée
    libération.
- itinéraires actuels du modèle Torii :
  -`/v1/soracloud/model/weight/{register,promote,rollback,status}`
  -`/v1/soracloud/model/artifact/{register,status}`
- les routes actuelles en location partagée Torii HF :
  -`/v1/soracloud/hf/{deploy,status,lease/leave,lease/renew}`

Le chemin du modèle téléchargé doit étendre ces surfaces. Il ne faut pas surcharger
Baux partagés HF, et il ne doit pas réutiliser `ram_lfe` comme environnement d’exécution de diffusion de modèles.

## Contrat de téléchargement canonique

Le contrat de modèle privé téléchargé de Soracloud ne devrait admettre que des informations canoniques
Dépôts de modèles de style Hugging Face :- fichiers de base requis :
  -`config.json`
  - fichiers de tokeniseur
  - les fichiers processeur/préprocesseur lorsque la famille en a besoin
  -`*.safetensors`
- groupes familiaux admis dans cette étape :
  - LM causals uniquement décodeurs avec sémantique RoPE/RMSNorm/SwiGLU/GQA
  - Modèles texte + image de style LLaVA
  - Modèles texte + image de style Qwen2-VL
- rejeté dans ce jalon :
  - GGUF comme contrat d'exécution privée téléchargé
  -ONNX
  - actifs de tokeniseur/processeur manquants
  - architectures/formes non prises en charge
  - des forfaits multimodaux audio/vidéo

Pourquoi ce contrat :

- il correspond à la disposition du modèle côté serveur déjà dominant utilisé par l'existant
  écosystème autour des safetensors et des dépôts Hugging Face ;
- il permet au plan modèle de partager un chemin de normalisation déterministe à travers
  Donc Ra, Torii et compilation d'exécution ; et
- cela évite de confondre les formats d'importation d'exécution locale tels que GGUF avec le
  Contrat d'exécution privée Soracloud.

Les baux partagés HF restent utiles pour les workflows d’importation de sources partagées/publiques, mais
le chemin du modèle téléchargé privé stocke les octets compilés cryptés sur la chaîne plutôt que
que de louer des octets de modèle à partir d’un pool source partagé.

## Conception de plan modèle en couches

### 1. Couche de provenance et de registre

Étendez la conception actuelle du registre de modèles au lieu de la remplacer :- ajouter `SoraModelProvenanceKindV1::UserUpload`
  - les genres actuels (`TrainingJob`, `HfImport`) ne suffisent pas à distinguer un
    modèle téléchargé et normalisé directement par un client tel que So Ra.
- conserver `SoraModelRegistryV1` comme index de version promue.
- conserver `SoraModelWeightVersionRecordV1` comme enregistrement de lignée/promotion/annulation.
- étendre `SoraModelArtifactRecordV1` avec un runtime privé téléchargé en option
  références:
  -`private_bundle_root`
  -`chunk_manifest_root`
  -`compile_profile_hash`
  -`privacy_mode`

Le registre des artefacts reste le point d'ancrage déterministe qui relie la provenance,
métadonnées de reproductibilité et regrouper l'identité. Le record de poids
reste l'objet de lignage de la version promue.

### 2. Couche de stockage bundle/morceau

Ajoutez des enregistrements Soracloud de première classe pour le matériel de modèle téléchargé chiffré :

-`SoraUploadedModelBundleV1`
  -`model_id`
  -`weight_version`
  -`family`
  -`modalities`
  -`runtime_format`
  -`bundle_root`
  -`chunk_count`
  -`plaintext_bytes`
  -`ciphertext_bytes`
  -`compile_profile_hash`
  -`pricing_policy`
  -`decryption_policy_ref`
-`SoraUploadedModelChunkV1`
  -`model_id`
  -`bundle_root`
  -`ordinal`
  -`offset_bytes`
  -`plaintext_len`
  -`ciphertext_len`
  -`ciphertext_hash`
  - charge utile cryptée (`SecretEnvelopeV1`)

Règles déterministes :- les octets de texte en clair sont fragmentés en morceaux fixes de 4 Mio avant le chiffrement ;
- l'ordre des fragments est strict et ordinal ;
- Les résumés de fragments/racines sont stables pendant la relecture ; et
- chaque fragment chiffré doit rester en dessous du `SecretEnvelopeV1` actuel
  plafond de texte chiffré d’octets `33,554,432`.

Ce jalon stocke les octets chiffrés littéraux dans l'état de la chaîne via un bloc
enregistrements. Il ne décharge pas les octets du modèle privé téléchargé vers SoraFS.

La chaîne étant publique, la confidentialité du téléchargement doit provenir d'une source réelle.
Clé de destinataire détenue par Soracloud, et non issue de clés déterministes dérivées de clés publiques
métadonnées. Le bureau doit récupérer un destinataire de chiffrement de téléchargement annoncé,
chiffrer les morceaux sous une clé de bundle aléatoire par téléchargement et publier uniquement le
métadonnées du destinataire plus enveloppe de clé de paquet enveloppée à côté du texte chiffré.

### 3. Couche de compilation/d'exécution

Ajoutez un compilateur/couche d'exécution de transformateur privé dédié sous Soracloud :- standardiser l'inférence compilée déterministe de faible précision basée sur BFV pour
  maintenant, parce que CKKS existe dans la discussion de schéma mais pas dans la version implémentée
  exécution locale ;
- compiler les modèles admis dans un IR privé Soracloud déterministe qui couvre
  intégrations, couches linéaires/projecteurs, attention matmuls, RoPE, RMSNorm /
  approximations LayerNorm, blocs MLP, projection de patch de vision et
  chemins de projecteur image-décodeur ;
- utiliser l'inférence déterministe en virgule fixe avec :
  - poids int8
  - activations int16
  - accumulation int32
  - approximations polynomiales approuvées pour les non-linéarités

Ce compilateur/runtime est distinct de `ram_lfe`. Il peut réutiliser les primitives BFV
et les objets de gouvernance Soracloud FHE, mais ce n'est pas le même moteur d'exécution
ou itinéraire familial.

### 4. Couche d'inférence/session

Ajoutez des enregistrements de session et de point de contrôle pour les exécutions privées :-`SoraPrivateCompileProfileV1`
  -`family`
  -`quantization`
  -`opset_version`
  -`max_context`
  -`max_images`
  -`vision_patch_policy`
  -`fhe_param_set`
  -`execution_policy`
-`SoraPrivateInferenceSessionV1`
  -`session_id`
  -`apartment`
  -`model_id`
  -`weight_version`
  -`bundle_root`
  -`input_commitments`
  -`token_budget`
  -`image_budget`
  -`status`
  -`receipt_root`
  -`xor_cost_nanos`
-`SoraPrivateInferenceCheckpointV1`
  -`session_id`
  -`step`
  -`ciphertext_state_root`
  -`receipt_hash`
  -`decrypt_request_id`
  -`released_token`
  -`compute_units`
  -`updated_at_ms`

L'exécution privée signifie :

- entrées d'invite/image cryptées ;
- poids et activations de modèles cryptés ;
- publication explicite de la politique de décryptage pour la sortie ;
- recettes publiques d'exécution et comptabilité analytique.

Cela ne signifie pas une exécution cachée sans engagements ni auditabilité.

## Responsabilités des clients

Ra ou un autre client doit donc effectuer un prétraitement local déterministe avant
le téléchargement atteint Soracloud :

- application de tokenisation ;
- prétraitement d'image en tenseurs de patch pour les familles texte+image admises ;
- normalisation déterministe des bundles ;
- Chiffrement côté client des identifiants de jetons et des tenseurs de correctifs d'image.

Torii doit recevoir des entrées cryptées ainsi que des engagements publics, et non une invite brute
texte ou images brutes, pour le chemin privé.

## Plan API et ISIConservez les routes de registre de modèles existantes comme couche de registre canonique et ajoutez
nouvelles routes de téléchargement/d'exécution en haut :

-`POST /v1/soracloud/model/upload/init`
-`POST /v1/soracloud/model/upload/chunk`
-`POST /v1/soracloud/model/upload/finalize`
-`GET /v1/soracloud/model/upload/encryption-recipient`
-`POST /v1/soracloud/model/compile`
-`POST /v1/soracloud/model/allow`
-`POST /v1/soracloud/model/run-private`
-`GET /v1/soracloud/model/run-status`
-`POST /v1/soracloud/model/decrypt-output`

Soutenez-les avec les ISI Soracloud correspondants :

- enregistrement du forfait
- ajout/finalisation d'un morceau
- compiler les admissions
- départ en course privée
- enregistrement du point de contrôle
- sortie de sortie

Le flux doit être :

1. upload/init établit la session de bundle déterministe et la racine attendue ;
2. Le téléchargement/morceau ajoute les fragments chiffrés dans l'ordre ordinal ;
3. télécharger/finaliser scelle la racine et le manifeste du bundle ;
4. compile produit un profil de compilation privé déterministe lié au
   forfait admis;
5. Les enregistrements de registre modèle/artefact + modèle/poids font référence à l'ensemble téléchargé
   plutôt qu'un simple travail de formation ;
6. allow-model lie le modèle téléchargé à un appartement qui l'admet déjà
   `allow_model_inference` ;
7. run-private enregistre une session et émet des points de contrôle/reçus ;
8. Les versions de décryptage sont régies par le matériel de sortie.

## Politique de tarification et de plan de contrôle

Étendez le comportement actuel du plan de charge/de contrôle de Soracloud :- `allow_model_inference` est requis pour les appartements exécutant des modèles téléchargés ;
- stockage des prix, compilation, étapes d'exécution et version de décryptage dans XOR ;
- garder la propagation narrative désactivée pour les exécutions de modèles téléchargés au cours de cette étape ;
- conservez les modèles téléchargés dans l'arène fermée de So Ra et dans les flux limités à l'exportation.

## Autorisation et sémantique de liaison

Le téléchargement, la compilation et l'exécution sont des fonctionnalités distinctes et doivent rester distinctes dans
l'avion modèle.

- le téléchargement d'un bundle de modèles ne doit pas implicitement autoriser un appartement à le gérer ;
- le succès de la compilation ne doit pas implicitement promouvoir une version du modèle à l'état actuel ;
- la liaison d'appartement doit être explicite via une mutation de style `allow-model`
  qui enregistre :
  - appartement,
  - identifiant du modèle,
  - version poids,
  - racine du paquet,
  - mode confidentialité,
  - séquence signataire/audit ;
- les appartements liés aux modèles téléchargés doivent déjà admettre
  `allow_model_inference` ;
- les routes de mutation devraient continuer à nécessiter la même demande signée Soracloud
  discipline utilisée par les itinéraires de modèle/artefact/formation existants et devrait être
  gardé par `CanManageSoracloud` ou une autorité déléguée tout aussi explicite
  modèle.

Cela évite "Je l'ai téléchargé, donc chaque appartement privé peut l'exécuter"
dérive et maintient la politique d’exécution des appartements explicite.

## Statut et modèle d'audit

Les nouveaux enregistrements ont besoin de surfaces de lecture et d'audit faisant autorité, et pas seulement d'une mutation
itinéraires.

Ajouts recommandés :- statut de téléchargement
  - requête par `service_name + model_name + weight_version` ou par
    `model_id + bundle_root` ;
- état de compilation
  - requête par `model_id + bundle_root + compile_profile_hash` ;
- statut d'exécution privée
  - requête par `session_id`, avec contexte appartement/modèle/version inclus dans le
    réponse ;
- état de sortie de déchiffrement
  - requête par `decrypt_request_id`.

L'audit doit rester sur la séquence globale Soracloud existante plutôt que sur
créer un deuxième compteur par fonctionnalité. Ajoutez des événements d'audit de première classe pour :

- télécharger init / finaliser
- ajouter/sceller un morceau
- compilation admise / compilation rejetée
- modèle d'appartement autoriser / révoquer
- démarrage/point de contrôle/achèvement/échec de l'exécution privée
- libération / refus de sortie

Cela permet de garder l'activité du modèle téléchargé visible dans la même rediffusion faisant autorité et
histoire des opérations comme le service actuel, la formation, le poids du modèle, le modèle-artefact,
Flux de baux partagés et d’audit d’appartements HF.

## Quotas d'admission et limites de croissance de l'État

Les octets de modèle chiffrés littéraux sur la chaîne ne sont viables que si l'admission est limitée
de manière agressive.

La mise en œuvre doit définir des limites déterministes pour au moins :

- nombre maximal d'octets de texte en clair par paquet téléchargé ;
- nombre maximum d'octets chiffrés par paquet ;
- nombre maximum de morceaux par paquet ;
- nombre maximum de sessions de téléchargement simultanées en vol par autorité/service ;
- nombre maximum de tâches de compilation par fenêtre de service/appartement ;
- nombre maximum de points de contrôle conservés par session privée ;
- nombre maximum de demandes de sortie-libération par session.Torii et Core doivent rejeter les téléchargements qui dépassent les limites déclarées auparavant.
une amplification d’état se produit. Les limites doivent être basées sur la configuration lorsque
approprié, mais les résultats de la validation doivent rester déterministes entre pairs.

## Replay et déterminisme du compilateur

Le chemin d'exécution/compilateur privé a une charge de déterminisme plus élevée qu'un chemin d'exécution normal.
déploiement de services.

Invariants requis :

- la détection et la normalisation des familles doivent produire un paquet canonique stable
  avant qu'un hachage de compilation ne soit émis ;
- les hachages de profil de compilation doivent lier :
  - racine de bundle normalisée,
  - la famille,
  - recette de quantification,
  - version opset,
  - Jeu de paramètres FHE,
  - politique d'exécution ;
- le runtime doit éviter les noyaux non déterministes, la dérive en virgule flottante et
  réductions spécifiques au matériel qui pourraient modifier les sorties ou les recettes à travers
  pairs.

Avant de mettre à l'échelle l'ensemble familial admis, posez un petit appareil déterministe pour
chaque classe familiale et verrouiller les sorties de compilation ainsi que les reçus d'exécution avec Golden
essais.

## Lacunes de conception restantes avant le code

Les plus grandes questions de mise en œuvre non résolues se limitent désormais au concret
décisions back-end :- formes exactes de téléchargement/morceau/demande DTO et schémas Norito ;
- clés d'indexation d'état mondial pour les recherches de bundles/morceaux/sessions/points de contrôle ;
- placement de la configuration quota/par défaut dans `iroha_config` ;
- si le statut du modèle/artefact doit devenir orienté version plutôt que
  orienté formation-emploi lorsque `UserUpload` est présent ;
- le comportement de révocation précis en cas de perte d'un appartement
  `allow_model_inference` ou une version de modèle épinglée est restaurée.

Ce sont les prochains éléments de pont entre la conception et le code. Le placement architectural de
la fonctionnalité devrait maintenant être stable.

## Matrice de test- validation du téléchargement :
  - accepter les dépôts canoniques de safetensors HF
  - rejeter GGUF, ONNX, actifs de tokenizer/processeur manquants, non pris en charge
    architectures et packages multimodaux audio/vidéo
- le découpage :
  - racines de faisceaux déterministes
  - ordre de morceaux stable
  - reconstruction exacte
  - respect du plafond de l'enveloppe
- cohérence du registre :
  - exactitude de la promotion du bundle/morceau/artefact/poids en replay
- compilateur :
  - un petit luminaire chacun pour le style décodeur uniquement, le style LLaVA et le style Qwen2-VL
  - rejet des opérations et des formes non prises en charge
- exécution privée :
  - test de fumée de bout en bout crypté sur un petit luminaire avec des reçus stables et
    libération de sortie de seuil
- tarification :
  - Frais XOR pour le téléchargement, la compilation, les étapes d'exécution et le décryptage
- Intégration So Ra :
  - télécharger, compiler, publier, lier à l'équipe, gérer une arène fermée, inspecter les reçus,
    enregistrer le projet, rouvrir, réexécuter de manière déterministe
- sécurité :
  - pas de contournement des portes d'exportation
  - pas d'auto-propagation narrative
  - La liaison d'appartement échoue sans `allow_model_inference`

## Tranches d'implémentation1. Ajoutez les champs de modèle de données manquants et les nouveaux types d'enregistrement.
2. Ajoutez les nouveaux types de requête/réponse et gestionnaires de route Torii.
3. Ajoutez les ISI Soracloud correspondants et le stockage d'état mondial.
4. Ajoutez une validation déterministe de bundle/morceau et un octet chiffré en chaîne
   stockage.
5. Ajoutez un petit chemin d'installation/d'exécution de transformateur privé soutenu par BFV.
6. Étendez les commandes du modèle CLI pour couvrir les flux de téléchargement/compilation/exécution privée.
7. Intégration de Land So Ra une fois que le chemin backend fait autorité.