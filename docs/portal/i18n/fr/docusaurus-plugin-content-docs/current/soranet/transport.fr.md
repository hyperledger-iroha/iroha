---
lang: fr
direction: ltr
source: docs/portal/docs/soranet/transport.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : transport
titre : Vue d'ensemble du transport SoraNet
sidebar_label : Vue d'ensemble du transport
description : Poignée de main, rotation des sels et guide des capacités pour l'overlay d'anonymat SoraNet.
---

:::note Source canonique
Cette page reflète la spécification de transport SNNet-1 dans `docs/source/soranet/spec.md`. Gardez les deux copies alignées jusqu'à la retraite de l'ancien ensemble de documentation.
:::

SoraNet est l'overlay d'anonymat qui prend en charge les range fetches de SoraFS, le streaming Norito RPC et les futures data lanes de Nexus. Le programme de transport (éléments de roadmap **SNNet-1**, **SNNet-1a** et **SNNet-1b**) a défini une poignée de main déterministe, une négociation de capacités post-quantiques (PQ) et un plan de rotation des sels afin que chaque relais, client et passerelle respecte la même posture de sécurité.

## Objectifs et modèle réseau

- Construire des circuits à trois sauts (entrée -> milieu -> sortie) sur QUIC v1 afin que les pairs abusifs n'atteignent jamais Torii directement.
- Superposer un handshake Noise XX *hybride* (Curve25519 + Kyber768) par-dessus QUIC/TLS pour lier les clés de session au transcript TLS.
- Exiger des TLVs de capacité qui annoncent le support PQ KEM/signature, le rôle du relais et la version de protocole ; GREASE les types inconnus pour que les extensions futures restent déployables.
- Rotation quotidienne des sels de contenu aveugle et épinglage des relais de garde pendant 30 jours afin que le churn du répertoire ne puisse pas désanonymiser les clients.
- Garder des cellules fixe un 1024 B, injecter du padding/des cellules factices, et exporter une télémétrie déterministe pour détecter rapidement les tentatives de downgrade.

## Pipeline de poignée de main (SNNet-1a)

1. **QUIC/TLS enveloppe** - les clients se connectent aux relais via QUIC v1 et terminent un handshake TLS 1.3 en utilisant des certificats Ed25519 signés par la gouvernance CA. Le TLS exportateur (`tls-exporter("soranet handshake", 64)`) alimente la couche Noise pour que les transcriptions soient inséparables.
2. **Noise XX hybrid** - chaîne de protocole `Noise_XXhybrid_25519+Kyber768_AESGCM_SHA256` avec prologue = exportateur TLS. Flux de messages :

   ```
   -> e, s
   <- e, ee, se, s, pq_ciphertext
   -> ee, se, pq_ciphertext
   ```

   La sortie DH Curve25519 et les deux encapsulations Kyber sont mélangées dans les clés symétriques finales. Un echec de négociation PQ annule le handshake complètement : aucun fallback classique seul n'est autorisé.

3. **Puzzle tickets et tokens** - les relais peuvent exiger un ticket de proof-of-work Argon2id avant `ClientHello`. Les tickets sont des frames préfixés par la longueur qui transportent la solution Argon2 hachée et expirent dans les bornes de la politique :

   ```norito
   struct PowTicketV1 {
       version: u8,
       difficulty: u8,
       expires_at: u64,
       client_nonce: [u8; 32],
       solution: [u8; 32],
   }
   ```

   Les jetons d'admission préfixes par `SNTK` contournent les puzzles lorsqu'une signature ML-DSA-44 de l'émetteur valide contre la politique active et la liste de révocation.4. **Echange de capacité TLV** - le payload final de Noise transporte les TLV de capacité décrits ci-dessous. Les clients abandonnent la connexion si une capacité obligatoire (PQ KEM/signature, rôle ou version) est manquante ou en conflit avec l'entrée du répertoire.

5. **Journalisation du transcript** - les relais journalisent le hash du transcript, l'empreinte TLS et le contenu TLV pour alimenter les détecteurs de downgrade et les pipelines de conformité.

## TLV de capacité (SNNet-1c)

