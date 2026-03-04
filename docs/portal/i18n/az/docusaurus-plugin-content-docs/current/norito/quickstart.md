---
slug: /norito/quickstart
lang: az
direction: ltr
source: docs/portal/docs/norito/quickstart.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Norito Quickstart
description: Build, validate, and deploy a Kotodama contract with the release tooling and default single-peer network.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

Bu təlimat tərtibatçıların öyrənərkən izləməsini gözlədiyimiz iş axını əks etdirir
Norito və Kotodama ilk dəfə: deterministik tək bərabərli şəbəkəni yükləyin,
müqavilə tərtib edin, onu yerli olaraq qurutun, sonra onu Torii vasitəsilə göndərin
istinad CLI.

Nümunə müqaviləsi zəng edənin hesabına açar/dəyər cütünü yazır ki, siz edə biləsiniz
yan təsirini dərhal `iroha_cli` ilə yoxlayın.

## İlkin şərtlər

- [Docker](https://docs.docker.com/engine/install/) Compose V2 aktivdir (istifadə olunur)
  `defaults/docker-compose.single.yml`-də müəyyən edilmiş nümunə həmyaşıdını başlamaq üçün).
- Yükləmədiyiniz halda köməkçi binaries yaratmaq üçün Rust alətlər silsiləsi (1.76+).
  nəşr olunanlar.
- `koto_compile`, `ivm_run` və `iroha_cli` binaries. Onları buradan qura bilərsiniz
  aşağıda göstərildiyi kimi iş yerini yoxlayın və ya uyğun buraxılış artefaktlarını endirin:

```sh
cargo install --locked --path crates/ivm --bin koto_compile --bin ivm_run
cargo install --locked --path crates/iroha_cli --bin iroha
```

> Yuxarıdakı ikili faylları iş sahəsinin qalan hissəsi ilə birlikdə quraşdırmaq təhlükəsizdir.
> Onlar heç vaxt `serde`/`serde_json` ilə əlaqələndirirlər; Norito kodekləri uçdan-uca tətbiq edilir.

## 1. Tək peerli inkişaf şəbəkəsinə başlayın

Repozitoriya `kagami swarm` tərəfindən yaradılan Docker Compose paketi daxildir.
(`defaults/docker-compose.single.yml`). Bu, standart genezisi, müştərini bağlayır
konfiqurasiya və sağlamlıq zondları beləliklə Torii `http://127.0.0.1:8080`-də əldə edilə bilər.

```sh
docker compose -f defaults/docker-compose.single.yml up --build
```

Konteyneri işlək vəziyyətdə qoyun (ya ön planda, ya da ayrı). Hamısı
sonrakı CLI zəngləri `defaults/client.toml` vasitəsilə bu həmyaşıdı hədəfləyir.

## 2. Müqavilənin müəllifi

İş qovluğu yaradın və minimal Kotodama nümunəsini yadda saxlayın:

```sh
mkdir -p target/quickstart
cat > target/quickstart/hello.ko <<'KO'
// Writes a deterministic account detail for the transaction authority.

seiyaku Hello {
  // Optional initializer invoked during deployment.
  hajimari() {
    info("Hello from Kotodama");
  }

  // Public entrypoint that records a JSON marker on the caller.
  kotoage fn write_detail() {
    set_account_detail(
      authority(),
      name!("example"),
      json!{ hello: "world" }
    );
  }
}
KO
```

> Versiya nəzarətində Kotodama mənbələrini saxlamağa üstünlük verin. Portalda yerləşdirilən nümunələrdir
> [Norito nümunələr qalereyası](./examples/) altında da mövcuddur
> daha zəngin başlanğıc nöqtəsi istəyirəm.

## 3. IVM ilə tərtib edin və quru işə salın

Müqaviləni IVM/Norito bayt koduna (`.to`) tərtib edin və onu yerli olaraq icra edin
Şəbəkəyə toxunmadan əvvəl host sistem çağırışlarının uğurlu olduğunu təsdiqləyin:

```sh
koto_compile target/quickstart/hello.ko \
  --abi 1 \
  --max-cycles 0 \
  -o target/quickstart/hello.to

ivm_run target/quickstart/hello.to --args '{}'
```

Qaçışçı `info("Hello from Kotodama")` jurnalını çap edir və yerinə yetirir
`SET_ACCOUNT_DETAIL` sistem çağırışı istehza edilmiş hosta qarşı. Əgər isteğe bağlı `ivm_tool`
binar mövcuddur, `ivm_tool inspect target/quickstart/hello.to` göstərir
ABI başlığı, xüsusiyyət bitləri və ixrac edilmiş giriş nöqtələri.

## 4. Torii vasitəsilə bayt kodunu təqdim edin

Düyün hələ də işlək vəziyyətdə, tərtib edilmiş bayt kodunu CLI-dən istifadə edərək Torii-ə göndərin.
Defolt inkişaf identifikasiyası açıq açardan əldə edilir
`defaults/client.toml`, hesab ID-si belədir
```
ih58...
```

Torii URL, zəncir identifikatoru və imza açarını təmin etmək üçün konfiqurasiya faylından istifadə edin:

```sh
iroha --config defaults/client.toml \
  transaction ivm \
  --path target/quickstart/hello.to
```

CLI əməliyyatı Norito ilə kodlayır, onu dev açarı ilə imzalayır və
onu qaçan həmyaşıdına təqdim edir. `set_account_detail` üçün Docker qeydlərinə baxın
syscall və ya törədilmiş əməliyyat hash üçün CLI çıxışına nəzarət edin.

## 5. Vəziyyət dəyişikliyini yoxlayın

Müqavilənin yazdığı hesab təfərrüatını əldə etmək üçün eyni CLI profilindən istifadə edin:

```sh
iroha --config defaults/client.toml \
  account meta get \
  --id ih58... \
  --key example | jq .
```

Norito tərəfindən dəstəklənən JSON yükünü görməlisiniz:

```json
{
  "hello": "world"
}
```

Dəyər yoxdursa, Docker tərtib xidmətinin hələ də işlədiyini təsdiqləyin
işləyir və `iroha` tərəfindən bildirilən əməliyyat hashının `Committed`-ə çatması
dövlət.

## Növbəti addımlar

- Görmək üçün avtomatik yaradılan [nümunə qalereya](./examples/) ilə tanış olun
  Norito sistem zənglərinə necə daha təkmil Kotodama fraqmentləri xəritəsi.
- Daha dərindən öyrənmək üçün [Norito başlanğıc təlimatını](./getting-started) oxuyun
  kompilyator/qaçış alətlərinin izahı, manifest yerləşdirilməsi və IVM
  metadata.
- Öz müqavilələrinizi təkrarlayarkən, `npm run sync-norito-snippets`-dən istifadə edin
  portal sənədləri və artefaktları qalması üçün yüklənə bilən fraqmentləri bərpa etmək üçün iş sahəsi
  `crates/ivm/docs/examples/` altındakı mənbələrlə sinxronlaşdırılır.