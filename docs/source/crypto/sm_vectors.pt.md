---
lang: pt
direction: ltr
source: docs/source/crypto/sm_vectors.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: fd46471945188bcb95c8ee411c48acc8915a92b408df196caa65bf25f0596732
source_last_modified: "2026-01-05T10:35:36.441790+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//! Vetores de teste de referência para trabalho de integração SM2/SM3/SM4.

# Notas de preparação de vetores SM

Este documento agrega testes de respostas conhecidas disponíveis publicamente que propagam os chicotes SM2/SM3/SM4 antes que os scripts de importação automatizados cheguem. Cópias legíveis por máquina residem em:

- `crates/iroha_crypto/tests/fixtures/sm_known_answers.toml` (vetores de anexo, casos RFC 8998, exemplo de anexo 1).
- `fixtures/sm/sm2_fixture.json` (acessório SDK determinístico compartilhado consumido por testes Rust/Python/JavaScript).
- `crates/iroha_crypto/tests/fixtures/wycheproof_sm2.json` — corpus de 52 casos com curadoria (acessórios determinísticos + negativos sintetizados de inversão de bits/mensagem/truncamento de cauda) espelhados no Apache-2.0 enquanto o conjunto SM2 upstream está pendente. `crates/iroha_crypto/tests/sm2_wycheproof.rs` verifica esses vetores usando o verificador SM2 padrão quando possível e recorre a uma implementação BigInt pura do domínio Anexo quando necessário.

## Verificação de assinatura SM2 contra OpenSSL / Tongsuo / GmSSL

O Exemplo de Anexo 1 (Fp-256) usa a identidade `ALICE123@YAHOO.COM` (ENTLA 0x0090), a mensagem `"message digest"` e a chave pública mostrada abaixo. Um fluxo de trabalho de copiar e colar OpenSSL/Tongsuo é:

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

* OpenSSL 3.x documenta as opções `distid:`/`hexdistid:`. Algumas versões do OpenSSL 1.1.1 expõem o botão como `sm2_id:` — use o que aparecer em `openssl pkeyutl -help`.
* GmSSL exporta a mesma superfície `pkeyutl`; compilações mais antigas também aceitaram `-pkeyopt sm2_id:...`.
* LibreSSL (padrão no macOS/OpenBSD) **não** implementa SM2/SM3, então o comando acima falha aí. Use OpenSSL ≥ 1.1.1, Tongsuo ou GmSSL.

O auxiliar DER emite `3044022040F1EC59F793D9F49E09DCEF49130D4194F79FB1EED2CAA55BACDB49C4E755D102206FC6DAC32C5D5CF10C77DFB20F7C2EB667A457872FB09EC56327A67EC7DEEBE7`, que corresponde à assinatura do anexo.

O anexo também imprime o hash de informações do usuário e o resumo resultante:

```
ZA = F4A38489E32B45B6F876E3AC2168CA392362DC8F23459C1D1146FC3DBFB7BC9A
e  = SM3(ZA || "message digest")
   = B524F552CD82B8B028476E005C377FB19A87E6FC682D48BB5D42E3D9B9EFFE76
```

Você pode confirmar com OpenSSL:

```bash
echo -n 'F4A38489E32B45B6F876E3AC2168CA392362DC8F23459C1D1146FC3DBFB7BC9A' \
  | xxd -r -p > za.bin
printf "message digest" >> za.bin
openssl dgst -sm3 -binary za.bin | xxd -p -c 256
# -> b524f552cd82b8b028476e005c377fb19a87e6fc682d48bb5d42e3d9b9effe76
```

Verificação de sanidade do Python para a equação da curva:

