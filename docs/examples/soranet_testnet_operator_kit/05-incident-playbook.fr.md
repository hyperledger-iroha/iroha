---
lang: fr
direction: ltr
source: docs/examples/soranet_testnet_operator_kit/05-incident-playbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: d2fbce156952c669e73d74c13284fca317013d706ee401359028c3638341d34b
source_last_modified: "2025-11-04T16:28:48.303168+00:00"
translation_last_reviewed: 2026-01-01
---

# Playbook de reponse brownout / downgrade

1. **Detecter**
   - L'alerte `soranet_privacy_circuit_events_total{kind="downgrade"}` se declenche ou le webhook brownout arrive depuis governance.
   - Confirmer via `kubectl logs soranet-relay` ou systemd journal sous 5 min.

2. **Stabiliser**
   - Geler la rotation guard (`relay guard-rotation disable --ttl 30m`).
   - Activer l'override direct-only pour les clients impactes
     (`sorafs fetch --transport-policy direct-only --write-mode read-only`).
   - Capturer le hash actuel de la config compliance (`sha256sum compliance.toml`).

3. **Diagnostiquer**
   - Collecter le dernier snapshot du directory et le bundle de metriques du relay:
     `soranet-relay support-bundle --output /tmp/bundle.tgz`.
   - Noter la profondeur de la file PoW, les compteurs de throttling et les pics de categorie GAR.
   - Identifier si l'evenement vient d'un deficit PQ, d'un override compliance ou d'une panne relay.

4. **Escalader**
   - Notifier le pont governance (`#soranet-incident`) avec un resume et le hash du bundle.
   - Ouvrir un ticket incident liant l'alerte, incluant timestamps et etapes de mitigation.

5. **Recuperer**
   - Une fois la cause racine resolue, re-activer la rotation
     (`relay guard-rotation enable`) et annuler les overrides direct-only.
   - Surveiller les KPI pendant 30 minutes; verifier qu'aucun nouveau brownout n'apparaisse.

6. **Postmortem**
   - Soumettre le rapport d'incident sous 48 heures avec le template governance.
   - Mettre a jour les runbooks si un nouveau mode de panne est decouvert.
