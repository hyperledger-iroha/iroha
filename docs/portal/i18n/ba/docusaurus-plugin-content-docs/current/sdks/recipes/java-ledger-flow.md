---
slug: /sdks/recipes/java-ledger-flow
lang: ba
direction: ltr
source: docs/portal/docs/sdks/recipes/java-ledger-flow.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Java ledger flow recipe
description: Drive the register → mint → transfer demo using the IrohaAndroid JVM library.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

импорт SapleDownload '@site/src/компоненттар/SampleDownload';

Был рецепт өҫтәл/JVM варианты I18NI000000007X модуленә йүнәлтелгән.
Ул көҙгө CLI проходка теркәү активтар билдәләмәһе, һуғыу .
админ иҫәбенә, икенсе иҫәп яҙмаһына күсереп, һөҙөмтәлә баҫтырыу
тигеҙлек.

<СэмплДау-лог
  href="/sdk-рецепттары/жава/срк/төп/java/миля/ЛеджерФло.java".
  файл исеме="срк/төп/java/алгес/ЛеджерФлоу.джава".
  тасуирлама="Скачать тулы Java миҫал импортлау өсөн һеҙҙең IDE йәки проект шаблон."
/>

## 1. актив билдәләмәһен теркәү (CLI)

```bash
iroha --config defaults/client.toml asset definition register --id 7Sp2j6zDvJFnMoscAiMaWbWHRDBZ
```

## 2. Экспорт ышаныс ҡағыҙҙары

```bash
# raw 32-byte Ed25519 private key in hex (without multicodec prefix)
export ADMIN_PRIVATE_KEY_RAW="4f94...<64 hex chars>..."
export ADMIN_ACCOUNT="<i105-account-id>"
export RECEIVER_ACCOUNT="<i105-account-id>"
```

> Ҡулланыу I18NI0000000008X, әгәр һеҙ
> кәрәк, тип сисергә мультихаш префикс ғәҙәттәге конфиг.

## 3.

```kts title="build.gradle.kts"
dependencies {
    implementation(project(":java:iroha_android"))
    implementation("org.bouncycastle:bcprov-jdk18on:1.78.1") // Ed25519 signer helper
}
```

## 4. Миҫал программаһы