```python
p = int("FFFFFFFEFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF00000000FFFFFFFFFFFFFFFF", 16)
a = int("FFFFFFFEFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF00000000FFFFFFFFFFFFFFFC", 16)
b = int("28E9FA9E9D9F5E344D5A9E4BCF6509A7F39789F515AB8F92DDBCBD414D940E93", 16)
x = int("0AE4C7798AA0F119471BEE11825BE46202BB79E2A5844495E97C04FF4DF2548A", 16)
y = int("7C0240F88F1CD4E16352A73C17B7F16F07353E53A176D684A9FE0C6BB798E857", 16)
assert (pow(y, 2, p) - (pow(x, 3, p) + a * x + b)) % p == 0
```

## Vetores hash SM3

| Entrada | Codificação hexadecimal | Resumo (hex) | Fonte |
|-------|-------------|--------------|--------|
| `""` (sequência vazia) | `""` | `1ab21d8355cfa17f8e61194831e81a8f22bec8c728fefb747ed035eb5082aa2b` | GM/T 0004-2012 Anexo A.1 |
| `"abc"` | `616263` | `66c7f0f462eeedd9d1f2d46bdc10e4e24167c4875cf2f7a2297da02b8f4ba8e0` | GM/T 0004-2012 Anexo A.2 |
| `"abcd"` repetido 16 vezes (64 bytes) | `61626364` ×16 | `debe9ff92275b8a138604889c18e5a4d6fdb70e5387e5765293dcba39c0c5732` | GB/T 32905-2016 Anexo A |

## Vetores de cifra de bloco SM4 (ECB)

| Chave (hexadecimal) | Texto simples (hexadecimal) | Texto cifrado (hex) | Fonte |
|-----------|-----------------|------------------|--------|
| `0123456789abcdeffedcba9876543210` | `0123456789abcdeffedcba9876543210` | `681edf34d206965e86b3e94f536e4246` | GM/T 0002-2012 Anexo A.1 |
| `0123456789abcdeffedcba9876543210` | `000102030405060708090a0b0c0d0e0f` | `59b50808d3dcf921fa30b5b3c1dddc19` | GM/T 0002-2012 Anexo A.2 |
| `0123456789abcdeffedcba9876543210` | `ffeeddccbbaa99887766554433221100` | `1c3b3f56186b70819d3f5aa11fe2c8b6` | GM/T 0002-2012 Anexo A.3 |

## Criptografia autenticada SM4-GCM

| Chave | IV | DAA | Texto simples | Texto cifrado | Etiqueta | Fonte |
|-----|----|-----|-----------|------------|-----|--------|
| `0123456789abcdeffedcba9876543210` | `00001234567800000000abcd` | `feedfacedeadbeeffeedfacedeadbeefabaddad2` | `d9313225f88406e5a55909c5aff5269a` | `42831ec2217774244b7221b784d0d49c` | `4d5c2af327cd64a62cf35abd2ba6fab4` | RFC 8998 Apêndice A.2 |

## Criptografia autenticada SM4-CCM| Chave | Nonce | DAA | Texto simples | Texto cifrado | Etiqueta | Fonte |
|-----|-------|-----|-----------|------------|-----|--------|
| `404142434445464748494a4b4c4d4e4f` | `10111213141516` | `000102030405060708090a0b0c0d0e0f` | `202122232425262728292a2b2c2d2e2f` | `7162015b4dac2555` | `4d26de5a` | RFC 8998 Apêndice A.3 |

### Casos negativos à prova de Wyche (SM4 GCM/CCM)

Esses casos informam o conjunto de regressões em `crates/iroha_crypto/tests/sm3_sm4_vectors.rs`. Cada caso deve falhar na verificação.

