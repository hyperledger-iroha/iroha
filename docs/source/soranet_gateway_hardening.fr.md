---
lang: fr
direction: ltr
source: docs/source/soranet_gateway_hardening.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1a7a7fb86b2d307aea1b367c9c83a09b19e24cea3f5f4ccd29937fcae3d80997
source_last_modified: "2025-11-21T15:11:47.334996+00:00"
translation_last_reviewed: 2026-01-21
---

# Durcissement du Gateway SoraGlobal (SNNet-15H)

L'outil de durcissement capture des preuves de securite et de confidentialite avant de promouvoir les builds du Gateway.

## Commande
- `cargo xtask soranet-gateway-hardening --sbom <path> --vuln-report <path> --hsm-policy <path> --sandbox-profile <path> --data-retention-days 30 --log-retention-days 30 --out artifacts/soranet/gateway_hardening`

## Sorties
- `gateway_hardening_summary.json` - statut par entree (SBOM, rapport de vulnerabilites, politique HSM, profil de sandbox) plus le signal de retention. Les entrees manquantes affichent `warn` ou `error`.
- `gateway_hardening_summary.md` - synthese lisible pour les paquets de gouvernance.

## Notes d'acceptation
- Les rapports SBOM et de vulnerabilites doivent exister ; les entrees absentes degradent le statut.
- Une retention au-dela de 30 jours marque `warn` pour revue ; fournir des valeurs par defaut plus strictes avant la GA.
- Utiliser les artefacts de synthese comme pieces jointes pour les revues GAR/SOC et les runbooks d'incident.
