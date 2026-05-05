<!-- Auto-generated stub for Azerbaijani (az) translation. Replace this content with the full translation. -->

---
lang: az
direction: ltr
source: docs/portal/docs/reference/torii-mcp.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 316a408473f53a9763a18f40d49cfd766b5b93a3611e277e5a761e366e85c082
source_last_modified: "2026-03-15T11:38:44.302824+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

---
id: torii-mcp
title: Torii MCP API
description: Torii-in doğma Model Kontekst Protokolu körpüsündən istifadə üçün istinad bələdçisi.
---

Torii `/v1/mcp`-də yerli Model Kontekst Protokolu (MCP) körpüsünü ifşa edir.
Bu son nöqtə agentlərə alətləri kəşf etməyə və JSON-RPC vasitəsilə Torii/Connect marşrutlarını işə salmağa imkan verir.

## Son nöqtə forması

- `GET /v1/mcp` imkanlar metadatasını qaytarır (JSON-RPC bükülmüş deyil).
- `POST /v1/mcp` JSON-RPC 2.0 sorğularını qəbul edir.
- Əgər `torii.mcp.enabled = false`, heç bir marşrut açıq deyil.
- `torii.require_api_token` aktivləşdirilibsə, JSON-RPC göndərilməzdən əvvəl çatışmayan/etibarsız nişan rədd edilir.

## Konfiqurasiya

`torii.mcp` altında MCP-ni aktivləşdirin:

```json
{
  "torii": {
    "mcp": {
      "enabled": true,
      "max_request_bytes": 1048576,
      "max_tools_per_list": 500,
      "profile": "read_only",
      "expose_operator_routes": false,
      "allow_tool_prefixes": [],
      "deny_tool_prefixes": [],
      "rate_per_minute": 240,
      "burst": 120,
      "async_job_ttl_secs": 300,
      "async_job_max_entries": 2000
    }
  }
}
```

Əsas davranış:

- `profile` alətin görünməsinə nəzarət edir (`read_only`, `writer`, `operator`).
- `allow_tool_prefixes`/`deny_tool_prefixes` əlavə ada əsaslanan siyasət tətbiq edin.
- `rate_per_minute`/`burst` MCP sorğuları üçün token-baxma məhdudiyyətini tətbiq edir.
- `tools/call_async`-dən async iş vəziyyəti `async_job_ttl_secs` və `async_job_max_entries` istifadə edərək yaddaşda saxlanılır.

## Tövsiyə olunan müştəri axını

1. `initialize` nömrəsinə zəng edin.
2. `tools/list` və `toolsetVersion` yaddaşına zəng edin.
3. Normal əməliyyatlar üçün `tools/call` istifadə edin.
4. Daha uzun əməliyyatlar üçün `tools/call_async` + `tools/jobs/get` istifadə edin.
5. `listChanged` `true` olduqda `tools/list`-i yenidən işə salın.

Tam alət kataloqunu sərt kodlamayın. İş vaxtı kəşf edin.

## Metodlar və semantika

Dəstəklənən JSON-RPC üsulları:

- `initialize`
- `tools/list`
- `tools/call`
- `tools/call_batch`
- `tools/call_async`
- `tools/jobs/get`

Qeydlər:- `tools/list` həm `toolset_version`, həm də `toolsetVersion` qəbul edir.
- `tools/jobs/get` həm `job_id`, həm də `jobId` qəbul edir.
- `tools/list.cursor` rəqəmli sətir ofsetidir; etibarsız dəyərlər `0`-ə qayıdır.
- `tools/call_batch` hər bir maddə üçün ən yaxşı səydir (bir uğursuz zəng bacı-qardaş zənglərini uğursuz etmir).
- `tools/call_async` yalnız zərf formasını dərhal təsdiqləyir; icra xətaları iş vəziyyətində daha sonra görünür.
- `jsonrpc` `"2.0"` olmalıdır; buraxılmış `jsonrpc` uyğunluq üçün qəbul edilir.

## Avtorizasiya və yönləndirmə

MCP göndərilməsi Torii icazəsindən yan keçmir. Zənglər normal marşrut idarəçilərini və auth yoxlamalarını həyata keçirir.

Torii alət göndərilməsi üçün daxil olan auth ilə əlaqəli başlıqları yönləndirir:

- `Authorization`
- `x-api-token`
- `x-iroha-account`
- `x-iroha-signature`
- `x-iroha-api-version`

Müştərilər həmçinin `arguments.headers` vasitəsilə zəng başına əlavə başlıqlar təqdim edə bilərlər.
`content-length`, `host` və `arguments.headers`-dən `connection` nəzərə alınmır.

## Xəta modeli

HTTP qatı:

- `400` etibarsız JSON
- `403` API işarəsi JSON-RPC-dən əvvəl rədd edildi
- `413` faydalı yükü `max_request_bytes`-i keçir
- `429` dərəcəsi məhduddur
- JSON-RPC cavabları üçün `200` (JSON-RPC xətaları daxil olmaqla)

JSON-RPC qatı:- Yüksək səviyyəli `error.data.error_code` sabitdir (məsələn, `invalid_request`, `invalid_params`, `tool_not_found`, `tool_not_allowed`, `job_not_found`, I018X, I010).
- Alət xətaları MCP aləti nəticəsində `isError = true` və strukturlaşdırılmış detallar kimi görünür.
- Marşrutla göndərilən alət xətaları HTTP statusunu `structuredContent.error_code` ilə əlaqələndirir (məsələn, `forbidden`, `not_found`, `server_error`).

## Alətin adlandırılması

OpenAPI-dən alınan alətlər sabit marşrut əsaslı adlardan istifadə edir:

- `torii.<method>_<path...>`
- Misal: `torii.get_v1_accounts`

Seçilmiş ləqəblər də `iroha.*` və `connect.*` altında ifşa olunur.

## Kanonik spesifikasiya

Tam tel səviyyəli müqavilə aşağıdakılarda saxlanılır:

- `crates/iroha_torii/docs/mcp_api.md`

`crates/iroha_torii/src/mcp.rs` və ya `crates/iroha_torii/src/lib.rs`-də davranış dəyişdikdə,
eyni dəyişiklikdə həmin spesifikasiyanı yeniləyin və sonra əsas istifadə təlimatını burada əks etdirin.