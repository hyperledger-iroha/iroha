---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/staging-manifest-playbook.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: manual de preparação do manifesto
título: Плейбук манифеста для staging
sidebar_label: Плейбук манифеста para teste
descrição: Verifique o chunker de perfil de perfil, parâmetro de ратифицированного, no staging-развертываниях Torii.
---

:::nota História Canônica
:::

##Obzor

Este é um exemplo de configuração do chunker de perfil, parâmetro de roteamento, no teste de teste Torii antes de fornecer a solução no produto. Por favor, verifique se você está atualizando SoraFS, e dispositivos canônicos são fornecidos em repositórios.

## 1. Uso Privado

1. Синхронизируйте канонические luminárias e подписи:

   ```bash
   cargo xtask sorafs-fetch-fixture \
     --signatures https://nexus.example/api/sorafs/manifest_signatures.json \
     --out fixtures/sorafs_chunker
   ci/check_sorafs_fixtures.sh
   ```

2. Selecione o catálogo de envelopes de admissão, como Torii, primeiro lugar (por exemplo): `/var/lib/iroha/admission/sorafs`.
3. Verifique se esta configuração Torii ativa o cache de descoberta e a admissão de execução:

   ```toml
   [torii.sorafs.discovery]
   discovery_enabled = true
   known_capabilities = ["torii_gateway", "chunk_range_fetch", "vendor_reserved"]

   [torii.sorafs.discovery.admission]
   envelopes_dir = "/var/lib/iroha/admission/sorafs"

   [torii.sorafs.storage]
   enabled = true

   [torii.sorafs.gateway]
   enforce_admission = true
   enforce_capabilities = true
   ```

## 2. Envelopes de admissão públicos

1. Selecione os envelopes de admissão do provedor disponíveis no catálogo, baixados em `torii.sorafs.discovery.admission.envelopes_dir`:

   ```bash
   install -m 0644 fixtures/sorafs_manifest/provider_admission/*.json \
     /var/lib/iroha/admission/sorafs/
   ```

2. Selecione Torii (ou abra o SIGHUP, exceto se você estiver executando o hot reload).
3. Следите за логами admissão:

   ```bash
   torii | grep "loaded provider admission envelope"
   ```

## 3. Проверка распространения descoberta

1. Selecione a carga útil do anúncio do provedor (Norito), сформированный seu pipeline testador:

   ```bash
   curl -sS -X POST --data-binary @provider_advert.to \
     http://staging-torii:8080/v1/sorafs/provider/advert
   ```

2. Faça a descoberta de endpoint e instale-o, este anúncio será gerado com aliases canônicos:

   ```bash
   curl -sS http://staging-torii:8080/v1/sorafs/providers | jq .
   ```

   Verifique se o `profile_aliases` substitui o `"sorafs.sf1@1.0.0"` pelo elemento principal.

## 4. Verifique o manifesto e o plano dos endpoints

1. Manifesto de metadados polidos (token de stream требуется, exceto admissão forçada):

   ```bash
   sorafs-fetch \
     --plan fixtures/chunk_fetch_specs.json \
     --gateway-provider name=staging,provider-id=<hex>,base-url=https://staging-gateway/,stream-token=<base64> \
     --gateway-manifest-id <manifest_id_hex> \
     --gateway-chunker-handle sorafs.sf1@1.0.0 \
     --json-out=reports/staging_manifest.json
   ```

2. Prove o JSON-вывод e убедитесь, aqui:
   - `chunk_profile_handle` ou `sorafs.sf1@1.0.0`.
   - `manifest_digest_hex` fornece uma determinação diferente.
   - `chunk_digests_blake3` permite a regeneração de luminárias.

## 5. Проверки телеметрии

- Verifique se Prometheus publica um novo perfil de métricas:

  ```bash
  curl -sS http://staging-torii:8080/metrics | grep torii_sorafs_chunk_range_requests_total
  ```

- Дашборды должны показывать staging-провайдера под ожидаемым alias e держать счетчики brownout на нуле, пока профиль ativo.

## 6. Implementação do Готовность к

1. Organize a cópia de segurança com URL-address, manifesto de ID e parâmetros de snapshot.
2. Verifique se o lançamento do canal Nexus está ativado no momento da produção.
3. Verifique a configuração do produto (Seção 4 em `chunker_registry_rollout_checklist.md`) para definir a configuração.

Поддержание этого плейбука в актуальном состоянии гарантирует, что каждый rollout chunker/admission следует одним и тем же детерминированным шагам между preparação e produção.