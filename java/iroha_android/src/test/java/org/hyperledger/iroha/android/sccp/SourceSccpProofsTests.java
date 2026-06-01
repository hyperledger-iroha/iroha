package org.hyperledger.iroha.android.sccp;

import java.nio.charset.StandardCharsets;
import java.util.Arrays;
import java.util.Collections;
import java.util.List;
import java.util.List;

public final class SourceSccpProofsTests {
  private SourceSccpProofsTests() {}

  public static void main(final String[] args) {
    derivesSourceAdapterVerifierVkHashesForUiTooling();
    derivesNativeDestinationBindingHashesForUiTooling();
    derivesEvmAndTronDestinationBindingsForUiTooling();
    derivesSourceMaterialAndDeploymentRecordHashesForUiTooling();
    derivesSourceProofHashesFromWitnessMaterial();
    derivesEthBeaconExecutionPayloadSszRootsFromWitnessMaterial();
    rejectsMalformedSourceProofWitnessMaterial();
  }

  private static void derivesSourceAdapterVerifierVkHashesForUiTooling() {
    assert SourceSccpProofs.sourceAdapterVerifierVkHash(SourceSccpProofs.DOMAIN_ETH)
        .equals("0x2140903293411cad0f0eb217d8beb18d3a188edf7bba455098589a2409445e46")
        : "ETH source-adapter VK hash must match Rust";
    assert SourceSccpProofs.sourceAdapterVerifierVkHash(SourceSccpProofs.DOMAIN_BSC)
        .equals("0x12536f25748a6520f10ebd42a7bcccd6ec181b9d53129795c8e186dc6e8b18cc")
        : "BSC source-adapter VK hash must match Rust";
    assert SourceSccpProofs.sourceAdapterVerifierVkHash(SourceSccpProofs.DOMAIN_SOL)
        .equals("0xe7bc29d06bf56184183c3fc59a0e934cd1d8e16751f1eda2efaaf88aa350b9d6")
        : "Solana source-adapter VK hash must match Rust";
    assert SourceSccpProofs.sourceAdapterVerifierVkHash(SourceSccpProofs.DOMAIN_TON)
        .equals("0xf03f70e8cb504e69b0611df224c2783d04d8f4ee93beae7a62e1cd0a49703bad")
        : "TON source-adapter VK hash must match Rust";
    assert SourceSccpProofs.sourceAdapterVerifierVkHash(SourceSccpProofs.DOMAIN_TRON)
        .equals("0x0e12ad03def9d75887d4d6437e63539cef97c54db4769881eeda757a88826364")
        : "TRON source-adapter VK hash must match Rust";
    assert SourceSccpProofs.sourceAdapterVerifierVkHash(SourceSccpProofs.DOMAIN_SORA_KUSAMA)
        .equals("0xf7768653132995511594e6e7edb4af22f78bba615650d9dda72f14bb18984daf")
        : "SORA-Kusama source-adapter VK hash must match Rust";
    assert SourceSccpProofs.sourceAdapterVerifierVkHash(SourceSccpProofs.DOMAIN_SORA_POLKADOT)
        .equals("0x4f8456bf8626436a16d763c40bf23dffb962232f0766c4ae33d6e594f8be1635")
        : "SORA-Polkadot source-adapter VK hash must match Rust";
    assert SourceSccpProofs.sourceAdapterVerifierVkHash(SourceSccpProofs.DOMAIN_SORA2)
        .equals("0x96bbfa08489249b28a1444d0dcb9d5b4023bd688091f31c0b435601dad48dbb4")
        : "SORA2 source-adapter VK hash must match Rust";

    boolean threw = false;
    try {
      SourceSccpProofs.sourceAdapterVerifierVkHash(
          SourceSccpProofs.DOMAIN_TON, SourceSccpProofs.DOMAIN_TON);
    } catch (final IllegalArgumentException exception) {
      threw = exception.getMessage().contains("targetDomain must be SORA");
    }
    assert threw : "source-adapter VK helper must reject non-SORA targets";
  }

  private static void derivesNativeDestinationBindingHashesForUiTooling() {
    final int[] domains = {
      SourceSccpProofs.DOMAIN_SOL,
      SourceSccpProofs.DOMAIN_TON,
      SourceSccpProofs.DOMAIN_SORA_KUSAMA,
      SourceSccpProofs.DOMAIN_SORA_POLKADOT,
      SourceSccpProofs.DOMAIN_SORA2
    };
    final String[] keys = {
      "sccp:0:3:sol:solana-program-v1:2",
      "sccp:0:4:ton:ton-contract-v1:3",
      "sccp:0:6:sora-kusama:substrate-runtime-v1:5",
      "sccp:0:7:sora-polkadot:substrate-runtime-v1:5",
      "sccp:0:8:sora2:substrate-runtime-v1:5"
    };
    final String[] hashes = {
      "0x078578f0aa27daa2972d6c19d1d26dbb6bf6ba1e8df84e283d7ef101fc46abf6",
      "0x8651c1b818973f92050f69e66e8491e9681d23db1cb37393b9ea15c5e7e02799",
      "0x2ee5c37634c3fab7e9086ea43af7553089fc24dc2ce27d76c46ef4c3da57bb56",
      "0x570ec340d4fee4a84eaa7a53b19baa53c9f4f8d7f64c3c43639fde0c6b3fdef0",
      "0xda5d48fe26518cd8cff6bdaa7cf8e37c7302d1e66469efed4ef2cf340c55b9e4"
    };
    for (int i = 0; i < domains.length; i++) {
      assert SourceSccpProofs.destinationBindingKey(domains[i]).equals(keys[i])
          : "destination binding key must match Rust";
      assert SourceSccpProofs.destinationBindingHash(domains[i]).equals(hashes[i])
          : "destination binding hash must match Rust";
    }

    boolean threw = false;
    try {
      SourceSccpProofs.destinationBindingHash(SourceSccpProofs.DOMAIN_ETH);
    } catch (final IllegalArgumentException exception) {
      threw = exception.getMessage().contains("native SCCP destination lane");
    }
    assert threw : "destination binding helper must reject unsupported destination domains";
  }

  private static void derivesEvmAndTronDestinationBindingsForUiTooling() {
    final SourceSccpProofs.EvmDestinationBinding evmBinding =
        SourceSccpProofs.evmDestinationBinding(
            SourceSccpProofs.DOMAIN_SORA,
            SourceSccpProofs.DOMAIN_ETH,
            "0x" + repeat("33", 32),
            "0x" + repeat("11", 20),
            "0x" + repeat("22", 20),
            "0x" + repeat("bb", 32),
            "0x" + repeat("cc", 32));
    assert evmBinding.key.equals(
            "evm:0:1:"
                + repeat("33", 32)
                + ":0x"
                + repeat("11", 20)
                + ":0x"
                + repeat("22", 20)
                + ":0x"
                + repeat("bb", 32)
                + ":0x"
                + repeat("cc", 32))
        : "EVM destination binding key must match the governed tuple";
    assert evmBinding.hash.equals(
            "0x3ad95ac3e5bc2892f768aae40a3b7ba673d561858b7d1318fbb9f6eba83207bf")
        : "EVM destination binding hash must match Rust";
    assert SourceSccpProofs.evmDestinationBindingHash(
            SourceSccpProofs.DOMAIN_SORA,
            SourceSccpProofs.DOMAIN_ETH,
            "0x" + repeat("33", 32),
            "0x" + repeat("11", 20),
            "0x" + repeat("22", 20),
            "0x" + repeat("bb", 32),
            "0x" + repeat("cc", 32))
        .equals(evmBinding.hash)
        : "EVM destination binding hash helper must match binding.hash";

    final String tronAddress = "TJRabPrwbZy45sbavfcjinPJC18kjpRTv8";
    final SourceSccpProofs.TronDestinationBinding tronBinding =
        SourceSccpProofs.tronDestinationBinding(
            SourceSccpProofs.DOMAIN_SORA,
            SourceSccpProofs.DOMAIN_TRON,
            "0x" + repeat("33", 32),
            tronAddress,
            "0x" + repeat("bb", 32),
            "0x" + repeat("cc", 32));
    assert tronBinding.key.equals(
            "tron:0:5:"
                + repeat("33", 32)
                + ":"
                + tronAddress
                + ":0x"
                + repeat("bb", 32)
                + ":0x"
                + repeat("cc", 32))
        : "TRON destination binding key must match the governed tuple";
    assert tronBinding.hash.equals(
            "0x17c953ad5b8c9a2b6f7102aca993fa7c427d018505cf4f58fac35ea454caba7f")
        : "TRON destination binding hash must match Rust";
    assert SourceSccpProofs.tronDestinationBindingHash(
            SourceSccpProofs.DOMAIN_SORA,
            SourceSccpProofs.DOMAIN_TRON,
            "0x" + repeat("33", 32),
            tronAddress,
            "0x" + repeat("bb", 32),
            "0x" + repeat("cc", 32))
        .equals(tronBinding.hash)
        : "TRON destination binding hash helper must match binding.hash";

    boolean sameEvmAddressThrew = false;
    try {
      SourceSccpProofs.evmDestinationBinding(
          SourceSccpProofs.DOMAIN_SORA,
          SourceSccpProofs.DOMAIN_ETH,
          "0x" + repeat("33", 32),
          "0x" + repeat("11", 20),
          "0x" + repeat("11", 20),
          "0x" + repeat("bb", 32),
          "0x" + repeat("cc", 32));
    } catch (final IllegalArgumentException exception) {
      sameEvmAddressThrew = exception.getMessage().contains("bridgeAddress");
    }
    assert sameEvmAddressThrew : "EVM binding helper must reject reused verifier/bridge address";

    boolean badTronAddressThrew = false;
    try {
      SourceSccpProofs.tronDestinationBinding(
          SourceSccpProofs.DOMAIN_SORA,
          SourceSccpProofs.DOMAIN_TRON,
          "0x" + repeat("33", 32),
          "TJRabPrwbZy45sbavfcjinPJC18kjpRTv9",
          "0x" + repeat("bb", 32),
          "0x" + repeat("cc", 32));
    } catch (final IllegalArgumentException exception) {
      badTronAddressThrew = exception.getMessage().contains("verifierAddress");
    }
    assert badTronAddressThrew : "TRON binding helper must reject invalid Base58Check address";
    expectThrowsMessage(
        () ->
            SourceSccpProofs.tronDestinationBinding(
                SourceSccpProofs.DOMAIN_SORA,
                SourceSccpProofs.DOMAIN_TRON,
                "0x" + repeat("33", 32),
                " " + tronAddress,
                "0x" + repeat("bb", 32),
                "0x" + repeat("cc", 32)),
        "canonical Base58Check");
  }

