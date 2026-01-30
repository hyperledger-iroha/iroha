---
lang: es
direction: ltr
source: docs/portal/docs/soranet/transport.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
id: transport
title: Vue d'ensemble du transport SoraNet
sidebar_label: Vue d'ensemble du transport
description: Handshake, rotation des salts et guide des capacites pour l'overlay d'anonymat SoraNet.
---

:::note Source canonique
Cette page reflete la specification de transport SNNet-1 dans `docs/source/soranet/spec.md`. Gardez les deux copies alignees jusqu'a la retraite de l'ancien ensemble de documentation.
:::

SoraNet est l'overlay d'anonymat qui soutient les range fetches de SoraFS, le streaming Norito RPC et les futurs data lanes de Nexus. Le programme transport (items de roadmap **SNNet-1**, **SNNet-1a** et **SNNet-1b**) a defini un handshake deterministe, une negotiation de capacites post-quantum (PQ) et un plan de rotation des salts afin que chaque relay, client et gateway observe la meme posture de securite.

## Objectifs et modele reseau

- Construire des circuits a trois sauts (entry -> middle -> exit) sur QUIC v1 afin que les peers abusifs n'atteignent jamais Torii directement.
- Superposer un handshake Noise XX *hybride* (Curve25519 + Kyber768) par-dessus QUIC/TLS pour lier les cles de session au transcript TLS.
- Exiger des TLVs de capacite qui annoncent le support PQ KEM/signature, le role du relay et la version de protocole; GREASE les types inconnus pour que les extensions futures restent deployables.
- Rotation quotidienne des salts de contenu aveugle et epinglage des guard relays pour 30 jours afin que le churn du directory ne puisse pas desanonymiser les clients.
- Garder des cells fixes a 1024 B, injecter du padding/des cells dummy, et exporter une telemetrie deterministe pour detecter rapidement les tentatives de downgrade.

## Pipeline de handshake (SNNet-1a)

1. **QUIC/TLS envelope** - les clients se connectent aux relays via QUIC v1 et terminent un handshake TLS 1.3 en utilisant des certificats Ed25519 signes par la governance CA. Le TLS exporter (`tls-exporter("soranet handshake", 64)`) alimente la couche Noise pour que les transcripts soient inseparables.
2. **Noise XX hybrid** - chaine de protocole `Noise_XXhybrid_25519+Kyber768_AESGCM_SHA256` avec prologue = TLS exporter. Flux de messages:

   ```
   -> e, s
   <- e, ee, se, s, pq_ciphertext
   -> ee, se, pq_ciphertext
   ```

   La sortie DH Curve25519 et les deux encapsulations Kyber sont melangees dans les cles symetriques finales. Un echec de negotiation PQ annule le handshake completement: aucun fallback classique seul n'est autorise.

3. **Puzzle tickets et tokens** - les relays peuvent exiger un ticket de proof-of-work Argon2id avant `ClientHello`. Les tickets sont des frames prefixees par la longueur qui transportent la solution Argon2 hachee et expirent dans les bornes de la politique:

   ```norito
   struct PowTicketV1 {
       version: u8,
       difficulty: u8,
       expires_at: u64,
       client_nonce: [u8; 32],
       solution: [u8; 32],
   }
   ```

   Les tokens d'admission prefixes par `SNTK` contournent les puzzles lorsqu'une signature ML-DSA-44 de l'emetteur valide contre la politique active et la liste de revocation.

4. **Echange de capability TLV** - le payload final de Noise transporte les TLVs de capacite decrits ci-dessous. Les clients abandonnent la connexion si une capacite obligatoire (PQ KEM/signature, role ou version) est manquante ou en conflit avec l'entree du directory.

5. **Journalisation du transcript** - les relays journalisent le hash du transcript, l'empreinte TLS et le contenu TLV pour alimenter les detecteurs de downgrade et les pipelines de conformite.

## Capability TLVs (SNNet-1c)

Les capacites reutilisent une enveloppe TLV fixe `typ/length/value`:

```norito
struct CapabilityTLV {
    typ: u16,
    length: u16,
    value: Vec<u8>,
}
```

Types definis aujourd'hui:

- `snnet.pqkem` - niveau Kyber (`kyber768` pour le rollout actuel).
- `snnet.pqsig` - suite de signature PQ (`ml-dsa-44`).
- `snnet.role` - role du relay (`entry`, `middle`, `exit`, `gateway`).
- `snnet.version` - identifiant de version du protocole.
- `snnet.grease` - entrees de remplissage aleatoires dans la plage reservee pour garantir la tolerance des futurs TLVs.

Les clients maintiennent une allow-list de TLVs requis et echouent le handshake lorsqu'ils sont omis ou degrades. Les relays publient le meme set dans leur microdescriptor de directory pour que la validation soit deterministe.

## Rotation des salts et CID blinding (SNNet-1b)

- La governance publie un enregistrement `SaltRotationScheduleV1` avec les valeurs `(epoch_id, salt, valid_after, valid_until)`. Les relays et gateways recuperent le calendrier signe depuis le directory publisher.
- Les clients appliquent le nouveau salt a `valid_after`, conservent le salt precedent pour une periode de grace de 12 h et gardent un historique de 7 epoques pour tolerer les mises a jour retardees.
- Les identifiants aveugles canoniques utilisent:

  ```
  cache_key = BLAKE3("soranet.blinding.canonical.v1" ∥ salt ∥ cid)
  ```

  Les gateways acceptent la cle aveugle via `Sora-Req-Blinded-CID` et la renvoient dans `Sora-Content-CID`. Le blinding circuit/request (`CircuitBlindingKey::derive`) est fourni dans `iroha_crypto::soranet::blinding`.
- Si un relay manque une epoque, il arrete de nouveaux circuits jusqu'a ce qu'il telecharge le calendrier et emet un `SaltRecoveryEventV1`, que les dashboards d'astreinte traitent comme un signal de paging.

## Donnees de directory et politique de guard

- Les microdescriptors portent l'identite du relay (Ed25519 + ML-DSA-65), les cles PQ, les TLVs de capacite, les tags de region, l'eligibilite guard et l'epoque de salt annoncee.
- Les clients epinglent les guard sets pendant 30 jours et persistent les caches `guard_set` aux cotes du snapshot signe du directory. Les wrappers CLI et SDK exposent l'empreinte du cache afin que les preuves de rollout soient attachees aux revues de changement.

## Telemetrie et checklist de rollout

- Metriques a exporter avant production:
  - `soranet_handshake_success_total{role}`
  - `soranet_handshake_failure_total{reason}`
  - `soranet_handshake_latency_seconds`
  - `soranet_capability_mismatch_total`
  - `soranet_salt_rotation_lag_seconds`
- Les seuils d'alerte vivent aux cotes de la matrice SLO du SOP de rotation des salts (`docs/source/soranet_salt_plan.md#slo--alert-matrix`) et doivent etre refletes dans Alertmanager avant la promotion du reseau.
- Alertes: >5 % de taux d'echec sur 5 minutes, salt lag >15 minutes, ou des mismatches de capacites observes en production.
- Etapes de rollout:
  1. Exercicer les tests d'interoperabilite relay/client en staging avec le handshake hybride et la pile PQ activee.
  2. Repeter le SOP de rotation des salts (`docs/source/soranet_salt_plan.md`) et joindre les artefacts de drill au dossier de changement.
  3. Activer la negotiation de capacites dans le directory, puis deployer aux relays d'entree, relays intermediaires, relays de sortie, et enfin aux clients.
  4. Enregistrer les empreintes de cache de guard, les calendriers de salt et les dashboards de telemetrie pour chaque phase; joindre le bundle de preuves a `status.md`.

Suivre cette checklist permet aux equipes d'operateurs, de clients et de SDK d'adopter les transports SoraNet de concert tout en respectant la determinisme et les exigences d'audit capturees dans la roadmap SNNet.