```java title="src/main/java/ledger/LedgerFlow.java"
package ledger;

import java.net.URI;
import java.net.URLEncoder;
import java.net.http.HttpClient;
import java.net.http.HttpRequest;
import java.net.http.HttpResponse;
import java.nio.charset.StandardCharsets;
import java.time.Duration;
import java.util.HexFormat;
import java.util.List;
import org.bouncycastle.crypto.params.Ed25519PrivateKeyParameters;
import org.bouncycastle.crypto.params.Ed25519PublicKeyParameters;
import org.bouncycastle.crypto.signers.Ed25519Signer;
import org.hyperledger.iroha.android.IrohaKeyManager;
import org.hyperledger.iroha.android.client.ClientConfig;
import org.hyperledger.iroha.android.client.HttpClientTransport;
import org.hyperledger.iroha.android.client.RetryPolicy;
import org.hyperledger.iroha.android.crypto.Signer;
import org.hyperledger.iroha.android.model.InstructionBox;
import org.hyperledger.iroha.android.model.TransactionPayload;
import java.util.Base64;
import org.hyperledger.iroha.android.norito.NoritoJavaCodecAdapter;
import org.hyperledger.iroha.android.tx.TransactionBuilder;

public final class LedgerFlow {
  private LedgerFlow() {}

  public static void main(String[] args) throws Exception {
    final var env = System.getenv();
    final String adminAccount = env.get("ADMIN_ACCOUNT");
    final String receiverAccount = env.get("RECEIVER_ACCOUNT");
    final String privateKeyHex = env.get("ADMIN_PRIVATE_KEY_RAW");
    if (adminAccount == null || receiverAccount == null || privateKeyHex == null) {
      throw new IllegalStateException("Set ADMIN_ACCOUNT, RECEIVER_ACCOUNT, ADMIN_PRIVATE_KEY_RAW");
    }

    final Signer signer = signerFromHex(privateKeyHex);
    // Replace placeholders with Norito wire payload bytes for each instruction.
    final List<InstructionBox> instructions =
        List.of(
            InstructionBox.fromWirePayload(
                "<WIRE_NAME_REGISTER_ASSET_DEFINITION>",
                Base64.getDecoder().decode("<WIRE_PAYLOAD_BASE64_REGISTER_ASSET_DEFINITION>")),
            InstructionBox.fromWirePayload(
                "<WIRE_NAME_MINT_ASSET>",
                Base64.getDecoder().decode("<WIRE_PAYLOAD_BASE64_MINT_ASSET>")),
            InstructionBox.fromWirePayload(
                "<WIRE_NAME_TRANSFER_ASSET>",
                Base64.getDecoder().decode("<WIRE_PAYLOAD_BASE64_TRANSFER_ASSET>")));

    final TransactionPayload payload =
        TransactionPayload.builder()
            .setChainId("00000000-0000-0000-0000-000000000000")
            .setAuthority(adminAccount)
            .setCreationTimeMs(System.currentTimeMillis())
            .setInstructions(instructions)
            .build();

    final TransactionBuilder builder =
        new TransactionBuilder(new NoritoJavaCodecAdapter(), IrohaKeyManager.withSoftwareFallback());
    final var signed = builder.encodeAndSign(payload, signer);

    final ClientConfig config =
        ClientConfig.builder()
            .setBaseUri(URI.create("http://127.0.0.1:8080"))
            .setRequestTimeout(Duration.ofSeconds(10))
            .setRetryPolicy(RetryPolicy.none())
            .build();
    final var transport = new HttpClientTransport(config);
    final var response = transport.submitTransaction(signed).join();
    System.out.println("Submitted tx hash: " + response.hashHex().orElse("(pending)"));

    // Fetch the receiver’s balances via the Torii JSON API.
    final HttpClient http = HttpClient.newHttpClient();
    final String encodedAccount = URLEncoder.encode(receiverAccount, StandardCharsets.UTF_8);
    final HttpRequest req =
        HttpRequest.newBuilder()
            .uri(URI.create("http://127.0.0.1:8080/v1/accounts/" + encodedAccount + "/assets"))
            .GET()
            .build();
    final HttpResponse<String> assets =
        http.send(req, HttpResponse.BodyHandlers.ofString());
    System.out.println("Receiver balances: " + assets.body());
  }

  private static Signer signerFromHex(String privateHex) {
    final byte[] seed = HexFormat.of().parseHex(privateHex);
    final Ed25519PrivateKeyParameters privateKey = new Ed25519PrivateKeyParameters(seed, 0);
    final Ed25519PublicKeyParameters publicKey = privateKey.generatePublicKey();
    return new Signer() {
      @Override
      public byte[] sign(byte[] message) {
        final Ed25519Signer signer = new Ed25519Signer();
        signer.init(true, privateKey);
        signer.update(message, 0, message.length);
        return signer.generateSignature();
      }

      @Override
      public byte[] publicKey() {
        return publicKey.getEncoded();
      }

      @Override
      public String algorithm() {
        return "Ed25519";
      }
    };
  }

  private static String assetId(String assetDefinition, String accountId) {
    final int idx = assetDefinition.indexOf('#');
    if (idx < 0) {
      return assetDefinition + "#" + accountId;
    }
    final String defName = assetDefinition.substring(0, idx);
    final String defDomain = assetDefinition.substring(idx + 1);
    return defName + "#" + defDomain + "#" + accountId;
  }
}
```

Һеҙҙең өҫтөнлөк төҙөү ҡоралы менән төҙөү (I18NI000000009X).

## 5. Паритетты раҫлау

- `iroha --config defaults/client.toml transaction get --hash <hash>` аша операция хештарын тикшерергә.
- CLI менән баланслаштырыу (`asset list filter '{"id":"norito:4e52543000000002"}'`).
- Һөҙөмтәләрҙе сағыштырыу менән сағыштырырға Rust/Python/JavaScript/Swift рецептары тәьмин итеү өсөн һәр SDK етештереү бер үк I18NT0000000001X файҙалы йөкләмәләр өсөн демо ағымы.