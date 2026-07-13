export const VALIDATION_FEE_POLICY_HASH_HEX =
  "658d86df8c744ccb8e21ea9212ce8fc6678fdc66dacbdf6ddb69573f6d1ee8b1";

export const VALIDATION_FEE_POLICY_CANONICAL_BYTES_HEX =
  "4e5254300000d69d739b639ab6866991ad67e4fc1a5a001b01000000000000dfff22c665d0ff4b020201000c0b626f692d746573746e657420070707070707070707070707070707070707070707070707070707070707070708010000000000000001001d1c375a6570734a544843564c4b737246464e5a475352475a677642687601020b05010000000104010000006160736f726175efbe9b314eefbe8cefbe806a61efbdbcefbe924148efbe8befbda6efbe8cefbdb2efbdb6efbe89317a50546377efbe9befbe94efbe98efbe885a35efbda64b68efbe88efbdb15067efbdb3efbda668efbdb6efbdb4384a4a5357340400000000080a000000000000000a010864000000000000001d1c76616c69646174696f6e2d6665652d676f7665726e616e63652d7631190100000000000000100f54524541535552595f5041594f5554";

export const VALIDATION_FEE_POLICY_SIGNATURE_PAYLOAD_HEX =
  "1312d32f7c8a32bc0829eb125c1081404616adf287476320a7bb71d7d7c204b5";

export function validationFeePolicyFixture(overrides = {}) {
  const policy = {
    schema_version: 1,
    network_id: "boi-testnet",
    genesis_hash: "07".repeat(32),
    policy_version: 1,
    previous_policy_hash: null,
    ds_asset_id: "7ZepsJTHCVLKsrFFNZGSRGZgvBhv",
    ds_scale: 2,
    fee: "0.1",
    treasury_account_id:
      "sorauﾛ1NﾌﾀjaｼﾒAHﾋｦﾌｲｶﾉ1zPTcwﾛﾔﾘﾈZ5ｦKhﾈｱPgｳｦhｶｴ8JJSW4",
    charging_mode: "PER_QUALIFYING_TRANSFER_INSTRUCTION",
    effective_from_height: 10,
    expires_after_height: 100,
    governance_keyset_id: "validation-fee-governance-v1",
    exemption_classes: ["TREASURY_PAYOUT"],
    ...(overrides.policy ?? {}),
  };
  const governanceKeyset = {
    keyset_id: "validation-fee-governance-v1",
    threshold: 2,
    public_keys_hex: [
      "8a88e3dd7409f195fd52db2d3cba5d72ca6709bf1d94121bf3748801b40f6f5c",
      "8139770ea87d175f56a35466c34c7ecccb8d8a91b4ee37a25df60f5b8fc9b394",
    ],
    ...(overrides.governanceKeyset ?? {}),
  };
  const signatures = [
    {
      signer_public_key:
        "8a88e3dd7409f195fd52db2d3cba5d72ca6709bf1d94121bf3748801b40f6f5c",
      signature:
        "ccc0ecf9d657729d469a4398826b91d8346b557bb62734b2d103164b19ddaddd3749d6186392b5dc572ad8d9cb23b12480291a64cca4a0607606c965443d9502",
    },
    {
      signer_public_key:
        "8139770ea87d175f56a35466c34c7ecccb8d8a91b4ee37a25df60f5b8fc9b394",
      signature:
        "585941e93f19764a02eb726bdd7a7385cbd481a5e7b656ead153a77a6e8cc247eda66b83ec012debb83050ff542794609124066780edb707b0f9b2efd82e5409",
    },
  ];
  const policyRegistry = {
    active_policy_hash: VALIDATION_FEE_POLICY_HASH_HEX,
    active_policy_version: 1,
    registered_policies: [
      {
        policy_version: 1,
        policy_hash: VALIDATION_FEE_POLICY_HASH_HEX,
        previous_policy_hash: null,
      },
    ],
    ...(overrides.policyRegistry ?? {}),
  };
  return {
    policy,
    signedPolicy: {
      policy,
      signatures: overrides.signatures ?? signatures,
    },
    governanceKeyset,
    policyRegistry,
    verificationContext: {
      networkId: "boi-testnet",
      genesisHash: "07".repeat(32),
      currentHeight: 10,
      governanceKeyset,
      policyRegistry,
      ...(overrides.verificationContext ?? {}),
    },
  };
}