  private static void derivesSourceMaterialAndDeploymentRecordHashesForUiTooling() {
    final int[] domains = {
      SourceSccpProofs.DOMAIN_ETH,
      SourceSccpProofs.DOMAIN_BSC,
      SourceSccpProofs.DOMAIN_SOL,
      SourceSccpProofs.DOMAIN_TON,
      SourceSccpProofs.DOMAIN_TRON,
      SourceSccpProofs.DOMAIN_SORA_KUSAMA,
      SourceSccpProofs.DOMAIN_SORA_POLKADOT,
      SourceSccpProofs.DOMAIN_SORA2
    };
    final String[] materialHashes = {
      "0x035c5a35f6412d45ed10389741016d067bd6d0b874a38cd744922c599e0a2fdd",
      "0x1630e4d75e2676cc443e07b0477303240ae4cff13bdf9fe61725b4a9a4ee959a",
      "0x499a7363142d5fcfe3a79b11a29ae2ad897e853649e80e39a162b8942f908331",
      "0x08b11177113ac2d9f612abdf767a017de560d805e965b3dc32e28c8748ea2ebc",
      "0x68c20262e44676bd5f3c4ec428f063373147a1ca14c5885648a9c651b3bcd8d8",
      "0x012c66498a85190d6075c441fad30fe01816796ee1713838fe8bb97f2ad1c924",
      "0x40cd55d64e92d688b839242e170f1722485cddf2e42b4ff22e53c5e7723e570d",
      "0x6fc968441106993502dd05ebeadea1dbfee0f7814680f1ad006d4584c99a8a2d"
    };
    final String[] deploymentHashes = {
      "0xd08e3344760aabfb4ba891990c852846d04a5735647174ce6e3ab0f2cad57f4d",
      "0x7d47ade779a5bddb3a5f283600af677db8605b75a00516a4328f3823ff28fb2d",
      "0xcdb2a81cb31e58d9bc1f4292d33c3f4990b2d2008dda1b9b1275aaac087461cc",
      "0x5c4e226c1f4619311762a9c889f8e3b99ea6f020317c2e8a0c76a08d7a70f887",
      "0x94dbe28a2fb16e043b83639b6dea8ec62f53679599ef1dd220fd13c71c7bdcb8",
      "0xda47a31715813ef5bff0882cd0e0e8b0cc89d426e005e37e0f94a2bdba2043cd",
      "0x2a57fe4beb69e8201299f2c01259a025cafc8388bb38e2a727c2fc872893e13a",
      "0xdac819bff0aa57f7596f06297dfec39027aaab63213497020b772c355a6eaecb"
    };
    for (int i = 0; i < domains.length; i++) {
      final int domain = domains[i];
      assert sampleSourceVerifierMaterialBytes(domain).length > 0
          : "source material record bytes must not be empty";
      assert sampleSourceVerifierMaterialHash(domain).equals(materialHashes[i])
          : "source material record hash must match Rust";
      assert sampleSourceAdapterDeploymentHash(domain, null).equals(deploymentHashes[i])
          : "source adapter deployment record hash must match Rust";
    }
    boolean unusedSourceStateThrew = false;
    try {
      SourceSccpProofs.canonicalSourceVerifierMaterialBytes(
          SourceSccpProofs.DOMAIN_ETH,
          "0x" + repeat("44", 32),
          "0x" + repeat("55", 32),
          "0x" + repeat("66", 32),
          "0x" + repeat("88", 32),
          "0x" + repeat("77", 32),
          "0x" + repeat("11", 20),
          "0x" + repeat("77", 32),
          null,
          null,
          null);
    } catch (final IllegalArgumentException exception) {
      unusedSourceStateThrew = exception.getMessage().contains("sourceStateVerifierHash");
    }
    assert unusedSourceStateThrew
        : "source material helper must reject inapplicable source-state verifier hash";

    boolean unusedSourceBridgeThrew = false;
    try {
      SourceSccpProofs.canonicalSourceVerifierMaterialBytes(
          SourceSccpProofs.DOMAIN_SOL,
          "0x" + repeat("44", 32),
          "0x" + repeat("55", 32),
          "0x" + repeat("66", 32),
          "0x" + repeat("88", 32),
          "0x" + repeat("77", 32),
          "0x" + repeat("11", 20),
          null,
          null,
          null,
          null);
    } catch (final IllegalArgumentException exception) {
      unusedSourceBridgeThrew = exception.getMessage().contains("sourceBridgeEmitterAddress");
    }
    assert unusedSourceBridgeThrew
        : "source material helper must reject inapplicable source-bridge address";

    boolean unusedSourceConfigThrew = false;
    try {
      SourceSccpProofs.canonicalSourceVerifierMaterialBytes(
          SourceSccpProofs.DOMAIN_ETH,
          "0x" + repeat("44", 32),
          "0x" + repeat("55", 32),
          "0x" + repeat("66", 32),
          "0x" + repeat("88", 32),
          null,
          "0x" + repeat("11", 20),
          "0x" + repeat("77", 32),
          "0x" + repeat("33", 32),
          null,
          null);
    } catch (final IllegalArgumentException exception) {
      unusedSourceConfigThrew = exception.getMessage().contains("sourceBridgeNetworkId");
    }
    assert unusedSourceConfigThrew
        : "source material helper must reject inapplicable source-bridge config";
    final String[][] tonTemplateComponentHashes = {
      {
        "sourceTrustAnchorHash",
        "0xd83b3a3eb920ac8338533535cf0d6c69c69d507e84aef8ec2094564b8427c56c"
      },
      {
        "consensusVerifierHash",
        "0xb0225e16477ea3420f7d0de76b87b6e99a43ab97f445d8565a384d4b655bc473"
      },
      {
        "messageInclusionVerifierHash",
        "0x89254256421c15da8c92842c7d6f448ef6c1d5ca1e2a173754643425fcee6353"
      },
      {
        "sourceStateVerifierHash",
        "0x540205f876591604ccf39f72a051ac5e82647c9e48dbd48cb129d2543971a34f"
      },
      {
        "finalityPolicyHash",
        "0x50044ee6db0eb0cdef097e69406b6c30d3406d8f784e8ba34e9b923b38bd0c43"
      }
    };
    for (final String[] component : tonTemplateComponentHashes) {
      boolean tonTemplateComponentThrew = false;
      final String field = component[0];
      final String templateHash = component[1];
      try {
        SourceSccpProofs.canonicalSourceVerifierMaterialBytes(
            SourceSccpProofs.DOMAIN_TON,
            "sourceTrustAnchorHash".equals(field) ? templateHash : "0x" + repeat("44", 32),
            "consensusVerifierHash".equals(field) ? templateHash : "0x" + repeat("55", 32),
            "messageInclusionVerifierHash".equals(field)
                ? templateHash
                : "0x" + repeat("66", 32),
            "finalityPolicyHash".equals(field) ? templateHash : "0x" + repeat("88", 32),
            "sourceStateVerifierHash".equals(field) ? templateHash : "0x" + repeat("77", 32),
            null,
            null,
            null,
            null,
            null);
      } catch (final IllegalArgumentException exception) {
        tonTemplateComponentThrew =
            exception.getMessage().contains("TON template verifier hash")
                || exception.getMessage().contains("TON template component hash");
      }
      assert tonTemplateComponentThrew
          : "TON source material helper must reject template component " + field;
    }
    final String[][] tronTemplateComponentHashes = {
      {
        "sourceTrustAnchorHash",
        "0x3550934cbdfe49449ec4aa383dcea7674541fedf66ab6159b1ed2f2c0be4755c"
      },
      {
        "consensusVerifierHash",
        "0x8a1de96a869b2f28f197a7835597f17cf77ff45f7cbb77da2f7c48e87df8c5ea"
      },
      {
        "messageInclusionVerifierHash",
        "0xf39db56474b288680ad9561389cca7a841bd1fd223719255324705e1038fcacc"
      },
      {
        "finalityPolicyHash",
        "0xad5a6a4f200e070400b5aaa1b7976c639e67571eb711eb6f69d01e3615423864"
      }
    };
    for (final String[] component : tronTemplateComponentHashes) {
      boolean tronTemplateComponentThrew = false;
      final String field = component[0];
      final String templateHash = component[1];
      try {
        SourceSccpProofs.canonicalSourceVerifierMaterialBytes(
            SourceSccpProofs.DOMAIN_TRON,
            "sourceTrustAnchorHash".equals(field) ? templateHash : "0x" + repeat("44", 32),
            "consensusVerifierHash".equals(field) ? templateHash : "0x" + repeat("55", 32),
            "messageInclusionVerifierHash".equals(field)
                ? templateHash
                : "0x" + repeat("66", 32),
            "finalityPolicyHash".equals(field) ? templateHash : "0x" + repeat("88", 32),
            null,
            "0x" + repeat("11", 20),
            "0x" + repeat("77", 32),
            "0x" + repeat("33", 32),
            "0x" + repeat("22", 20),
            "0xe986dd67bfa2307b4e00cf46bde41a88003a55c5b7fea311fa106614b2252f9d");
      } catch (final IllegalArgumentException exception) {
        tronTemplateComponentThrew =
            exception.getMessage().contains("TRON template component hash");
      }
      assert tronTemplateComponentThrew
          : "TRON source material helper must reject template component " + field;
    }
    final String[][] solanaTemplateComponentHashes = {
      {
        "sourceTrustAnchorHash",
        "0x113bdb7601d84f2098daec386346a7123857d181b3ac5bd23df50fa9e1b2cbe3"
      },
      {
        "consensusVerifierHash",
        "0x97ea89019e6c79305d06dfc27640ee14a6b42ba6eaf86e1835ee9b433dba48ba"
      },
      {
        "messageInclusionVerifierHash",
        "0xb8358bfef1e428a6a7e9115687cb2b88d9c21dad4021bea3e11d43489eb3dcb0"
      },
      {
        "sourceStateVerifierHash",
        SolanaSccpProver.TEMPLATE_SOURCE_STATE_VERIFIER_HASH_V1
      },
      {
        "finalityPolicyHash",
        "0x9df7ea90cf1bbba036788b14804f63f4be1e908390be89524fd4486f74344f56"
      }
    };
    for (final String[] component : solanaTemplateComponentHashes) {
      boolean solanaTemplateComponentThrew = false;
      final String field = component[0];
      final String templateHash = component[1];
      try {
        SourceSccpProofs.canonicalSourceVerifierMaterialBytes(
            SourceSccpProofs.DOMAIN_SOL,
            "sourceTrustAnchorHash".equals(field) ? templateHash : "0x" + repeat("44", 32),
            "consensusVerifierHash".equals(field) ? templateHash : "0x" + repeat("55", 32),
            "messageInclusionVerifierHash".equals(field)
                ? templateHash
                : "0x" + repeat("66", 32),
            "finalityPolicyHash".equals(field) ? templateHash : "0x" + repeat("88", 32),
            "sourceStateVerifierHash".equals(field) ? templateHash : "0x" + repeat("77", 32),
            null,
            null,
            null,
            null,
            null);
      } catch (final IllegalArgumentException exception) {
        solanaTemplateComponentThrew =
            exception.getMessage().contains("Solana template verifier hash")
                || exception.getMessage().contains("Solana template component hash");
      }
      assert solanaTemplateComponentThrew
          : "Solana source material helper must reject template component " + field;
    }
    boolean mismatchedTronConfigHashThrew = false;
    try {
      SourceSccpProofs.canonicalSourceVerifierMaterialBytes(
          SourceSccpProofs.DOMAIN_TRON,
          "0x" + repeat("44", 32),
          "0x" + repeat("55", 32),
          "0x" + repeat("66", 32),
          "0x" + repeat("88", 32),
          null,
          "0x" + repeat("11", 20),
          "0x" + repeat("77", 32),
          "0x" + repeat("33", 32),
          "0x" + repeat("22", 20),
          "0x" + repeat("99", 32));
    } catch (final IllegalArgumentException exception) {
      mismatchedTronConfigHashThrew =
          exception.getMessage().contains("TRON source bridge config fields");
    }
    assert mismatchedTronConfigHashThrew
        : "TRON source material helper must reject mismatched source bridge config hash";

    boolean reusedSourceMaterialRoleThrew = false;
    try {
      SourceSccpProofs.canonicalSourceVerifierMaterialBytes(
          SourceSccpProofs.DOMAIN_ETH,
          "0x" + repeat("44", 32),
          "0x" + repeat("44", 32),
          "0x" + repeat("66", 32),
          "0x" + repeat("88", 32),
          null,
          "0x" + repeat("11", 20),
          "0x" + repeat("77", 32),
          null,
          null,
          null);
    } catch (final IllegalArgumentException exception) {
      reusedSourceMaterialRoleThrew = exception.getMessage().contains("role-separated");
    }
    assert reusedSourceMaterialRoleThrew
        : "source material helper must reject reused role hashes";

    boolean reusedDeploymentRoleThrew = false;
    try {
      SourceSccpProofs.canonicalSourceAdapterEngineDeploymentBytes(
          SourceSccpProofs.DOMAIN_ETH,
          "0x" + repeat("44", 32),
          "0x" + repeat("55", 32),
          "0x" + repeat("66", 32),
          "0x" + repeat("88", 32),
          SourceSccpProofs.sourceAdapterVerifierVkHash(SourceSccpProofs.DOMAIN_ETH),
          SourceSccpProofs.DOMAIN_SORA,
          null,
          null,
          "0x" + repeat("11", 20),
          "0x" + repeat("77", 32),
          null,
          null,
          null);
    } catch (final IllegalArgumentException exception) {
      reusedDeploymentRoleThrew = exception.getMessage().contains("role-separated");
    }
    assert reusedDeploymentRoleThrew
        : "source deployment helper must reject reused role hashes";

    assert sampleSourceAdapterDeploymentHash(
            SourceSccpProofs.DOMAIN_SOL,
            null,
            "0x" + repeat("bb", 32),
            "0x" + repeat("cc", 32),
            "0x" + repeat("dd", 32))
        .equals("0x97e5c4196aff6387b9d973e663de3ce9345e1d8c3de89d22505b2197e282dc61")
        : "Solana audited deployment record hash must match Rust";
    assert sampleSolanaFullLightClientGateHash(
            "0x" + repeat("bb", 32), "0x" + repeat("cc", 32), "0x" + repeat("dd", 32))
        .equals("0x2c94b86a665bb68708b762c678661f5e9879bd588627e93a640796eeaef970f9")
        : "Solana full light-client gate hash must match Rust";

    boolean zeroGateThrew = false;
    try {
      sampleSolanaFullLightClientGateHash(
          "0x" + repeat("00", 32), "0x" + repeat("cc", 32), "0x" + repeat("dd", 32));
    } catch (final IllegalArgumentException exception) {
      zeroGateThrew = exception.getMessage().contains("solanaTowerReplayVerifierHash");
    }
    assert zeroGateThrew : "Solana full light-client gate hash must reject zero verifier hashes";

    boolean duplicateSolanaAuditThrew = false;
    try {
      sampleSolanaFullLightClientGateHash(
          "0x" + repeat("bb", 32), "0x" + repeat("bb", 32), "0x" + repeat("dd", 32));
    } catch (final IllegalArgumentException exception) {
      duplicateSolanaAuditThrew = exception.getMessage().contains("role-separated");
    }
    assert duplicateSolanaAuditThrew
        : "Solana full light-client gate hash must reject duplicate audit verifier hashes";

    boolean reusedSolanaStateThrew = false;
    try {
      sampleSolanaFullLightClientGateHash(
          sourceStateVerifierHash(SourceSccpProofs.DOMAIN_SOL),
          "0x" + repeat("cc", 32),
          "0x" + repeat("dd", 32));
    } catch (final IllegalArgumentException exception) {
      reusedSolanaStateThrew =
          exception.getMessage().contains("source-adapter material");
    }
    assert reusedSolanaStateThrew
        : "Solana full light-client gate hash must reject audit reuse of source material";

    boolean reusedSolanaTemplateAuditThrew = false;
    try {
      sampleSolanaFullLightClientGateHash(
          "0x113bdb7601d84f2098daec386346a7123857d181b3ac5bd23df50fa9e1b2cbe3",
          "0x" + repeat("cc", 32),
          "0x" + repeat("dd", 32));
    } catch (final IllegalArgumentException exception) {
      reusedSolanaTemplateAuditThrew =
          exception.getMessage().contains("template material");
    }
    assert reusedSolanaTemplateAuditThrew
        : "Solana full light-client gate hash must reject audit reuse of template material";

    boolean templateSolanaStateThrew = false;
    try {
      sampleSolanaFullLightClientGateHash(
          "0x" + repeat("bb", 32),
          "0x" + repeat("cc", 32),
          "0x" + repeat("dd", 32),
          SolanaSccpProver.TEMPLATE_SOURCE_STATE_VERIFIER_HASH_V1);
    } catch (final IllegalArgumentException exception) {
      templateSolanaStateThrew =
          exception.getMessage().contains("Solana template verifier hash");
    }
    assert templateSolanaStateThrew
        : "Solana full light-client gate hash must reject the template source-state verifier";

    boolean partialAuditThrew = false;
    try {
      sampleSourceAdapterDeploymentHash(
          SourceSccpProofs.DOMAIN_SOL, null, "0x" + repeat("bb", 32), null, null);
    } catch (final IllegalArgumentException exception) {
      partialAuditThrew = exception.getMessage().contains("Solana audit verifier hashes");
    }
    assert partialAuditThrew : "partial Solana deployment audit material must be rejected";

    boolean nonSolAuditThrew = false;
    try {
      sampleSourceAdapterDeploymentHash(
          SourceSccpProofs.DOMAIN_TON,
          null,
          "0x" + repeat("bb", 32),
          "0x" + repeat("cc", 32),
          "0x" + repeat("dd", 32));
    } catch (final IllegalArgumentException exception) {
      nonSolAuditThrew = exception.getMessage().contains("only used for Solana deployments");
    }
    assert nonSolAuditThrew : "non-Solana deployment audit material must be rejected";

    assert sampleSourceAdapterDeploymentHash(
            SourceSccpProofs.DOMAIN_TON,
            null,
            null,
            null,
            null,
            "0x" + repeat("bb", 32),
            "0x" + repeat("cc", 32),
            "0x" + repeat("dd", 32))
        .equals("0x61e5d710ccbc902be00a38a5a80d05c19de97105605a3f93d4f8067862d81f07")
        : "TON audited deployment record hash must match Rust";
    assert sampleTonFullLightClientGateHash(
            "0x" + repeat("bb", 32), "0x" + repeat("cc", 32), "0x" + repeat("dd", 32))
        .equals("0xc32d8cfc2e273646abb00911b9a15e7ee0ab1721b04a6e89a060422dd3cc4596")
        : "TON full light-client gate hash must match Rust";

    boolean zeroTonGateThrew = false;
    try {
      sampleTonFullLightClientGateHash(
          "0x" + repeat("00", 32), "0x" + repeat("cc", 32), "0x" + repeat("dd", 32));
    } catch (final IllegalArgumentException exception) {
      zeroTonGateThrew = exception.getMessage().contains("tonMasterchainConfigVerifierHash");
    }
    assert zeroTonGateThrew : "TON full light-client gate hash must reject zero verifier hashes";

    boolean duplicateTonAuditThrew = false;
    try {
      sampleTonFullLightClientGateHash(
          "0x" + repeat("bb", 32), "0x" + repeat("bb", 32), "0x" + repeat("dd", 32));
    } catch (final IllegalArgumentException exception) {
      duplicateTonAuditThrew = exception.getMessage().contains("role-separated");
    }
    assert duplicateTonAuditThrew : "TON audit verifier hashes must be role-separated";

    boolean reusedTonAuditThrew = false;
    try {
      sampleTonFullLightClientGateHash(
          sourceStateVerifierHash(SourceSccpProofs.DOMAIN_TON),
          "0x" + repeat("cc", 32),
          "0x" + repeat("dd", 32));
    } catch (final IllegalArgumentException exception) {
      reusedTonAuditThrew = exception.getMessage().contains("source-adapter material");
    }
    assert reusedTonAuditThrew
        : "TON audit verifier hashes must not reuse source-adapter material";

    boolean reusedTonTemplateAuditThrew = false;
    try {
      sampleTonFullLightClientGateHash(
          "0xd83b3a3eb920ac8338533535cf0d6c69c69d507e84aef8ec2094564b8427c56c",
          "0x" + repeat("cc", 32),
          "0x" + repeat("dd", 32));
    } catch (final IllegalArgumentException exception) {
      reusedTonTemplateAuditThrew = exception.getMessage().contains("template material");
    }
    assert reusedTonTemplateAuditThrew
        : "TON audit verifier hashes must not reuse template material";

    boolean partialTonAuditThrew = false;
    try {
      sampleSourceAdapterDeploymentHash(
          SourceSccpProofs.DOMAIN_TON,
          null,
          null,
          null,
          null,
          "0x" + repeat("bb", 32),
          null,
          null);
    } catch (final IllegalArgumentException exception) {
      partialTonAuditThrew = exception.getMessage().contains("TON audit verifier hashes");
    }
    assert partialTonAuditThrew : "partial TON deployment audit material must be rejected";

    boolean nonTonAuditThrew = false;
    try {
      sampleSourceAdapterDeploymentHash(
          SourceSccpProofs.DOMAIN_SOL,
          null,
          null,
          null,
          null,
          "0x" + repeat("bb", 32),
          "0x" + repeat("cc", 32),
          "0x" + repeat("dd", 32));
    } catch (final IllegalArgumentException exception) {
      nonTonAuditThrew = exception.getMessage().contains("only used for TON deployments");
    }
    assert nonTonAuditThrew : "non-TON deployment audit material must be rejected";

    boolean threw = false;
    try {
      sampleSourceAdapterDeploymentHash(SourceSccpProofs.DOMAIN_ETH, "0x" + repeat("99", 32));
    } catch (final IllegalArgumentException exception) {
      threw = exception.getMessage().contains("canonical source-adapter verifier profile");
    }
    assert threw : "deployment record helper must reject noncanonical adapter verifier VKs";
  }

