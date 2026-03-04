---
lang: fr
direction: ltr
source: docs/source/crypto/sm_vectors.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: fd46471945188bcb95c8ee411c48acc8915a92b408df196caa65bf25f0596732
source_last_modified: "2026-01-05T10:35:36.441790+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//! Vecteurs de test de référence pour les travaux d'intégration SM2/SM3/SM4.

# Notes de mise en scène des vecteurs SM

Ce document regroupe les tests à réponse connue accessibles au public qui amorcent les harnais SM2/SM3/SM4 avant l'arrivée des scripts d'importation automatisés. Des copies lisibles par machine se trouvent dans :

- `crates/iroha_crypto/tests/fixtures/sm_known_answers.toml` (vecteurs d'annexe, cas RFC 8998, exemple d'annexe 1).
- `fixtures/sm/sm2_fixture.json` (fixation SDK déterministe partagée consommée par les tests Rust/Python/JavaScript).
- `crates/iroha_crypto/tests/fixtures/wycheproof_sm2.json` — corpus organisé de 52 cas (fixations déterministes + négatifs synthétisés de retournement de bits/message/troncation de queue) mis en miroir sous Apache-2.0 pendant que la suite SM2 en amont est en attente. `crates/iroha_crypto/tests/sm2_wycheproof.rs` vérifie ces vecteurs à l'aide du vérificateur SM2 standard lorsque cela est possible et revient à une implémentation pure BigInt du domaine Annex lorsque cela est nécessaire.

## Vérification de signature SM2 contre OpenSSL / Tongsuo / GmSSL

L'exemple 1 de l'annexe (Fp-256) utilise l'identité `ALICE123@YAHOO.COM` (ENTLA 0x0090), le message `"message digest"` et la clé publique indiquée ci-dessous. Un workflow copier-coller OpenSSL/Tongsuo est :

```bash
# 1. Public key (SubjectPublicKeyInfo)
cat > pubkey.pem <<'PEM'
-----BEGIN PUBLIC KEY-----
MFkwEwYHKoZIzj0CAQYIKoEcz1UBgi0DQgAECuTHeYqg8RlHG+4RglvkYgK7eeKlhESV6XwE/03y
VIp8AkD4jxzU4WNSpzwXt/FvBzU+U6F21oSp/gxrt5joVw==
-----END PUBLIC KEY-----
PEM

# 2. Message (no trailing newline)
printf "message digest" > msg.bin

# 3. Signature (DER form)
cat > sig.b64 <<'EOF'
MEQCIEDx7Fn3k9n0ngnc70kTDUGU95+x7tLKpVus20nE51XRAiBvxtrDLF1c8Qx337IPfC62Z6RX
hy+wnsVjJ6Z+x97r5w==
EOF
base64 -d sig.b64 > sig.der

# 4. Verify (expects "Signature Verified Successfully")
openssl pkeyutl -verify -pubin -inkey pubkey.pem \
  -in msg.bin -sigfile sig.der \
  -rawin -digest sm3 \
  -pkeyopt distid:ALICE123@YAHOO.COM
```

* OpenSSL 3.x documente les options `distid:`/`hexdistid:`. Certaines versions d'OpenSSL 1.1.1 exposent le bouton sous le nom `sm2_id:` : utilisez celui qui apparaît dans `openssl pkeyutl -help`.
* GmSSL exporte la même surface `pkeyutl` ; les versions plus anciennes acceptaient également `-pkeyopt sm2_id:...`.
* LibreSSL (par défaut sur macOS/OpenBSD) n'implémente **pas** SM2/SM3, donc la commande ci-dessus échoue là-bas. Utilisez OpenSSL ≥ 1.1.1, Tongsuo ou GmSSL.

L'assistant DER émet `3044022040F1EC59F793D9F49E09DCEF49130D4194F79FB1EED2CAA55BACDB49C4E755D102206FC6DAC32C5D5CF10C77DFB20F7C2EB667A457872FB09EC56327A67EC7DEEBE7`, qui correspond à la signature de l'annexe.

L'annexe imprime également le hachage des informations utilisateur et le résumé résultant :

```
ZA = F4A38489E32B45B6F876E3AC2168CA392362DC8F23459C1D1146FC3DBFB7BC9A
e  = SM3(ZA || "message digest")
   = B524F552CD82B8B028476E005C377FB19A87E6FC682D48BB5D42E3D9B9EFFE76
```

