---
title: Registro de curvas de conta
description: Mapeamento canônico entre identificadores de curva do controlador de conta e algoritmos de assinatura.
---

# Registro de curvas de conta

Os endereços de conta codificam seus controladores como um payload marcado que
começa com um identificador de curva de 8 bits. Validadores, SDKs e ferramentas
dependem de um registro compartilhado para que os identificadores de curva
permaneçam estáveis entre versões e permitam uma decodificação determinística
entre implementações.

A tabela abaixo é a referência normativa para cada `curve_id` atribuído. Uma
cópia legível por máquina é publicada junto a este documento em
[`address_curve_registry.json`](address_curve_registry.json); ferramentas
automatizadas DEVEM consumir a versão JSON e fixar o campo `version` ao gerar
fixtures.

## Curvas registradas

| ID (`curve_id`) | Algoritmo | Gate de funcionalidade | Status | Codificação de chave pública | Notas |
|-----------------|-----------|------------------------|--------|------------------------------|-------|
| `0x01` (1) | `ed25519` | — | Produção | Chave Ed25519 comprimida de 32 bytes | Curva canônica para V1. Todos os SDKs DEVEM suportar este identificador. |
| `0x02` (2) | `ml-dsa` | — | Produção (controlado por configuração) | Chave pública Dilithium3 (1952 bytes) | Disponível em todas as builds. Ative em `crypto.allowed_signing` + `crypto.curves.allowed_curve_ids` antes de emitir payloads de controlador. |
| `0x03` (3) | `bls_normal` | `bls` | Produção (controlado por feature) | Chave pública G1 comprimida de 48 bytes | Necessária para validadores de consenso. A admissão permite controladores BLS mesmo quando `allowed_signing`/`allowed_curve_ids` os omitem. |
| `0x04` (4) | `secp256k1` | — | Produção | Chave SEC1 comprimida de 33 bytes | ECDSA determinístico sobre SHA-256; assinaturas usam o layout canônico de 64 bytes `r∥s`. |
| `0x05` (5) | `bls_small` | `bls` | Produção (controlado por feature) | Chave pública G2 comprimida de 96 bytes | Perfil BLS de assinatura compacta (assinaturas menores, chaves públicas maiores). |
| `0x0A` (10) | `gost3410-2012-256-paramset-a` | `gost` | Reservado | Ponto TC26 param set A de 64 bytes little-endian | Libera com a feature `gost` quando a governança aprovar o rollout. |
| `0x0B` (11) | `gost3410-2012-256-paramset-b` | `gost` | Reservado | Ponto TC26 param set B de 64 bytes little-endian | Espelha o conjunto de parâmetros TC26 B; bloqueado pelo gate `gost`. |
| `0x0C` (12) | `gost3410-2012-256-paramset-c` | `gost` | Reservado | Ponto TC26 param set C de 64 bytes little-endian | Reservado para aprovação futura da governança. |
| `0x0D` (13) | `gost3410-2012-512-paramset-a` | `gost` | Reservado | Ponto TC26 param set A de 128 bytes little-endian | Reservado aguardando demanda por curvas GOST de 512 bits. |
| `0x0E` (14) | `gost3410-2012-512-paramset-b` | `gost` | Reservado | Ponto TC26 param set B de 128 bytes little-endian | Reservado aguardando demanda por curvas GOST de 512 bits. |
| `0x0F` (15) | `sm2` | `sm` | Reservado | Comprimento DistID (u16 BE) + bytes DistID + chave SM2 SEC1 não comprimida de 65 bytes | Fica disponível quando a feature `sm` sair do preview. |

### Diretrizes de uso

- **Fail closed:** Codificadores DEVEM rejeitar algoritmos não suportados com
  `ERR_UNSUPPORTED_ALGORITHM`. Decodificadores DEVEM gerar `ERR_UNKNOWN_CURVE`
  para qualquer identificador não listado neste registro.