  private static void derivesSourceProofHashesFromWitnessMaterial() {
    final String sourceEventDigest = repeat("34", 32);
    final String zeroSourceEventDigest = repeat("00", 32);
    final java.util.List<byte[]> branch = Collections.singletonList(bytes(0xee));
    final java.util.List<byte[]> changedBranch = Collections.singletonList(bytes(0x12));
    final String evmReceiptRootMptValueHex =
        "f8409e736363703a65766d3a726563656970742d726f6f742d76616c75653a7631a0"
            + repeat("bb", 32);
    final java.util.List<byte[]> evmReceiptTrieProofNodes =
        Collections.singletonList(hexBytes("f847822080b842" + evmReceiptRootMptValueHex));
    final java.util.List<byte[]> changedReceiptTrieProofNodes =
        Collections.singletonList(
            hexBytes(
                "f847822080b842f8409e736363703a65766d3a726563656970742d726f6f742d76616c75653a7631a0"
                    + repeat("aa", 32)));
    final String evmReceiptsRoot =
        "6438aaabb78989f2803c6b0f227ee0f94beecde07cdd9c737e258e4faf581b68";

    final byte[] evmBytes =
        SourceSccpProofs.canonicalEvmReceiptProofBytes(
            sourceEventDigest,
            "11",
            "12",
            repeat("aa", 32),
            evmReceiptsRoot,
            repeat("cc", 32),
            repeat("dd", 32),
            "0",
            evmReceiptTrieProofNodes,
            branch);
    assert evmBytes.length == 306 : "EVM receipt proof transcript must have expected length";
    final String evmHash =
        SourceSccpProofs.evmReceiptProofHash(
            sourceEventDigest,
            "11",
            "12",
            repeat("aa", 32),
            evmReceiptsRoot,
            repeat("cc", 32),
            repeat("dd", 32),
            "0",
            evmReceiptTrieProofNodes,
            branch);
    final String changedEvmHash =
        SourceSccpProofs.evmReceiptProofHash(
            sourceEventDigest,
            "11",
            "12",
            repeat("aa", 32),
            evmReceiptsRoot,
            repeat("cc", 32),
            repeat("dd", 32),
            "0",
            evmReceiptTrieProofNodes,
            changedBranch);
    final String changedEvmReceiptTrieHash =
        SourceSccpProofs.evmReceiptProofHash(
            sourceEventDigest,
            "11",
            "12",
            repeat("aa", 32),
            evmReceiptsRoot,
            repeat("cc", 32),
            repeat("dd", 32),
            "0",
            changedReceiptTrieProofNodes,
            branch);
    assert evmHash.matches("0x[0-9a-f]{64}") : "EVM receipt proof hash must be hex";
    assert !evmHash.equals(changedEvmHash) : "EVM receipt proof hash must bind branch";
    assert !evmHash.equals(changedEvmReceiptTrieHash)
        : "EVM receipt proof hash must bind receipt trie proof nodes";
    expectThrowsMessage(
        () ->
            SourceSccpProofs.canonicalEvmReceiptProofBytes(
                zeroSourceEventDigest,
                "11",
                "12",
                repeat("aa", 32),
                evmReceiptsRoot,
                repeat("cc", 32),
                repeat("dd", 32),
                "0",
                evmReceiptTrieProofNodes,
                branch),
        "sourceEventDigest must not be zero");

    assert SourceSccpProofs.canonicalBscReceiptProofBytes(
                sourceEventDigest,
                "21",
                "22",
                repeat("aa", 32),
                evmReceiptsRoot,
                repeat("cc", 32),
                repeat("dd", 32),
                "0",
                evmReceiptTrieProofNodes,
                branch)
            .length
        == 306 : "BSC receipt proof transcript must have expected length";
    expectThrowsMessage(
        () ->
            SourceSccpProofs.canonicalBscReceiptProofBytes(
                zeroSourceEventDigest,
                "21",
                "22",
                repeat("aa", 32),
                evmReceiptsRoot,
                repeat("cc", 32),
                repeat("dd", 32),
                "0",
                evmReceiptTrieProofNodes,
                branch),
        "sourceEventDigest must not be zero");
    assert !SourceSccpProofs.bscReceiptProofHash(
            sourceEventDigest,
            "21",
            "22",
            repeat("aa", 32),
            evmReceiptsRoot,
            repeat("cc", 32),
            repeat("dd", 32),
            "0",
            evmReceiptTrieProofNodes,
            branch)
        .equals(
            SourceSccpProofs.bscReceiptProofHash(
                sourceEventDigest,
                "21",
                "22",
                repeat("aa", 32),
                evmReceiptsRoot,
                repeat("cc", 32),
                repeat("dd", 32),
                "0",
                evmReceiptTrieProofNodes,
                changedBranch)) : "BSC receipt proof hash must bind branch";
    assert !SourceSccpProofs.bscReceiptProofHash(
            sourceEventDigest,
            "21",
            "22",
            repeat("aa", 32),
            evmReceiptsRoot,
            repeat("cc", 32),
            repeat("dd", 32),
            "0",
            evmReceiptTrieProofNodes,
            branch)
        .equals(
            SourceSccpProofs.bscReceiptProofHash(
                sourceEventDigest,
                "21",
                "22",
                repeat("aa", 32),
                evmReceiptsRoot,
                repeat("cc", 32),
                repeat("dd", 32),
                "0",
                changedReceiptTrieProofNodes,
                branch)) : "BSC receipt proof hash must bind receipt trie proof nodes";

    final byte[] validatorPayload =
        SourceSccpProofs.canonicalBscValidatorSetPayloadBytes(
            Arrays.asList(repeat("11", 20), repeat("22", 20)), Arrays.asList("1", "2"));
    assert bytesToHex(validatorPayload)
            .equals(
                "0102000000"
                    + repeat("11", 20)
                    + "0100000000000000"
                    + repeat("22", 20)
                    + "0200000000000000")
        : "BSC validator-set payload must be canonical";
    assert SourceSccpProofs.bscValidatorSetPayloadHash(validatorPayload)
            .equals("0xdc6190956bc147c9a0a2fbf1384d40a1deb4b211a709f229275d1ea5ac3f8370")
        : "BSC validator-set payload hash must match Rust verifier";
    assert SourceSccpProofs.bscValidatorSetPayloadHash(
            Arrays.asList(repeat("11", 20), repeat("22", 20)), Arrays.asList("1", "2"))
        .equals("0xdc6190956bc147c9a0a2fbf1384d40a1deb4b211a709f229275d1ea5ac3f8370")
        : "BSC validator-set payload hash must accept address/power input";
    assert SourceSccpProofs.bscValidatorSetHashFromPayload(validatorPayload)
            .equals("0x3ef5ecfb6dc4f5fc9e970cc18cd72164495c827e96f77851813973a286f5c762")
        : "BSC validator-set hash must derive from payload";
    final java.util.List<String> oversizedBscValidators = new java.util.ArrayList<>();
    for (int i = 1; i <= 256; i++) {
      oversizedBscValidators.add(String.format("%040x", i));
    }
    expectThrows(
        () ->
            SourceSccpProofs.canonicalBscValidatorSetPayloadBytes(
                oversizedBscValidators, Collections.nCopies(256, "1")));
    final List<byte[]> bscCommitValidatorPublicKeys =
        Arrays.asList(
            hexBytes("0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798"),
            hexBytes("02c6047f9441ed7d6d3045406e95c07cd85c778e4b8cef3ca7abac09b95c709ee5"),
            hexBytes("02f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9"),
            hexBytes("02e493dbf1c10d80f3581e4904930b1404cc6c13900ee0758474fa94abe8c4cd13"));
    final String bscCommitValidatorSetHash =
        "0xc5152802f6ca9ec72a4249646aca7476496f00b71ab5b1482c881a31fb42dd8c";
    final String bscCommitMessageHash =
        "0x5832165d1a87ed49a323f2ecaecbef973489aed1a42e7eab369244e7abec43c7";
    final List<byte[]> bscCommitSignatures =
        Arrays.asList(
            hexBytes("1b8802069b82c3d4cb6d7bec82323853f36d965c1e71647560084e7c7a0de9c17c85fcc3c6222f905cbbc4ba5b5f3f005f07d144304184181be67b3d02d1ba9f00"),
            hexBytes("921d39c29fb793c496f96cf647128232d228024ed2f3e68cc6a52aa4cf64facf6bbd9dfcf7d703165f7880e7e1310f34d1b0fb8ca6dd8f506bf289ba012387f001"),
            hexBytes("cfa11aa1ec214278afdb4ef7f3c40af97a2784e0336afb5ebef345c0d2eaa9ef629ad2d25cf9709eb9b842fb2fb3f749ce365af97af6e7064771614312d3619600"));
    assert SourceSccpProofs.canonicalBscCommitMessageBytes(
                "2",
                "401",
                repeat("22", 32),
                repeat("33", 32),
                bscCommitValidatorSetHash)
            .length
        == 117 : "BSC commit message transcript must have expected length";
    assert SourceSccpProofs.bscCommitMessageHash(
            "2", "401", repeat("22", 32), repeat("33", 32), bscCommitValidatorSetHash)
        .equals(bscCommitMessageHash) : "BSC commit message hash must match Rust verifier";
    expectThrowsMessage(
        () ->
            SourceSccpProofs.bscCommitMessageHash(
                "2",
                "401",
                repeat("22", 32),
                repeat("33", 32),
                bscCommitValidatorSetHash,
                SourceSccpProofs.DOMAIN_ETH),
        "sourceDomain");
    final SourceSccpProofs.BscCommitSealProof bscCommitSeal =
        new SourceSccpProofs.BscCommitSealProof(
            1,
            "4",
            "3",
            bscCommitMessageHash,
            bscCommitValidatorPublicKeys,
            Arrays.asList("1", "1", "1", "1"),
            hexBytes("07"),
            bscCommitSignatures,
            bscCommitValidatorSetHash);
    assert SourceSccpProofs.canonicalBscCommitSealBytes(bscCommitSeal).length == 297
        : "BSC commit seal transcript must have expected length";
    assert SourceSccpProofs.bscCommitSealHash(bscCommitSeal)
            .equals("0xcd9d87b24d8c1cf7615cb4267cde5a3fc24bbb770807134ee75d4ddaba992172")
        : "BSC commit seal hash must match Rust verifier";
    expectThrowsMessage(
        () ->
            SourceSccpProofs.canonicalBscCommitSealBytes(
                new SourceSccpProofs.BscCommitSealProof(
                    1,
                    "4",
                    "2",
                    bscCommitMessageHash,
                    bscCommitValidatorPublicKeys,
                    Arrays.asList("1", "1", "1", "1"),
                    hexBytes("03"),
                    bscCommitSignatures.subList(0, 2),
                    bscCommitValidatorSetHash)),
        "two thirds");
    expectThrowsMessage(
        () ->
            SourceSccpProofs.canonicalBscCommitSealBytes(
                new SourceSccpProofs.BscCommitSealProof(
                    1,
                    "4",
                    "3",
                    bscCommitMessageHash,
                    bscCommitValidatorPublicKeys,
                    Arrays.asList("1", "1", "1", "1"),
                    hexBytes("1f"),
                    bscCommitSignatures,
                    bscCommitValidatorSetHash)),
        "padding bits");
    final byte[] changedBscCommitSignature = Arrays.copyOf(bscCommitSignatures.get(0), 65);
    changedBscCommitSignature[0] = 0x31;
    expectThrowsMessage(
        () ->
            SourceSccpProofs.canonicalBscCommitSealBytes(
                new SourceSccpProofs.BscCommitSealProof(
                    1,
                    "4",
                    "3",
                    bscCommitMessageHash,
                    bscCommitValidatorPublicKeys,
                    Arrays.asList("1", "1", "1", "1"),
                    hexBytes("07"),
                    Arrays.asList(
                        changedBscCommitSignature,
                        bscCommitSignatures.get(1),
                        bscCommitSignatures.get(2)),
                    bscCommitValidatorSetHash)),
        "recover");
    expectThrowsMessage(
        () ->
            SourceSccpProofs.canonicalBscCommitSealBytes(
                new SourceSccpProofs.BscCommitSealProof(
                    1,
                    "4",
                    "3",
                    bscCommitMessageHash,
                    bscCommitValidatorPublicKeys,
                    Arrays.asList("1", "1", "1", "1"),
                    hexBytes("07"),
                    bscCommitSignatures,
                    repeat("aa", 32))),
        "validatorSetHash");
    final byte[] storageValue = hexBytes("02");
    final String storageValueHash = SourceSccpProofs.bscValidatorSetStorageValueHash(storageValue);
    final SourceSccpProofs.BscValidatorSetMetadataProof metadataProof =
        new SourceSccpProofs.BscValidatorSetMetadataProof(
            1,
            repeat("aa", 32),
            SourceSccpProofs.bscValidatorSetPayloadHash(validatorPayload),
            hexBytes(repeat("00", 18) + "1000"),
            Collections.singletonList(hexBytes("f842a0" + repeat("11", 32))),
            repeat("bb", 32),
            repeat("cc", 32),
            storageValue,
            storageValueHash,
            Collections.singletonList(hexBytes("e4822080a0" + repeat("22", 32))),
            Arrays.asList(
                new SourceSccpProofs.BscValidatorStorageProof(
                    1,
                    0,
                    repeat("dd", 32),
                    hexBytes("94" + repeat("11", 20)),
                    SourceSccpProofs.bscValidatorSetStorageValueHash(
                        hexBytes("94" + repeat("11", 20))),
                    Collections.singletonList(hexBytes("e4822080a0" + repeat("33", 32)))),
                new SourceSccpProofs.BscValidatorStorageProof(
                    1,
                    1,
                    repeat("ee", 32),
                    hexBytes("94" + repeat("22", 20)),
                    SourceSccpProofs.bscValidatorSetStorageValueHash(
                        hexBytes("94" + repeat("22", 20))),
                    Collections.singletonList(hexBytes("e4822080a0" + repeat("44", 32))))));
    assert SourceSccpProofs.canonicalBscValidatorSetMetadataProofBytes(metadataProof).length == 560
        : "BSC ValidatorSet metadata proof transcript must have expected length";
    expectThrows(
        () ->
            SourceSccpProofs.canonicalBscValidatorSetMetadataProofBytes(
                new SourceSccpProofs.BscValidatorSetMetadataProof(
                    0,
                    metadataProof.stateRoot,
                    metadataProof.nextValidatorSetPayloadHash,
                    metadataProof.validatorContractAddress,
                    metadataProof.accountProofNodes,
                    metadataProof.storageRoot,
                    metadataProof.validatorSetLengthSlot,
                    metadataProof.validatorSetLengthValue,
                    metadataProof.validatorSetLengthValueHash,
                    metadataProof.validatorSetLengthProofNodes,
                    metadataProof.validatorStorageProofs)));
    final SourceSccpProofs.BscValidatorStorageProof firstStorageProof =
        metadataProof.validatorStorageProofs.get(0);
    expectThrows(
        () ->
            SourceSccpProofs.canonicalBscValidatorSetMetadataProofBytes(
                new SourceSccpProofs.BscValidatorSetMetadataProof(
                    1,
                    metadataProof.stateRoot,
                    metadataProof.nextValidatorSetPayloadHash,
                    metadataProof.validatorContractAddress,
                    metadataProof.accountProofNodes,
                    metadataProof.storageRoot,
                    metadataProof.validatorSetLengthSlot,
                    metadataProof.validatorSetLengthValue,
                    metadataProof.validatorSetLengthValueHash,
                    metadataProof.validatorSetLengthProofNodes,
                    Collections.singletonList(
                        new SourceSccpProofs.BscValidatorStorageProof(
                            0,
                            firstStorageProof.validatorIndex,
                            firstStorageProof.storageSlot,
                            firstStorageProof.storageValue,
                            firstStorageProof.storageValueHash,
                            firstStorageProof.storageProofNodes)))));
    expectThrows(
        () ->
            SourceSccpProofs.canonicalBscValidatorSetMetadataProofBytes(
                bscMetadataProofLike(metadataProof, bytes(0x12, 19), null, null, null, null)));
    expectThrows(
        () ->
            SourceSccpProofs.canonicalBscValidatorSetMetadataProofBytes(
                bscMetadataProofLike(
                    metadataProof, null, Collections.emptyList(), null, null, null)));
    expectThrows(
        () ->
            SourceSccpProofs.canonicalBscValidatorSetMetadataProofBytes(
                bscMetadataProofLike(
                    metadataProof, null, null, Collections.emptyList(), null, null)));
    expectThrows(
        () ->
            SourceSccpProofs.canonicalBscValidatorSetMetadataProofBytes(
                bscMetadataProofLike(metadataProof, null, null, null, null, Collections.emptyList())));
    expectThrows(
        () ->
            SourceSccpProofs.canonicalBscValidatorSetMetadataProofBytes(
                bscMetadataProofLike(
                    metadataProof,
                    null,
                    null,
                    null,
                    null,
                    Collections.singletonList(
                        new SourceSccpProofs.BscValidatorStorageProof(
                            1,
                            firstStorageProof.validatorIndex,
                            firstStorageProof.storageSlot,
                            firstStorageProof.storageValue,
                            firstStorageProof.storageValueHash,
                            Collections.emptyList())))));
    expectThrows(
        () ->
            SourceSccpProofs.canonicalBscValidatorSetMetadataProofBytes(
                bscMetadataProofLike(metadataProof, null, null, null, repeat("ff", 32), null)));
    expectThrows(
        () ->
            SourceSccpProofs.canonicalBscValidatorSetMetadataProofBytes(
                bscMetadataProofLike(
                    metadataProof,
                    null,
                    null,
                    null,
                    null,
                    Collections.singletonList(
                        new SourceSccpProofs.BscValidatorStorageProof(
                            1,
                            firstStorageProof.validatorIndex,
                            firstStorageProof.storageSlot,
                            firstStorageProof.storageValue,
                            repeat("ff", 32),
                            firstStorageProof.storageProofNodes)))));
    final String metadataHash = SourceSccpProofs.bscValidatorSetMetadataProofHash(metadataProof);
    assert metadataHash.matches("0x[0-9a-f]{64}") : "BSC metadata proof hash must be hex";
    final SourceSccpProofs.BscValidatorSetMetadataProof changedMetadataProof =
        new SourceSccpProofs.BscValidatorSetMetadataProof(
            1,
            repeat("12", 32),
            metadataProof.nextValidatorSetPayloadHash,
            metadataProof.validatorContractAddress,
            metadataProof.accountProofNodes,
            metadataProof.storageRoot,
            metadataProof.validatorSetLengthSlot,
            metadataProof.validatorSetLengthValue,
            metadataProof.validatorSetLengthValueHash,
            metadataProof.validatorSetLengthProofNodes,
            metadataProof.validatorStorageProofs);
    assert !metadataHash.equals(
            SourceSccpProofs.bscValidatorSetMetadataProofHash(changedMetadataProof))
        : "BSC metadata proof hash must bind the state root";
    assert SourceSccpProofs.canonicalBscValidatorSetTransitionMessageBytes(
                "41",
                "42",
                "8400",
                repeat("aa", 32),
                repeat("bb", 32),
                SourceSccpProofs.bscValidatorSetHashFromPayload(validatorPayload),
                SourceSccpProofs.bscValidatorSetPayloadHash(validatorPayload),
                metadataHash)
            .length
        == 189 : "BSC ValidatorSet transition message transcript must have expected length";
    assert !SourceSccpProofs.bscValidatorSetTransitionMessageHash(
            "41",
            "42",
            "8400",
            repeat("aa", 32),
            repeat("bb", 32),
            SourceSccpProofs.bscValidatorSetHashFromPayload(validatorPayload),
            SourceSccpProofs.bscValidatorSetPayloadHash(validatorPayload),
            metadataHash)
        .equals(
            SourceSccpProofs.bscValidatorSetTransitionMessageHash(
                "41",
                "42",
                "8400",
                repeat("aa", 32),
                repeat("bb", 32),
                SourceSccpProofs.bscValidatorSetHashFromPayload(validatorPayload),
                SourceSccpProofs.bscValidatorSetPayloadHash(validatorPayload),
                repeat("12", 32))) : "BSC transition message hash must bind metadata proof hash";
    expectThrowsMessage(
        () ->
            SourceSccpProofs.bscValidatorSetTransitionMessageHash(
                "41",
                "42",
                "8401",
                repeat("aa", 32),
                repeat("bb", 32),
                SourceSccpProofs.bscValidatorSetHashFromPayload(validatorPayload),
                SourceSccpProofs.bscValidatorSetPayloadHash(validatorPayload),
                metadataHash),
        "epoch-start block");
    expectThrowsMessage(
        () ->
            SourceSccpProofs.bscValidatorSetTransitionMessageHash(
                "41",
                "43",
                "8400",
                repeat("aa", 32),
                repeat("bb", 32),
                SourceSccpProofs.bscValidatorSetHashFromPayload(validatorPayload),
                SourceSccpProofs.bscValidatorSetPayloadHash(validatorPayload),
                metadataHash),
        "fromValidatorEpoch");
    expectThrowsMessage(
        () ->
            SourceSccpProofs.bscValidatorSetTransitionMessageHash(
                "41",
                "42",
                "8400",
                repeat("aa", 32),
                repeat("bb", 32),
                SourceSccpProofs.bscValidatorSetHashFromPayload(validatorPayload),
                SourceSccpProofs.bscValidatorSetPayloadHash(validatorPayload),
                metadataHash,
                0),
        "sourceDomain");
    final byte[] parliaPayload =
        SourceSccpProofs.canonicalBscValidatorSetPayloadBytes(
            Arrays.asList(repeat("11", 20), repeat("22", 20)), Arrays.asList("1", "1"));
    final byte[] parliaExtra = sampleBscParliaExtra();
    assert bytesToHex(SourceSccpProofs.bscValidatorSetPayloadFromParliaExtra(parliaExtra))
            .equals(bytesToHex(parliaPayload))
        : "BSC Parlia extraData must extract validator payload";
    assert bytesToHex(
            SourceSccpProofs.bscValidatorSetPayloadFromHeaderRlp(
                sampleBscParliaHeaderRlp(parliaExtra)))
            .equals(bytesToHex(parliaPayload))
        : "BSC Parlia header RLP must extract validator payload";
    expectThrows(() -> SourceSccpProofs.bscValidatorSetPayloadFromHeaderRlp(new byte[] {(byte) 0x80}));
    expectThrows(
        () ->
            SourceSccpProofs.canonicalBscValidatorSetPayloadBytes(
                Arrays.asList(repeat("11", 20), repeat("11", 20)), Arrays.asList("1", "2")));
    expectThrows(
        () ->
            SourceSccpProofs.canonicalBscValidatorSetPayloadBytes(
                Collections.singletonList(repeat("11", 20)), Collections.singletonList("0")));

    final java.util.List<byte[]> tonValidatorPublicKeys =
        Arrays.asList(bytes(0x11, 32), bytes(0x22, 32));
    final java.util.List<String> tonValidatorWeights = Arrays.asList("1", "2");
    final byte[] tonValidatorSetPayload =
        SourceSccpProofs.canonicalTonValidatorSetPayloadBytes(
            tonValidatorPublicKeys, tonValidatorWeights);
    assert bytesToHex(tonValidatorSetPayload)
            .equals(
                "0102000000"
                    + repeat("11", 32)
                    + "0100000000000000"
                    + repeat("22", 32)
                    + "0200000000000000")
        : "TON validator-set payload must be canonical";
    assert SourceSccpProofs.tonValidatorSetHash(tonValidatorPublicKeys, tonValidatorWeights)
            .equals("0x68bfccd52bc19cf8cdaffc611d58e53824d0aee395a4d813eca0bcefb3970938")
        : "TON validator-set hash must match Rust verifier";
    assert SourceSccpProofs.tonValidatorSetPayloadHash(tonValidatorSetPayload)
            .equals("0xb322afe2faa070a2ed88a922c5ac5d27e5f9fecc41a11ffbed37cca293c4aeb0")
        : "TON validator-set payload hash must match Rust verifier";
    assert SourceSccpProofs.canonicalTonMasterchainBlockMessageBytes(
                "19",
                -1,
                "9223372036854775808",
                repeat("aa", 32),
                repeat("a5", 32),
                "0x68bfccd52bc19cf8cdaffc611d58e53824d0aee395a4d813eca0bcefb3970938",
                "0x5bf87008e0e76085d6db977b53a89329de49a4eed8fd1ff90d8c78f096ef05af",
                "0x99c5bb835574b49d4aea21ae2820044f403b987c1aa1cdfa0ec5f7a262b5139e",
                0,
                "9223372036854775808",
                "7",
                repeat("bb", 32),
                repeat("bc", 32),
                repeat("cc", 32),
                repeat("dd", 32),
                repeat("ee", 32),
                SourceSccpProofs.DOMAIN_TON)
            .length
        == 365 : "TON masterchain block-message transcript must have expected length";
    final String tonBlockMessageHash =
        SourceSccpProofs.tonMasterchainBlockMessageHash(
            "19",
            -1,
            "9223372036854775808",
            repeat("aa", 32),
            repeat("a5", 32),
            "0x68bfccd52bc19cf8cdaffc611d58e53824d0aee395a4d813eca0bcefb3970938",
            "0x5bf87008e0e76085d6db977b53a89329de49a4eed8fd1ff90d8c78f096ef05af",
            "0x99c5bb835574b49d4aea21ae2820044f403b987c1aa1cdfa0ec5f7a262b5139e",
            0,
            "9223372036854775808",
            "7",
            repeat("bb", 32),
            repeat("bc", 32),
            repeat("cc", 32),
            repeat("dd", 32),
            repeat("ee", 32),
            SourceSccpProofs.DOMAIN_TON);
    assert tonBlockMessageHash.equals(
            "0xa00389d016059db04cc59c3032047ffb214782d4aa747302568636344fa7c74f")
        : "TON masterchain block-message hash must match Rust verifier";
    final SourceSccpProofs.TonValidatorSignatureProof tonSignatureProof =
        new SourceSccpProofs.TonValidatorSignatureProof(
            1,
            "3",
            "3",
            tonBlockMessageHash,
            tonValidatorPublicKeys,
            tonValidatorWeights,
            new byte[] {0x03},
            Arrays.asList(bytes(0xab, 64), bytes(0xcd, 64)),
            "0x68bfccd52bc19cf8cdaffc611d58e53824d0aee395a4d813eca0bcefb3970938");
    assert SourceSccpProofs.canonicalTonMasterchainValidatorSignaturesBytes(tonSignatureProof)
            .length
        == 322 : "TON masterchain signatures transcript must have expected length";
    assert SourceSccpProofs.tonMasterchainValidatorSignaturesHash(tonSignatureProof)
            .equals("0xc31577a0488fe754d44eb0aafae46a8e4be36b0088b0cdec4ad34f8d0a7acedd")
        : "TON masterchain signatures hash must match Rust verifier";
    expectThrows(
        () ->
            SourceSccpProofs.canonicalTonMasterchainBlockMessageBytes(
                "19",
                0,
                "9223372036854775808",
                repeat("aa", 32),
                repeat("a5", 32),
                "0x68bfccd52bc19cf8cdaffc611d58e53824d0aee395a4d813eca0bcefb3970938",
                "0x5bf87008e0e76085d6db977b53a89329de49a4eed8fd1ff90d8c78f096ef05af",
                "0x99c5bb835574b49d4aea21ae2820044f403b987c1aa1cdfa0ec5f7a262b5139e",
                0,
                "9223372036854775808",
                "7",
                repeat("bb", 32),
                repeat("bc", 32),
                repeat("cc", 32),
                repeat("dd", 32),
                repeat("ee", 32),
                SourceSccpProofs.DOMAIN_TON));
    expectThrows(
        () ->
            SourceSccpProofs.canonicalTonMasterchainValidatorSignaturesBytes(
                new SourceSccpProofs.TonValidatorSignatureProof(
                    1,
                    "3",
                    "3",
                    tonBlockMessageHash,
                    tonValidatorPublicKeys,
                    tonValidatorWeights,
                    new byte[] {0x03},
                    Arrays.asList(bytes(0xab, 64), bytes(0xcd, 64)),
                    repeat("bb", 32))));
    expectThrows(
        () ->
            SourceSccpProofs.canonicalTonMasterchainValidatorSignaturesBytes(
                new SourceSccpProofs.TonValidatorSignatureProof(
                    0,
                    "3",
                    "3",
                    tonBlockMessageHash,
                    tonValidatorPublicKeys,
                    tonValidatorWeights,
                    new byte[] {0x03},
                    Arrays.asList(bytes(0xab, 64), bytes(0xcd, 64)),
                    "0x68bfccd52bc19cf8cdaffc611d58e53824d0aee395a4d813eca0bcefb3970938")));
    expectThrows(
        () ->
            SourceSccpProofs.canonicalTonMasterchainValidatorSignaturesBytes(
                new SourceSccpProofs.TonValidatorSignatureProof(
                    1,
                    "4",
                    "3",
                    tonBlockMessageHash,
                    tonValidatorPublicKeys,
                    tonValidatorWeights,
                    new byte[] {0x03},
                    Arrays.asList(bytes(0xab, 64), bytes(0xcd, 64)),
                    "0x68bfccd52bc19cf8cdaffc611d58e53824d0aee395a4d813eca0bcefb3970938")));
    expectThrows(
        () ->
            SourceSccpProofs.canonicalTonMasterchainValidatorSignaturesBytes(
                new SourceSccpProofs.TonValidatorSignatureProof(
                    1,
                    "3",
                    "2",
                    tonBlockMessageHash,
                    tonValidatorPublicKeys,
                    tonValidatorWeights,
                    new byte[] {0x03},
                    Arrays.asList(bytes(0xab, 64), bytes(0xcd, 64)),
                    "0x68bfccd52bc19cf8cdaffc611d58e53824d0aee395a4d813eca0bcefb3970938")));
    expectThrows(
        () ->
            SourceSccpProofs.canonicalTonMasterchainValidatorSignaturesBytes(
                new SourceSccpProofs.TonValidatorSignatureProof(
                    1,
                    "3",
                    "1",
                    tonBlockMessageHash,
                    tonValidatorPublicKeys,
                    tonValidatorWeights,
                    new byte[] {0x01},
                    Collections.singletonList(bytes(0xab, 64)),
                    "0x68bfccd52bc19cf8cdaffc611d58e53824d0aee395a4d813eca0bcefb3970938")));
    expectThrows(
        () ->
            SourceSccpProofs.canonicalTonMasterchainValidatorSignaturesBytes(
                new SourceSccpProofs.TonValidatorSignatureProof(
                    1,
                    "3",
                    "0",
                    tonBlockMessageHash,
                    tonValidatorPublicKeys,
                    tonValidatorWeights,
                    new byte[] {0x00},
                    Collections.<byte[]>emptyList(),
                    "0x68bfccd52bc19cf8cdaffc611d58e53824d0aee395a4d813eca0bcefb3970938")));
    expectThrows(
        () ->
            SourceSccpProofs.canonicalTonMasterchainValidatorSignaturesBytes(
                new SourceSccpProofs.TonValidatorSignatureProof(
                    1,
                    "3",
                    "3",
                    tonBlockMessageHash,
                    tonValidatorPublicKeys,
                    tonValidatorWeights,
                    new byte[] {0x04},
                    Collections.<byte[]>emptyList(),
                    "0x68bfccd52bc19cf8cdaffc611d58e53824d0aee395a4d813eca0bcefb3970938")));
    expectThrows(
        () ->
            SourceSccpProofs.canonicalTonMasterchainValidatorSignaturesBytes(
                new SourceSccpProofs.TonValidatorSignatureProof(
                    1,
                    "3",
                    "3",
                    tonBlockMessageHash,
                    tonValidatorPublicKeys,
                    tonValidatorWeights,
                    new byte[] {0x03},
                    Arrays.asList(new byte[64], bytes(0xcd, 64)),
                    "0x68bfccd52bc19cf8cdaffc611d58e53824d0aee395a4d813eca0bcefb3970938")));
    expectThrows(
        () ->
            SourceSccpProofs.canonicalTonMasterchainValidatorSignaturesBytes(
                new SourceSccpProofs.TonValidatorSignatureProof(
                    1,
                    "3",
                    "3",
                    tonBlockMessageHash,
                    tonValidatorPublicKeys,
                    tonValidatorWeights,
                    new byte[] {0x03},
                    Arrays.asList(bytes(0xab, 63), bytes(0xcd, 64)),
                    "0x68bfccd52bc19cf8cdaffc611d58e53824d0aee395a4d813eca0bcefb3970938")));
    expectThrows(
        () ->
            SourceSccpProofs.canonicalTonMasterchainValidatorSignaturesBytes(
                new SourceSccpProofs.TonValidatorSignatureProof(
                    1,
                    "3",
                    "3",
                    tonBlockMessageHash,
                    Arrays.asList(new byte[32], tonValidatorPublicKeys.get(1)),
                    tonValidatorWeights,
                    new byte[] {0x03},
                    Arrays.asList(bytes(0xab, 64), bytes(0xcd, 64)),
                    "0x68bfccd52bc19cf8cdaffc611d58e53824d0aee395a4d813eca0bcefb3970938")));
    final byte[] zeroTonValidatorSetPayload = new byte[45];
    zeroTonValidatorSetPayload[0] = 1;
    zeroTonValidatorSetPayload[1] = 1;
    zeroTonValidatorSetPayload[37] = 1;
    expectThrows(
        () -> SourceSccpProofs.tonValidatorSetHashFromPayload(zeroTonValidatorSetPayload));

    final java.util.List<byte[]> parentSyncPublicKeys =
        Arrays.asList(bytes(0x11, 48), bytes(0x22, 48));
    final java.util.List<String> parentSyncWeights = Arrays.asList("1", "2");
    final java.util.List<byte[]> parentSyncPops =
        Arrays.asList(bytes(0xaa, 96), bytes(0xbb, 96));
    final byte[] nextSyncPayload =
        SourceSccpProofs.canonicalEthSyncCommitteePayloadBytes(
            Arrays.asList(bytes(0x33, 48), bytes(0x44, 48)),
            Arrays.asList("3", "4"),
            Arrays.asList(bytes(0xcc, 96), bytes(0xdd, 96)));
    assert SourceSccpProofs.ethSyncCommitteeHash(
            parentSyncPublicKeys, parentSyncWeights, parentSyncPops)
        .equals("0xa95be780d50a9f42f4b1871e29798dbee0352d08027f0c4c6f4fc6466b4bd536")
        : "ETH sync-committee hash must derive from witness material";
    assert bytesToHex(nextSyncPayload)
            .equals(
                "010200000030000000"
                    + repeat("33", 48)
                    + "030000000000000060000000"
                    + repeat("cc", 96)
                    + "30000000"
                    + repeat("44", 48)
                    + "040000000000000060000000"
                    + repeat("dd", 96))
        : "ETH sync-committee payload must be canonical";
    assert SourceSccpProofs.ethSyncCommitteeHashFromPayload(nextSyncPayload)
            .equals("0xb3343685e8ab63a2d66bccebb6c03a149a53330389473b4a495598065c17b445")
        : "ETH sync-committee hash must derive from payload";
    assert SourceSccpProofs.ethSyncCommitteePayloadHash(nextSyncPayload)
            .equals("0xfdba6ad2ff9acca564b1042eec01c2d6356d5e2ade5e653c9d47360e55d53e17")
        : "ETH sync-committee payload hash must match Rust verifier";
    final String ethTransitionMessageHash =
        SourceSccpProofs.ethSyncCommitteeTransitionMessageHash(
            "7",
            "8",
            "19",
            repeat("aa", 32),
            "0xa95be780d50a9f42f4b1871e29798dbee0352d08027f0c4c6f4fc6466b4bd536",
            "0xb3343685e8ab63a2d66bccebb6c03a149a53330389473b4a495598065c17b445",
            "0xfdba6ad2ff9acca564b1042eec01c2d6356d5e2ade5e653c9d47360e55d53e17",
            repeat("be", 32));
    assert ethTransitionMessageHash.equals(
            "0xc5cbfaf915a63e59bc142277814f13fab1e8012a0bd56db7033b18bc02637bec")
        : "ETH sync-committee transition message hash must match Rust verifier";
    assert SourceSccpProofs.canonicalEthSyncCommitteeTransitionSignatureBytes(
                "7",
                "8",
                "19",
                repeat("aa", 32),
                "0xa95be780d50a9f42f4b1871e29798dbee0352d08027f0c4c6f4fc6466b4bd536",
                "0xb3343685e8ab63a2d66bccebb6c03a149a53330389473b4a495598065c17b445",
                nextSyncPayload,
                "0xfdba6ad2ff9acca564b1042eec01c2d6356d5e2ade5e653c9d47360e55d53e17",
                repeat("be", 32),
                ethTransitionMessageHash,
                "3",
                "3",
                parentSyncPublicKeys,
                parentSyncWeights,
                parentSyncPops,
                new byte[] {0x03},
                bytes(0xee, 96))
            .length
        == 1068 : "ETH sync-committee transition signature bytes must match Rust length";
    assert SourceSccpProofs.ethSyncCommitteeTransitionSignatureHash(
            "7",
            "8",
            "19",
            repeat("aa", 32),
            "0xa95be780d50a9f42f4b1871e29798dbee0352d08027f0c4c6f4fc6466b4bd536",
            "0xb3343685e8ab63a2d66bccebb6c03a149a53330389473b4a495598065c17b445",
            nextSyncPayload,
            "0xfdba6ad2ff9acca564b1042eec01c2d6356d5e2ade5e653c9d47360e55d53e17",
            repeat("be", 32),
            ethTransitionMessageHash,
            "3",
            "3",
            parentSyncPublicKeys,
            parentSyncWeights,
            parentSyncPops,
            new byte[] {0x03},
            bytes(0xee, 96))
        .equals("0x2d03886e7ea307f7b5a77af00075b32536cbf016d0d8554bec2b1e424252f858")
        : "ETH sync-committee transition signature hash must match Rust verifier";
    expectThrows(
        () ->
            SourceSccpProofs.canonicalEthSyncCommitteeTransitionSignatureBytes(
                "7",
                "8",
                "19",
                repeat("aa", 32),
                "0xa95be780d50a9f42f4b1871e29798dbee0352d08027f0c4c6f4fc6466b4bd536",
                "0xb3343685e8ab63a2d66bccebb6c03a149a53330389473b4a495598065c17b445",
                nextSyncPayload,
                "0xfdba6ad2ff9acca564b1042eec01c2d6356d5e2ade5e653c9d47360e55d53e17",
                repeat("be", 32),
                ethTransitionMessageHash,
                "3",
                "3",
                parentSyncPublicKeys,
                parentSyncWeights,
                parentSyncPops,
                new byte[] {0x03},
                bytes(0xee, 96),
                SourceSccpProofs.DOMAIN_ETH,
                0,
                1));
    expectThrows(
        () ->
            SourceSccpProofs.canonicalEthSyncCommitteeTransitionSignatureBytes(
                "7",
                "8",
                "19",
                repeat("aa", 32),
                "0xa95be780d50a9f42f4b1871e29798dbee0352d08027f0c4c6f4fc6466b4bd536",
                "0xb3343685e8ab63a2d66bccebb6c03a149a53330389473b4a495598065c17b445",
                nextSyncPayload,
                "0xfdba6ad2ff9acca564b1042eec01c2d6356d5e2ade5e653c9d47360e55d53e17",
                repeat("be", 32),
                ethTransitionMessageHash,
                "3",
                "3",
                parentSyncPublicKeys,
                parentSyncWeights,
                parentSyncPops,
                new byte[] {0x03},
                bytes(0xee, 96),
                SourceSccpProofs.DOMAIN_ETH,
                1,
                0));
    expectThrows(
        () ->
            SourceSccpProofs.canonicalEthSyncCommitteePayloadBytes(
                Collections.nCopies(513, bytes(0x11, 48)),
                Collections.nCopies(513, "1"),
                Collections.nCopies(513, bytes(0xaa, 96))));
    expectThrows(
        () ->
            SourceSccpProofs.canonicalEthSyncCommitteePayloadBytes(
                Arrays.asList(bytes(0x11, 47), parentSyncPublicKeys.get(1)),
                parentSyncWeights,
                parentSyncPops));
    expectThrows(
        () ->
            SourceSccpProofs.canonicalEthSyncCommitteePayloadBytes(
                Arrays.asList(new byte[48], parentSyncPublicKeys.get(1)),
                parentSyncWeights,
                parentSyncPops));
    expectThrows(
        () ->
            SourceSccpProofs.canonicalEthSyncCommitteePayloadBytes(
                parentSyncPublicKeys,
                parentSyncWeights,
                Arrays.asList(new byte[96], parentSyncPops.get(1))));
    expectThrows(
        () ->
            SourceSccpProofs.canonicalEthBeaconSyncCommitteeProofBytes(
                "3",
                "3",
                ethTransitionMessageHash,
                parentSyncPublicKeys,
                parentSyncWeights,
                parentSyncPops,
                new byte[65],
                bytes(0xee, 96)));
    expectThrows(
        () ->
            SourceSccpProofs.canonicalEthBeaconSyncCommitteeProofBytes(
                "3",
                "0",
                ethTransitionMessageHash,
                parentSyncPublicKeys,
                parentSyncWeights,
                parentSyncPops,
                new byte[] {0x00},
                bytes(0xee, 96)));
    expectThrows(
        () ->
            SourceSccpProofs.canonicalEthBeaconSyncCommitteeProofBytes(
                "3",
                "3",
                ethTransitionMessageHash,
                parentSyncPublicKeys,
                parentSyncWeights,
                parentSyncPops,
                new byte[] {0x04},
                bytes(0xee, 96)));
    expectThrows(
        () ->
            SourceSccpProofs.canonicalEthBeaconSyncCommitteeProofBytes(
                "3",
                "2",
                ethTransitionMessageHash,
                parentSyncPublicKeys,
                parentSyncWeights,
                parentSyncPops,
                new byte[] {0x01},
                bytes(0xee, 96)));
    expectThrows(
        () ->
            SourceSccpProofs.canonicalEthBeaconSyncCommitteeProofBytes(
                "4",
                "3",
                ethTransitionMessageHash,
                parentSyncPublicKeys,
                parentSyncWeights,
                parentSyncPops,
                new byte[] {0x03},
                bytes(0xee, 96)));
    expectThrows(
        () ->
            SourceSccpProofs.canonicalEthBeaconSyncCommitteeProofBytes(
                "3",
                "1",
                ethTransitionMessageHash,
                parentSyncPublicKeys,
                parentSyncWeights,
                parentSyncPops,
                new byte[] {0x01},
                bytes(0xee, 96)));
    expectThrows(
        () ->
            SourceSccpProofs.canonicalEthBeaconSyncCommitteeProofBytes(
                "3",
                "3",
                ethTransitionMessageHash,
                parentSyncPublicKeys,
                parentSyncWeights,
                parentSyncPops,
                new byte[] {0x03},
                new byte[96]));

    final byte[] witnessPayload =
        SourceSccpProofs.canonicalTronWitnessSchedulePayloadBytes(
            Arrays.asList("41" + repeat("11", 20), "41" + repeat("22", 20)),
            Arrays.asList("1", "2"));
    assert bytesToHex(witnessPayload)
            .equals(
                "010200000041"
                    + repeat("11", 20)
                    + "010000000000000041"
                    + repeat("22", 20)
                    + "0200000000000000")
        : "TRON witness-schedule payload must be canonical";
    assert SourceSccpProofs.tronWitnessSchedulePayloadHash(witnessPayload)
            .equals("0xd6087d6ea6a1b58b17523587f28e457d84d5d2214298f93a09dbb509ea2cf429")
        : "TRON witness-schedule payload hash must match Rust verifier";
    assert SourceSccpProofs.tronWitnessSchedulePayloadHash(
            Arrays.asList("41" + repeat("11", 20), "41" + repeat("22", 20)),
            Arrays.asList("1", "2"))
        .equals("0xd6087d6ea6a1b58b17523587f28e457d84d5d2214298f93a09dbb509ea2cf429")
        : "TRON witness-schedule payload hash must accept address/weight input";
    assert SourceSccpProofs.tronWitnessScheduleHashFromPayload(witnessPayload)
            .equals("0x0c5eca6f96572fe939e640d8951abd126d2e966ffc4e3d0d087dbff6052577be")
        : "TRON witness-schedule hash must derive from payload";
    final byte[] zeroWitnessPayload = hexBytes("010100000041" + repeat("00", 20) + "0100000000000000");
    expectThrows(() -> SourceSccpProofs.tronWitnessSchedulePayloadHash(zeroWitnessPayload));
    expectThrows(() -> SourceSccpProofs.tronWitnessScheduleHashFromPayload(zeroWitnessPayload));
    final java.util.List<String> oversizedWitnessAddresses = new java.util.ArrayList<String>();
    final java.util.List<String> oversizedWitnessWeights = new java.util.ArrayList<String>();
    for (int i = 0; i < 65; i++) {
      oversizedWitnessAddresses.add("41" + repeat("11", 19) + String.format("%02x", i));
      oversizedWitnessWeights.add("1");
    }
    expectThrows(
        () ->
            SourceSccpProofs.canonicalTronWitnessSchedulePayloadBytes(
                oversizedWitnessAddresses, oversizedWitnessWeights));
    expectThrows(
        () ->
            SourceSccpProofs.canonicalTronWitnessSchedulePayloadBytes(
                Arrays.asList("41" + repeat("11", 20), "41" + repeat("11", 20)),
                Arrays.asList("1", "2")));
    expectThrows(
        () ->
            SourceSccpProofs.canonicalTronWitnessSchedulePayloadBytes(
                Collections.singletonList("41" + repeat("00", 20)), Collections.singletonList("1")));
    expectThrows(
        () ->
            SourceSccpProofs.canonicalTronWitnessSchedulePayloadBytes(
                Collections.singletonList("41" + repeat("11", 20)), Collections.singletonList("0")));
    expectThrows(
        () ->
            SourceSccpProofs.canonicalTronWitnessSchedulePayloadBytes(
                Arrays.asList("41" + repeat("11", 20), "41" + repeat("22", 20)),
                Arrays.asList("18446744073709551615", "1")));
    final byte[] overflowingWitnessPayload =
        hexBytes(
            "010200000041"
                + repeat("11", 20)
                + "ffffffffffffffff41"
                + repeat("22", 20)
                + "0100000000000000");
    expectThrows(() -> SourceSccpProofs.tronWitnessSchedulePayloadHash(overflowingWitnessPayload));
    expectThrows(() -> SourceSccpProofs.tronWitnessScheduleHashFromPayload(overflowingWitnessPayload));

    final String tronWitnessScheduleHash =
        "0x0c5eca6f96572fe939e640d8951abd126d2e966ffc4e3d0d087dbff6052577be";
    assert SourceSccpProofs.tronSolidBlockMessageHash(
            SourceSccpProofs.DOMAIN_TRON,
            "12345",
            "0x0000000000003039b6bc08fb34f737c093d9dd2adefccb04344715e2619c8286",
            tronWitnessScheduleHash,
            repeat("bb", 32),
            repeat("dd", 32),
            repeat("cc", 32))
        .equals("0x065173d89272a549b504258936729c5226dfdb866ccb9422757d95ec9fa6d688")
        : "TRON solid-block message hash must match Rust verifier";
    expectThrows(
        () ->
            SourceSccpProofs.canonicalTronSolidBlockMessageBytes(
                SourceSccpProofs.DOMAIN_ETH,
                "12345",
                "0x0000000000003039b6bc08fb34f737c093d9dd2adefccb04344715e2619c8286",
                tronWitnessScheduleHash,
                repeat("bb", 32),
                repeat("dd", 32),
                repeat("cc", 32)));
    final String tronTestOwnerAddress = "0x417e5f4552091a69125d5dfcb7b8c2659029395bdf";
    final String tronSourceEventTransactionId =
        "be9223cdfd6728fd2512f270a44f928fbd58df98f8e9e5fe13c4dc73503192e4";
    final byte[] tronSourceEventSignature =
        hexBytes(
            "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798"
                + "38508a4cf743e4a97ab3550672d69d980545ff8d776f6e9bade4ff4196f3693b"
                + "00");
    assert SourceSccpProofs.tronWitnessSealHash(
            "1",
            "1",
            "0x" + tronSourceEventTransactionId,
            Collections.singletonList(tronTestOwnerAddress),
            Collections.singletonList("1"),
            new byte[] {0x01},
            Collections.singletonList(tronSourceEventSignature))
        .equals("0x4266cf4de71c96e4fde925b686abbd50e67026f63ad90e0cf4899d4925d45849")
        : "TRON witness seal hash must match Rust verifier";
    expectThrows(
        () ->
            SourceSccpProofs.canonicalTronWitnessSealBytes(
                "1",
                "1",
                "0x" + tronSourceEventTransactionId,
                Collections.singletonList("0x41" + repeat("11", 20)),
                Collections.singletonList("1"),
                new byte[] {0x01},
                Collections.singletonList(tronSourceEventSignature)));
    final byte[] parentWitnessSchedulePayload =
        hexBytes("0101000000417e5f4552091a69125d5dfcb7b8c2659029395bdf0100000000000000");
    final String parentWitnessScheduleHash =
        "0x87174bbfde1c4b8473a6be18df37b60979c7609ebf1788ce8cf97604311474b6";
    assert SourceSccpProofs.tronWitnessScheduleHashFromPayload(parentWitnessSchedulePayload)
        .equals(parentWitnessScheduleHash) : "parent witness-schedule hash must match vector";
    final byte[] transitionMessage =
        SourceSccpProofs.canonicalTronWitnessScheduleTransitionMessageBytes(
            SourceSccpProofs.DOMAIN_TRON,
            "7",
            "8",
            "12345",
            "0x0000000000003039b6bc08fb34f737c093d9dd2adefccb04344715e2619c8286",
            parentWitnessScheduleHash,
            tronWitnessScheduleHash,
            null,
            witnessPayload);
    assert transitionMessage.length == 157 : "TRON witness-schedule transition message size";
    assert bytesToHex(transitionMessage)
            .equals(
                "0105000000070000000000000008000000000000003930000000000000"
                    + "0000000000003039b6bc08fb34f737c093d9dd2adefccb04344715e2619c8286"
                    + "87174bbfde1c4b8473a6be18df37b60979c7609ebf1788ce8cf97604311474b6"
                    + "0c5eca6f96572fe939e640d8951abd126d2e966ffc4e3d0d087dbff6052577be"
                    + "d6087d6ea6a1b58b17523587f28e457d84d5d2214298f93a09dbb509ea2cf429")
        : "TRON witness-schedule transition message must be canonical";
    final String transitionMessageHash =
        "0x6e53d3f7d1253223a70a163a02544a8df27b74171cb0c76c8f42d71419fabd43";
    assert SourceSccpProofs.tronWitnessScheduleTransitionMessageHash(
            SourceSccpProofs.DOMAIN_TRON,
            "7",
            "8",
            "12345",
            "0x0000000000003039b6bc08fb34f737c093d9dd2adefccb04344715e2619c8286",
            parentWitnessScheduleHash,
            tronWitnessScheduleHash,
            null,
            witnessPayload)
        .equals(transitionMessageHash) : "TRON transition message hash must match Rust verifier";
    final byte[] transitionSignature =
        hexBytes(
            "c6047f9441ed7d6d3045406e95c07cd85c778e4b8cef3ca7abac09b95c709ee5"
                + "65d3d639f676a837945854abb3f59c4b93355bb55a789e31a25aee261500932d01");
    assert SourceSccpProofs.tronWitnessScheduleTransitionSealHash(
            SourceSccpProofs.DOMAIN_TRON,
            "7",
            "8",
            "12345",
            "0x0000000000003039b6bc08fb34f737c093d9dd2adefccb04344715e2619c8286",
            parentWitnessScheduleHash,
            tronWitnessScheduleHash,
            witnessPayload,
            transitionMessageHash,
            "1",
            "1",
            Collections.singletonList(tronTestOwnerAddress),
            Collections.singletonList("1"),
            new byte[] {0x01},
            Collections.singletonList(transitionSignature))
        .equals("0xbb3b7ef87bd3efb77d9b7f0a4dba8e7398827621d59039c694c285a7e2deacce")
        : "TRON transition seal hash must match Rust verifier";
    expectThrows(
        () ->
            SourceSccpProofs.canonicalTronWitnessScheduleTransitionSealBytes(
                SourceSccpProofs.DOMAIN_TRON,
                "7",
                "8",
                "12345",
                "0x0000000000003039b6bc08fb34f737c093d9dd2adefccb04344715e2619c8286",
                parentWitnessScheduleHash,
                tronWitnessScheduleHash,
                witnessPayload,
                "0x" + repeat("dd", 32),
                "1",
                "1",
                Collections.singletonList(tronTestOwnerAddress),
                Collections.singletonList("1"),
                new byte[] {0x01},
                Collections.singletonList(transitionSignature)));

    final byte[] authorityPayload =
        SourceSccpProofs.canonicalSubstrateAuthoritySetPayloadBytes(
            Arrays.asList(repeat("11", 32), repeat("22", 32)), Arrays.asList("1", "2"));
    assert bytesToHex(authorityPayload)
            .equals(
                "0102000000"
                    + repeat("11", 32)
                    + "0100000000000000"
                    + repeat("22", 32)
                    + "0200000000000000")
        : "Substrate authority-set payload must be canonical";
    assert SourceSccpProofs.substrateAuthoritySetPayloadHash(authorityPayload)
            .equals("0xdedc4ebe5f91162a5029cb67f88cdbbf94c2bf2b9d0d373bd3e670321565cc16")
        : "Substrate authority-set payload hash must match Rust verifier";
    assert SourceSccpProofs.substrateAuthoritySetPayloadHash(
            Arrays.asList(repeat("11", 32), repeat("22", 32)), Arrays.asList("1", "2"))
        .equals("0xdedc4ebe5f91162a5029cb67f88cdbbf94c2bf2b9d0d373bd3e670321565cc16")
        : "Substrate authority-set payload hash must accept authority/weight input";
    assert SourceSccpProofs.substrateAuthoritySetHashFromPayload(authorityPayload)
            .equals("0xde84b8b7a5409c0f2cff1191173d6caa681d902b35e42669106ec6ea3193a117")
        : "Substrate authority-set hash must derive from payload";
    expectThrows(
        () ->
            SourceSccpProofs.canonicalSubstrateAuthoritySetPayloadBytes(
                Arrays.asList(repeat("11", 32), repeat("11", 32)), Arrays.asList("1", "2")));
    expectThrows(
        () ->
            SourceSccpProofs.canonicalSubstrateAuthoritySetPayloadBytes(
                Collections.singletonList(repeat("00", 32)), Collections.singletonList("1")));
    final byte[] zeroAuthorityPayload = new byte[45];
    zeroAuthorityPayload[0] = 1;
    zeroAuthorityPayload[1] = 1;
    zeroAuthorityPayload[37] = 1;
    expectThrows(() -> SourceSccpProofs.substrateAuthoritySetHashFromPayload(zeroAuthorityPayload));
    expectThrows(
        () ->
            SourceSccpProofs.canonicalSubstrateAuthoritySetPayloadBytes(
                Collections.singletonList(repeat("11", 32)), Collections.singletonList("0")));
    final java.util.List<String> oversizedAuthorityKeys = new java.util.ArrayList<String>();
    final java.util.List<String> oversizedAuthorityWeights = new java.util.ArrayList<String>();
    for (int i = 0; i < 2049; i++) {
      oversizedAuthorityKeys.add(repeat("11", 32));
      oversizedAuthorityWeights.add("1");
    }
    expectThrows(
        () ->
            SourceSccpProofs.canonicalSubstrateAuthoritySetPayloadBytes(
                oversizedAuthorityKeys, oversizedAuthorityWeights));

    final java.util.List<String> parentAuthorityKeys =
        Arrays.asList(repeat("11", 32), repeat("22", 32), repeat("33", 32));
    final java.util.List<String> parentAuthorityWeights = Arrays.asList("5", "7", "11");
    final java.util.List<String> nextAuthorityKeys =
        Arrays.asList(repeat("aa", 32), repeat("bb", 32), repeat("cc", 32));
    final java.util.List<String> nextAuthorityWeights = Arrays.asList("13", "17", "19");
    final byte[] parentAuthorityPayload =
        SourceSccpProofs.canonicalSubstrateAuthoritySetPayloadBytes(
            parentAuthorityKeys, parentAuthorityWeights);
    final byte[] nextAuthorityPayload =
        SourceSccpProofs.canonicalSubstrateAuthoritySetPayloadBytes(
            nextAuthorityKeys, nextAuthorityWeights);
    assert bytesToHex(parentAuthorityPayload)
            .equals(
                "0103000000"
                    + repeat("11", 32)
                    + "0500000000000000"
                    + repeat("22", 32)
                    + "0700000000000000"
                    + repeat("33", 32)
                    + "0b00000000000000")
        : "Substrate parent authority-set payload must be canonical";
    assert bytesToHex(nextAuthorityPayload)
            .equals(
                "0103000000"
                    + repeat("aa", 32)
                    + "0d00000000000000"
                    + repeat("bb", 32)
                    + "1100000000000000"
                    + repeat("cc", 32)
                    + "1300000000000000")
        : "Substrate next authority-set payload must be canonical";
    final String parentAuthorityHash =
        "0xb2efd5d86304ea728a8a9ed4013aab8f3e10c0cf862e859c9cade55e660934ef";
    final String nextAuthorityHash =
        "0x07cdbba0d61fdd4324b571dd793965e52acbf7f4c163af328e26c92c047501b3";
    final String nextAuthorityPayloadHash =
        "0x12ce972498ba5cd8a760aee0429fdc30d8b6447890e1bf77d8dde46f86b40d85";
    assert SourceSccpProofs.substrateAuthoritySetHashFromPayload(parentAuthorityPayload)
            .equals(parentAuthorityHash)
        : "Substrate parent authority-set hash must match Rust verifier";
    assert SourceSccpProofs.substrateAuthoritySetHashFromPayload(nextAuthorityPayload)
            .equals(nextAuthorityHash)
        : "Substrate next authority-set hash must match Rust verifier";
    assert SourceSccpProofs.substrateAuthoritySetPayloadHash(nextAuthorityPayload)
            .equals(nextAuthorityPayloadHash)
        : "Substrate next authority-set payload hash must match Rust verifier";
    final String substrateTransitionMessageHash =
        SourceSccpProofs.substrateAuthoritySetTransitionMessageHash(
            SourceSccpProofs.DOMAIN_SORA_KUSAMA,
            "41",
            "42",
            "9001",
            repeat("44", 32),
            parentAuthorityHash,
            nextAuthorityHash,
            nextAuthorityPayloadHash);
    assert substrateTransitionMessageHash.equals(
            "0x60589333bf798bf592b2642d0fbac39b4e9305576cd2ebe9dd1f448a97a0596b")
        : "Substrate transition message hash must match Rust verifier";
    assert SourceSccpProofs.canonicalSubstrateAuthoritySetTransitionMessageBytes(
                SourceSccpProofs.DOMAIN_SORA_KUSAMA,
                "41",
                "42",
                "9001",
                repeat("44", 32),
                parentAuthorityHash,
                nextAuthorityHash,
                nextAuthorityPayloadHash)
            .length
        == 157 : "Substrate transition message bytes must match Rust length";
    assert SourceSccpProofs.canonicalSubstrateAuthoritySetTransitionJustificationBytes(
                1,
                SourceSccpProofs.DOMAIN_SORA_KUSAMA,
                "41",
                "42",
                "9001",
                repeat("44", 32),
                parentAuthorityHash,
                nextAuthorityHash,
                nextAuthorityPayload,
                nextAuthorityPayloadHash,
                substrateTransitionMessageHash,
                1,
                "23",
                "18",
                parentAuthorityKeys,
                parentAuthorityWeights,
                new byte[] {0x06},
                Arrays.asList(bytes(0x77, 64), bytes(0x88, 64)))
            .length
        == 684 : "Substrate transition justification bytes must match Rust length";
    assert SourceSccpProofs.substrateAuthoritySetTransitionJustificationHash(
            1,
            SourceSccpProofs.DOMAIN_SORA_KUSAMA,
            "41",
            "42",
            "9001",
            repeat("44", 32),
            parentAuthorityHash,
            nextAuthorityHash,
            nextAuthorityPayload,
            nextAuthorityPayloadHash,
            substrateTransitionMessageHash,
            1,
            "23",
            "18",
            parentAuthorityKeys,
            parentAuthorityWeights,
            new byte[] {0x06},
            Arrays.asList(bytes(0x77, 64), bytes(0x88, 64)))
        .equals("0x4d50a606c6858d3a4af5caf991a6dd8ac10dce717b14bd36ba70e5b0b098d302")
        : "Substrate transition justification hash must match Rust verifier";
    expectThrows(
        () ->
            SourceSccpProofs.canonicalSubstrateAuthoritySetTransitionJustificationBytes(
                0,
                SourceSccpProofs.DOMAIN_SORA_KUSAMA,
                "41",
                "42",
                "9001",
                repeat("44", 32),
                parentAuthorityHash,
                nextAuthorityHash,
                nextAuthorityPayload,
                nextAuthorityPayloadHash,
                substrateTransitionMessageHash,
                1,
                "23",
                "18",
                parentAuthorityKeys,
                parentAuthorityWeights,
                new byte[] {0x06},
                Arrays.asList(bytes(0x77, 64), bytes(0x88, 64))));
    expectThrows(
        () ->
            SourceSccpProofs.canonicalSubstrateAuthoritySetTransitionJustificationBytes(
                1,
                SourceSccpProofs.DOMAIN_SORA_KUSAMA,
                "41",
                "42",
                "9001",
                repeat("44", 32),
                parentAuthorityHash,
                nextAuthorityHash,
                nextAuthorityPayload,
                nextAuthorityPayloadHash,
                substrateTransitionMessageHash,
                0,
                "23",
                "18",
                parentAuthorityKeys,
                parentAuthorityWeights,
                new byte[] {0x06},
                Arrays.asList(bytes(0x77, 64), bytes(0x88, 64))));
    expectThrows(
        () ->
            SourceSccpProofs.canonicalSubstrateAuthoritySetTransitionJustificationBytes(
                1,
                SourceSccpProofs.DOMAIN_SORA_KUSAMA,
                "41",
                "42",
                "9001",
                repeat("44", 32),
                parentAuthorityHash,
                nextAuthorityHash,
                nextAuthorityPayload,
                nextAuthorityPayloadHash,
                substrateTransitionMessageHash,
                1,
                "23",
                "18",
                parentAuthorityKeys,
                parentAuthorityWeights,
                bytes(0xff, 257),
                Arrays.asList(bytes(0x77, 64), bytes(0x88, 64))));
    expectThrows(
        () ->
            SourceSccpProofs.canonicalSubstrateAuthoritySetTransitionJustificationBytes(
                1,
                SourceSccpProofs.DOMAIN_SORA_KUSAMA,
                "41",
                "42",
                "9001",
                repeat("44", 32),
                parentAuthorityHash,
                nextAuthorityHash,
                nextAuthorityPayload,
                nextAuthorityPayloadHash,
                substrateTransitionMessageHash,
                1,
                "22",
                "18",
                parentAuthorityKeys,
                parentAuthorityWeights,
                new byte[] {0x06},
                Arrays.asList(bytes(0x77, 64), bytes(0x88, 64))));
    expectThrows(
        () ->
            SourceSccpProofs.canonicalSubstrateAuthoritySetTransitionJustificationBytes(
                1,
                SourceSccpProofs.DOMAIN_SORA_KUSAMA,
                "41",
                "42",
                "9001",
                repeat("44", 32),
                parentAuthorityHash,
                nextAuthorityHash,
                nextAuthorityPayload,
                nextAuthorityPayloadHash,
                substrateTransitionMessageHash,
                1,
                "23",
                "17",
                parentAuthorityKeys,
                parentAuthorityWeights,
                new byte[] {0x06},
                Arrays.asList(bytes(0x77, 64), bytes(0x88, 64))));
    expectThrows(
        () ->
            SourceSccpProofs.canonicalSubstrateAuthoritySetTransitionJustificationBytes(
                1,
                SourceSccpProofs.DOMAIN_SORA_KUSAMA,
                "41",
                "42",
                "9001",
                repeat("44", 32),
                parentAuthorityHash,
                nextAuthorityHash,
                nextAuthorityPayload,
                nextAuthorityPayloadHash,
                substrateTransitionMessageHash,
                1,
                "23",
                "12",
                parentAuthorityKeys,
                parentAuthorityWeights,
                new byte[] {0x03},
                Arrays.asList(bytes(0x77, 64), bytes(0x88, 64))));
    expectThrows(
        () ->
            SourceSccpProofs.canonicalSubstrateAuthoritySetTransitionJustificationBytes(
                1,
                SourceSccpProofs.DOMAIN_SORA_KUSAMA,
                "41",
                "42",
                "9001",
                repeat("44", 32),
                parentAuthorityHash,
                nextAuthorityHash,
                nextAuthorityPayload,
                nextAuthorityPayloadHash,
                substrateTransitionMessageHash,
                1,
                "23",
                "18",
                parentAuthorityKeys,
                parentAuthorityWeights,
                new byte[] {0x00},
                Collections.<byte[]>emptyList()));
    expectThrows(
        () ->
            SourceSccpProofs.canonicalSubstrateAuthoritySetTransitionJustificationBytes(
                1,
                SourceSccpProofs.DOMAIN_SORA_KUSAMA,
                "41",
                "42",
                "9001",
                repeat("44", 32),
                parentAuthorityHash,
                nextAuthorityHash,
                nextAuthorityPayload,
                nextAuthorityPayloadHash,
                substrateTransitionMessageHash,
                1,
                "23",
                "18",
                parentAuthorityKeys,
                parentAuthorityWeights,
                new byte[] {0x08},
                Collections.<byte[]>emptyList()));
    expectThrows(
        () ->
            SourceSccpProofs.canonicalSubstrateAuthoritySetTransitionJustificationBytes(
                1,
                SourceSccpProofs.DOMAIN_SORA_KUSAMA,
                "41",
                "42",
                "9001",
                repeat("44", 32),
                parentAuthorityHash,
                nextAuthorityHash,
                nextAuthorityPayload,
                nextAuthorityPayloadHash,
                substrateTransitionMessageHash,
                1,
                "23",
                "18",
                parentAuthorityKeys,
                parentAuthorityWeights,
                new byte[] {0x06},
                Arrays.asList(new byte[64], bytes(0x88, 64))));
    expectThrows(
        () ->
            SourceSccpProofs.canonicalSubstrateAuthoritySetTransitionJustificationBytes(
                1,
                SourceSccpProofs.DOMAIN_SORA_KUSAMA,
                "41",
                "42",
                "9001",
                repeat("44", 32),
                parentAuthorityHash,
                nextAuthorityHash,
                nextAuthorityPayload,
                nextAuthorityPayloadHash,
                substrateTransitionMessageHash,
                1,
                "23",
                "18",
                parentAuthorityKeys,
                parentAuthorityWeights,
                new byte[] {0x06},
                Arrays.asList(bytes(0x77, 63), bytes(0x88, 64))));

    assert bytesToHex(SourceSccpProofs.canonicalEvmReceiptRootMptValue(repeat("bb", 32)))
            .equals(evmReceiptRootMptValueHex)
        : "EVM receipt-root MPT value must match Rust verifier";
    expectThrows(() -> SourceSccpProofs.canonicalEvmReceiptRootMptValue("1234"));

    assert bytesToHex(SourceSccpProofs.canonicalTronReceiptRootMptValue(repeat("bb", 32)))
        .equals(
            "f8419f736363703a74726f6e3a726563656970742d726f6f742d76616c75653a7631a0"
                + repeat("bb", 32))
        : "TRON receipt-root MPT value must match Rust verifier";
    expectThrows(() -> SourceSccpProofs.canonicalTronReceiptRootMptValue("1234"));
    final String zeroHash = repeat("00", 32);
    expectThrows(() -> SourceSccpProofs.canonicalTronReceiptRootMptValue(zeroHash));

    assert SourceSccpProofs.canonicalTronReceiptProofBytes(
                sourceEventDigest, repeat("bb", 32), repeat("dd", 32), branch)
            .length
        == 133 : "TRON receipt proof transcript must have expected length";
    expectThrowsMessage(
        () ->
            SourceSccpProofs.canonicalTronReceiptProofBytes(
                sourceEventDigest + " ", repeat("bb", 32), repeat("dd", 32), branch),
        "sourceEventDigest must be canonical hex");
    expectThrows(
        () ->
            SourceSccpProofs.canonicalTronReceiptProofBytes(
                zeroHash, repeat("bb", 32), repeat("dd", 32), branch));
    expectThrows(
        () ->
            SourceSccpProofs.canonicalTronReceiptProofBytes(
                sourceEventDigest, zeroHash, repeat("dd", 32), branch));
    expectThrows(
        () ->
            SourceSccpProofs.canonicalTronReceiptProofBytes(
                sourceEventDigest, repeat("bb", 32), zeroHash, branch));
    expectThrows(
        () ->
            SourceSccpProofs.canonicalTronReceiptProofBytes(
                sourceEventDigest, repeat("bb", 32), repeat("dd", 32), Collections.emptyList()));
    assert !SourceSccpProofs.tronReceiptProofHash(
            sourceEventDigest, repeat("bb", 32), repeat("dd", 32), branch)
        .equals(
            SourceSccpProofs.tronReceiptProofHash(
                sourceEventDigest, repeat("bb", 32), repeat("dd", 32), changedBranch))
        : "TRON receipt proof hash must bind branch";
    final byte[] tronReceiptStateNode = hexBytes("e4822080a0" + repeat("bb", 32));
    final java.util.List<byte[]> tronReceiptStateProofNodes =
        Collections.singletonList(tronReceiptStateNode);
    assert SourceSccpProofs.canonicalTronReceiptStateProofBytes(
                sourceEventDigest,
                repeat("bb", 32),
                "21789ae4e9fb0f13a9d7ef876ccbc90ee2fe1d1eddeec5c35e33e0a09c768079",
                "0",
                tronReceiptStateProofNodes,
                branch)
            .length
        == 186 : "TRON receipt-state proof transcript must have expected length";
    expectThrows(
        () ->
            SourceSccpProofs.canonicalTronReceiptStateProofBytes(
                zeroHash,
                repeat("bb", 32),
                "21789ae4e9fb0f13a9d7ef876ccbc90ee2fe1d1eddeec5c35e33e0a09c768079",
                "0",
                tronReceiptStateProofNodes,
                branch));
    expectThrows(
        () ->
            SourceSccpProofs.canonicalTronReceiptStateProofBytes(
                sourceEventDigest,
                zeroHash,
                "21789ae4e9fb0f13a9d7ef876ccbc90ee2fe1d1eddeec5c35e33e0a09c768079",
                "0",
                tronReceiptStateProofNodes,
                branch));
    expectThrows(
        () ->
            SourceSccpProofs.canonicalTronReceiptStateProofBytes(
                sourceEventDigest,
                repeat("bb", 32),
                zeroHash,
                "0",
                tronReceiptStateProofNodes,
                branch));
    expectThrows(
        () ->
            SourceSccpProofs.canonicalTronReceiptStateProofBytes(
                sourceEventDigest,
                repeat("bb", 32),
                "21789ae4e9fb0f13a9d7ef876ccbc90ee2fe1d1eddeec5c35e33e0a09c768079",
                "0",
                tronReceiptStateProofNodes,
                Collections.emptyList()));
    assert SourceSccpProofs.tronReceiptStateProofHash(
            sourceEventDigest,
            repeat("bb", 32),
            "21789ae4e9fb0f13a9d7ef876ccbc90ee2fe1d1eddeec5c35e33e0a09c768079",
            "0",
            tronReceiptStateProofNodes,
            branch)
        .equals("0x847c5ee3e6f4f83fef4d754a9aed93fae38c6677011cae03b10228c17c60b13b")
        : "TRON receipt-state proof hash must match Rust verifier";
    assert !SourceSccpProofs.tronReceiptStateProofHash(
            sourceEventDigest,
            repeat("bb", 32),
            "21789ae4e9fb0f13a9d7ef876ccbc90ee2fe1d1eddeec5c35e33e0a09c768079",
            "0",
            tronReceiptStateProofNodes,
            branch)
        .equals(
            SourceSccpProofs.tronReceiptStateProofHash(
                sourceEventDigest,
                repeat("bb", 32),
                "21789ae4e9fb0f13a9d7ef876ccbc90ee2fe1d1eddeec5c35e33e0a09c768079",
                "1",
                tronReceiptStateProofNodes,
                branch))
        : "TRON receipt-state proof hash must bind receipt-root index";
    assert bytesToHex(SourceSccpProofs.tronSourceMessageCallData(5, 0, sourceEventDigest))
            .equals("06841e30" + repeat("00", 31) + "05" + repeat("00", 32) + repeat("34", 32))
        : "TRON source-message calldata must match Rust verifier";
    expectThrows(() -> SourceSccpProofs.tronSourceMessageCallData(0, 0, sourceEventDigest));
    expectThrows(() -> SourceSccpProofs.tronSourceMessageCallData(5, 5, sourceEventDigest));
    expectThrows(() -> SourceSccpProofs.tronSourceMessageCallData(5, 0, repeat("00", 32)));
    final byte[] transactionSourceBytes =
        hexBytes(
            "0af3010a02123418b9602208565656565656565640959aef3a5acf01081f12ca"
                + "010a31747970652e676f6f676c65617069732e636f6d2f70726f746f636f6c2e"
                + "54726967676572536d617274436f6e74726163741294010a15417e5f4552091a"
                + "69125d5dfcb7b8c2659029395bdf121541454545454545454545454545454545"
                + "4545454545226406841e30000000000000000000000000000000000000000000"
                + "0000000000000000000005000000000000000000000000000000000000000000"
                + "0000000000000000000000343434343434343434343434343434343434343434"
                + "34343434343434343434347090e5ee3a900180e1eb171241cc58d7ac52c91117"
                + "92495fee682b53cab96ff4229043c5b8b90c31447f5934553d8854ab35de3437"
                + "2c13331bf3ef5cefd8f2cc5ad026faf223da83969fe8973c012a0410001801");
    final java.util.List<byte[]> transactionSourceBranch = Collections.emptyList();
    final String transactionSourceRoot =
        "1751c62dce36d5d642e48480b45d48ed16dd1b9b40ce216bc2f15c1b1ccf300b";
    final java.util.List<byte[]> transactionSourceInclusionBranch =
        Collections.singletonList(bytes(0xaa));
    assert SourceSccpProofs.canonicalTronTransactionSourceProofBytes(
                sourceEventDigest,
                repeat("bb", 32),
                transactionSourceRoot,
                "0",
                "1",
                transactionSourceBytes,
                transactionSourceBranch,
                transactionSourceInclusionBranch)
            .length
        == 476 : "TRON transaction source proof transcript must have expected length";
    assert Arrays.equals(
            SourceSccpProofs.canonicalTronTransactionSourceProofBytes(
                sourceEventDigest,
                repeat("bb", 32),
                transactionSourceRoot,
                "0",
                "1",
                transactionSourceBytes,
                transactionSourceBranch,
                transactionSourceInclusionBranch,
                repeat("45", 20),
                "7e5f4552091a69125d5dfcb7b8c2659029395bdf"),
            SourceSccpProofs.canonicalTronTransactionSourceProofBytes(
                sourceEventDigest,
                repeat("bb", 32),
                transactionSourceRoot,
                "0",
                "1",
                transactionSourceBytes,
                transactionSourceBranch,
                transactionSourceInclusionBranch))
        : "TRON bound transaction source proof transcript bytes must remain unchanged";
    expectThrows(
        () ->
            SourceSccpProofs.canonicalTronTransactionSourceProofBytes(
                sourceEventDigest,
                repeat("bb", 32),
                transactionSourceRoot,
                "0",
                "1",
                transactionSourceBytes,
                transactionSourceBranch,
                transactionSourceInclusionBranch,
                repeat("46", 20),
                "7e5f4552091a69125d5dfcb7b8c2659029395bdf"));
    expectThrows(
        () ->
            SourceSccpProofs.canonicalTronTransactionSourceProofBytes(
                sourceEventDigest,
                repeat("bb", 32),
                transactionSourceRoot,
                "0",
                "1",
                transactionSourceBytes,
                transactionSourceBranch,
                transactionSourceInclusionBranch,
                repeat("45", 20),
                repeat("22", 20)));
    assert SourceSccpProofs.tronTransactionSourceProofHash(
            sourceEventDigest,
            repeat("bb", 32),
            transactionSourceRoot,
            "0",
            "1",
            transactionSourceBytes,
            transactionSourceBranch,
            transactionSourceInclusionBranch)
        .equals("0xfc98a09ae9e7f63ccd383b2f3e104efce0d2c291dc7900ffd49e4f391e6016b6")
        : "TRON transaction source proof hash must match Rust verifier";
    final byte[] omittedDefaultRetTransactionSourceBytes =
        hexBytes(bytesToHex(transactionSourceBytes).replace("2a0410001801", "2a021801"));
    assert SourceSccpProofs.canonicalTronTransactionSourceProofBytes(
                sourceEventDigest,
                repeat("bb", 32),
                "62489e5ad22dd0fc7a4b8444c2b17ef28c2c885a01bd0f97fd7f63fbfb1552bd",
                "0",
                "1",
                omittedDefaultRetTransactionSourceBytes,
                transactionSourceBranch,
                transactionSourceInclusionBranch)
            .length
        == 474 : "TRON transaction source proof must accept omitted default ret";
    assert SourceSccpProofs.tronTransactionSourceProofHash(
            sourceEventDigest,
            repeat("bb", 32),
            "62489e5ad22dd0fc7a4b8444c2b17ef28c2c885a01bd0f97fd7f63fbfb1552bd",
            "0",
            "1",
            omittedDefaultRetTransactionSourceBytes,
            transactionSourceBranch,
            transactionSourceInclusionBranch)
        .equals("0xdb367957f5100b81ef1b074867c5c7c846c8bb3b44353668f65bf1c8ec805a18")
        : "TRON transaction source proof hash must match Rust omitted-ret verifier";
    final byte[] nonCanonicalTransactionSourceBytes =
        Arrays.copyOf(transactionSourceBytes, transactionSourceBytes.length);
    nonCanonicalTransactionSourceBytes[nonCanonicalTransactionSourceBytes.length - 7] = 0x1f;
    expectThrows(
        () ->
            SourceSccpProofs.canonicalTronTransactionSourceProofBytes(
                sourceEventDigest,
                repeat("bb", 32),
                transactionSourceRoot,
                "0",
                "1",
                nonCanonicalTransactionSourceBytes,
                transactionSourceBranch,
                transactionSourceInclusionBranch));
    final byte[] wrongSignerTransactionSourceBytes =
        replaceFirst(
            transactionSourceBytes,
            hexBytes(
                "cc58d7ac52c9111792495fee682b53cab96ff4229043c5b8b90c31447f5934553d8854ab35de34372c13331bf3ef5cefd8f2cc5ad026faf223da83969fe8973c01"),
            hexBytes(
                "b50455577deef2a0d6c3c521d97de050d5b9ba46df00c8ddad014bac4ca3345173223f1d4c5940538f1b1da069bed6828a9b27794bd1eac1a35810baaef28d2101"));
    expectThrows(
        () ->
            SourceSccpProofs.canonicalTronTransactionSourceProofBytes(
                sourceEventDigest,
                repeat("bb", 32),
                transactionSourceRoot,
                "0",
                "1",
                wrongSignerTransactionSourceBytes,
                transactionSourceBranch,
                transactionSourceInclusionBranch));
    expectThrows(
        () ->
            SourceSccpProofs.canonicalTronTransactionSourceProofBytes(
                sourceEventDigest,
                repeat("bb", 32),
                transactionSourceRoot,
                "1",
                "1",
                transactionSourceBytes,
                transactionSourceBranch,
                transactionSourceInclusionBranch));
    expectThrows(
        () ->
            SourceSccpProofs.canonicalTronTransactionSourceProofBytes(
                sourceEventDigest,
                repeat("bb", 32),
                repeat("cc", 32),
                "0",
                "1",
                transactionSourceBytes,
                transactionSourceBranch,
                transactionSourceInclusionBranch));
    expectThrows(
        () ->
            SourceSccpProofs.canonicalTronTransactionSourceProofBytes(
                sourceEventDigest,
                repeat("bb", 32),
                "e4a77765ae41dc30b8bf3f7d9847170e0646e3dd0189433d2e3c88296221c942",
                "1",
                "3",
                hexBytes("123456"),
                Arrays.asList(bytes(0x11), bytes(0x22)),
                transactionSourceInclusionBranch));
    expectThrows(
        () ->
            SourceSccpProofs.canonicalTronTransactionSourceProofBytes(
                sourceEventDigest,
                repeat("bb", 32),
                transactionSourceRoot,
                "0",
                "1",
                transactionSourceBytes,
                transactionSourceBranch,
                Collections.emptyList()));

    final String tronParentRawHeaderHex =
        "08b8b096ffbc311220"
            + repeat("cc", 32)
            + "1a20"
            + repeat("bb", 32)
            + "38b8604a1541"
            + repeat("11", 20)
            + "50015a20"
            + repeat("aa", 32);
    final String tronRawHeaderHex =
        "08b9b096ffbc311220"
            + repeat("dd", 32)
            + "1a200000000000003038701e5a1cd89912e6118f8aa18222c8b90867fedcca84c4d438b9604a1541"
            + repeat("11", 20)
            + "50015a20"
            + repeat("ee", 32);
    final String tronParentRawHeaderHash =
        "0x5647d462e78851c6701e5a1cd89912e6118f8aa18222c8b90867fedcca84c4d4";
    final String tronRawHeaderHash =
        "0x614a09275b6d0fffb6bc08fb34f737c093d9dd2adefccb04344715e2619c8286";
    final String tronParentBlockId =
        "0x0000000000003038701e5a1cd89912e6118f8aa18222c8b90867fedcca84c4d4";
    final String tronBlockId =
        "0x0000000000003039b6bc08fb34f737c093d9dd2adefccb04344715e2619c8286";
    final byte[] parentRawHeader =
        SourceSccpProofs.canonicalTronRawBlockHeaderBytes(
            "12344", repeat("cc", 32), repeat("aa", 32), repeat("bb", 32), "41" + repeat("11", 20), 1, "1700000012344");
    final byte[] rawHeader =
        SourceSccpProofs.canonicalTronRawBlockHeaderBytes(
            "12345", repeat("dd", 32), repeat("ee", 32), tronParentBlockId, "41" + repeat("11", 20), 1, "1700000012345");
    assert bytesToHex(parentRawHeader).equals(tronParentRawHeaderHex)
        : "TRON parent raw header must be canonical";
    assert bytesToHex(rawHeader).equals(tronRawHeaderHex) : "TRON raw header must be canonical";
    assert SourceSccpProofs.tronRawBlockHeaderHash(parentRawHeader).equals(tronParentRawHeaderHash)
        : "TRON parent raw header hash must match Rust verifier";
    assert SourceSccpProofs.tronRawBlockHeaderHash(rawHeader).equals(tronRawHeaderHash)
        : "TRON raw header hash must match Rust verifier";
    assert SourceSccpProofs.tronBlockIdFromRawDataHash("12344", tronParentRawHeaderHash)
            .equals(tronParentBlockId)
        : "TRON parent block ID must splice height into raw-data hash";
    assert SourceSccpProofs.tronBlockIdFromRawDataHash("12345", tronRawHeaderHash)
            .equals(tronBlockId)
        : "TRON block ID must splice height into raw-data hash";
    expectThrowsMessage(
        () ->
            SourceSccpProofs.canonicalTronRawBlockHeaderBytes(
                "12345",
                " " + repeat("dd", 32),
                repeat("ee", 32),
                tronParentBlockId,
                "41" + repeat("11", 20),
                1,
                "1700000012345"),
        "txTrieRoot must be canonical hex");
    for (final String nonCanonicalNumber :
        Arrays.asList("012345", "0x3039", "+12345", " 12345")) {
      expectThrowsMessage(
          () -> SourceSccpProofs.tronBlockIdFromRawDataHash(nonCanonicalNumber, tronRawHeaderHash),
          "number must be an unsigned integer");
    }
    expectThrows(
        () ->
            SourceSccpProofs.canonicalTronRawBlockHeaderBytes(
                "12346",
                repeat("dd", 32),
                repeat("ee", 32),
                tronBlockId,
                "41" + repeat("00", 20),
                1,
                "1700000012346"));
    assert SourceSccpProofs.canonicalTronSolidBlockHeaderProofBytes(
                rawHeader,
                tronHeaderSignature(0),
                parentRawHeader,
                tronHeaderSignature(27),
                tronRawHeaderHash,
                tronParentRawHeaderHash,
                tronBlockId,
                repeat("dd", 32),
                repeat("ee", 32),
                tronParentBlockId,
                "41" + repeat("11", 20),
                "1700000012345",
                1)
            .length
        == 650 : "TRON solid-block header proof transcript must have expected length";
    expectThrows(
        () ->
            SourceSccpProofs.canonicalTronSolidBlockHeaderProofBytes(
                rawHeader,
                tronHeaderSignature(0),
                parentRawHeader,
                tronHeaderSignature(27),
                repeat("aa", 32),
                tronParentRawHeaderHash,
                tronBlockId,
                repeat("dd", 32),
                repeat("ee", 32),
                tronParentBlockId,
                "41" + repeat("11", 20),
                "1700000012345",
                1));
    final byte[] overlongKeyRawHeader =
        concat(new byte[] {(byte) 0x88, 0x00}, Arrays.copyOfRange(rawHeader, 1, rawHeader.length));
    final String overlongKeyRawHeaderHash =
        SourceSccpProofs.tronRawBlockHeaderHash(overlongKeyRawHeader);
    expectThrows(
        () ->
            SourceSccpProofs.canonicalTronSolidBlockHeaderProofBytes(
                overlongKeyRawHeader,
                tronHeaderSignature(0),
                parentRawHeader,
                tronHeaderSignature(27),
                overlongKeyRawHeaderHash,
                tronParentRawHeaderHash,
                SourceSccpProofs.tronBlockIdFromRawDataHash("12345", overlongKeyRawHeaderHash),
                repeat("dd", 32),
                repeat("ee", 32),
                tronParentBlockId,
                "41" + repeat("11", 20),
                "1700000012345",
                1));
    expectThrows(
        () ->
            SourceSccpProofs.canonicalTronSolidBlockHeaderProofBytes(
                rawHeader,
                tronHeaderSignature(0),
                parentRawHeader,
                tronHeaderSignature(27),
                tronRawHeaderHash,
                tronParentRawHeaderHash,
                tronBlockId,
                repeat("dd", 32),
                repeat("ee", 32),
                tronParentBlockId,
                "41" + repeat("00", 20),
                "1700000012345",
                1));
    assert SourceSccpProofs.tronSolidBlockHeaderProofHash(
            rawHeader,
            tronHeaderSignature(0),
            parentRawHeader,
            tronHeaderSignature(27),
            tronRawHeaderHash,
            tronParentRawHeaderHash,
            tronBlockId,
            repeat("dd", 32),
            repeat("ee", 32),
            tronParentBlockId,
            "41" + repeat("11", 20),
            "1700000012345",
            1)
        .equals("0x25416bda5734ecef1ab9920d15f1011e962f6ff90e9c6247ff6b2ce34a5ab49f")
        : "TRON solid-block header proof hash must match Rust verifier";

    assert SourceSccpProofs.canonicalSubstrateStorageProofBytes(
                SourceSccpProofs.DOMAIN_SORA_KUSAMA,
                sourceEventDigest,
                "0",
                "31",
                "32",
                repeat("aa", 32),
                repeat("cc", 32),
                repeat("bb", 32),
                branch)
            .length
        == 225 : "Substrate storage proof transcript must have expected length";
    expectThrowsMessage(
        () ->
            SourceSccpProofs.canonicalSubstrateStorageProofBytes(
                SourceSccpProofs.DOMAIN_SORA_KUSAMA,
                zeroHash,
                "0",
                "31",
                "32",
                repeat("aa", 32),
                repeat("cc", 32),
                repeat("bb", 32),
                branch),
        "sourceEventDigest must not be zero");
    final byte[] substrateRuntimeStatement =
        SourceSccpProofs.canonicalSubstrateRuntimeStorageVerificationStatementBytes(
            SourceSccpProofs.DOMAIN_SORA_KUSAMA,
            sourceEventDigest,
            "0",
            "31",
            "32",
            repeat("aa", 32),
            repeat("cc", 32),
            repeat("bb", 32),
            branch,
            null);
    assert Arrays.equals(
            substrateRuntimeStatement,
            SourceSccpProofs.canonicalSubstrateStorageProofBytes(
                SourceSccpProofs.DOMAIN_SORA_KUSAMA,
                sourceEventDigest,
                "0",
                "31",
                "32",
                repeat("aa", 32),
                repeat("cc", 32),
                repeat("bb", 32),
                branch))
        : "Substrate runtime-storage statement must reuse canonical storage transcript";
    final String runtimeStoragePublicInputsHash =
        SourceSccpProofs.substrateRuntimeStorageProofPublicInputsHash(
            SourceSccpProofs.DOMAIN_SORA_KUSAMA,
            sourceEventDigest,
            "0",
            "31",
            "32",
            repeat("aa", 32),
            repeat("cc", 32),
            repeat("bb", 32),
            branch,
            null);
    assert runtimeStoragePublicInputsHash.matches("0x[0-9a-f]{64}")
        : "Substrate runtime-storage public inputs hash must be hex32";
    final java.util.List<java.util.List<String>> runtimeStorageColumns =
        SourceSccpProofs.substrateRuntimeStoragePublicInputColumns(
            SourceSccpProofs.DOMAIN_SORA_KUSAMA,
            sourceEventDigest,
            "0",
            "31",
            "32",
            repeat("aa", 32),
            repeat("cc", 32),
            repeat("bb", 32),
            branch,
            null);
    assert runtimeStorageColumns.size() == 11
        : "Substrate runtime-storage public input columns must match Rust";
    assert runtimeStorageColumns.get(8).get(0)
        .equals("0x26aa394eea5630e07c48ae0c9558cef780d41e5e16056765bc8461851072c9d7")
        : "Substrate runtime-storage columns must expose the System.Events storage key";
    assert runtimeStorageColumns.get(10).get(0).equals(runtimeStoragePublicInputsHash)
        : "Substrate runtime-storage columns must expose public inputs hash";
    final SourceSccpProofs.SubstrateRuntimeStorageProofRequest runtimeStorageRequest =
        SourceSccpProofs.buildSubstrateRuntimeStorageProofRequest(
            SourceSccpProofs.DOMAIN_SORA_KUSAMA,
            sourceEventDigest,
            "0",
            "31",
            "32",
            repeat("aa", 32),
            repeat("cc", 32),
            repeat("bb", 32),
            repeat("aa", 32),
            repeat("bb", 32),
            repeat("cc", 32),
            repeat("dd", 32),
            repeat("12", 32),
            branch,
            null);
    assert runtimeStorageRequest.circuitId
        .equals(SourceSccpProofs.SUBSTRATE_RUNTIME_STORAGE_OPEN_VERIFY_CIRCUIT_ID_V1)
        : "Substrate runtime-storage request must bind the circuit id";
    assert runtimeStorageRequest.runtimeStorageProofPublicInputsHash.equals(runtimeStoragePublicInputsHash)
        : "Substrate runtime-storage request must expose public inputs hash";
    assert runtimeStorageRequest.fastpqPublicInputs.slot.equals("31")
        : "Substrate runtime-storage request must expose finalized block slot";
    assert runtimeStorageRequest.fastpqTransitions.get(0).key
        .equals("sccp:substrate:runtime-storage:v1:context")
        : "Substrate runtime-storage transitions must be sorted";
    final byte[] originalStatementBytes = runtimeStorageRequest.statementBytes();
    final byte[] exposedStatementBytes = runtimeStorageRequest.statementBytes();
    exposedStatementBytes[0] = 0;
    assert Arrays.equals(originalStatementBytes, runtimeStorageRequest.statementBytes())
        : "Substrate runtime-storage request must defensively copy statement bytes";
    final byte[] originalSchemaDescriptor = runtimeStorageRequest.schemaDescriptor();
    final byte[] exposedSchemaDescriptor = runtimeStorageRequest.schemaDescriptor();
    exposedSchemaDescriptor[0] = 0;
    assert Arrays.equals(originalSchemaDescriptor, runtimeStorageRequest.schemaDescriptor())
        : "Substrate runtime-storage request must defensively copy schema bytes";
    boolean columnsImmutable = false;
    try {
      runtimeStorageRequest.publicInputColumns.clear();
    } catch (final UnsupportedOperationException exception) {
      columnsImmutable = true;
    }
    assert columnsImmutable : "Substrate runtime-storage public input columns must be immutable";
    boolean transitionsImmutable = false;
    try {
      runtimeStorageRequest.fastpqTransitions.clear();
    } catch (final UnsupportedOperationException exception) {
      transitionsImmutable = true;
    }
    assert transitionsImmutable : "Substrate runtime-storage transitions must be immutable";
    boolean badStorageHashThrew = false;
    try {
      SourceSccpProofs.buildSubstrateRuntimeStorageProofRequest(
          SourceSccpProofs.DOMAIN_SORA_KUSAMA,
          sourceEventDigest,
          "0",
          "31",
          "32",
          repeat("aa", 32),
          repeat("cc", 32),
          repeat("bb", 32),
          repeat("aa", 32),
          repeat("bb", 32),
          repeat("cc", 32),
          repeat("dd", 32),
          repeat("12", 32),
          branch,
          repeat("aa", 32));
    } catch (final IllegalArgumentException exception) {
      badStorageHashThrew = exception.getMessage().contains("storageProofHash");
    }
    assert badStorageHashThrew : "Substrate runtime-storage request must reject mismatched storage hash";
    boolean templateVerifierHashThrew = false;
    try {
      SourceSccpProofs.buildSubstrateRuntimeStorageProofRequest(
          SourceSccpProofs.DOMAIN_SORA_KUSAMA,
          sourceEventDigest,
          "0",
          "31",
          "32",
          repeat("aa", 32),
          repeat("cc", 32),
          repeat("bb", 32),
          repeat("aa", 32),
          repeat("bb", 32),
          repeat("cc", 32),
          repeat("dd", 32),
          "af2d28b3e07447239f28e90ce4fdee7e6cd3778c087eaeda7170781eb4b76b9c",
          branch,
          null);
    } catch (final IllegalArgumentException exception) {
      templateVerifierHashThrew = exception.getMessage().contains("template verifier hash");
    }
    assert templateVerifierHashThrew
        : "Substrate runtime-storage request must reject the template verifier hash";
    assert !SourceSccpProofs.substrateStorageProofHash(
            SourceSccpProofs.DOMAIN_SORA_KUSAMA,
            sourceEventDigest,
            "0",
            "31",
            "32",
            repeat("aa", 32),
            repeat("cc", 32),
            repeat("bb", 32),
            branch)
        .equals(
            SourceSccpProofs.substrateStorageProofHash(
                SourceSccpProofs.DOMAIN_SORA_KUSAMA,
                sourceEventDigest,
                "0",
                "31",
                "32",
                repeat("aa", 32),
                repeat("cc", 32),
                repeat("bb", 32),
                changedBranch)) : "Substrate storage proof hash must bind branch";
    assert !SourceSccpProofs.substrateStorageProofHash(
            SourceSccpProofs.DOMAIN_SORA_KUSAMA,
            sourceEventDigest,
            "0",
            "31",
            "32",
            repeat("aa", 32),
            repeat("cc", 32),
            repeat("bb", 32),
            branch)
        .equals(
            SourceSccpProofs.substrateStorageProofHash(
                SourceSccpProofs.DOMAIN_SORA_KUSAMA,
                sourceEventDigest,
                "1",
                "31",
                "32",
                repeat("aa", 32),
                repeat("cc", 32),
                repeat("bb", 32),
                branch)) : "Substrate storage proof hash must bind leaf index";
  }

