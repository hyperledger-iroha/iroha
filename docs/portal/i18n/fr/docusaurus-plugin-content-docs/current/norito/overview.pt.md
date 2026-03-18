---
lang: fr
direction: ltr
source: docs/portal/docs/norito/overview.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Visa général du Norito

Norito et la caméra de sérialisation binaire utilisée dans tout ou Iroha : définir les structures de données codifiées sur le réseau, persistantes en discothèque et trouvées entre les contrats et les hôtes. Chaque caisse ne dépend pas de l'espace de travail de Norito plutôt que de `serde` pour que les pairs dans le matériel produisent différents octets identiques.

Ce visa général reprend en tant que parties principales et en tant que références canoniques.

## Architecture dans le CV

- **Cabecalho + payload** - Chaque message Norito vient avec un cabecalho de négociation de fonctionnalités (drapeaux, somme de contrôle) suivi du payload puro. Layouts empacotados e compressao sao negociados via bits do cabecalho.
- **Codificacao deterministica** - `norito::codec::{Encode, Decode}` implémente une base de codification. La même mise en page est réutilisée pour impliquer des charges utiles dans les câbles pour que le hachage et l'analyse de la nature soient déterministes.
- **Schéma + dérivés** - `norito_derive` permet d'implémenter `Encode`, `Decode` et `IntoSchema`. Structs/sequences empacotadas sao ativadas por padrao et documentadas em `norito.md`.
- **Registro multicodec** - Les identifiants de hachage, les types de codes et les descripteurs de charge utile sont présents dans `norito::multicodec`. Un tableau de référence fica em `multicodec.md`.

## Ferramentas| Taréfa | Commande / API | Notes |
| --- | --- | --- |
| Inspecter cabecalho/secoes | `ivm_tool inspect <file>.to` | Afficher vers ABI, les drapeaux et les points d'entrée. |
| Codifier/décodifier dans Rust | `norito::codec::{Encode, Decode}` | Implémenté pour tous les types de principes du modèle de données. |
| Interopérabilité JSON | `norito::json::{to_json_pretty, from_json}` | JSON déterministe appliqué aux valeurs Norito. |
| Gerar docs/spécifications | `norito.md`, `multicodec.md` | Documentation fonte de verdade na raid do repo. |

## Flux de travail de développement

1. **Adicionar dérive** - Prefira `#[derive(Encode, Decode, IntoSchema)]` para novas estruturas de dados. Évitez les sérialisations feitos à mao salvo se pour absolument nécessaire.
2. **Valider les mises en page empacotados** - Utilisez `cargo test -p norito` (et une matrice de fonctionnalités empacotadas em `scripts/run_norito_feature_matrix.sh`) pour garantir que de nouvelles mises en page soient enregistrées.
3. **Régénérer la documentation** - Lorsque vous changez de code, actualisez `norito.md` et le tableau multicodec, puis actualisez-vous en tant que page du portail (`/reference/norito-codec` et cette vue générale).
4. **Manter testes Norito-first** - Les tests d'intégration doivent utiliser les helpers JSON de Norito à la place de `serde_json` pour exercer nos propres chemins de production.

## Liens rapides

- Spécification : [`norito.md`](https://github.com/hyperledger-iroha/iroha/blob/master/norito.md)
- Attributs multicodec : [`multicodec.md`](https://github.com/hyperledger-iroha/iroha/blob/master/multicodec.md)
- Script de matrice de fonctionnalités : `scripts/run_norito_feature_matrix.sh`
- Exemples de mise en page empacotado : `crates/norito/tests/`Combinez cette visa général avec le guide de démarrage rapide (`/norito/getting-started`) pour une étape pratique de compilation et d'exécution du bytecode utilisant les charges utiles Norito.