Vous pouvez confirmer avec OpenSSL :

```bash
echo -n 'F4A38489E32B45B6F876E3AC2168CA392362DC8F23459C1D1146FC3DBFB7BC9A' \
  | xxd -r -p > za.bin
printf "message digest" >> za.bin
openssl dgst -sm3 -binary za.bin | xxd -p -c 256
# -> b524f552cd82b8b028476e005c377fb19a87e6fc682d48bb5d42e3d9b9effe76
```

Vérification de l'intégrité Python pour l'équation de la courbe :

```python
p = int("FFFFFFFEFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF00000000FFFFFFFFFFFFFFFF", 16)
a = int("FFFFFFFEFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF00000000FFFFFFFFFFFFFFFC", 16)
b = int("28E9FA9E9D9F5E344D5A9E4BCF6509A7F39789F515AB8F92DDBCBD414D940E93", 16)
x = int("0AE4C7798AA0F119471BEE11825BE46202BB79E2A5844495E97C04FF4DF2548A", 16)
y = int("7C0240F88F1CD4E16352A73C17B7F16F07353E53A176D684A9FE0C6BB798E857", 16)
assert (pow(y, 2, p) - (pow(x, 3, p) + a * x + b)) % p == 0
```

## Vecteurs de hachage SM3

| Entrée | Encodage hexadécimal | Résumé (hexadécimal) | Source |
|-------|--------------|--------------|--------|
| `""` (chaîne vide) | `""` | `1ab21d8355cfa17f8e61194831e81a8f22bec8c728fefb747ed035eb5082aa2b` | GM/T 0004-2012 Annexe A.1 |
| `"abc"` | `616263` | `66c7f0f462eeedd9d1f2d46bdc10e4e24167c4875cf2f7a2297da02b8f4ba8e0` | GM/T 0004-2012 Annexe A.2 |
| `"abcd"` répété 16 fois (64 octets) | `61626364` ×16 | `debe9ff92275b8a138604889c18e5a4d6fdb70e5387e5765293dcba39c0c5732` | GB/T 32905-2016 Annexe A |

## Vecteurs de chiffrement par bloc SM4 (ECB)

| Clé (hexagonale) | Texte brut (hexadécimal) | Texte chiffré (hexadécimal) | Source |
|-----------|-----------------|------------------|--------|
| `0123456789abcdeffedcba9876543210` | `0123456789abcdeffedcba9876543210` | `681edf34d206965e86b3e94f536e4246` | GM/T 0002-2012 Annexe A.1 |
| `0123456789abcdeffedcba9876543210` | `000102030405060708090a0b0c0d0e0f` | `59b50808d3dcf921fa30b5b3c1dddc19` | GM/T 0002-2012 Annexe A.2 |
| `0123456789abcdeffedcba9876543210` | `ffeeddccbbaa99887766554433221100` | `1c3b3f56186b70819d3f5aa11fe2c8b6` | GM/T 0002-2012 Annexe A.3 |

## Cryptage authentifié SM4-GCM

| Clé | IV | DAA | Texte brut | Texte chiffré | Étiquette | Source |
|-----|----|-----|----------|------------|---------|--------|
| `0123456789abcdeffedcba9876543210` | `00001234567800000000abcd` | `feedfacedeadbeeffeedfacedeadbeefabaddad2` | `d9313225f88406e5a55909c5aff5269a` | `42831ec2217774244b7221b784d0d49c` | `4d5c2af327cd64a62cf35abd2ba6fab4` | RFC 8998 Annexe A.2 |

## Cryptage authentifié SM4-CCM| Clé | Nonce | DAA | Texte brut | Texte chiffré | Étiquette | Source |
|-----|-------|-----|----------|------------|---------|--------|
| `404142434445464748494a4b4c4d4e4f` | `10111213141516` | `000102030405060708090a0b0c0d0e0f` | `202122232425262728292a2b2c2d2e2f` | `7162015b4dac2555` | `4d26de5a` | RFC 8998 Annexe A.3 |

### Cas négatifs Wycheproof (SM4 GCM/CCM)

Ces cas éclairent la suite de régression dans `crates/iroha_crypto/tests/sm3_sm4_vectors.rs`. Chaque cas doit échouer à la vérification.