  private static void rejectsMalformedSourceProofWitnessMaterial() {
    expectThrows(
        () ->
            SourceSccpProofs.canonicalTronReceiptProofBytes(
                repeat("34", 32),
                repeat("bb", 32),
                repeat("dd", 32),
                Collections.singletonList(new byte[] {1, 2, 3})));
    expectThrows(
        () ->
            SourceSccpProofs.canonicalTronReceiptStateProofBytes(
                repeat("34", 32),
                repeat("bb", 32),
                "21789ae4e9fb0f13a9d7ef876ccbc90ee2fe1d1eddeec5c35e33e0a09c768079",
                "0",
                Collections.<byte[]>emptyList(),
                Collections.singletonList(bytes(0xee))));
    expectThrows(
        () ->
            SourceSccpProofs.canonicalTronRawBlockHeaderBytes(
                "0",
                repeat("bb", 32),
                repeat("aa", 32),
                repeat("cc", 32),
                "41" + repeat("11", 20),
                1,
                "1700000012345"));
    expectThrows(
        () ->
            SourceSccpProofs.canonicalTronSolidBlockHeaderProofBytes(
                new byte[] {1},
                new byte[64],
                new byte[] {2},
                new byte[65],
                repeat("aa", 32),
                repeat("bb", 32),
                repeat("cc", 32),
                repeat("dd", 32),
                repeat("aa", 32),
                repeat("ee", 32),
                "41" + repeat("11", 20),
                "1700000012345",
                1));
    expectThrows(
        () ->
            SourceSccpProofs.canonicalTronSolidBlockHeaderProofBytes(
                bytes(0xaa, 16 * 1024 + 1),
                tronHeaderSignature(0),
                new byte[] {2},
                tronHeaderSignature(27),
                repeat("aa", 32),
                repeat("bb", 32),
                repeat("cc", 32),
                repeat("dd", 32),
                repeat("aa", 32),
                repeat("ee", 32),
                "41" + repeat("11", 20),
                "1700000012345",
                1));
    expectThrows(
        () ->
            SourceSccpProofs.canonicalTronSolidBlockHeaderProofBytes(
                new byte[] {1},
                bytes(0xaa, 65),
                new byte[] {2},
                tronHeaderSignature(27),
                repeat("aa", 32),
                repeat("bb", 32),
                repeat("cc", 32),
                repeat("dd", 32),
                repeat("aa", 32),
                repeat("ee", 32),
                "41" + repeat("11", 20),
                "1700000012345",
                1));
    expectThrows(
        () ->
            SourceSccpProofs.canonicalTronSolidBlockHeaderProofBytes(
                new byte[] {1},
                tronHeaderSignature(0),
                new byte[] {2},
                tronHeaderSignature(4),
                repeat("aa", 32),
                repeat("bb", 32),
                repeat("cc", 32),
                repeat("dd", 32),
                repeat("aa", 32),
                repeat("ee", 32),
                "41" + repeat("11", 20),
                "1700000012345",
                1));
    final byte[] zeroRSignature = tronHeaderSignature(0);
    Arrays.fill(zeroRSignature, 0, 32, (byte) 0);
    expectThrows(
        () ->
            SourceSccpProofs.canonicalTronSolidBlockHeaderProofBytes(
                new byte[] {1},
                zeroRSignature,
                new byte[] {2},
                tronHeaderSignature(27),
                repeat("aa", 32),
                repeat("bb", 32),
                repeat("cc", 32),
                repeat("dd", 32),
                repeat("aa", 32),
                repeat("ee", 32),
                "41" + repeat("11", 20),
                "1700000012345",
                1));
    expectThrows(
        () ->
            SourceSccpProofs.canonicalSubstrateStorageProofBytes(
                -1,
                repeat("34", 32),
                "0",
                "31",
                "32",
                repeat("aa", 32),
                repeat("cc", 32),
                repeat("bb", 32),
                Collections.<byte[]>emptyList()));
  }

