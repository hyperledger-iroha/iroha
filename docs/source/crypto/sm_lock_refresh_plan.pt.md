---
lang: pt
direction: ltr
source: docs/source/crypto/sm_lock_refresh_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 3065571b34a226a5871c4fb68063f9419e48074b20096de215f440bdf54a4e59
source_last_modified: "2026-01-03T18:07:57.085103+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//! Procedimento para agendamento da atualização do Cargo.lock exigida pelo pico do SM.

# Recurso SM `Cargo.lock` Plano de atualização

O pico de recurso `sm` para `iroha_crypto` originalmente não pôde ser concluído `cargo check` enquanto `--locked` foi aplicado. Esta nota registra as etapas de coordenação para uma atualização `Cargo.lock` sancionada e rastreia o status atual dessa necessidade.

> **Atualização de 12/02/2026:** A validação recente mostra que o recurso `sm` opcional agora é compilado com o arquivo de bloqueio existente (`cargo check -p iroha_crypto --features sm --locked` tem sucesso em 7,9s frio/0,23s quente). O conjunto de dependências já contém `base64ct`, `ghash`, `opaque-debug`, `pem-rfc7468`, `pkcs8`, `polyval`, `primeorder`, `sm2`, `sm3`, `sm4` e `sm4-gcm`, portanto, nenhuma atualização imediata de bloqueio é necessária. Mantenha o procedimento abaixo em espera para futuros problemas de dependência ou novas caixas opcionais.

## Por que a atualização é necessária
- As iterações anteriores do pico exigiam a adição de caixas opcionais que estavam faltando no arquivo de bloqueio. Os instantâneos de bloqueio atuais já incluem a pilha RustCrypto (`sm2`, `sm3`, `sm4`, codecs de suporte e auxiliares AES).
- A política de repositório ainda bloqueia edições oportunistas de lockfiles; se uma futura atualização de dependência for necessária, o procedimento abaixo permanecerá aplicável.
- Mantenha este plano para que a equipe possa executar uma atualização controlada quando novas dependências relacionadas ao SM forem introduzidas ou quando as existentes precisarem de alterações de versão.

## Etapas de coordenação propostas
1. **Levantar solicitação no Crypto WG + sincronização do Release Eng (proprietário: @crypto-wg lead).**
   - Consulte `docs/source/crypto/sm_program.md` e observe a natureza opcional do recurso.
   - Confirme que não há janelas de alteração simultâneas de lockfile (por exemplo, congelamentos de dependências).
2. **Prepare o patch com diferencial de bloqueio (proprietário: @release-eng).**
   - Execute `scripts/sm_lock_refresh.sh` (após aprovação) para atualizar apenas as caixas necessárias.
   - Capture a saída `cargo tree -p iroha_crypto --features sm` (o script emite `target/sm_dep_tree.txt`).
3. **Revisão de segurança (proprietário: @security-reviews).**
   - Verifique se novas caixas/versões correspondem ao registro de auditoria e às expectativas de licenciamento.
   - Grave hashes no rastreador da cadeia de suprimentos.
4. **Mesclar execução da janela.**
   - Enviar PR contendo apenas o delta do lockfile, instantâneo da árvore de dependência (anexado como artefato) e notas de auditoria atualizadas.
   - Certifique-se de que o CI seja executado com `cargo check -p iroha_crypto --features sm` antes da mesclagem.
5. **Tarefas de acompanhamento.**
   - Atualizar lista de verificação de itens de ação `docs/source/crypto/sm_program.md`.
   - Notifique a equipe do SDK de que o recurso pode ser compilado localmente com `--features sm`.## Linha do tempo e proprietários
| Etapa | Alvo | Proprietário | Estado |
|------|--------|-------|--------|
| Solicitar vaga na agenda na próxima teleconferência do Crypto WG | 22/01/2025 | Líder do Crypto WG | ✅ Concluído (o pico da revisão concluída pode prosseguir sem atualização) |
| Rascunho do comando `cargo update` seletivo + diferença de sanidade | 24/01/2025 | Engenharia de Liberação | ⚪ Em espera (reativar caso apareçam novas caixas) |
| Revisão de segurança de novas caixas | 27/01/2025 | Avaliações de segurança | ⚪ Em espera (reutilize a lista de verificação de auditoria quando a atualização for retomada) |
| Mesclar atualização do lockfile PR | 29/01/2025 | Engenharia de Liberação | ⚪ Em espera |
| Atualizar lista de verificação de documentos do programa SM | Após mesclar | Líder do Crypto WG | ✅ Endereçado via entrada `docs/source/crypto/sm_program.md` (12/02/2026) |

## Notas
- Mantenha qualquer atualização futura restrita às caixas relacionadas ao SM listadas acima (e aos auxiliares de suporte como `rfc6979`), evitando `cargo update` em todo o espaço de trabalho.
- Se alguma dependência transitiva introduzir desvio de MSRV, revele-a antes da mesclagem.
- Depois de mesclado, habilite um trabalho de CI efêmero para monitorar os tempos de construção do recurso `sm`.