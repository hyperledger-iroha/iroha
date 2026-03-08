# Nota de implantação do IH58 para responsáveis por SDK e codecs

Equipes: SDK Rust, SDK TypeScript/JavaScript, SDK Python, SDK Kotlin, ferramentas de codec

Contexto: `docs/account_structure.md` agora reflete a implementação de AccountId IH58 em produção.
Alinhem o comportamento e os testes dos SDKs com a especificação canônica.

Referências-chave:
- Codec de endereço + layout do cabeçalho — `docs/account_structure.md` §2
- Registro de curvas — `docs/source/references/address_curve_registry.md`
- Tratamento de domínio Norm v1 — `docs/source/references/address_norm_v1.md`
- Vetores de fixtures — `fixtures/account/address_vectors.json`

Ações:
1. **Saída canônica:** `AccountId::to_string()`/Display DEVE emitir apenas IH58
   (sem sufixo `@domain`). O hex canônico é para depuração (`0x...`).
2. **Entradas aceitas:** parsers DEVEM aceitar IH58 (preferido), `sora`
   comprimido e hex canônico (somente `0x...`; hex sem prefixo é rejeitado).
   Entradas PODEM carregar sufixo `@<domain>` para dicas de roteamento;
   aliases `<label>@<domain>` (rejected legacy form) exigem resolver. 
   (hex multihash) continua suportado.
3. **Resolvers:** parsing IH58/sora sem domínio requer resolver de seletor de
   domínio, a menos que o seletor seja o default implícito (usar o rótulo de
   domínio padrão configurado). Literais UAID (`uaid:...`) e opaque (`opaque:...`)
   exigem resolvers.
4. **Checksum IH58:** use Blake2b-512 sobre `IH58PRE || prefix || payload`,
   pegue os primeiros 2 bytes. A base do alfabeto comprimido é **105**.
5. **Gate de curvas:** SDKs são Ed25519-only por padrão. Forneça opt-in explícito
   para ML‑DSA/GOST/SM (flags de build no Swift; `configureCurveSupport` em JS/Android).
   Não assuma secp256k1 habilitado por padrão fora de Rust.
6. **Sem CAIP-10:** ainda não há mapeamento CAIP‑10 entregue; não exponha nem
   dependa de conversões CAIP‑10.

Por favor confirmem quando codecs/testes estiverem atualizados; dúvidas podem
ser acompanhadas no tópico do RFC de endereçamento de contas.