  private static void derivesEthBeaconExecutionPayloadSszRootsFromWitnessMaterial() {
    final byte[] headerRlp = sampleEthExecutionHeaderRlp();
    final String executionPayloadRoot =
        SourceSccpProofs.ethExecutionPayloadHeaderRootFromRlp(headerRlp);
    final java.util.List<byte[]> executionPayloadBranch =
        Arrays.asList(bytes(0xee), bytes(0xff), bytes(0x11), bytes(0x22));
    final String beaconBodyRoot =
        SourceSccpProofs.ethBeaconBodyRootFromExecutionPayloadBranch(
            executionPayloadRoot, executionPayloadBranch);
    final String beaconHeaderRoot =
        SourceSccpProofs.ethBeaconBlockHeaderRoot(
            "320", "17", repeat("aa", 32), repeat("bb", 32), beaconBodyRoot);

    assert executionPayloadRoot.equals(
            "0xc029dda492d2e41ad72bd83f1727a67e5331f413ec29d5c31de955d0bea24624")
        : "ETH execution-payload header root must match Rust verifier";
    assert beaconBodyRoot.equals(
            "0x431e6bef5e759e8fdf32d8e8ed1ff761933ddb4de24ec9ae8e2aa0d25fe861ba")
        : "ETH beacon body root must match Rust verifier";
    assert beaconHeaderRoot.equals(
            "0xd54b406debae26e6ebaef512cc4f9e6bc12cf02af0d4476895383b37f682a179")
        : "ETH beacon block header root must match Rust verifier";
    assert !SourceSccpProofs.ethBeaconBodyRootFromExecutionPayloadBranch(
            executionPayloadRoot, Arrays.asList(bytes(0xff), bytes(0xff), bytes(0x11), bytes(0x22)))
        .equals(beaconBodyRoot) : "ETH beacon body root must bind execution payload branch";
    expectThrows(
        () ->
            SourceSccpProofs.ethBeaconBodyRootFromExecutionPayloadBranch(
                executionPayloadRoot, Collections.singletonList(bytes(0xee))));
    expectThrows(
        () -> SourceSccpProofs.ethExecutionPayloadHeaderRootFromRlp(new byte[] {(byte) 0x80}));
  }

