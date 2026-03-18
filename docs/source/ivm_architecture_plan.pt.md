---
lang: pt
direction: ltr
source: docs/source/ivm_architecture_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: da8a99adbbcf1d8b209a25da32e256c0dad2860633f373d7410a3a91d790c938
source_last_modified: "2026-01-21T10:21:48.087325+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Plano de refatoração de arquitetura IVM

Este plano captura os marcos de curto prazo para remodelar a máquina virtual Iroha
(IVM) em camadas mais claras, preservando as características de segurança e desempenho.
Ele se concentra em isolar responsabilidades, tornar as integrações de host mais seguras e
preparando a pilha de idiomas Kotodama para extração em uma caixa independente.

## Metas

1. **Façade de tempo de execução em camadas** – introduza uma interface de tempo de execução explícita para que a VM
   o núcleo pode ser incorporado atrás de uma característica estreita e front-ends alternativos podem evoluir
   sem tocar nos módulos internos.
2. **Reforço de limite de host/syscall** – roteia o despacho de syscall por meio de um
   adaptador dedicado que impõe política ABI e validação de ponteiro antes de qualquer host
   o código é executado.
3. **Separação de idioma/ferramentas** – mova o código específico Kotodama para uma nova caixa e
   mantenha apenas a superfície de execução do bytecode em `ivm`.
4. **Coesão de configuração** – unifique a aceleração e alterne recursos para que sejam
   conduzido através de `iroha_config`, removendo botões baseados no ambiente na produção
   caminhos.

## Divisão de fases

### Fase 1 – Fachada em tempo de execução (em andamento)
- Adicione um módulo `runtime` que define uma característica `VmEngine` que descreve o ciclo de vida
  operações (`load_program`, `execute`, encanamento de host).
- Ensine `IVM` a implementar a característica.  Isso mantém a estrutura existente, mas permite
  consumidores (e testes futuros) dependam da interface em vez de concreto
  tipos.
- Comece a eliminar reexportações diretas de módulos de `lib.rs` para que os chamadores importem por meio do
  fachada quando possível.

**Impacto na segurança/desempenho**: A fachada restringe o acesso direto ao interior
estado; apenas pontos de entrada seguros são expostos.  Isso torna mais fácil auditar o host
interações e razões sobre o manuseio de gás ou TLV.

### Fase 2 – Despachante Syscall
- Introduzir um componente `SyscallDispatcher` que envolve `IVMHost` e aplica ABI
  validação de política e ponteiro uma vez, em um único local.
- Migre o host padrão e os hosts simulados para usar o despachante, removendo
  lógica de validação duplicada.
- Torne o despachante conectável para que os hosts possam fornecer instrumentação personalizada sem
  ignorando as verificações de segurança.
- Forneça um auxiliar `SyscallDispatcher::shared(...)` para que as VMs clonadas possam encaminhar
  syscalls por meio de um host `Arc<Mutex<..>>` compartilhado sem cada construção de trabalhador
  embalagens personalizadas.

**Impacto na segurança/desempenho**: o gate centralizado protege contra hosts que
esqueça de chamar `is_syscall_allowed` e isso permitirá o armazenamento futuro do ponteiro em cache
validações para syscalls repetidos.

### Fase 3 – Extração Kotodama
- Compilador Kotodama extraído para `crates/kotodama_lang` (de `crates/ivm/src/kotodama`).
- Forneça uma API de bytecode mínimo que a VM consome (`compile_to_ivm_bytecode`).

**Impacto na segurança/desempenho**: a dissociação reduz a superfície de ataque da VM
núcleo e permite a inovação da linguagem sem arriscar regressões do intérprete.### Fase 4 – Consolidação da configuração
- Opções de aceleração de thread por meio de predefinições `iroha_config` (por exemplo, habilitando back-ends de GPU), mantendo as substituições de ambiente existentes (`IVM_DISABLE_CUDA`, `IVM_DISABLE_METAL`) como interruptores de interrupção de tempo de execução.
- Expor um objeto `RuntimeConfig` através da nova fachada para que os hosts selecionem
  explicitamente políticas de aceleração determinísticas.

**Impacto na segurança/desempenho**: a eliminação de alternâncias baseadas no ambiente evita o silêncio
desvio de configuração e garante comportamento determinístico em implantações.

## Próximas etapas imediatas

- Conclua a Fase 1 adicionando o traço de fachada e atualizando locais de chamada de alto nível para
  depende disso.
- Auditar reexportações públicas para garantir apenas a fachada e APIs deliberadamente públicas
  vazar da caixa.
- Prototipo da API do despachante syscall em um módulo separado e migre o
  host padrão uma vez validado.

O progresso em cada fase será acompanhado em `status.md` assim que a implementação for
em andamento.