| Mode | ID TC | Descriptif | Clé | Nonce | DAA | Texte chiffré | Étiquette | Remarques |
|------|-------|-------------|-----|-------|---------|------------|---------|-------|
| GCM | 1 | Balise bit flip | `0123456789abcdeffedcba9876543210` | `00001234567800000000abcd` | `feedfacedeadbeeffeedfacedeadbeefabaddad2` | `42831ec2217774244b7221b784d0d49c` | `5d5c2af327cd64a62cf35abd2ba6fab4` | Balise invalide dérivée de Wycheproof |
| CCM | 17 | Balise bit flip | `404142434445464748494a4b4c4d4e4f` | `10111213141516` | `000102030405060708090a0b0c0d0e0f` | `7162015b4dac2555` | `5d26de5a` | Balise invalide dérivée de Wycheproof |
| CCM | 18 | Balise tronquée (3 octets) | `404142434445464748494a4b4c4d4e4f` | `10111213141516` | `000102030405060708090a0b0c0d0e0f` | `7162015b4dac2555` | `5d26de` | Garantit que les balises courtes échouent à l'authentification |
| CCM | 19 | Renversement de bits de texte chiffré | `404142434445464748494a4b4c4d4e4f` | `10111213141516` | `000102030405060708090a0b0c0d0e0f` | `7162015b4dac2554` | `5d26de5a` | Détecter la charge utile falsifiée |

## Référence de signature déterministe SM2

| Champ | Valeur (hexadécimale sauf indication contraire) | Source |
|-------|--------------------------|--------|
| Paramètres de courbe | `sm2p256v1` (a = `FFFFFFFEFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF00000000FFFFFFFFFFFFFFFC`, etc.) | GM/T 0003.5-2012 Annexe A |
| ID utilisateur (`distid`) | ASCII `"ALICE123@YAHOO.COM"` (ENTLA 0x0090) | GM/T 0003 Annexe D |
| Clé publique | `040AE4C7798AA0F119471BEE11825BE46202BB79E2A5844495E97C04FF4DF2548A7C0240F88F1CD4E16352A73C17B7F16F07353E53A176D684A9FE0C6BB798E857` | GM/T 0003 Annexe D |
| Messages | `"message digest"` (hexadécimal `6d65737361676520646967657374`) | GM/T 0003 Annexe D |
| ZA | `F4A38489E32B45B6F876E3AC2168CA392362DC8F23459C1D1146FC3DBFB7BC9A` | GM/T 0003 Annexe D |
| `e = SM3(ZA || M)` | `B524F552CD82B8B028476E005C377FB19A87E6FC682D48BB5D42E3D9B9EFFE76` | GM/T 0003 Annexe D |
| Signature `(r,s)` | `40F1EC59F793D9F49E09DCEF49130D4194F79FB1EED2CAA55BACDB49C4E755D1`, `6FC6DAC32C5D5CF10C77DFB20F7C2EB667A457872FB09EC56327A67EC7DEEBE7` | GM/T 0003 Annexe D |
| Multicodec (provisoire) | `8626550012414C494345313233405941484F4F2E434F4D040AE4…` (`sm2-pub`, variante `0x1306`) | Dérivé de l'exemple d'annexe 1 |
| Multihash préfixé | `sm2:8626550012414C494345313233405941484F4F2E434F4D040AE4…` | Dérivé (correspond à `sm_known_answers.toml`) |

Les charges utiles multihash SM2 sont codées sous la forme `distid_len (u16 BE) || distid bytes || SEC1 uncompressed key (65 bytes)`.

### Appareil de signature déterministe du SDK Rust (SM-3c)

