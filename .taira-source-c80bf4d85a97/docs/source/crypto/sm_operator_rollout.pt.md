---
lang: pt
direction: ltr
source: docs/source/crypto/sm_operator_rollout.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: dffc2cf6c6e59f54d1fc22136ba93f75466509c699a4361a381bf7e0ce0d1dda
source_last_modified: "2026-01-03T18:07:57.089544+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Lista de verificação de implementação de recurso SM e telemetria

Esta lista de verificação ajuda as equipes de SRE e operadores a habilitar o recurso SM (SM2/SM3/SM4)
definido com segurança assim que as portas de auditoria e conformidade forem liberadas. Siga este documento
junto com o resumo de configuração em `docs/source/crypto/sm_program.md` e o
orientação legal/de exportação em `docs/source/crypto/sm_compliance_brief.md`.

## 1. Preparação pré-voo
- [] Confirme se as notas de versão do espaço de trabalho mostram `sm` como somente verificação ou assinatura,
      dependendo do estágio de implementação.
- [] Verifique se a frota está executando binários criados a partir de um commit que inclui o
      Contadores de telemetria SM e botões de configuração. (Lançamento alvo a definir; rastrear
      no ticket de lançamento.)
- [] Execute `scripts/sm_perf.sh --tolerance 0.25` em um nó de teste (por destino
      arquitetura) e arquive a saída resumida. O script agora seleciona automaticamente
      a linha de base escalar como alvo de comparação para modos de aceleração
      (O padrão `--compare-tolerance` é 5,25 enquanto o SM3 NEON funciona);
      investigue ou bloqueie a implementação se o principal ou o de comparação
      guarda falha. Ao capturar em hardware Linux/aarch64 Neoverse, passe
      `--baseline crates/iroha_crypto/benches/sm_perf_baseline_aarch64_unknown_linux_gnu_<mode>.json --write-baseline`
      para substituir as medianas `m3-pro-native` exportadas pela captura do host
      antes do envio.
- [ ] Garantir que `status.md` e o tíquete de implementação registrem os registros de conformidade para
      quaisquer nós operando em jurisdições que os exijam (consulte o resumo de conformidade).
- [] Prepare atualizações KMS/HSM se os validadores armazenarem chaves de assinatura SM
      módulos de hardware.

## 2. Mudanças de configuração
1. Execute o auxiliar xtask para gerar o inventário de chaves SM2 e o snippet pronto para colar:
   ```bash
   cargo xtask sm-operator-snippet \
     --distid CN12345678901234 \
     --json-out sm2-key.json \
     --snippet-out client-sm2.toml
   ```
   Use `--snippet-out -` (e opcionalmente `--json-out -`) para transmitir as saídas para stdout quando você precisar apenas inspecioná-las.
   Se você preferir acionar manualmente os comandos CLI de nível inferior, o fluxo equivalente é:
   ```bash
   cargo run -p iroha_cli --features sm -- \
     crypto sm2 keygen \
     --distid CN12345678901234 \
     --output sm2-key.json

   cargo run -p iroha_cli --features sm -- \
     crypto sm2 export \
     --private-key-hex "$(jq -r .private_key_hex sm2-key.json)" \
     --distid CN12345678901234 \
     --snippet-output client-sm2.toml \
     --emit-json --quiet
   ```
   Se `jq` não estiver disponível, abra `sm2-key.json`, copie o valor `private_key_hex` e passe-o diretamente para o comando de exportação.
2. Adicione o snippet resultante à configuração de cada nó (valores mostrados para o
   estágio somente de verificação; ajuste por ambiente e mantenha as chaves ordenadas conforme mostrado):
```toml
[crypto]
default_hash = "sm3-256"
allowed_signing = ["ed25519", "sm2"]   # remove "sm2" to stay in verify-only mode
sm2_distid_default = "1234567812345678"
# enable_sm_openssl_preview = true  # optional: only when deploying the OpenSSL/Tongsuo path
```
3. Reinicie o nó e confirme `crypto.sm_helpers_available` e (se você ativou o back-end de visualização) a superfície `crypto.sm_openssl_preview_enabled` conforme esperado em:
   - `/status` JSON (`"crypto":{"sm_helpers_available":true,"sm_openssl_preview_enabled":true,...}`).
   - O `config.toml` renderizado para cada nó.
4. Atualize entradas de manifestos/gênese para adicionar algoritmos SM à lista de permissões se
   a assinatura é habilitada posteriormente na implementação. Ao usar `--genesis-manifest-json`
   sem um bloco de gênese pré-assinado, `irohad` agora semeia a criptografia de tempo de execução
   snapshot diretamente do bloco `crypto` do manifesto — certifique-se de que o manifesto esteja
   verificou seu plano de mudança antes de avançar.## 3. Telemetria e monitoramento
- Raspe os endpoints Prometheus e certifique-se de que os seguintes contadores/medidores apareçam:
  -`iroha_sm_syscall_total{kind="verify"}`
  -`iroha_sm_syscall_total{kind="hash"}`
  -`iroha_sm_syscall_total{kind="seal|open",mode="gcm|ccm"}`
  - `iroha_sm_openssl_preview` (medidor 0/1 relatando o estado de alternância de visualização)
  -`iroha_sm_syscall_failures_total{kind="verify|hash|seal|open",reason="..."}`
- Caminho de assinatura do gancho quando a assinatura SM2 estiver habilitada; adicionar contadores para
  `iroha_sm_sign_total` e `iroha_sm_sign_failures_total`.
- Criar painéis/alertas Grafana para:
  - Picos nos contadores de falhas (janela 5m).
  - Quedas repentinas na taxa de transferência do syscall do SM.
  - Diferenças entre nós (por exemplo, habilitação incompatível).

