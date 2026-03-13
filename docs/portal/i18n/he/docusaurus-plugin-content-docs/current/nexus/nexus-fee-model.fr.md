---
lang: he
direction: rtl
source: docs/portal/docs/nexus/nexus-fee-model.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
מזהה: nexus-fee-model
כותרת: Mises a jour du modele de frais Nexus
תיאור: Miroir de `docs/source/nexus_fee_model.md`, מתעד les recus de reglement de lanes et les surfaces de reconciliation.
---

:::הערה מקור קנוניק
Cette page reflete `docs/source/nexus_fee_model.md`. Gardez les deux copies alignees תליון la migration des traductions japonaise, Hebraique, Espagnole, Portugaise, Francaise, Russe, Arabe et Ourdoue.
:::

# Mises a jour du modele de frais Nexus

Le router de reglement איחוד לכידת תחזוקה des recus deterministes par lane afin que les operators puissent reconcilier les debits de gas avec le modele de frais Nexus.

- Pour l'architecture complete du routeur, la politique de buffer, la matrice de telemetrie et la sequence deploiement, voir `docs/settlement-router.md`. המדריך המפורש הערה לפרמטרים המסמכים ici ratachent au livrable du מפת הדרכים NX-3 ו-SRE להגיב על מעקב אחר נתיב בייצור.
- La configuration de l'actif de gas (`pipeline.gas.units_per_gas`) כולל אחד עשרוני `twap_local_per_xor`, un `liquidity_profile` (`tier1`, `tier2`, או I01800X), et un I018NI `volatility_class` (`stable`, `elevated`, `dislocated`). Ces indikers alimentent le routeur de reglement afin que la cotation XOR corresponde au TWAP canonique et au palier de haircut pour la lane.
- עסקת צ'אקה qui paie du gasregistre un `LaneSettlementReceipt`. Chaque recu stocke l'identifiant source fourni par l'appelant, le micro-montant local, le XOR a regler immediatement, le XOR attendu apres le haircut, la variance realisee (`xor_variance_micro`), et l'horodatage du bloc in millisecondes.
- L'execution de blocs agrege les recus par lane/dataspace et les publie via `lane_settlement_commitments` dans `/v2/sumeragi/status`. Les totalexposent `total_local_micro`, `total_xor_due_micro`, et `total_xor_after_haircut_micro` additionnes sur le bloc pour les exports nocturnes de reconciliation.
- Un nouveau compteur `total_xor_variance_micro` suit la marge de securite consommee (הבדל בין XOR du et l'attendu לאחר תספורת), et `swap_metadata` מתעד את הפרמטרים של המרה קביעת המרה (TWAP, epsilon, פרופיל נזילות, מחלקה לביקורת) מנות ראשונות דה לה cotation independamment de la configuration d'execution.

Les consommateurs peuvent suivre `lane_settlement_commitments` aux cotes des snapshots existants de commitments de lane and de dataspace pour verifier que les buffers de frais, les paliers de haircut and l'execute du swap correspondent au modele de frais Nexus.