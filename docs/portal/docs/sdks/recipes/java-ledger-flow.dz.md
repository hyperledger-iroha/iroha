---
lang: dz
direction: ltr
source: docs/portal/docs/sdks/recipes/java-ledger-flow.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 29669312b7e43caa320fb6710b78202069b8c9fb04a083f90af4a7d9468d4a51
source_last_modified: "2026-01-30T18:06:01.648526+00:00"
translation_last_reviewed: 2026-02-07
title: Java ledger flow recipe
description: Drive the register → mint → transfer demo using the IrohaAndroid JVM library.
slug: /sdks/recipes/java-ledger-flow
translator: machine-google-reviewed
---

filight Sample load འདི་ '@site/src/ཆ་ཤས་/ཆ་ཤས་/དཔེ་ཚད་ཕབ་ལེན་';

བཀོད་སྒྲིག་འདི་གིས་ I18NI0000007X ཚད་གཞི་གི་ ཌེཀསི་ཊོཔ་/ཇེ་ཝི་ཨེམ་གྱི་དབྱེ་བ་ལུ་དམིགས་གཏད་བསྐྱེདཔ་ཨིན།
འདི་གིས་ རྒྱུ་དངོས་ངེས་ཚིག་ཅིག་ ཐོ་བཀོད་འབད་དེ་ སི་ཨེལ་ཨའི་ འགྲུལ་བསྐྱོད་འདི་ གསལ་སྟོན་འབདཝ་ཨིན།
བདག་སྐྱོང་རྩིས་ཐོ་ རྩིས་ཁྲ་གཉིས་པ་ལུ་སྤོ་བཤུད་འབད་ནི་ དེ་ལས་ གྲུབ་འབྲས་དཔར་བསྐྲུན་འབད་ནི།
སྙོམས་ཏོག་ཏོ།

<དཔེ་ཚད་ཕབ་ལེན་འབད།
  href="/sdk-ལེན་ཚུ་/ཇ་བ་/src/main/java/ledger/LedgerFlow.java"
  filen="src/main/java/lendger/LedgerFlow.java"
  deང་ཁྱོད་རའི་ཨའི་ཌི་ཨི་ཡང་ན་ལས་འགུལ་ཊེམ་པེལེཊི་ནང་ལུ་ནང་འདྲེན་འབད་ནི་ལུ་ ཇ་བ་གི་དཔེ་ཆ་ཚང་འདི་ཕབ་ལེན་འབད།"
།/>།

## 1. རྒྱུ་ནོར་ངེས་ཚིག་ (CLI) ཐོ་བཀོད་འབད།

```bash
iroha --config defaults/client.toml asset definition register --id coffee#wonderland
```

## 2. ཕྱིར་འདྲེན་འབད།

I18NF0000004X

> ཁྱོད་ཀྱིས་ I18NI0000008X ལག་ལེན་འཐབ།
> སྔོན་སྒྲིག་རིམ་སྒྲིག་ལས་ མལ་ཊི་ཧཤ་སྔོན་ཚིག་འདི་ བཏོན་དགོཔ་ཨིན།

## 3. རྟེན་འབྲེལ་བསྣན་པ།

```kts title="build.gradle.kts"
dependencies {
    implementation(project(":java:iroha_android"))
    implementation("org.bouncycastle:bcprov-jdk18on:1.78.1") // Ed25519 signer helper
}
```

## 4. དཔེར་བརྗོད།

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

ཁྱོད་རའི་དགའ་གདམ་གྱི་བཟོ་བསྐྲུན་ལག་ཆས་ (I18NI0000009X) དང་ཅིག་ཁར་ བསྡུ་སྒྲིག་འབད་དགོ།

## 5. ཆ་སྙོམས་བདེན་པ།

- `iroha --config defaults/client.toml transaction get --hash <hash>` བརྒྱུད་དེ་ ཚོང་འབྲེལ་ཧ་ཤི་ཚུ་ བརྟག་ཞིབ་འབད།
- སི་ཨེལ་ཨའི་ (`asset list filter '{"id":"norito:4e52543000000002"}'`) དང་གཅིག་ཁར་ ཕིཊི་ཆི་ལྷག་ལུས་འབདཝ་ཨིན།
- བརྡ་བཀོད་རྒྱུན་འགྲུལ་གྱི་དོན་ལུ་ ཨེསི་ཌི་ཀེ་རེ་རེ་གིས་ I18NT000000001X གི་ སྤྲོད་ལེན་ཚུ་ ངེས་གཏན་བཟོ་ནི་ལུ་ Rust/Python/Python/JavaScript/Swift བཟོ་ཐངས་ཚུ་དང་གཅིག་ཁར་ གྲུབ་འབྲས་ཚུ་ ག་བསྡུར་རྐྱབ།