| Modo | ID do TC | Descrição | Chave | Nonce | DAA | Texto cifrado | Etiqueta | Notas |
|------|-------|-------------|-----|-------|-----|------------|-----|-------|
| GCM | 1 | Tag bit flip | `0123456789abcdeffedcba9876543210` | `00001234567800000000abcd` | `feedfacedeadbeeffeedfacedeadbeefabaddad2` | `42831ec2217774244b7221b784d0d49c` | `5d5c2af327cd64a62cf35abd2ba6fab4` | Tag inválida derivada de Wycheproof |
| CCM | 17 | Tag bit flip | `404142434445464748494a4b4c4d4e4f` | `10111213141516` | `000102030405060708090a0b0c0d0e0f` | `7162015b4dac2555` | `5d26de5a` | Tag inválida derivada de Wycheproof |
| CCM | 18 | Tag truncada (3 bytes) | `404142434445464748494a4b4c4d4e4f` | `10111213141516` | `000102030405060708090a0b0c0d0e0f` | `7162015b4dac2555` | `5d26de` | Garante que tags curtas falhem na autenticação |
| CCM | 19 | Inversão de bit de texto cifrado | `404142434445464748494a4b4c4d4e4f` | `10111213141516` | `000102030405060708090a0b0c0d0e0f` | `7162015b4dac2554` | `5d26de5a` | Detectar carga útil adulterada |

## Referência de assinatura determinística SM2

| Campo | Valor (hexadecimal, salvo indicação em contrário) | Fonte |
|-------|--------------------------|--------|
| Parâmetros de curva | `sm2p256v1` (a = `FFFFFFFEFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF00000000FFFFFFFFFFFFFFFC`, etc.) | GM/T 0003.5-2012 Anexo A |
| ID do usuário (`distid`) | ASCII `"ALICE123@YAHOO.COM"` (ENTLA 0x0090) | GM/T 0003 Anexo D |
| Chave pública | `040AE4C7798AA0F119471BEE11825BE46202BB79E2A5844495E97C04FF4DF2548A7C0240F88F1CD4E16352A73C17B7F16F07353E53A176D684A9FE0C6BB798E857` | GM/T 0003 Anexo D |
| Mensagem | `"message digest"` (hexadecimal `6d65737361676520646967657374`) | GM/T 0003 Anexo D |
| ZA | `F4A38489E32B45B6F876E3AC2168CA392362DC8F23459C1D1146FC3DBFB7BC9A` | GM/T 0003 Anexo D |
| `e = SM3(ZA || M)` | `B524F552CD82B8B028476E005C377FB19A87E6FC682D48BB5D42E3D9B9EFFE76` | GM/T 0003 Anexo D |
| Assinatura `(r,s)` | `40F1EC59F793D9F49E09DCEF49130D4194F79FB1EED2CAA55BACDB49C4E755D1`, `6FC6DAC32C5D5CF10C77DFB20F7C2EB667A457872FB09EC56327A67EC7DEEBE7` | GM/T 0003 Anexo D |
| Multicodec (provisório) | `8626550012414C494345313233405941484F4F2E434F4D040AE4…` (`sm2-pub`, variante `0x1306`) | Derivado do Anexo Exemplo 1 |
| Multihash prefixado | `sm2:8626550012414C494345313233405941484F4F2E434F4D040AE4…` | Derivado (corresponde a `sm_known_answers.toml`) |

As cargas multihash SM2 são codificadas como `distid_len (u16 BE) || distid bytes || SEC1 uncompressed key (65 bytes)`.

### Dispositivo de assinatura determinística Rust SDK (SM-3c)

