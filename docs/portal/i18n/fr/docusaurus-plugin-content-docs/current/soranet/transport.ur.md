---
lang: fr
direction: ltr
source: docs/portal/docs/soranet/transport.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : transport
titre : Présentation du transport SoraNet
sidebar_label : aperçu des transports
description : superposition d'anonymat SoraNet, poignée de main, rotation du sel, conseils en matière de capacités
---

:::note Source canonique
Il s'agit de la spécification de transport SNNet-1 `docs/source/soranet/spec.md` et de la spécification de transport SNNet-1. جب تک پرانا documentation set retiré نہ ہو، دونوں کاپیاں رکھیں۔
:::

SoraNet et la superposition d'anonymat et les récupérations de plage SoraFS, le streaming RPC Norito et les voies de données Nexus. ہے۔ programme de transport (éléments de la feuille de route **SNNet-1**, **SNNet-1a**, et **SNNet-1b**) pour une poignée de main déterministe, une négociation de capacité post-quantique (PQ) et un plan de rotation du sel pour un relais, un client, une passerelle et une posture de sécurité, observez کرے۔

## Objectifs et modèle de réseau

- QUIC v1 pour les circuits à trois sauts (entrée -> milieu -> sortie) pour les pairs abusifs comme Torii pour les pairs abusifs Torii pour les pairs abusifs
- QUIC/TLS et Noise XX *hybride* poignée de main (Curve25519 + Kyber768) superposition de touches de session Transcription TLS et liaison
- Capacité TLV pour la prise en charge PQ KEM/signature, rôle de relais et version du protocole publicitaire. types inconnus et GREASE et futures extensions déployables
- les sels à contenu aveugle font pivoter les relais de garde et les relais de garde à 30 broches pour gérer le répertoire des clients et désanonymiser les clients
- Cellules 1024 B pour remplissage fixe/cellules factices injectées et exportation de télémétrie déterministe pour tentatives de rétrogradation

## Pipeline de prise de contact (SNNet-1a)

1. **Enveloppe QUIC/TLS** - clients QUIC v1 et relais pour se connecter et certificats Ed25519 pour une prise de contact TLS 1.3 terminée et pour une gouvernance CA et un signe ہیں۔ Exportateur TLS (`tls-exporter("soranet handshake", 64)`) Couche de bruit et graines de transcription et transcriptions inséparables
2. **Noise XX hybride** - chaîne de protocole `Noise_XXhybrid_25519+Kyber768_AESGCM_SHA256` avec prologue = exportateur TLS۔ Flux de messages :

   ```
   -> e, s
   <- e, ee, se, s, pq_ciphertext
   -> ee, se, pq_ciphertext
   ```

   Sortie Curve25519 DH et encapsulations Kyber touches symétriques finales et mélange de couleurs Matériel PQ négocier نہ ہو تو poignée de main مکمل طور پر abandonner ہوتا ہے - repli classique uniquement

3. **Tickets et jetons de puzzle** - relais `ClientHello` pour le ticket de preuve de travail Argon2id مانگ سکتے ہیں۔ Les trames préfixées par la longueur des tickets et la solution Argon2 hachée sont définies et les limites de la politique expirent automatiquement :

   ```norito
   struct PowTicketV1 {
       version: u8,
       difficulty: u8,
       expires_at: u64,
       client_nonce: [u8; 32],
       solution: [u8; 32],
   }
   ```

   Préfixe `SNTK` et contournement des puzzles de jetons d'admission pour l'émetteur et la signature ML-DSA-44 politique active et liste de révocation pour valider et valider

4. **Échange de capacité TLV** - charge utile de bruit finale et capacité TLV pour chaque élément Capacité de stockage (PQ KEM/signature, rôle, version) manquante, entrée de répertoire, non-concordance, abandon de connexion des clients, etc.

5. **Journalisation des transcriptions** - relaie le hachage de la transcription, les empreintes digitales TLS et le journal du contenu TLV ainsi que les détecteurs de déclassement et les pipelines de conformité et les flux d'alimentation.

## TLV de capacité (SNNet-1c)Capacités de réutilisation de l'enveloppe TLV `typ/length/value` :