  private static void expectThrows(final Runnable action) {
    try {
      action.run();
      throw new AssertionError("expected IllegalArgumentException");
    } catch (final IllegalArgumentException expected) {
      // expected
    }
  }

  private static void expectThrowsMessage(final Runnable action, final String messagePart) {
    try {
      action.run();
      throw new AssertionError("expected IllegalArgumentException");
    } catch (final IllegalArgumentException expected) {
      assert expected.getMessage() != null && expected.getMessage().contains(messagePart)
          : "expected message to contain " + messagePart + ", got " + expected.getMessage();
    }
  }

  private static byte[] bytes(final int value) {
    return bytes(value, 32);
  }

  private static byte[] minimalBeLengthBytes(final int value) {
    int working = value;
    int length = 0;
    do {
      length++;
      working >>>= 8;
    } while (working != 0);
    final byte[] out = new byte[length];
    working = value;
    for (int index = length - 1; index >= 0; index--) {
      out[index] = (byte) (working & 0xff);
      working >>>= 8;
    }
    return out;
  }

  private static byte[] concat(final byte[]... parts) {
    int size = 0;
    for (final byte[] part : parts) {
      size += part.length;
    }
    final byte[] out = new byte[size];
    int offset = 0;
    for (final byte[] part : parts) {
      System.arraycopy(part, 0, out, offset, part.length);
      offset += part.length;
    }
    return out;
  }