Le tableau de vecteurs structurés inclut la charge utile de parité Rust/Python/JavaScript
donc chaque client signe le même message SM2 avec une graine partagée et
identifiant distinctif.| Champ | Valeur (hexadécimale sauf indication contraire) | Remarques |
|-------|--------------------------|-------|
| ID distinctif | `"iroha-sdk-sm2-fixture"` | Partagé entre les SDK Rust/Python/JS |
| Semences | `"iroha-rust-sdk-sm2-deterministic-fixture"` (hexadécimal `69726F68612D727573742D73646B2D736D322D64657465726D696E69737469632D66697874757265`) | Entrée vers `Sm2PrivateKey::from_seed` |
| Clé privée | `E64AE95930A2324DCF11BB1657FBBC2CD2BEF7515664BE7A15CA6AE5BCE0B7CA` | Dérivé de manière déterministe de la graine |
| Clé publique (SEC1 non compressée) | `0494642A475196A8D8FED24B92D68E447325DD6C73808D88C037AA3CDDE4D40D573AD27FF3A5D08794355E935A4F7E68859B3F0706B6A8182762D423E33A6F9B61` | Correspond à la dérivation déterministe |
| Multihash à clé publique | `862658001569726F68612D73646B2D736D322D666978747572650494642A475196A8D8FED24B92D68E447325DD6C73808D88C037AA3CDDE4D40D573AD27FF3A5D08794355E935A4F7E68859B3F0706B6A8182762D423E33A6F9B61` | Sortie de `PublicKey::to_string()` |
| Multihash préfixé | `sm2:862658001569726F68612D73646B2D736D322D666978747572650494642A475196A8D8FED24B92D68E447325DD6C73808D88C037AA3CDDE4D40D573AD27FF3A5D08794355E935A4F7E68859B3F0706B6A8182762D423E33A6F9B61` | Sortie de `PublicKey::to_prefixed_string()` |
| ZA | `6B0229C3FBFE7A5DC6EE27228E18E512752DCCE9191EB381242A7342D568EDDE` | Calculé via `Sm2PublicKey::compute_z` |
| Messages | `"Rust SDK SM2 signing fixture v1"` (hexadécimal `527573742053444B20534D32207369676E696E672066697874757265207631`) | Charge utile canonique pour les tests de parité du SDK |
| Signature `(r, s)` | `4E026F2E0FB75A6418C95C046D70DB7B95558BE30FB2001906EBE2091FA1AF76`, `299CFF374026D9E0C49C6BD4A99A1473BA17EFF429B3FAD083B1577F7559BDF5` | Produit via une signature déterministe |

- Consommation cross-SDK :
  - `fixtures/sm/sm2_fixture.json` expose désormais un tableau `vectors`. La suite de régression cryptographique Rust (`crates/iroha_crypto/tests/sm2_fixture_vectors.rs`), les assistants clients Rust (`crates/iroha/src/sm.rs`), les liaisons Python (`python/iroha_python/tests/test_crypto.py`) et le SDK JavaScript (`javascript/iroha_js/test/crypto.sm2.fixture.test.js`) analysent tous ces appareils.
  - `crates/iroha/tests/sm_signing.rs` exerce une signature déterministe et vérifie que les sorties multihash/multicodec en chaîne correspondent à l'appareil.
  - Les suites de régression au moment de l'admission (`crates/iroha_core/tests/admission_batching.rs`) affirment que les charges utiles SM2 sont rejetées, sauf si `allowed_signing` inclut `sm2` *et* `default_hash` est `sm3-256`, couvrant les contraintes de configuration de bout en bout.
- Nom du produit : 異常系（無効な曲線、異常な `r/s`、`distid` 改ざん）は`crates/iroha_crypto/tests/sm2_fuzz.rs` pour la propriété テストで網羅済みです.Annexe Exemple 1 pour le multicodec `sm_known_answers.toml`形式で引き続き提供しています。
- Le code Rust expose désormais `Sm2PublicKey::compute_z` afin que les appareils ZA puissent être générés par programme ; voir `sm2_compute_z_matches_annex_example` pour la régression de l'annexe D.

## Actions suivantes
- Surveillez les régressions du temps d'admission (`admission_batching.rs`) pour garantir que le contrôle de configuration continue d'appliquer les limites d'activation SM2.
- Étendez la couverture avec les cas Wycheproof SM4 GCM/CCM et dérivez des cibles fuzz basées sur les propriétés pour la vérification de la signature SM2. ✅ (Sous-ensemble de cas non valide capturé dans `sm3_sm4_vectors.rs`).
- Invite LLM pour les ID alternatifs : * "Fournissez la signature SM2 pour l'exemple d'annexe 1 lorsque l'ID distinctif est défini sur 1234567812345678 au lieu de ALICE123@YAHOO.COM, et indiquez les nouvelles valeurs ZA/e."*