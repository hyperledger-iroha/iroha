---
lang: ka
direction: ltr
source: docs/source/docker_build.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a5ac4be3d387269898112d465ec404490f67c6c2b9267c0a0781d0de70cf783d
source_last_modified: "2025-12-29T18:16:35.951567+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Docker აღმაშენებლის სურათი

ეს კონტეინერი განსაზღვრულია `Dockerfile.build`-ში და აერთიანებს ყველა ხელსაწყოს ჯაჭვს
დამოკიდებულებები, რომლებიც საჭიროა CI და ადგილობრივი გამოშვებისთვის. სურათი ახლა მუშაობს როგორც a
ნაგულისხმევად არა root მომხმარებელი, ამიტომ Git ოპერაციები განაგრძობს მუშაობას Arch Linux-თან
`libgit2` პაკეტი გლობალური `safe.directory` გამოსავლის გარეშე.

## შექმენით არგუმენტები

- `BUILDER_USER` – შესვლის სახელი შექმნილია კონტეინერის შიგნით (ნაგულისხმევი: `iroha`).
- `BUILDER_UID` – მომხმარებლის რიცხვითი ID (ნაგულისხმევი: `1000`).
- `BUILDER_GID` – ძირითადი ჯგუფის ID (ნაგულისხმევი: `1000`).

როდესაც თქვენ დაამონტაჟებთ სამუშაო სივრცეს თქვენი ჰოსტიდან, გადაიტანეთ შესაბამისი UID/GID მნიშვნელობები
გენერირებული არტეფაქტები რჩება დასაწერად:

```bash
docker build \
  -f Dockerfile.build \
  --build-arg BUILDER_UID="$(id -u)" \
  --build-arg BUILDER_GID="$(id -g)" \
  --build-arg BUILDER_USER="iroha" \
  -t iroha-builder .
```

ხელსაწყოების საქაღალდეები (`/usr/local/rustup`, `/usr/local/cargo`, `/opt/poetry`)
კონფიგურირებული მომხმარებლის საკუთრებაა, ამიტომ Cargo, rustup და Poetry ბრძანებები სრულად რჩება
ფუნქციონირებს მას შემდეგ, რაც კონტეინერი ჩამოაგდებს root პრივილეგიებს.

## გაშვებული ნაგებობები

მიამაგრეთ თქვენი სამუშაო ადგილი `/workspace`-ზე (კონტეინერი `WORKDIR`) გამოძახებისას
გამოსახულება. მაგალითი:

```bash
docker run --rm -it \
  -v "$PWD":/workspace \
  iroha-builder \
  cargo build --workspace
```

სურათი ინახავს `docker` ჯგუფის წევრობას ასე ჩადგმულ Docker ბრძანებებს (მაგ.
`docker buildx bake`) რჩება ხელმისაწვდომი CI სამუშაო პროცესებისთვის, რომლებიც ამონტაჟებენ ჰოსტის PID-ს
და სოკეტი. დაარეგულირეთ ჯგუფური რუკების საჭიროება თქვენი გარემოსთვის.

## Iroha 2 vs Iroha 3 არტეფაქტი

სამუშაო სივრცე ახლა ასხივებს ცალკეულ ბინარებს თითო გამოშვების ხაზზე, რათა თავიდან აიცილოს შეჯახება:
`iroha3`/`iroha3d` (ნაგულისხმევი) და `iroha2`/`iroha2d` (Iroha 2). გამოიყენეთ დამხმარეები
შექმენით სასურველი წყვილი:

- `make build` (ან `BUILD_PROFILE=deploy bash scripts/build_line.sh --i3`) Iroha 3-ისთვის
- `make build-i2` (ან `BUILD_PROFILE=deploy bash scripts/build_line.sh --i2`) Iroha 2-ისთვის

ამომრჩეველი ამაგრებს ფუნქციების კომპლექტს (`telemetry` + `schema-endpoint` პლუს
ხაზის სპეციფიკური `build-i{2,3}` დროშა) ასე რომ, Iroha 2 კონსტრუქციები შემთხვევით ვერ აიღებს
Iroha 3-მხოლოდ ნაგულისხმევი.

გამოუშვით `scripts/build_release_bundle.sh`-ის საშუალებით აშენებული პაკეტები, აირჩიეთ სწორი ორობითი
ასახელებს ავტომატურად, როდესაც `--profile` დაყენებულია `iroha2` ან `iroha3`.