```norito
struct CapabilityTLV {
    typ: u16,
    length: u16,
    value: Vec<u8>,
}
```

Il y a des types définis :

- `snnet.pqkem` - Niveau Kyber (déploiement `kyber768` موجودہ کے لئے).
- `snnet.pqsig` - Suite de signatures PQ (`ml-dsa-44`).
- `snnet.role` - rôle de relais (`entry`, `middle`, `exit`, `gateway`).
- `snnet.version` - identifiant de version du protocole.
- `snnet.grease` - plage réservée pour les entrées de remplissage aléatoires et les futurs TLV tolèrent ہوں۔

Les clients ont besoin de TLV et d'une liste d'autorisation ou d'un manquement ou d'un déclassement ou d'un échec de la prise de contact. Relais définir un microdescripteur d'annuaire publier un message de validation déterministe

## Rotation du sel et aveuglement CID (SNNet-1b)

- Gouvernance `SaltRotationScheduleV1` enregistrement publier کرتی ہے جس میں `(epoch_id, salt, valid_after, valid_until)` valeurs ہوتے ہیں۔ Relais et passerelles signées par calendrier et éditeur d'annuaire pour récupérer les informations
- Clients `valid_after` pour appliquer le sel et mettre à jour le sel pendant un délai de grâce de 12 heures et conserver l'historique de 7 époques کرتے ہیں۔
- Identifiants canoniques aveugles یوں بنتے ہیں :

  ```
  cache_key = BLAKE3("soranet.blinding.canonical.v1" ∥ salt ∥ cid)
  ```

  Passerelles `Sora-Req-Blinded-CID` avec clé aveugle pour `Sora-Content-CID` avec écho Blindage de circuit/demande (`CircuitBlindingKey::derive`) `iroha_crypto::soranet::blinding` میں موجود ہے۔
- Le relais de l'époque manque les circuits et les circuits ainsi que le téléchargement du calendrier et `SaltRecoveryEventV1` émettent des messages. tableaux de bord d'astreinte signal de radiomessagerie کے طور پر traiter کرتے ہیں۔

## Données de l'annuaire et politique de garde

- Les microdescripteurs relayent l'identité (Ed25519 + ML-DSA-65), les clés PQ, les capacités TLV, les balises de région, l'éligibilité de la garde, et l'époque du sel annoncée pour l'époque du sel.
- Les clients gardent des ensembles de 30 broches pour les caches `guard_set` et les instantanés de répertoire signés qui persistent et persistent. CLI et les wrappers du SDK mettent en cache la surface des empreintes digitales et vérifient le déploiement des preuves de modification des révisions et attachent les pièces jointes.

## Liste de contrôle de télémétrie et de déploiement

- Mesures de production et d'exportation et d'exportation :
  -`soranet_handshake_success_total{role}`
  -`soranet_handshake_failure_total{reason}`
  -`soranet_handshake_latency_seconds`
  -`soranet_capability_mismatch_total`
  -`soranet_salt_rotation_lag_seconds`
- Seuils d'alerte rotation du sel Matrice SOP SLO (`docs/source/soranet_salt_plan.md#slo--alert-matrix`) pour la promotion du réseau Alertmanager et le miroir du miroir
- Alertes : 5 heures >5 % de taux d'échec, décalage salin >15 minutes, production et inadéquation des capacités.
- Étapes de déploiement :
  1. Mise en scène d'une poignée de main hybride et d'une pile PQ permettant des tests d'interopérabilité relais/client
  2. SOP de rotation du sel (`docs/source/soranet_salt_plan.md`) répéter les artefacts de forage modifier l'enregistrement et joindre les éléments
  3. La négociation des capacités d'annuaire permet d'activer les relais d'entrée, les relais intermédiaires, les relais de sortie, pour les clients et le déploiement.
  4. Phase d'enregistrement des empreintes digitales du cache de garde, des horaires de sel et des enregistrements des tableaux de bord de télémétrie. ensemble de preuves `status.md` à joindre en pièce jointeListe de contrôle Suivre l'opérateur, le client et les équipes SDK Transports SoraNet et le déterminisme et les exigences d'audit پوری کرتے ہیں۔