- **Gating de feature:** BLS/GOST/SM2 continuam atrás das features listadas em
  tempo de build. Operadores devem habilitar as entradas correspondentes em
  `iroha_config.crypto.allowed_signing` e as features de build antes de emitir
  endereços com essas curvas.
- **Exceções de admissão:** Controladores BLS são permitidos para validadores de
  consenso mesmo quando `allowed_signing`/`allowed_curve_ids` não os listam.
- **Paridade config + manifest:** Use `iroha_config.crypto.allowed_curve_ids`
  (e o `ManifestCrypto.allowed_curve_ids` correspondente) para publicar quais
  identificadores de curva o cluster aceita para controladores; a admissão agora
  aplica essa lista junto com `allowed_signing`.
- **Codificação determinística:** Chaves públicas são codificadas exatamente
  como retornadas pela implementação de assinatura (bytes comprimidos Ed25519,
  chaves públicas ML‑DSA, pontos BLS comprimidos, etc.). SDKs devem expor erros
  de validação antes de enviar payloads malformados.
- **Paridade de manifests:** Manifests de gênese e de controlador DEVEM usar os
  mesmos identificadores para que a admissão rejeite controladores que excedam
  as capacidades do cluster.

## Anúncio do bitmap de capacidades

`GET /v1/node/capabilities` agora expõe a lista `allowed_curve_ids` e o array
compactado `allowed_curve_bitmap` sob `crypto.curves`. O bitmap é little-endian
em lanes de 64 bits (até quatro valores para cobrir o espaço de identificadores
`u8` de 0–255). Um bit `i` ligado significa que o identificador de curva `i` é
permitido pela política de admissão do cluster.

- Exemplo: `{ allowed_curve_ids: [1, 15] }` ⇒ `allowed_curve_bitmap: [32770]`
  porque `(1 << 1) | (1 << 15) = 32770`.
- Curvas acima de `63` definem bits em lanes posteriores. Lanes finais zeradas
  são omitidas para manter payloads curtos, então uma configuração que também
  habilite `curve_id = 130` emitiria `allowed_curve_bitmap = [32768, 0, 4]`
  (bits 15 e 130 ativados).

Prefira o bitmap para dashboards e health checks: um único teste de bit responde
perguntas de capacidade sem varrer o array completo, enquanto ferramentas que
precisam de identificadores ordenados podem continuar usando `allowed_curve_ids`.
Expor ambas as visões atende ao requisito **ADDR-3** do roadmap de publicar
bitmaps de capacidade determinísticos para operadores e SDKs.

## Checklist de validação

Cada componente que ingere controladores (Torii, admissão, codificadores de SDK,
ferramentas offline) deve aplicar as mesmas verificações determinísticas antes
de aceitar um payload. Os passos abaixo devem ser tratados como lógica obrigatória
de validação:

1. **Resolver a política do cluster:** Analise o byte inicial `curve_id` do
   payload da conta e rejeite o controlador se o identificador não estiver em
   `iroha_config.crypto.allowed_curve_ids` (e o espelho
   `ManifestCrypto.allowed_curve_ids`). Controladores BLS são a exceção: quando
   compilados, a admissão os permite independentemente das allowlists para que
   as chaves de validadores de consenso continuem funcionando. Isso impede que
   clusters aceitem curvas em preview que operadores não habilitaram.
2. **Forçar o comprimento de codificação:** Compare o comprimento do payload com
   o tamanho canônico do algoritmo antes de tentar descomprimir ou expandir a
   chave. Rejeite qualquer valor que falhe na verificação de comprimento para
   eliminar entradas malformadas cedo.
3. **Decodificação específica do algoritmo:** Use os mesmos decodificadores
   canônicos de `iroha_crypto` (`ed25519_dalek`, `pqcrypto_mldsa`,
   `w3f_bls`/`blstrs`, `sm2`, helpers TC26, etc.) para que todas as
   implementações compartilhem o mesmo comportamento de validação de subgrupo
   e de pontos.
4. **Verificar tamanhos de assinatura:** A admissão e os SDKs devem impor os
   tamanhos de assinatura listados abaixo e rejeitar qualquer payload com
   assinatura truncada ou longa demais antes de executar o verificador.