Les capacités réutilisent une enveloppe TLV fixe `typ/length/value` :

```norito
struct CapabilityTLV {
    typ: u16,
    length: u16,
    value: Vec<u8>,
}
```

Types définis aujourd'hui :

- `snnet.pqkem` - niveau Kyber (`kyber768` pour le déploiement actuel).
- `snnet.pqsig` - suite de signature PQ (`ml-dsa-44`).
- `snnet.role` - rôle du relais (`entry`, `middle`, `exit`, `gateway`).
- `snnet.version` - identifiant de version du protocole.
- `snnet.grease` - entrées de remplissage aléatoires dans la plage réservée pour garantir la tolérance des futurs TLV.

Les clients maintiennent une liste d'autorisation de TLV requise et font écho à la poignée de main lorsqu'ils sont omis ou dégradés. Les relais publient le meme set dans leur microdescripteur de répertoire pour que la validation soit déterministe.

## Rotation des sels et CID blinding (SNNet-1b)

- La gouvernance publie un enregistrement `SaltRotationScheduleV1` avec les valeurs `(epoch_id, salt, valid_after, valid_until)`. Les relais et passerelles récupèrent le calendrier signé depuis l'éditeur d'annuaire.
- Les clients appliquent le nouveau sel à `valid_after`, conservant le sel précédent pour une période de grâce de 12 heures et jardinant un historique de 7 époques pour tolérer les mises à jour retardées.
- Les identifiants aveugles canoniques utilisent :

  ```
  cache_key = BLAKE3("soranet.blinding.canonical.v1" ∥ salt ∥ cid)
  ```

  Les passerelles acceptent la clé aveugle via `Sora-Req-Blinded-CID` et la renvoient dans `Sora-Content-CID`. Le circuit/demande d'aveuglement (`CircuitBlindingKey::derive`) est fourni dans `iroha_crypto::soranet::blinding`.
- Si un relais manque une époque, il arrête de nouveaux circuits jusqu'à ce qu'il télécharge le calendrier et emet un `SaltRecoveryEventV1`, que les tableaux de bord d'astreinte traitent comme un signal de paging.

## Données de directoire et politique de garde

- Les microdescripteurs portent l'identité du relais (Ed25519 + ML-DSA-65), les clés PQ, les TLV de capacité, les tags de région, l'éligibilité garde et l'époque de sel annoncée.
- Les clients epinglent les guard sets pendant 30 jours et persistants les caches `guard_set` aux cotes du snapshot signé du répertoire. Les wrappers CLI et SDK exposent l'empreinte du cache afin que les preuves de déploiement soient attachées aux revues de changement.

## Télémétrie et checklist de déploiement- Métriques à un exportateur avant production :
  -`soranet_handshake_success_total{role}`
  -`soranet_handshake_failure_total{reason}`
  -`soranet_handshake_latency_seconds`
  -`soranet_capability_mismatch_total`
  -`soranet_salt_rotation_lag_seconds`
- Les seuils d'alerte vivent aux cotes de la matrice SLO du SOP de rotation des sels (`docs/source/soranet_salt_plan.md#slo--alert-matrix`) et doivent être reflétés dans Alertmanager avant la promotion du réseau.
- Alertes : >5 % de taux d'echec sur 5 minutes, salt lag >15 minutes, ou des inadéquations de capacités observées en production.
- Étapes de déploiement :
  1. Exercer les tests d'interopérabilité relais/client en staging avec le handshake hybride et la pile PQ active.
  2. Répétez le SOP de rotation des sels (`docs/source/soranet_salt_plan.md`) et joignez les artefacts de forage au dossier de changement.
  3. Activer la négociation de capacités dans l'annuaire, puis déployer aux relais d'entrée, relais intermédiaires, relais de sortie, et enfin aux clients.
  4. Enregistrer les empreintes de cache de garde, les calendriers de sel et les tableaux de bord de télémétrie pour chaque phase ; joindre le bundle de preuves a `status.md`.

Suivre cette checklist permet aux équipes d'opérateurs, de clients et de SDK d'adopter les transports SoraNet de concert tout en respectant le déterminisme et les exigences d'audit capturées dans la feuille de route SNNet.