A matriz de vetores estruturados inclui a carga útil de paridade Rust/Python/JavaScript
então cada cliente assina a mesma mensagem SM2 com uma semente compartilhada e
identificador distintivo.| Campo | Valor (hexadecimal, salvo indicação em contrário) | Notas |
|-------|--------------------------|-------|
| ID distintiva | `"iroha-sdk-sm2-fixture"` | Compartilhado entre SDKs Rust/Python/JS |
| Semente | `"iroha-rust-sdk-sm2-deterministic-fixture"` (hexadecimal `69726F68612D727573742D73646B2D736D322D64657465726D696E69737469632D66697874757265`) | Entrada para `Sm2PrivateKey::from_seed` |
| Chave privada | `E64AE95930A2324DCF11BB1657FBBC2CD2BEF7515664BE7A15CA6AE5BCE0B7CA` | Derivado deterministicamente da semente |
| Chave pública (SEC1 não comprimida) | `0494642A475196A8D8FED24B92D68E447325DD6C73808D88C037AA3CDDE4D40D573AD27FF3A5D08794355E935A4F7E68859B3F0706B6A8182762D423E33A6F9B61` | Corresponde à derivação determinística |
| Multihash de chave pública | `862658001569726F68612D73646B2D736D322D666978747572650494642A475196A8D8FED24B92D68E447325DD6C73808D88C037AA3CDDE4D40D573AD27FF3A5D08794355E935A4F7E68859B3F0706B6A8182762D423E33A6F9B61` | Saída de `PublicKey::to_string()` |
| Multihash prefixado | `sm2:862658001569726F68612D73646B2D736D322D666978747572650494642A475196A8D8FED24B92D68E447325DD6C73808D88C037AA3CDDE4D40D573AD27FF3A5D08794355E935A4F7E68859B3F0706B6A8182762D423E33A6F9B61` | Saída de `PublicKey::to_prefixed_string()` |
| ZA | `6B0229C3FBFE7A5DC6EE27228E18E512752DCCE9191EB381242A7342D568EDDE` | Calculado via `Sm2PublicKey::compute_z` |
| Mensagem | `"Rust SDK SM2 signing fixture v1"` (hexadecimal `527573742053444B20534D32207369676E696E672066697874757265207631`) | Carga canônica para testes de paridade SDK |
| Assinatura `(r, s)` | `4E026F2E0FB75A6418C95C046D70DB7B95558BE30FB2001906EBE2091FA1AF76`, `299CFF374026D9E0C49C6BD4A99A1473BA17EFF429B3FAD083B1577F7559BDF5` | Produzido via assinatura determinística |

- Consumo entre SDKs:
  - `fixtures/sm/sm2_fixture.json` agora expõe uma matriz `vectors`. O conjunto de regressão criptográfica Rust (`crates/iroha_crypto/tests/sm2_fixture_vectors.rs`), os auxiliares de cliente Rust (`crates/iroha/src/sm.rs`), as ligações Python (`python/iroha_python/tests/test_crypto.py`) e o SDK JavaScript (`javascript/iroha_js/test/crypto.sm2.fixture.test.js`) analisam esses acessórios.
  - `crates/iroha/tests/sm_signing.rs` exerce assinatura determinística e verifica se as saídas multihash/multicodec na cadeia correspondem ao equipamento.
  - Os conjuntos de regressão de tempo de admissão (`crates/iroha_core/tests/admission_batching.rs`) afirmam que as cargas úteis do SM2 são rejeitadas, a menos que `allowed_signing` inclua `sm2` *e* `default_hash` seja `sm3-256`, cobrindo as restrições de configuração de ponta a ponta.
- 追加カバレッジ: 異常系(無効な曲線、異常な `r/s`、`distid` 改ざん）は`crates/iroha_crypto/tests/sm2_fuzz.rs` na propriedade テストで網羅済みです。Anexo Exemplo 1 の正規ベクトルは `sm_known_answers.toml` に多言語対応のmulticodec 形式で引き続き提供しています。
- O código Rust agora expõe `Sm2PublicKey::compute_z` para que os fixtures ZA possam ser gerados programaticamente; consulte `sm2_compute_z_matches_annex_example` para a regressão do Anexo D.

## Próximas ações
- Monitore regressões de tempo de admissão (`admission_batching.rs`) para garantir que o controle de configuração continue a impor os limites de ativação do SM2.
- Amplie a cobertura com casos Wycheproof SM4 GCM/CCM e obtenha alvos fuzz baseados em propriedades para verificação de assinatura SM2. ✅ (Subconjunto de casos inválidos capturado em `sm3_sm4_vectors.rs`).
- Solicitação do LLM para IDs alternativos: *"Forneça a assinatura SM2 para o Anexo Exemplo 1 quando o ID diferenciador for definido como 1234567812345678 em vez de ALICE123@YAHOO.COM e descreva os novos valores ZA/e."*