  private static byte[] rlpString(final byte[] value) {
    if (value.length == 1 && (value[0] & 0xff) < 0x80) {
      return value;
    }
    if (value.length < 56) {
      return concat(new byte[] {(byte) (0x80 + value.length)}, value);
    }
    final byte[] lengthBytes = minimalBeLengthBytes(value.length);
    return concat(new byte[] {(byte) (0xb7 + lengthBytes.length)}, lengthBytes, value);
  }

  private static byte[] rlpList(final byte[]... fields) {
    final byte[] payload = concat(fields);
    if (payload.length < 56) {
      return concat(new byte[] {(byte) (0xc0 + payload.length)}, payload);
    }
    final byte[] lengthBytes = minimalBeLengthBytes(payload.length);
    return concat(new byte[] {(byte) (0xf7 + lengthBytes.length)}, lengthBytes, payload);
  }

  private static byte[] sampleBscParliaExtra() {
    return concat(
        bytes(0x11, 32),
        new byte[] {2},
        bytes(0x11, 20),
        bytes(0x01, 48),
        bytes(0x22, 20),
        bytes(0x02, 48),
        bytes(0x99, 65));
  }

  private static byte[] sampleBscParliaHeaderRlp(final byte[] extraData) {
    return rlpList(
        rlpString(bytes(0x10, 32)),
        rlpString(bytes(0x11, 32)),
        rlpString(bytes(0x12, 20)),
        rlpString(bytes(0x13, 32)),
        rlpString(bytes(0x14, 32)),
        rlpString(bytes(0x15, 32)),
        rlpString(bytes(0x00, 256)),
        rlpString(new byte[] {2}),
        rlpString(new byte[] {1}),
        rlpString(new byte[] {1}),
        rlpString(new byte[] {1}),
        rlpString(new byte[] {1}),
        rlpString(extraData),
        rlpString(bytes(0x00, 32)),
        rlpString(bytes(0x00, 8)));
  }

