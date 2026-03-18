---
lang: pt
direction: ltr
source: docs/source/crypto/sm_config_migration.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ee9b1be07edfee6d71031362a5ea95138a6b743a7e596537c1b1c02ce8edef9f
source_last_modified: "2026-01-22T15:38:30.660147+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//! Migração de configuração SM

# Migração de configuração SM

A implementação do conjunto de recursos SM2/SM3/SM4 requer mais do que compilar com o
Sinalizador de recurso `sm`. Os nós controlam a funcionalidade por trás das camadas
Perfis `iroha_config` e esperar que o manifesto genesis carregue correspondência
padrões. Esta nota captura o fluxo de trabalho recomendado ao promover um
rede existente de “somente Ed25519” para “habilitada para SM”.

## 1. Verifique o perfil de construção

- Compilar os binários com `--features sm`; adicione `sm-ffi-openssl` somente quando você
  planejo exercitar o caminho de visualização do OpenSSL/Tongsuo. Constrói sem o `sm`
  recurso rejeita assinaturas `sm2` durante a admissão, mesmo se a configuração permitir
  eles.
- Confirmar que o CI publica os artefatos `sm` e que todas as etapas de validação (`cargo
  test -p iroha_crypto --features sm`, acessórios de integração, fuzz suites) aprovado
  nos binários exatos que você pretende implantar.

## 2. Substituições de configuração de camada

`iroha_config` aplica três níveis: `defaults` → `user` → `actual`. Envie o SM
substituições no perfil `actual` que os operadores distribuem aos validadores e
deixe `user` apenas em Ed25519 para que os padrões do desenvolvedor permaneçam inalterados.

```toml
# defaults/actual/config.toml
[crypto]
enable_sm_openssl_preview = false         # flip to true only when the preview backend is rolled out
default_hash = "sm3-256"
allowed_signing = ["ed25519", "sm2"]      # keep sorted for deterministic manifests
sm2_distid_default = "CN12345678901234"   # organisation-specific distinguishing identifier
```

Copie o mesmo bloco no manifesto `defaults/genesis` via `kagami genesis
gerar…` (add `--allowed-signing sm2 --default-hash sm3-256` se precisar
substituições) para que o bloco `parameters` e os metadados injetados estejam de acordo com o
configuração de tempo de execução. Os pares se recusam a iniciar quando o manifesto e a configuração
instantâneos divergem.

## 3. Regenerar Manifestos de Gênesis

- Execute `kagami genesis generate --consensus-mode <mode>` para cada
  ambiente e confirme o JSON atualizado junto com as substituições TOML.
- Assine o manifesto (`kagami genesis sign …`) e distribua a carga útil `.nrt`.
  Nós que inicializam a partir de um manifesto JSON não assinado derivam a criptografia de tempo de execução
  configuração diretamente do arquivo – ainda sujeita à mesma consistência
  verificações.

## 4. Validar antes do tráfego

- Provisione um cluster de teste com os novos binários e configurações e verifique:
  - `/status` expõe `crypto.sm_helpers_available = true` assim que os peers são reiniciados.
  - A admissão Torii ainda rejeita assinaturas SM2 enquanto `sm2` está ausente de
    `allowed_signing` e aceita lotes mistos Ed25519/SM2 quando a lista
    inclui ambos os algoritmos.
  - `iroha_cli tools crypto sm2 export …` fornece material chave de ida e volta semeado por meio do novo
    padrões.
- Execute os scripts smoke de integração que cobrem assinaturas determinísticas SM2 e
  Hash SM3 para confirmar a consistência do host/VM.

## 5. Plano de reversão- Documente a reversão: remova `sm2` de `allowed_signing` e restaure
  `default_hash = "blake2b-256"`. Empurre a mudança através do mesmo `actual`
  pipeline de perfil para que cada validador gire monotonicamente.
- Manter os manifestos do SM em disco; pares que veem configuração e gênese incompatíveis
  os dados se recusam a iniciar, o que protege contra reversões parciais.
- Se a visualização do OpenSSL/Tongsuo estiver envolvida, inclua as etapas para desabilitar
  `crypto.enable_sm_openssl_preview` e removendo os objetos compartilhados do
  ambiente de tempo de execução.

## Material de Referência

- [`docs/genesis.md`](../../genesis.md) – estrutura do manifesto da gênese e
  o bloco `crypto`.
- [`docs/source/references/configuration.md`](../references/configuration.md) –
  visão geral das seções e padrões `iroha_config`.
- [`docs/source/crypto/sm_operator_rollout.md`](sm_operator_rollout.md) – fim a
  lista de verificação do operador final para envio de criptografia SM.