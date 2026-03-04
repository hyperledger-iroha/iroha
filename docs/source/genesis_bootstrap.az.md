---
lang: az
direction: ltr
source: docs/source/genesis_bootstrap.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6feb2b03bd8f6a41de693a0c3f3c4ffc058072bc7942e2bc50b3fd9770aa56d4
source_last_modified: "2025-12-29T18:16:35.962003+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

Etibarlı həmyaşıdlardan # Genesis Bootstrap

Yerli `genesis.file` olmayan Iroha həmyaşıdları etibarlı həmyaşıdlarından imzalanmış genezis blokunu əldə edə bilər
Norito kodlu bootstrap protokolundan istifadə etməklə.

- **Protokol:** həmyaşıdları mübadiləsi `GenesisRequest` (metadata üçün `Preflight`, faydalı yük üçün `Fetch`) və
  `GenesisResponse` çərçivələri `request_id` tərəfindən açar. Cavab verənlərə zəncirvari id, imzalayan pubkey,
  hash və isteğe bağlı ölçü işarəsi; faydalı yüklər yalnız `Fetch` və dublikat sorğu id-lərində qaytarılır
  `DuplicateRequest` qəbul edin.
- **Mühafizəçilər:** cavab verənlər icazə siyahısını tətbiq edir (`genesis.bootstrap_allowlist` və ya etibarlı həmyaşıdlar
  set), zəncirvari id/pubkey/hesh uyğunluğu, dərəcə limitləri (`genesis.bootstrap_response_throttle`) və
  ölçü qapağı (`genesis.bootstrap_max_bytes`). İcazə siyahısından kənar sorğular `NotAllowed` alır və
  yanlış düymə ilə imzalanmış faydalı yüklər `MismatchedPubkey` alır.
- **Tələb axını:** yaddaş boş olduqda və `genesis.file` ayarlanmadıqda (və
  `genesis.bootstrap_enabled=true`), node isteğe bağlı olan etibarlı həmyaşıdları əvvəlcədən idarə edir
  `genesis.expected_hash`, sonra faydalı yükü götürür, `validate_genesis_block` vasitəsilə imzaları təsdiqləyir,
  və blok tətbiq etməzdən əvvəl Kür ilə yanaşı `genesis.bootstrap.nrt`-ni saxlayır. Bootstrap yenidən cəhd edir
  honor `genesis.bootstrap_request_timeout`, `genesis.bootstrap_retry_interval` və
  `genesis.bootstrap_max_attempts`.
- **Uğursuzluq rejimləri:** sorğular icazə verilən siyahıların buraxılması, zəncir/pubkey/hesh uyğunsuzluğu, ölçü üçün rədd edilir
  limit pozuntuları, tarif limitləri, çatışmayan yerli genezis və ya dublikat sorğu id-ləri. Ziddiyyətli hashlər
  həmyaşıdları arasında gətirməni dayandırın; yerli konfiqurasiyaya cavab verənlər/taym-autları geri qayıtmır.
- **Operator addımları:** etibarlı genezisi olan ən azı bir etibarlı həmyaşıdın əlçatan olmasını təmin edin, konfiqurasiya edin
  `bootstrap_allowlist`/`bootstrap_max_bytes`/`bootstrap_response_throttle` və təkrar cəhd düymələri və
  uyğun olmayan faydalı yükləri qəbul etməmək üçün isteğe bağlı olaraq `expected_hash`-i bağlayın. Davamlı yüklər ola bilər
  `genesis.file`-i `genesis.bootstrap.nrt`-ə göstərərək sonrakı çəkmələrdə təkrar istifadə olunur.