  private static SourceSccpProofs.BscValidatorSetMetadataProof bscMetadataProofLike(
      final SourceSccpProofs.BscValidatorSetMetadataProof proof,
      final byte[] validatorContractAddress,
      final java.util.List<byte[]> accountProofNodes,
      final java.util.List<byte[]> validatorSetLengthProofNodes,
      final String validatorSetLengthValueHash,
      final java.util.List<SourceSccpProofs.BscValidatorStorageProof> validatorStorageProofs) {
    return new SourceSccpProofs.BscValidatorSetMetadataProof(
        proof.version,
        proof.stateRoot,
        proof.nextValidatorSetPayloadHash,
        validatorContractAddress == null ? proof.validatorContractAddress : validatorContractAddress,
        accountProofNodes == null ? proof.accountProofNodes : accountProofNodes,
        proof.storageRoot,
        proof.validatorSetLengthSlot,
        proof.validatorSetLengthValue,
        validatorSetLengthValueHash == null
            ? proof.validatorSetLengthValueHash
            : validatorSetLengthValueHash,
        validatorSetLengthProofNodes == null
            ? proof.validatorSetLengthProofNodes
            : validatorSetLengthProofNodes,
        validatorStorageProofs == null ? proof.validatorStorageProofs : validatorStorageProofs);
  }

  private static byte[] sampleEthExecutionHeaderRlp() {
    return sampleEthExecutionHeaderRlp(bytes(0x15));
  }

  private static byte[] sampleSourceVerifierMaterialBytes(final int domain) {
    return SourceSccpProofs.canonicalSourceVerifierMaterialBytes(
        domain,
        "0x" + repeat("44", 32),
        "0x" + repeat("55", 32),
        "0x" + repeat("66", 32),
        "0x" + repeat("88", 32),
        sourceStateVerifierHash(domain),
        bridgeAddress(domain),
        sourceBridgeCodeHash(domain),
        networkId(domain),
        ownerAddress(domain),
        configHash(domain));
  }

  private static String sampleSourceVerifierMaterialHash(final int domain) {
    return SourceSccpProofs.sourceVerifierMaterialHash(
        domain,
        "0x" + repeat("44", 32),
        "0x" + repeat("55", 32),
        "0x" + repeat("66", 32),
        "0x" + repeat("88", 32),
        sourceStateVerifierHash(domain),
        bridgeAddress(domain),
        sourceBridgeCodeHash(domain),
        networkId(domain),
        ownerAddress(domain),
        configHash(domain));
  }

  private static String sampleSourceAdapterDeploymentHash(
      final int domain, final String adapterVerifierVkHash) {
    return sampleSourceAdapterDeploymentHash(domain, adapterVerifierVkHash, null, null, null);
  }

  private static String sampleSourceAdapterDeploymentHash(
      final int domain,
      final String adapterVerifierVkHash,
      final String solanaTowerReplayVerifierHash,
      final String solanaFullAccountsdbLatticeVerifierHash,
      final String solanaBankForkChoiceVerifierHash) {
    return sampleSourceAdapterDeploymentHash(
        domain,
        adapterVerifierVkHash,
        solanaTowerReplayVerifierHash,
        solanaFullAccountsdbLatticeVerifierHash,
        solanaBankForkChoiceVerifierHash,
        null,
        null,
        null);
  }

  private static String sampleSourceAdapterDeploymentHash(
      final int domain,
      final String adapterVerifierVkHash,
      final String solanaTowerReplayVerifierHash,
      final String solanaFullAccountsdbLatticeVerifierHash,
      final String solanaBankForkChoiceVerifierHash,
      final String tonMasterchainConfigVerifierHash,
      final String tonValidatorSetTransitionVerifierHash,
      final String tonShardAccountsDictionaryVerifierHash) {
    return SourceSccpProofs.sourceAdapterEngineDeploymentHash(
        domain,
        "0x" + repeat("44", 32),
        "0x" + repeat("55", 32),
        "0x" + repeat("66", 32),
        "0x" + repeat("88", 32),
        "0x" + repeat("aa", 32),
        SourceSccpProofs.DOMAIN_SORA,
        adapterVerifierVkHash,
        sourceStateVerifierHash(domain),
        bridgeAddress(domain),
        sourceBridgeCodeHash(domain),
        networkId(domain),
        ownerAddress(domain),
        configHash(domain),
        solanaTowerReplayVerifierHash,
        solanaFullAccountsdbLatticeVerifierHash,
        solanaBankForkChoiceVerifierHash,
        tonMasterchainConfigVerifierHash,
        tonValidatorSetTransitionVerifierHash,
        tonShardAccountsDictionaryVerifierHash);
  }

  private static String sampleSolanaFullLightClientGateHash(
      final String towerReplayHash,
      final String fullAccountsdbLatticeHash,
      final String bankForkChoiceHash) {
    return sampleSolanaFullLightClientGateHash(
        towerReplayHash,
        fullAccountsdbLatticeHash,
        bankForkChoiceHash,
        sourceStateVerifierHash(SourceSccpProofs.DOMAIN_SOL));
  }

  private static String sampleSolanaFullLightClientGateHash(
      final String towerReplayHash,
      final String fullAccountsdbLatticeHash,
      final String bankForkChoiceHash,
      final String sourceStateVerifierHash) {
    return SourceSccpProofs.solanaFullLightClientGateHash(
        SourceSccpProofs.DOMAIN_SOL,
        "0x" + repeat("44", 32),
        "0x" + repeat("55", 32),
        "0x" + repeat("66", 32),
        "0x" + repeat("88", 32),
        "0x" + repeat("aa", 32),
        towerReplayHash,
        fullAccountsdbLatticeHash,
        bankForkChoiceHash,
        SourceSccpProofs.DOMAIN_SORA,
        null,
        sourceStateVerifierHash,
        null,
        null,
        null,
        null,
        null);
  }

  private static String sampleTonFullLightClientGateHash(
      final String masterchainConfigHash,
      final String validatorSetTransitionHash,
      final String shardAccountsDictionaryHash) {
    return SourceSccpProofs.tonFullLightClientGateHash(
        SourceSccpProofs.DOMAIN_TON,
        "0x" + repeat("44", 32),
        "0x" + repeat("55", 32),
        "0x" + repeat("66", 32),
        "0x" + repeat("88", 32),
        "0x" + repeat("aa", 32),
        masterchainConfigHash,
        validatorSetTransitionHash,
        shardAccountsDictionaryHash,
        SourceSccpProofs.DOMAIN_SORA,
        null,
        sourceStateVerifierHash(SourceSccpProofs.DOMAIN_TON),
        null,
        null,
        null,
        null,
        null);
  }

  private static String sourceStateVerifierHash(final int domain) {
    if (domain == SourceSccpProofs.DOMAIN_SOL
        || domain == SourceSccpProofs.DOMAIN_TON
        || domain == SourceSccpProofs.DOMAIN_SORA_KUSAMA
        || domain == SourceSccpProofs.DOMAIN_SORA_POLKADOT
        || domain == SourceSccpProofs.DOMAIN_SORA2) {
      return "0x" + repeat("77", 32);
    }
    return null;
  }

  private static String bridgeAddress(final int domain) {
    if (domain == SourceSccpProofs.DOMAIN_ETH
        || domain == SourceSccpProofs.DOMAIN_BSC
        || domain == SourceSccpProofs.DOMAIN_TRON) {
      return "0x" + repeat("11", 20);
    }
    return null;
  }

  private static String sourceBridgeCodeHash(final int domain) {
    return bridgeAddress(domain) == null ? null : "0x" + repeat("77", 32);
  }

  private static String networkId(final int domain) {
    return domain == SourceSccpProofs.DOMAIN_TRON ? "0x" + repeat("33", 32) : null;
  }

  private static String ownerAddress(final int domain) {
    return domain == SourceSccpProofs.DOMAIN_TRON ? "0x" + repeat("22", 20) : null;
  }

  private static String configHash(final int domain) {
    if (domain == SourceSccpProofs.DOMAIN_TRON) {
      return "0xe986dd67bfa2307b4e00cf46bde41a88003a55c5b7fea311fa106614b2252f9d";
    }
    return null;
  }

  private static byte[] sampleEthExecutionHeaderRlp(final byte[] receiptsRoot) {
    return rlpList(
        rlpString(bytes(0x10, 32)),
        rlpString(bytes(0x11, 32)),
        rlpString(bytes(0x12, 20)),
        rlpString(bytes(0x13, 32)),
        rlpString(bytes(0x14, 32)),
        rlpString(receiptsRoot),
        rlpString(bytes(0x00, 256)),
        rlpString(new byte[0]),
        rlpString(new byte[] {0x2a}),
        rlpString(new byte[] {0x01, (byte) 0xc9, (byte) 0xc3, (byte) 0x80}),
        rlpString(new byte[] {0x52, 0x08}),
        rlpString(new byte[] {0x65, 0x53, (byte) 0xf1, 0x00}),
        rlpString("iroha-sccp-test".getBytes(StandardCharsets.UTF_8)),
        rlpString(bytes(0x16, 32)),
        rlpString(bytes(0x00, 8)),
        rlpString(new byte[] {0x3b, (byte) 0x9a, (byte) 0xca, 0x00}),
        rlpString(bytes(0x17, 32)),
        rlpString(new byte[0]),
        rlpString(new byte[0]),
        rlpString(bytes(0x18, 32)));
  }

  private static byte[] bytes(final int value, final int length) {
    final byte[] out = new byte[length];
    for (int index = 0; index < out.length; index++) {
      out[index] = (byte) value;
    }
    return out;
  }

  private static byte[] tronHeaderSignature(final int recoveryId) {
    final byte[] signature = bytes(0xaa, 65);
    for (int index = 32; index < 64; index++) {
      signature[index] = 0x01;
    }
    signature[64] = (byte) recoveryId;
    return signature;
  }

  private static String repeat(final String text, final int count) {
    final StringBuilder builder = new StringBuilder(text.length() * count);
    for (int index = 0; index < count; index++) {
      builder.append(text);
    }
    return builder.toString();
  }

  private static String bytesToHex(final byte[] bytes) {
    final StringBuilder builder = new StringBuilder(bytes.length * 2);
    for (final byte value : bytes) {
      builder.append(String.format("%02x", value & 0xff));
    }
    return builder.toString();
  }

  private static byte[] hexBytes(final String value) {
    if (value.length() % 2 != 0) {
      throw new IllegalArgumentException("hex length must be even");
    }
    final byte[] out = new byte[value.length() / 2];
    for (int index = 0; index < out.length; index++) {
      final int hi = Character.digit(value.charAt(index * 2), 16);
      final int lo = Character.digit(value.charAt(index * 2 + 1), 16);
      if (hi < 0 || lo < 0) {
        throw new IllegalArgumentException("hex must be canonical");
      }
      out[index] = (byte) ((hi << 4) | lo);
    }
    return out;
  }

  private static byte[] replaceFirst(
      final byte[] input, final byte[] needle, final byte[] replacement) {
    if (needle.length != replacement.length) {
      throw new IllegalArgumentException("replacement must keep length");
    }
    for (int offset = 0; offset <= input.length - needle.length; offset++) {
      boolean matches = true;
      for (int index = 0; index < needle.length; index++) {
        if (input[offset + index] != needle[index]) {
          matches = false;
          break;
        }
      }
      if (matches) {
        final byte[] out = Arrays.copyOf(input, input.length);
        System.arraycopy(replacement, 0, out, offset, replacement.length);
        return out;
      }
    }
    throw new IllegalArgumentException("needle not found");
  }
}
