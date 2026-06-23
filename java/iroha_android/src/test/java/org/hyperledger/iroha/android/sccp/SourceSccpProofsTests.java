package org.hyperledger.iroha.android.sccp;

import java.nio.charset.StandardCharsets;
import java.util.Arrays;
import java.util.Collections;
import java.util.List;

public final class SourceSccpProofsTests {
  private SourceSccpProofsTests() {}

  public static void main(final String[] args) {
    derivesSourceAdapterVerifierVkHashesForUiTooling();
    derivesEvmAndTronDestinationBindingsForUiTooling();
    derivesSourceMaterialAndDeploymentRecordHashesForUiTooling();
    derivesEthBeaconExecutionPayloadSszRootsFromWitnessMaterial();
    derivesEthereumReceiptRootAndSyncCommitteeGuardsForUiTooling();
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

    boolean threw = false;
    try {
      SourceSccpProofs.sourceAdapterVerifierVkHash(
          SourceSccpProofs.DOMAIN_TON, SourceSccpProofs.DOMAIN_TON);
    } catch (final IllegalArgumentException exception) {
      threw = exception.getMessage().contains("targetDomain must be SORA");
    }
    assert threw : "source-adapter VK helper must reject non-SORA targets";
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
      SourceSccpProofs.DOMAIN_TRON
    };
    final String[] materialHashes = {
      "0x4d1e9d15bc59c0a2157aa967eb033f5778c805aea4707785a31ef6b60f694d77",
      "0x1630e4d75e2676cc443e07b0477303240ae4cff13bdf9fe61725b4a9a4ee959a",
      "0x499a7363142d5fcfe3a79b11a29ae2ad897e853649e80e39a162b8942f908331",
      "0x08b11177113ac2d9f612abdf767a017de560d805e965b3dc32e28c8748ea2ebc",
      "0x68c20262e44676bd5f3c4ec428f063373147a1ca14c5885648a9c651b3bcd8d8"
    };
    final String[] deploymentHashes = {
      "0xfeb62925410b1376a2cd3704c3822e335da96c3dcc283b041a559d7b08ab1cc4",
      "0x7d47ade779a5bddb3a5f283600af677db8605b75a00516a4328f3823ff28fb2d",
      "0xcdb2a81cb31e58d9bc1f4292d33c3f4990b2d2008dda1b9b1275aaac087461cc",
      "0x5c4e226c1f4619311762a9c889f8e3b99ea6f020317c2e8a0c76a08d7a70f887",
      "0x94dbe28a2fb16e043b83639b6dea8ec62f53679599ef1dd220fd13c71c7bdcb8"
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

    expectThrowsMessage(
        () ->
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
                "0x871a910500648c68576f7d8fb044de1c494ae24c74f435c87dd451e6ae169c6b"),
        "sourceBridgeNetworkId");
    expectThrowsMessage(
        () ->
            SourceSccpProofs.canonicalSourceVerifierMaterialBytes(
                SourceSccpProofs.DOMAIN_ETH,
                "0x" + repeat("44", 32),
                "0x" + repeat("55", 32),
                "0x" + repeat("66", 32),
                "0x" + repeat("88", 32),
                null,
                "0x" + repeat("11", 20),
                SourceSccpProofs.ETH_MAINNET_NETWORK_ID,
                SourceSccpProofs.ETH_MAINNET_NETWORK_ID,
                null,
                "0x871a910500648c68576f7d8fb044de1c494ae24c74f435c87dd451e6ae169c6b"),
        "sourceBridgeEmitterCodeHash");
    expectThrowsMessage(
        () ->
            SourceSccpProofs.canonicalSourceVerifierMaterialBytes(
                SourceSccpProofs.DOMAIN_ETH,
                "0x" + repeat("44", 32),
                "0x" + repeat("55", 32),
                "0x" + repeat("66", 32),
                "0x" + repeat("88", 32),
                null,
                "0x" + repeat("11", 20),
                "0x" + repeat("77", 32),
                SourceSccpProofs.ETH_MAINNET_NETWORK_ID,
                "0x" + repeat("22", 20),
                "0x871a910500648c68576f7d8fb044de1c494ae24c74f435c87dd451e6ae169c6b"),
        "sourceBridgeOwnerAddress");
    expectThrowsMessage(
        () ->
            SourceSccpProofs.canonicalSourceVerifierMaterialBytes(
                SourceSccpProofs.DOMAIN_ETH,
                "0x" + repeat("44", 32),
                "0x" + repeat("55", 32),
                "0x" + repeat("66", 32),
                "0x" + repeat("88", 32),
                null,
                "0x" + repeat("11", 20),
                "0x" + repeat("77", 32),
                SourceSccpProofs.ETH_MAINNET_NETWORK_ID,
                null,
                "0x" + repeat("99", 32)),
        "sourceBridgeConfigHash");
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
          SourceSccpProofs.ETH_MAINNET_NETWORK_ID,
          null,
          "0x871a910500648c68576f7d8fb044de1c494ae24c74f435c87dd451e6ae169c6b");
    } catch (final IllegalArgumentException exception) {
      reusedSourceMaterialRoleThrew = exception.getMessage().contains("role-separated");
    }
    assert reusedSourceMaterialRoleThrew
        : "source material helper must reject reused role hashes";

    boolean reusedEthNetworkIdRoleThrew = false;
    final String ethNetworkIdRoleReplay = SourceSccpProofs.ETH_MAINNET_NETWORK_ID;
    try {
      SourceSccpProofs.canonicalSourceVerifierMaterialBytes(
          SourceSccpProofs.DOMAIN_ETH,
          ethNetworkIdRoleReplay,
          "0x" + repeat("55", 32),
          "0x" + repeat("66", 32),
          "0x" + repeat("88", 32),
          null,
          "0x" + repeat("11", 20),
          "0x" + repeat("77", 32),
          SourceSccpProofs.ETH_MAINNET_NETWORK_ID,
          null,
          "0x871a910500648c68576f7d8fb044de1c494ae24c74f435c87dd451e6ae169c6b");
    } catch (final IllegalArgumentException exception) {
      reusedEthNetworkIdRoleThrew =
          exception.getMessage().contains("sourceBridgeNetworkId");
    }
    assert reusedEthNetworkIdRoleThrew
        : "source material helper must reject source role replaying ETH network id";

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
          SourceSccpProofs.ETH_MAINNET_NETWORK_ID,
          null,
          "0x871a910500648c68576f7d8fb044de1c494ae24c74f435c87dd451e6ae169c6b");
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
        .equals("0xe23b2c175909e222c1ebe371661bda8c0687cf8d7e7acf2b62957a51c420be02")
        : "Solana full light-client gate hash must match Rust";
    assert !sampleSolanaFullLightClientGateHash(
            "0x" + repeat("bb", 32),
            "0x" + repeat("cc", 32),
            "0x" + repeat("dd", 32),
            sourceStateVerifierHash(SourceSccpProofs.DOMAIN_SOL),
            "0x" + repeat("ab", 32))
        .equals(
            sampleSolanaFullLightClientGateHash(
                "0x" + repeat("bb", 32), "0x" + repeat("cc", 32), "0x" + repeat("dd", 32)))
        : "Solana full light-client gate hash must bind the deployment receipt hash";

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
        .equals("0x5047e655523aa7ce8db0cc4dfb8f9551b7912c262e0b65177620c494c57faa48")
        : "TON full light-client gate hash must match Rust";
    assert !sampleTonFullLightClientGateHash(
            "0x" + repeat("bb", 32),
            "0x" + repeat("cc", 32),
            "0x" + repeat("dd", 32),
            "0x" + repeat("ab", 32))
        .equals(
            sampleTonFullLightClientGateHash(
                "0x" + repeat("bb", 32), "0x" + repeat("cc", 32), "0x" + repeat("dd", 32)))
        : "TON full light-client gate hash must bind the deployment receipt hash";

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

  private static void derivesEthereumReceiptRootAndSyncCommitteeGuardsForUiTooling() {
    final String zeroHash = repeat("00", 32);
    expectThrows(() -> SourceSccpProofs.canonicalEvmReceiptRootMptValue(zeroHash));

    final byte[] nextSyncPayload =
        SourceSccpProofs.canonicalEthSyncCommitteePayloadBytes(
            syncCommitteeBytes(0x11, 48), syncCommitteeWeights(), syncCommitteeBytes(0xcc, 96));
    assert nextSyncPayload.length == 81925
        : "Ethereum sync-committee payload must include the complete 512-authority roster";
    assert syncCommitteeSignersBitmap(342).length == 64
        : "Ethereum sync-committee bitmap must cover the complete 512-authority roster";
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
    return sampleSolanaFullLightClientGateHash(
        towerReplayHash,
        fullAccountsdbLatticeHash,
        bankForkChoiceHash,
        sourceStateVerifierHash,
        "0x" + repeat("aa", 32));
  }

  private static String sampleSolanaFullLightClientGateHash(
      final String towerReplayHash,
      final String fullAccountsdbLatticeHash,
      final String bankForkChoiceHash,
      final String sourceStateVerifierHash,
      final String deploymentReceiptHash) {
    return SourceSccpProofs.solanaFullLightClientGateHash(
        SourceSccpProofs.DOMAIN_SOL,
        "0x" + repeat("44", 32),
        "0x" + repeat("55", 32),
        "0x" + repeat("66", 32),
        "0x" + repeat("88", 32),
        deploymentReceiptHash,
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
    return sampleTonFullLightClientGateHash(
        masterchainConfigHash,
        validatorSetTransitionHash,
        shardAccountsDictionaryHash,
        "0x" + repeat("aa", 32));
  }

  private static String sampleTonFullLightClientGateHash(
      final String masterchainConfigHash,
      final String validatorSetTransitionHash,
      final String shardAccountsDictionaryHash,
      final String deploymentReceiptHash) {
    return SourceSccpProofs.tonFullLightClientGateHash(
        SourceSccpProofs.DOMAIN_TON,
        "0x" + repeat("44", 32),
        "0x" + repeat("55", 32),
        "0x" + repeat("66", 32),
        "0x" + repeat("88", 32),
        deploymentReceiptHash,
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
        || domain == SourceSccpProofs.DOMAIN_TON) {
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
    if (domain == SourceSccpProofs.DOMAIN_ETH) {
      return SourceSccpProofs.ETH_MAINNET_NETWORK_ID;
    }
    return domain == SourceSccpProofs.DOMAIN_TRON ? "0x" + repeat("33", 32) : null;
  }

  private static String ownerAddress(final int domain) {
    return domain == SourceSccpProofs.DOMAIN_TRON ? "0x" + repeat("22", 20) : null;
  }

  private static String configHash(final int domain) {
    if (domain == SourceSccpProofs.DOMAIN_ETH) {
      return "0x871a910500648c68576f7d8fb044de1c494ae24c74f435c87dd451e6ae169c6b";
    }
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

  private static List<byte[]> syncCommitteeBytes(final int value, final int length) {
    final java.util.ArrayList<byte[]> out = new java.util.ArrayList<>(512);
    for (int index = 0; index < 512; index++) {
      final byte[] bytes = bytes(value, length);
      bytes[length - 2] = (byte) ((index >>> 8) & 0xff);
      bytes[length - 1] = (byte) (index & 0xff);
      out.add(bytes);
    }
    return out;
  }

  private static List<String> syncCommitteeWeights() {
    return Collections.nCopies(512, "1");
  }

  private static List<byte[]> prepend(final List<byte[]> values, final byte[] first) {
    final java.util.ArrayList<byte[]> out = new java.util.ArrayList<>(values);
    out.set(0, first);
    return out;
  }

  private static byte[] syncCommitteeSignersBitmap(final int count) {
    final byte[] bitmap = new byte[64];
    for (int index = 0; index < count; index++) {
      bitmap[index / 8] = (byte) (bitmap[index / 8] | (1 << (index % 8)));
    }
    return bitmap;
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
