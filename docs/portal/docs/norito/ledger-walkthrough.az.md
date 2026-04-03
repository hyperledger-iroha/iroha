---
lang: az
direction: ltr
source: docs/portal/docs/norito/ledger-walkthrough.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2c61035c0e4b0fd478f08beeef34d7ae41415f55b09dc93dfda9490efe94fb91
source_last_modified: "2026-01-22T16:26:46.505734+00:00"
translation_last_reviewed: 2026-02-07
title: Ledger Walkthrough
description: Reproduce a deterministic register → mint → transfer flow with the `iroha` CLI and verify the resulting ledger state.
slug: /norito/ledger-walkthrough
translator: machine-google-reviewed
---

Bu təlimat [Norito sürətli başlanğıcı](./quickstart.md) göstərərək tamamlayır
`iroha` CLI ilə kitab vəziyyətini necə mutasiya etmək və yoxlamaq olar. Qeydiyyatdan keçəcəksiniz a
yeni aktiv tərifi, bəzi vahidləri standart operator hesabına köçürmək, köçürmək
balansın bir hissəsini başqa hesaba köçürün və nəticədə aparılan əməliyyatları yoxlayın
və holdinqlər. Hər bir addım Rust/Python/JavaScript-də əhatə olunmuş axınları əks etdirir
CLI və SDK davranışı arasında pariteti təsdiqləmək üçün SDK sürətli işə salınır.

## İlkin şərtlər

- Tək-peer şəbəkəni yükləmək üçün [sürətli başlanğıc](./quickstart.md) izləyin
  `docker compose -f defaults/docker-compose.single.yml up --build`.
- `iroha` (CLI) qurulduğundan və ya endirildiyindən və bu əlaqəni əldə edə bildiyinizdən əmin olun.
  `defaults/client.toml` istifadə edərək peer.
- Əlavə köməkçilər: `jq` (JSON cavablarının formatlaşdırılması) və POSIX qabığı
  aşağıda istifadə olunan mühit dəyişən fraqmentləri.

Bələdçi boyunca `$ADMIN_ACCOUNT` və `$RECEIVER_ACCOUNT` ilə əvəz edin.
istifadə etməyi planlaşdırdığınız hesab identifikatorları. Defolt paketə artıq iki hesab daxildir
demo açarlarından əldə edilmişdir:

```sh
export ADMIN_ACCOUNT="<i105-account-id>"
export RECEIVER_ACCOUNT="<i105-account-id>"
```

İlk bir neçə hesabı sadalayaraq dəyərləri təsdiqləyin:

```sh
iroha --config defaults/client.toml account list all --limit 5 --table
```

## 1. Yaranma vəziyyətini yoxlayın

CLI-nin hədəf aldığı kitabçanı araşdırmaqla başlayın:

```sh
# Domains registered in genesis
iroha --config defaults/client.toml domain list all --table

# Accounts inside wonderland (replace --limit with a higher number if needed)
iroha --config defaults/client.toml account list filter \
  '{"domain":"wonderland.universal"}' \
  --limit 10 --table

# Asset definitions that already exist
iroha --config defaults/client.toml asset definition list all --table
```

Bu əmrlər Norito tərəfindən dəstəklənən cavablara əsaslanır, ona görə də filtrləmə və səhifələmə
deterministik və SDK-ların qəbul etdiklərinə uyğundur.

## 2. Aktiv tərifini qeydiyyatdan keçirin

`wonderland` daxilində `coffee` adlı yeni, sonsuz zərrəlik aktiv yaradın
domen:

```sh
iroha --config defaults/client.toml asset definition register \
  --id 7Sp2j6zDvJFnMoscAiMaWbWHRDBZ
```

CLI təqdim edilmiş əməliyyat hashını çap edir (məsələn,
`0x5f…`). Daha sonra statusu sorğulaya bilmək üçün onu yadda saxlayın.

## 3. Vahidləri operator hesabına köçürün

Aktiv kəmiyyətləri `(asset definition, account)` cütü altında yaşayır. Nanə 250
`7Sp2j6zDvJFnMoscAiMaWbWHRDBZ` vahidləri `$ADMIN_ACCOUNT`-ə:

```sh
iroha --config defaults/client.toml asset mint \
  --id norito:4e52543000000002 \
  --quantity 250
```

Yenə də CLI çıxışından əməliyyat hashını (`$MINT_HASH`) əldə edin. Kimə
balansı iki dəfə yoxlayın, qaçın:

```sh
iroha --config defaults/client.toml asset list all --limit 5 --table
```

və ya yalnız yeni aktivi hədəfləmək üçün:

```sh
iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000002\"}" \
  --limit 1 | jq .
```

## 4. Balansın bir hissəsini başqa hesaba köçürün

Operator hesabından 50 vahidi `$RECEIVER_ACCOUNT`-ə köçürün:

```sh
iroha --config defaults/client.toml asset transfer \
  --id norito:4e52543000000002 \
  --to ${RECEIVER_ACCOUNT} \
  --quantity 50
```

Əməliyyat hashini `$TRANSFER_HASH` olaraq saxlayın. Hər ikisinin holdinqlərini sorğulayın
yeni qalıqları yoxlamaq üçün hesablar:

```sh
iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000002\"}" --limit 1 | jq .

iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000003\"}" --limit 1 | jq .
```

## 5. Kitab dəlillərini yoxlayın

Hər iki əməliyyatın həyata keçirildiyini təsdiqləmək üçün yadda saxlanan heşlərdən istifadə edin:

```sh
iroha --config defaults/client.toml transaction get --hash $MINT_HASH | jq .
iroha --config defaults/client.toml transaction get --hash $TRANSFER_HASH | jq .
```

Siz həmçinin hansı blokun transferin daxil olduğunu görmək üçün son blokları yayımlaya bilərsiniz:

```sh
# Stream from the latest block and stop after ~5 seconds
iroha --config defaults/client.toml blocks 0 --timeout 5s --table
```

Yuxarıdakı hər bir əmr SDK-lar kimi eyni Norito faydalı yüklərdən istifadə edir. Təkrar etsəniz
kod vasitəsilə bu axın (aşağıdakı SDK sürətli başlanğıclarına baxın), hashlar və balanslar olacaq
eyni şəbəkə və defoltları hədəflədiyiniz müddətcə sıraya düzün.

## SDK paritet keçidləri

- [Rust SDK quickstart](../sdks/rust) — qeydiyyat təlimatlarını nümayiş etdirir,
  əməliyyatların təqdim edilməsi və Rustdan sorğu statusu.
- [Python SDK sürətli başlanğıc](../sdks/python) — eyni registr/nanəni göstərir
  Norito dəstəkli JSON köməkçiləri ilə əməliyyatlar.
- [JavaScript SDK sürətli başlanğıc](../sdks/javascript) — Torii sorğularını əhatə edir,
  idarəetmə köməkçiləri və çap edilmiş sorğu sarğıları.

Əvvəlcə CLI prospektini işə salın, sonra seçim etdiyiniz SDK ilə ssenarini təkrarlayın
hər iki səthin əməliyyat hashləri, balanslar və sorğu ilə razılaşdığından əmin olmaq
çıxışlar.