| Algoritmo | `curve_id` | Bytes de chave pública | Bytes de assinatura | Verificações críticas |
|-----------|------------|------------------------|--------------------|-----------------------|
| `ed25519` | `0x01` | 32 | 64 | Rejeitar pontos comprimidos não canônicos, forçar limpeza do cofactor (sem pontos de ordem pequena) e garantir `s < L` ao validar assinaturas. |
| `ml-dsa` (Dilithium3) | `0x02` | 1952 | 3309 | Rejeitar payloads que não tenham exatamente 1952 bytes antes de decodificar; analisar a chave pública Dilithium3 e verificar assinaturas usando pqcrypto-mldsa com comprimentos canônicos. |
| `bls_normal` | `0x03` | 48 | 96 | Aceitar apenas chaves públicas G1 comprimidas canônicas e assinaturas G2 comprimidas; rejeitar pontos identidade e codificações não canônicas. |
| `secp256k1` | `0x04` | 33 | 64 | Aceitar apenas pontos SEC1 comprimidos; descomprimir e rejeitar pontos não canônicos/inválidos, e verificar assinaturas usando a codificação canônica de 64 bytes `r∥s` (normalização low‑`s` aplicada pelo signatário). |
| `bls_small` | `0x05` | 96 | 48 | Aceitar apenas chaves públicas G2 comprimidas canônicas e assinaturas G1 comprimidas; rejeitar pontos identidade e codificações não canônicas. |
| `gost3410-2012-256-paramset-a` | `0x0A` | 64 | 64 | Interpretar o payload como coordenadas `(x||y)` little-endian, garantir que cada coordenada `< p`, rejeitar o ponto identidade e impor limbs `r`/`s` canônicos de 32 bytes ao verificar assinaturas. |
| `gost3410-2012-256-paramset-b` | `0x0B` | 64 | 64 | Mesma validação do conjunto A, mas com os parâmetros de domínio TC26 B. |
| `gost3410-2012-256-paramset-c` | `0x0C` | 64 | 64 | Mesma validação do conjunto A, mas com os parâmetros de domínio TC26 C. |
| `gost3410-2012-512-paramset-a` | `0x0D` | 128 | 128 | Interpretar `(x||y)` como limbs de 64 bytes, garantir `< p`, rejeitar o ponto identidade e exigir limbs `r`/`s` de 64 bytes para assinaturas. |
| `gost3410-2012-512-paramset-b` | `0x0E` | 128 | 128 | Mesma validação do conjunto A, mas com os parâmetros de domínio TC26 B de 512 bits. |
| `sm2` | `0x0F` | 2 + distid + 65 | 64 | Decodificar o comprimento distid (u16 BE), validar os bytes DistID, analisar o ponto SEC1 não comprimido, aplicar regras de subgrupo GM/T 0003, aplicar o DistID configurado e exigir limbs canônicos `(r, s)` conforme SM2. |

Cada linha mapeia para o objeto `validation` em
[`address_curve_registry.json`](address_curve_registry.json). Ferramentas que
consomem a exportação JSON podem se basear nos campos `public_key_bytes`,
`signature_bytes` e `checks` para automatizar os mesmos passos de validação
acima; codificações de comprimento variável (por exemplo SM2) definem
`public_key_bytes` como null e documentam a regra de comprimento em `checks`.

## Solicitar um novo identificador de curva

1. Redija a especificação do algoritmo (codificação, validação, tratamento de
   erros) e obtenha aprovação da governança para o rollout.
2. Envie um pull request atualizando este documento e
   `address_curve_registry.json`. Novos identificadores devem ser únicos e cair
   no intervalo inclusivo `0x01..=0xFE`.
3. Atualize SDKs, fixtures Norito e a documentação de operadores com o novo
   identificador antes de implantar em redes de produção.
4. Coordene com as lideranças de segurança e observabilidade para garantir que
   telemetria, runbooks e políticas de admissão reflitam o novo algoritmo.
