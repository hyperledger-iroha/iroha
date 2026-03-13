---
lang: fr
direction: ltr
source: docs/source/crypto/sm_audit_success_checklist.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 624ef9305dc14d477a616923c80445094c692bc6a38d69465f679b54ccd52e92
source_last_modified: "2026-01-03T18:07:57.081283+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

% des critères de réussite de l'audit SM2/SM3/SM4
% Groupe de travail sur la cryptographie Iroha
% 2026-01-30

# Objectif

Cette liste de contrôle capture les critères concrets requis pour une réussite
réalisation de l’audit externe SM2/SM3/SM4. Il devrait être revu au cours
coup d'envoi, revisité à chaque point de contrôle de statut et utilisé pour confirmer la sortie
conditions avant d’activer la signature SM pour les validateurs de production.

# Préparation préalable à l'engagement

- [ ] Contrat signé, y compris la portée, les livrables, la confidentialité et
      langage de support de remédiation.
- [ ] L'équipe d'audit reçoit un accès miroir au référentiel, un compartiment d'artefacts CI et
      ensemble de documentation répertorié dans `docs/source/crypto/sm_audit_brief.md`.
- [ ] Points de contact confirmés avec des sauvegardes pour chaque rôle
      (crypto, IVM, opérations de plateforme, sécurité, documentation).
- [ ] Les parties prenantes internes s'alignent sur la date de sortie cible et gèlent les fenêtres.
-[ ] Export SBOM (`cargo auditable` + CycloneDX) généré et partagé.
- [ ] Package de provenance de build OpenSSL/Tongsuo préparé
      (hachage de l'archive tar source, script de construction, notes de reproductibilité).
- [ ] Derniers résultats de tests déterministes capturés :
      `scripts/sm_openssl_smoke.sh`, `cargo test -p iroha_crypto sm` et
      Luminaires aller-retour Norito.
-[ ] Annonce Torii `/v2/node/capabilities` (via `iroha runtime capabilities`) enregistrée, vérifiant les champs du manifeste `crypto.sm` et l'instantané de la politique d'accélération.

# Exécution des missions

- [ ] Atelier de lancement complété par une compréhension partagée des objectifs,
      les délais et la cadence de communication.
- [ ] Rapports de situation hebdomadaires reçus et triés ; registre des risques mis à jour.
- [ ] Résultats communiqués dans un délai d'un jour ouvrable après la découverte en cas de gravité
      est élevé ou critique.
- [ ] L'équipe d'audit valide les chemins de déterminisme sur ≥2 architectures CPU (x86_64,
      aarch64) avec des sorties correspondantes.
-[ ] L'examen par canal secondaire comprend des preuves en temps constant ou des tests empiriques
      preuves des chemins Rust et FFI.
- [ ] L'examen de la conformité et de la documentation confirme la correspondance des directives de l'opérateur
      obligations réglementaires.
- [ ] Tests différentiels par rapport aux implémentations de référence (RustCrypto,
      OpenSSL/Tongsuo) exécuté sous la supervision d'un auditeur.
- [ ] Harnais Fuzz évalués ; de nouveaux corpus de semences sont fournis là où des lacunes existent.

# Correction et sortie

- [ ] Tous les résultats classés par gravité, impact, exploitabilité et
      mesures correctives recommandées.
- [ ] Les problèmes élevés/critiques reçoivent des correctifs ou des atténuations approuvés par l'auditeur.
      vérification; risques résiduels documentés.
- [ ] L'auditeur fournit une validation de re-test mettant en évidence les problèmes résolus (diff, test
      courses, ou attestation signée).
- [ ] Rapport final remis : synthèse, conclusions détaillées, méthodologie,
      verdict de déterminisme, verdict de conformité.
- [ ] La réunion de signature interne conclut les prochaines étapes, publie les ajustements,
      et mises à jour de la documentation.
-[ ] `status.md` mis à jour avec le résultat de l'audit et les mesures correctives en suspens
      suivis.
-[ ] Post-mortem capturé dans `docs/source/crypto/sm_program.md` (leçons
      apprises, tâches de durcissement futures).