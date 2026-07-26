---
lang: pt
direction: ltr
source: docs/portal/docs/reference/publishing-checklist.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Lista de verificação de publicação

جب بھی آپ portal do desenvolvedor اپڈیٹ کریں تو اس checklist کا استعمال کریں۔ یہ یقینی بناتی ہے کہ CI build, implantação de GitHub Pages, اور دستی testes de fumaça ہر سیکشن کو کور کریں اس سے پہلے کہ کوئی lançamento یا marco do roteiro آئے۔

## 1. Validação local

- `npm run sync-openapi -- --version=current --latest` (Torii OpenAPI بدلنے پر ایک یا زیادہ `--mirror=<label>` flags شامل کریں تاکہ ایک instantâneo congelado).
- `npm run build` – تصدیق کریں کہ `Build on Iroha with confidence` cópia herói اب بھی `build/index.html` میں موجود ہے۔
- `./docs/portal/scripts/preview_verify.sh --build-dir build` – verificação do manifesto da soma de verificação کریں (artefatos CI baixados ٹیسٹ کرتے وقت `--descriptor`/`--archive` شامل کریں).
- `npm run serve` – auxiliar de visualização com verificação de soma de verificação کبھی instantâneo não assinado نہ براؤز کریں (`serve:verified` alias واضح کالز کیلئے موجود رہتا ہے).
- `npm run start` اور servidor de recarga ao vivo کے ذریعے اس markdown کو verificação pontual کریں جسے آپ نے touch کیا ہے۔

## 2. Verificações de solicitação pull

- `.github/workflows/check-docs.yml` میں `docs-portal-build` trabalho کی کامیابی verificar کریں۔
- تصدیق کریں کہ `ci/check_docs_portal.sh` چلا (CI logs میں hero smoke check دکھائی دیتا ہے)۔
- یقینی بنائیں کہ fluxo de trabalho de visualização نے manifesto (`build/checksums.sha256`) upload کیا اور script de verificação de visualização کامیاب ہوا (logs CI میں saída `scripts/preview_verify.sh` دکھائی دیتا ہے)۔
- Ambiente GitHub Pages کی URL de visualização publicado کو Descrição PR میں شامل کریں۔

## 3. Aprovação da seção

| Seção | Proprietário | Lista de verificação |
|--------|-------|-----------|
| Página inicial | DevRel | Renderização de cópia de herói ہو، cartões de início rápido rotas válidas پر جائیں, botões CTA resolver ہوں۔ |
| Norito | Norito WG | Visão geral e guias de primeiros passos, sinalizadores CLI e documentos de esquema Norito, consulte کریں۔ |
| SoraFS | Equipe de armazenamento | Início rápido مکمل ہو، campos de relatório de manifesto documentados ہوں، buscar instruções de simulação verificar ہوں۔ |
| Guias do SDK | Leads do SDK | Guias Rust/Python/JS موجودہ exemplos compilar کریں اور live repos سے link ہوں۔ |
| Referência | Documentos/DevRel | Índice تازہ ترین especificações دکھائے, referência do codec Norito `norito.md` سے correspondência کرے۔ |
| Artefato de visualização | Documentos/DevRel | Artefato `docs-portal-preview` PR کے ساتھ anexar ہو، verificação de fumaça passar ہوں, revisores de link کے ساتھ compartilhar ہو۔ |
| Segurança e experimente sandbox | Documentos/DevRel · Segurança | Configuração de login do código do dispositivo OAuth ہو (`DOCS_OAUTH_*`), `security-hardening.md` lista de verificação executada ہو, cabeçalhos CSP/Trusted Types `npm run build` یا `npm run probe:portal` سے verificar ہوں۔ |

ہر linha کو اپنے PR review کا حصہ بنائیں, یا tarefas de acompanhamento لکھیں تاکہ rastreamento de status درست رہے۔

## 4. Notas de lançamento

- `https://docs.iroha.tech/` (trabalho de implantação e URL do ambiente) کو notas de versão e atualizações de status میں شامل کریں۔
- نئے یا تبدیل شدہ seções کو واضح طور پر بیان کریں تاکہ equipes downstream جان سکیں کہ انہیں اپنے testes de fumaça کہاں دوبارہ چلانے ہیں۔