## 4. Etapas de implementação
| Fase | Ações | Notas |
|-------|---------|-------|
| Somente verificação | Atualize `crypto.default_hash` para `sm3-256`, deixe `allowed_signing` sem `sm2`, monitore os contadores de verificação. | Objetivo: exercitar caminhos de verificação de SM sem arriscar divergência de consenso. |
| Piloto de assinatura mista | Permitir assinatura SM limitada (subconjunto de validadores); monitorar contadores de assinatura e latência. | Certifique-se de que o substituto para Ed25519 permaneça disponível; interromper se a telemetria mostrar incompatibilidades. |
| Assinatura GA | Estenda `allowed_signing` para incluir `sm2`, atualizar manifestos/SDKs e publicar runbook final. | Requer resultados de auditoria fechados, registros de conformidade atualizados e telemetria estável. |

### Avaliações de prontidão
- **Prontidão somente para verificação (SM-RR1).** Convocar Release Eng, Crypto WG, Ops e Legal. Exigir:
  - `status.md` observa o status de arquivamento de conformidade + proveniência do OpenSSL.
  - `docs/source/crypto/sm_program.md` / `sm_compliance_brief.md` / esta lista de verificação foi atualizada na última janela de lançamento.
  - `defaults/genesis` ou o manifesto específico do ambiente mostra `crypto.allowed_signing = ["ed25519","sm2"]` e `crypto.default_hash = "sm3-256"` (ou a variante somente verificação sem `sm2` se ainda estiver no estágio um).
  - Logs `scripts/sm_openssl_smoke.sh` + `scripts/sm_interop_matrix.sh` anexados ao ticket de implementação.
  - Painel de telemetria (`iroha_sm_*`) revisado para comportamento em estado estacionário.
- **Assinatura de prontidão do piloto (SM-RR2).** Portões adicionais:
  - Relatório de auditoria para pilha RustCrypto SM fechada ou RFC para controles de compensação assinados pela Segurança.
  - Runbooks do operador (específicos da instalação) atualizados com etapas de fallback/reversão de assinatura.
  - Os manifestos Genesis para a coorte piloto incluem `allowed_signing = ["ed25519","sm2"]` e a lista de permissões é espelhada em cada configuração de nó.
  - Plano de saída/reversão documentado (mudar `allowed_signing` de volta para Ed25519, restaurar manifestos, redefinir painéis).
- **Prontidão GA (SM-RR3).** Requer relatório piloto positivo, registros de conformidade atualizados para todas as jurisdições validadoras, linhas de base de telemetria assinadas e aprovação de tíquete de liberação da tríade Release Eng + Crypto WG + Ops/Legal.## 5. Lista de verificação de embalagem e conformidade
- **Agrupe artefatos OpenSSL/Tongsuo.** Envie bibliotecas compartilhadas OpenSSL/Tongsuo 3.0+ (`libcrypto`/`libssl`) com cada pacote de validador ou documente a dependência exata do sistema. Registre a versão, os sinalizadores de compilação e as somas de verificação SHA256 no manifesto de lançamento para que os auditores possam rastrear a compilação do fornecedor.
- **Verificar durante CI.** Adicione uma etapa de CI que execute `scripts/sm_openssl_smoke.sh` nos artefatos empacotados em cada plataforma de destino. O trabalho deverá falhar se o sinalizador de visualização estiver habilitado, mas o provedor não puder ser inicializado (cabeçalhos ausentes, algoritmo não suportado, etc.).
- **Publicar notas de conformidade.** Atualize as notas de versão / `status.md` com a versão do fornecedor empacotado, referências de controle de exportação (GM/T, GB/T) e quaisquer registros específicos de jurisdição necessários para algoritmos SM.
- **Atualizações do runbook do operador.** Documente o fluxo de atualização: prepare os novos objetos compartilhados, reinicie os pares com `crypto.enable_sm_openssl_preview = true`, confirme o campo `/status` e o medidor `iroha_sm_openssl_preview` mude para `true` e mantenha um plano de reversão (inverta o sinalizador de configuração ou reverta o pacote) se a telemetria de visualização se desviar na frota.
- **Retenção de evidências.** Arquive os logs de construção e os atestados de assinatura dos pacotes OpenSSL/Tongsuo junto com os artefatos de lançamento do validador para que auditorias futuras possam reproduzir a cadeia de procedência.

## 6. Resposta a incidentes
- **Picos de falha na verificação:** reverter para uma compilação sem suporte SM ou remover `sm2`
  de `allowed_signing` (revertendo `default_hash` conforme necessário) e faça failover para o anterior
  liberar enquanto investiga. Capture cargas com falha, hashes comparativos e logs de nós.
- **Regressões de desempenho:** compare as métricas SM com as linhas de base Ed25519/SHA2.
  Se o caminho intrínseco do ARM causar divergência, defina `crypto.sm_intrinsics = "force-disable"`
  (alternância de recursos com implementação pendente) e relatar descobertas.
- **Lacunas de telemetria:** se os contadores estiverem ausentes ou não forem atualizados, registre um problema
  contra Engenharia de Liberação; não prossiga com uma implementação mais ampla até que a lacuna
  está resolvido.

## 7. Modelo de lista de verificação
- [] Configuração preparada e ponto reiniciado.
- [] Contadores de telemetria visíveis e painéis configurados.
- [ ] Conformidade/etapas legais registradas.
- [ ] Fase de rollout aprovada pelo Crypto WG / Release TL.
- [ ] Revisão pós-lançamento concluída e resultados documentados.

Mantenha esta lista de verificação no ticket de implementação e atualize `status.md` quando o
transições da frota entre fases.