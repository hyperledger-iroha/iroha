#!/usr/bin/env python3
"""Guard the typed enabled-metric method tables in iroha_core telemetry."""

from __future__ import annotations

import hashlib
import json
import re
import unittest
from dataclasses import dataclass
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SOURCE_PATH = ROOT / "crates/iroha_core/src/telemetry.rs"
MACRO_PROVIDER_PATH = ROOT / "crates/iroha_core/src/telemetry/enabled_metric_macros.rs"
MACRO_PROVIDER_INCLUDE = 'include!("telemetry/enabled_metric_macros.rs");'
MACRO_PROVIDER_BINDING = MACRO_PROVIDER_INCLUDE + "\nimpl StateTelemetry {"
EXPECTED_MACRO_PROVIDER_SHA256 = (
    "93251b200edb32cb57dd634c75f45403e01b86a55a2a072041da370fd6369bb9"
)
PREIMAGE_BLOB = "cd5cf22c6f9b13c1857182cf6a3526356564cb07"
PREIMAGE_SHA256 = "ab2a41f254cdff6794f9160168e301de29ad89a7a945720df15fb2319ef43ee8"
PREIMAGE_LINES = 14_258
SELECTED_PREIMAGE_LINES = 1_687
MAX_GOVERNED_LINES = 13_307
MINIMUM_NET_REDUCTION = 900
EXPECTED_FORWARD_ROWS = 64
EXPECTED_ROWS = (('StateTelemetry',
  'inc_storage_budget_exceeded',
  'is_enabled_early_return',
  'explicit',
  '26d596292244cf01f3a1beb79778e3d8e73e0b8ab2f59aff10934b0860269d05'),
 ('StateTelemetry',
  'inc_storage_da_cache',
  'is_enabled_early_return',
  'explicit',
  'c483128ac70fa6fbeb2e322cb90cacf3d594a3e77d3b210274ae975f2b5dab06'),
 ('StateTelemetry',
  'record_isi_total',
  'is_enabled_early_return',
  'explicit',
  '51c3cc53885fb6ae9be940f758eaf95fe6ef18af03f3ca9831900953cf9b6f5f'),
 ('StateTelemetry',
  'record_isi_success',
  'is_enabled_early_return',
  'explicit',
  'e174f3dc461e8b3f5734db67445a71d820861728053bab5c0a87f0229ac1dd9b'),
 ('StateTelemetry',
  'note_sm_syscall_success',
  'is_enabled_if',
  'explicit',
  'a10982bf4a66c25e34809daa8330c3bda09ed49a02401e64b8cd5ed2d20dfcbe'),
 ('StateTelemetry',
  'note_sm_syscall_failure',
  'is_enabled_if',
  'explicit',
  '10caa71dd6a78db04fa80e53a1c2d97c88eead4a41dbb8e61413aa6652b0462f'),
 ('StateTelemetry',
  'record_subscription_billing_attempt',
  'is_enabled_if',
  'explicit',
  'f927e6582137476e0450154da4c0a3311277365268e1b049c209556110bf1e83'),
 ('StateTelemetry',
  'record_subscription_billing_outcome',
  'is_enabled_if',
  'explicit',
  '1851018ba52b4ea0d2f1c6a267fadb0ba43d7c362f659deb42824e9fefa21159'),
 ('StateTelemetry',
  'note_settlement_success',
  'is_enabled_if',
  'explicit',
  'c26761667aa21c3437b1605c79c5f992eca33c1859a8fdc88b770fcb9aa8c819'),
 ('StateTelemetry',
  'note_settlement_failure',
  'is_enabled_if',
  'explicit',
  'fff65f0327c6e3722aeb39459e84de1b58fad6c9ade3555d23c192f51f402ba9'),
 ('StateTelemetry',
  'observe_tx_amount',
  'is_enabled_if',
  'explicit',
  'eee7d604f625b8ee86526ce6bb345a1b569eb865924cafaf5c9b1b738e5ae30a'),
 ('StateTelemetry',
  'inc_runtime_upgrade_event',
  'is_enabled_if',
  'explicit',
  '5e4ea0889f4be2b792b771e0e846d6c7bcad9d6821adc23f80b3438bcf47bc0b'),
 ('StateTelemetry',
  'record_governance_bond_event',
  'is_enabled_early_return',
  'explicit',
  '5239a5296561e996927748c45cf15ffd37da651777c0a26616b72833f931bceb'),
 ('StateTelemetry',
  'record_citizens_total',
  'is_enabled_early_return',
  'explicit',
  'b71a0cefde221aca82b928f3ffb6d5de24673add4d51e0734bbbee8f70eac2c3'),
 ('StateTelemetry',
  'record_protected_namespace_enforcement',
  'is_enabled_if',
  'explicit',
  '5d70648f38ef35d87e773f5e9eab84b9463f13c902f51c64c6949c8bb91f6e90'),
 ('StateTelemetry',
  'record_manifest_quorum_enforcement',
  'is_enabled_if',
  'explicit',
  '21f75ce3dfaf8b7871f9682396815376d70f5128a55013c92e7b1d3658982b21'),
 ('StateTelemetry',
  'record_manifest_admission',
  'is_enabled_if',
  'explicit',
  '9576b8f5b7e97e162789fd28d6b8159994b170ab38066198c051ecbbf99a85c9'),
 ('StateTelemetry',
  'record_manifest_hook_enforcement',
  'is_enabled_if',
  'explicit',
  '7f426525c1f6ab90f3c53bcae3e515eb097418fa6a9ea9d7950b83b40d658f8a'),
 ('StateTelemetry',
  'set_block_gas_used',
  'is_enabled_if',
  'explicit',
  '421f796c22274328ec98b6e746fa866786c637c9d2867f6f1d5d16890202aecb'),
 ('StreamingTelemetry',
  'inc_storage_budget_exceeded',
  'is_enabled_early_return',
  'explicit',
  '43137d3febdaf469099b7d68bc0228d40a28edafd27c08c2bd8cd1e7dfb5ebe9'),
 ('StreamingTelemetry',
  'inc_quic_datagrams_sent',
  'is_enabled_if',
  'explicit',
  '93108887d73cbdd27b20101b654873a1381d88d6b2109a402a93425552bc0997'),
 ('StreamingTelemetry',
  'inc_quic_datagrams_dropped',
  'is_enabled_if',
  'explicit',
  '2fcfa9fef1dd5a3aeca5048ddd7f0593ee90a4e306f8de5290ca266782c8e623'),
 ('StreamingTelemetry',
  'inc_feedback_timeout',
  'is_enabled_if',
  'explicit',
  'adee6bd6fe85afe8afa71b971a50cde68dcc6e0e1279c475f70a82c62d2b1601'),
 ('StreamingTelemetry',
  'inc_soranet_provision_failure',
  'is_enabled_if',
  'explicit',
  '00ba28b874685c2e5ac35600b1cea81d53eed1e6d570a73e5015916bee390595'),
 ('StreamingTelemetry',
  'inc_soranet_provision_queue_drop',
  'is_enabled_if',
  'explicit',
  '4d02eb34ea506c436698aee1fe5625c4e0bb0f9d2e60d42fd6c1e372ba5a08f3'),
 ('StreamingTelemetry',
  'inc_privacy_redaction_failure',
  'is_enabled_if',
  'explicit',
  '35c53e1105174b688fc98a67089dcffe984eef6818d28080b5172d2ac985649a'),
 ('Telemetry',
  'record_musubi_cursor_failure',
  'atomic_if',
  'explicit',
  'e198de9c34888137581c4591d96a8a64f31fe0bec66363ce956bfd97606e796c'),
 ('Telemetry',
  'inc_new_view_publish',
  'atomic_if',
  'explicit',
  '54eba41d335c300d1fd1c05dd739c8b981874b75a53dba9f3d258510783682db'),
 ('Telemetry',
  'inc_new_view_recv',
  'atomic_if',
  'explicit',
  '0b0577d9ae0c9b424c187bb029b2a34fe2add1041752375618dfb4a8d79d14eb'),
 ('Telemetry',
  'inc_new_view_dropped_by_lock',
  'atomic_if',
  'explicit',
  'bbd13b2bb62a3584f862f9697ed1d204e1ce21165adc6bd100a104261a354b07'),
 ('Telemetry',
  'inc_commit_conflict_detected',
  'atomic_if',
  'explicit',
  '52e0531a3275d3f948b9cf97ca9479837749cfb919310bdbaae1725fb93be9e0'),
 ('Telemetry',
  'inc_blocksync_qc_quarantine',
  'atomic_early_return',
  'explicit',
  '959431bf3b2d444205d39c702a167a7ea407de093dee576d09ae90c45e3561ed'),
 ('Telemetry',
  'inc_blocksync_qc_revalidated',
  'atomic_early_return',
  'explicit',
  '8503782fc590936074e3f2ead5476a55cef1629d39c119c4e12a811c679e3d31'),
 ('Telemetry',
  'inc_blocksync_qc_final_drop',
  'atomic_early_return',
  'explicit',
  '83cae0e1874112caafa3f50d9fe2ae5584d560ab9e4ff4eb3a8e66d66cb9ffa7'),
 ('Telemetry',
  'inc_qc_deferred_missing_payload',
  'atomic_early_return',
  'explicit',
  '15a032a98de9564a1ed4838e977d29d43ed0e5f2b4167acc16d0b684287217cc'),
 ('Telemetry',
  'inc_qc_deferred_resolved',
  'atomic_early_return',
  'explicit',
  'f20e98659c6bc634ff9ffa6f1e24cc2e9e14bec4f80e49ec447f525e9e32a49f'),
 ('Telemetry',
  'inc_qc_deferred_expired',
  'atomic_early_return',
  'explicit',
  'ef2fa979ba6c6a6e62a7a74625ee0b85947ea411c9537410f653ea7c41061c61'),
 ('Telemetry',
  'inc_consensus_empty_commit_topology_defer',
  'atomic_early_return',
  'explicit',
  '7b895fb7972dcdcaf6668c4b35e9fc68b2348f1c0b67757c5d3edeaa265422df'),
 ('Telemetry',
  'inc_consensus_empty_commit_topology_escalation',
  'atomic_early_return',
  'explicit',
  'c6a8ea238f545ff2aac8d29411d0b6d383631c54a4b72fc9a90d2fa03e0645c4'),
 ('Telemetry',
  'inc_consensus_recovery_state_transition',
  'atomic_early_return',
  'explicit',
  '48af53ef07672ef5e22d707dbd0fb48e7c5bfd451b4a2100640b9dfdcc6fdfc9'),
 ('Telemetry',
  'inc_consensus_missing_block_height_escalation',
  'atomic_early_return',
  'explicit',
  'd3f9be0be2ba2800000664bf9675a3303d93e8fbb310da72197a58510904c4fb'),
 ('Telemetry',
  'inc_consensus_sidecar_quarantine',
  'atomic_early_return',
  'explicit',
  'a68c7801cffc9bc1ced8fa2e3a2dbd002e1ece47686671748b24649d1c6f8afd'),
 ('Telemetry',
  'inc_consensus_sidecar_final_drop',
  'atomic_early_return',
  'explicit',
  'b24b7d55f24ce70f354dffae388b1c02cd02dc71c0f0001b552c66fafdc1fc61'),
 ('Telemetry',
  'inc_blocksync_range_pull_escalation',
  'atomic_early_return',
  'explicit',
  '7416362fef18ed675b578b1d03dc83545f5a18c9bca4a71ecfd524d996469379'),
 ('Telemetry',
  'inc_blocksync_range_pull_success',
  'atomic_early_return',
  'explicit',
  '76394859775ed75e18d425722edc04a24867ccac51f977d4e885da4343d1e99b'),
 ('Telemetry',
  'inc_blocksync_range_pull_failure',
  'atomic_early_return',
  'explicit',
  '6a6d49543640f31a5f82f79bb4f3369aaf5caa8856bccfaf6de972cfe7702540'),
 ('Telemetry',
  'observe_consensus_recovery_stuck_round',
  'atomic_early_return',
  'explicit',
  'a4ba9905ce6acd7beb674ea7a7271de8ec219d52f94bd644f9a3689968f25e53'),
 ('Telemetry',
  'note_da_manifest_guard',
  'atomic_early_return',
  'explicit',
  '7d5eabec8bfc7542413e3de49135c9b4b7fa277157596c6eea4c9d01ddaf7b89'),
 ('Telemetry',
  'note_da_manifest_cache',
  'atomic_early_return',
  'explicit',
  '61524807f93897c3a3d64c28c037bdcd122d923d8bbc9c4e564567d92f0b11af'),
 ('Telemetry',
  'note_da_spool_cache',
  'atomic_early_return',
  'explicit',
  'ff188759aed92d0dbb94f832501abe679c2c540ee1878c78124a9f26f54cfe9b'),
 ('Telemetry',
  'note_da_pin_intent_spool',
  'atomic_early_return',
  'explicit',
  '73a74a2a5ceaf0e6093be082eeea3309a6fd1d96337d27643209d27b55805b34'),
 ('Telemetry',
  'note_qc_validation_error',
  'atomic_early_return',
  'explicit',
  'e28fab50e56cf5b995223df0754eb2e0b8587800bc14fb55f8ddddbaeb1fcc19'),
 ('Telemetry',
  'note_block_sync_unsolicited_share_blocks_drop',
  'atomic_early_return',
  'explicit',
  '355c3cf89b55e6890b441f18ac916c2f32825a02393aa40ddac6ce7f8adda56b'),
 ('Telemetry',
  'inc_invalid_signature',
  'atomic_early_return',
  'explicit',
  'ce465386d7091a57c00b28696e2a995f4103ec60e292f50e5f4688d192a20f95'),
 ('Telemetry',
  'note_axt_policy_snapshot_cache_event',
  'atomic_if',
  'explicit',
  '9353f181bc14fe4d3423f92446ebd31cae348f029a338feae21116814c1e9275'),
 ('Telemetry',
  'note_axt_proof_cache_event',
  'atomic_if',
  'explicit',
  'b8cbbee419413ceed8190ed40a5c667c22498390dbcee534861ba1ba3c589b40'),
 ('Telemetry',
  'set_missing_block_retry_window_ms',
  'atomic_if',
  'explicit',
  '32cf42f43dd99c4b68bf8ef79765bfa93eddb9235bfde1d319e888c1aaaabaa6'),
 ('Telemetry',
  'inc_wa_qc_assembled',
  'atomic_if',
  'explicit',
  'b906a8d726ef51e58a504971a0c833a27fba137a117439ecbf138d6f29840f1a'),
 ('Telemetry',
  'set_highest_qc_height',
  'atomic_if',
  'explicit',
  'e5cd6e9209f916ecf36df721a0c5077a49b65407ac7b173fba8ed803d46be3ec'),
 ('Telemetry',
  'set_leader_index',
  'atomic_if',
  'explicit',
  'afa6cbaf48a3a0547d9ae25e49bcc65f6c4ea99977eae6e886d19ed568640a57'),
 ('Telemetry',
  'set_locked_qc_height',
  'atomic_if',
  'explicit',
  '1fe921fa482fda37c50cfc7f98c7aa91523aeca344f102611e2beb9894b0ffac'),
 ('Telemetry',
  'set_locked_qc_view',
  'atomic_if',
  'explicit',
  'c911ec9cf53ad5ffa6a9e2b3ed696745030ce17b0e2d193fe1b185629f995584'),
 ('Telemetry',
  'inc_torii_pre_auth_reject',
  'atomic_if',
  'explicit',
  'aefd0a0b983543855f47a5edbdd88b90e19c00946623fab3d34fdf342a33e837'),
 ('Telemetry',
  'inc_torii_operator_auth',
  'atomic_if',
  'forward',
  '384b96e0075085f9f709e74fc84aa29b16391ffa75cf439c019d1764b0d1b77a'),
 ('Telemetry',
  'inc_torii_operator_auth_lockout',
  'atomic_if',
  'forward',
  'f28b3de20e9267e28ab0fa328c9828302bc967c2cd535a01c454a2b06343b0bc'),
 ('Telemetry',
  'inc_torii_nts_unhealthy_reject',
  'atomic_if',
  'explicit',
  '94964bea9f2955ddc6e58e77711f97ef29163cfc7ddae948d8003e95d5bd0825'),
 ('Telemetry',
  'inc_torii_multisig_direct_sign_reject',
  'atomic_if',
  'explicit',
  'ba0e1b63b6f22748e2a4726acf7750002c3ef250183fc2d2fb153d7e7c7ac08f'),
 ('Telemetry',
  'inc_torii_sorafs_admission',
  'atomic_if',
  'explicit',
  'b93fec9131b5cdc2f618135bd9ee39010e3056ef2e914cc8c9018b57f4139497'),
 ('Telemetry',
  'inc_torii_address_invalid',
  'atomic_if',
  'forward',
  'a29dce2746376c9b11a4714b7ef3242f65326fac71a026fa84f006cdfc9993cd'),
 ('Telemetry',
  'inc_torii_account_literal',
  'atomic_if',
  'forward',
  'a2e8723d1a2c51e96516e9789c74fb95cdeae8c3f41a4c92fe2d44955409f45a'),
 ('Telemetry',
  'inc_torii_norito_rpc_gate',
  'atomic_if',
  'forward',
  'ba1c9f1177378abf255cd89e63d2c1880ebb4d00102face420a264b0f11b2ce5'),
 ('Telemetry',
  'record_da_rent_quote',
  'atomic_if',
  'forward',
  'f70dd2f78ce86f75c8d876d29633a1a8004b6cd00be1192c34c7a250acfbe377'),
 ('Telemetry',
  'observe_da_chunking_seconds',
  'atomic_if',
  'forward',
  '537bd86930367032ac21a014559eb52f8d454bea50ee28b0b0802394ceea4972'),
 ('Telemetry',
  'record_torii_da_spool_batch',
  'atomic_if',
  'forward',
  'e8cd0a60492a6e512b8e8f303a43c83236ccb78b3db76eef1ad71cb81e0d7042'),
 ('Telemetry',
  'record_torii_da_spool_artifact',
  'atomic_if',
  'forward',
  '94c78f83b008e7198ad4036b0298d53ff360e3059a3f030822ecc3bb700f060d'),
 ('Telemetry',
  'set_torii_da_spool_queue_depth',
  'atomic_if',
  'forward',
  'e0a0c5ab2f2857805ec49ffa56ffd5e45ceb9b5291a9a44e2f2788d49bf47120'),
 ('Telemetry',
  'record_da_receipt_outcome',
  'atomic_if',
  'forward',
  '35cfd26d3cea52604639d3b51fbf0951c28a1a02c0bac6ce93bed6eebdcea33c'),
 ('Telemetry',
  'set_da_receipt_cursor',
  'atomic_if',
  'forward',
  'ace163c21b592571d783ea539cd2b51faa212f45a1a1fc72a58fcaa5c2d9a040'),
 ('Telemetry',
  'prune_da_receipt_lanes',
  'atomic_if',
  'forward',
  'e36aab970ead4765357c4bdf7e3edef59f49395a92d72b9652e9c894b0540549'),
 ('Telemetry',
  'record_da_shard_cursor_event',
  'atomic_if',
  'forward',
  '096112eeded1bfa56863490b9d5d9718a5c42f4fbdb20a0b2445c9d7b978117f'),
 ('Telemetry',
  'record_da_shard_cursor_lag',
  'atomic_if',
  'explicit',
  '132e6e22a998099ac8dfc5e4daed74d568ebe7a7e54e3c1a591afad4f7d8f009'),
 ('Telemetry',
  'record_sorafs_fee_projection',
  'atomic_if',
  'forward',
  'f515123557941527244aac3eddc3be3b3661cb26d0436c8cdb0813eed0998a81'),
 ('Telemetry',
  'record_sorafs_egress_reconciliation',
  'atomic_if',
  'forward',
  '52a89c157a3f6c48ddc368c1fe2edc31b1e49f0b9f430805dd52d94571b69016'),
 ('Telemetry',
  'record_sorafs_reputation_snapshot',
  'atomic_if',
  'forward',
  'ab4a615417d2ba9b69cbd4b4c1cbced00b2eb07d0e07d046592f7a63f30b6321'),
 ('Telemetry',
  'record_sorafs_orderbook_api_request',
  'atomic_if',
  'forward',
  'f1094f8546964e811fdc106665b509b806c090312c5862402c91da4e2c3d5f88'),
 ('Telemetry',
  'mark_sorafs_orderbook_finalized_projection_unready',
  'atomic_if',
  'forward',
  '8c176e67b53949c1ef0bd21041a1ef2fa49174dc8fe123fe93ee8b7504b02b81'),
 ('Telemetry',
  'record_sorafs_orderbook_finalized_projection_failure',
  'atomic_if',
  'forward',
  '6ac3171bc747f83dcff697e74ef8dec626edd163f63f0fe1eea35a1a8d044b72'),
 ('Telemetry',
  'record_sorafs_orderbook_finalized_projection',
  'atomic_if',
  'forward',
  'a691a04f9cf4eae7a87f84c9dc7f20a7043ef9d854093f81c95e4d3a9af6f61c'),
 ('Telemetry',
  'record_sorafs_gateway_compliance_request',
  'atomic_if',
  'forward',
  '0259024ae3a88bec37cc6c26584f39a5150f4d0f2201df789a37a32a635a31e1'),
 ('Telemetry',
  'record_sorafs_gateway_compliance_failure',
  'atomic_if',
  'forward',
  '0cef40f38c1821d89ace5131391b91c39a2469eba7ba6a145d89d20aeaccc941'),
 ('Telemetry',
  'record_sorafs_gateway_compliance_serving_decision',
  'atomic_if',
  'forward',
  '29b11932e48d180f9a23f756f4bdfa8f0ea65fb699b59d23360e7ada101afb93'),
 ('Telemetry',
  'record_sorafs_gateway_compliance_serving_catalog',
  'atomic_if',
  'forward',
  'ecd5061bb5af17a4458df054b14ca7596b4bc7433c7115fe405138ca3579720c'),
 ('Telemetry',
  'mark_sorafs_gateway_compliance_unready',
  'atomic_if',
  'forward',
  '2c7c69b61398e7572fd35a348e367fca9f0090fe6ab7d0894e69bb53efd956c1'),
 ('Telemetry',
  'record_sorafs_reserve_finalized_projection',
  'atomic_if',
  'forward',
  '70d68e557b3709e9dd4d3b57fa57cf73d2ad9d208210bb3edf45a1303f3fb44c'),
 ('Telemetry',
  'mark_sorafs_reserve_finalized_projection_unready',
  'atomic_if',
  'forward',
  'aadfcb6dc6cbb1b8a8e5c26c32cb541f0b49fd9b7abe8c06da0b630d94ccebe5'),
 ('Telemetry',
  'record_sorafs_reserve_finalized_projection_failure',
  'atomic_if',
  'forward',
  '5075b3b8ff522073094597dce194c6a76630d7855101a646dd14694046d60d94'),
 ('Telemetry',
  'record_sorafs_reserve_service_request',
  'atomic_if',
  'forward',
  '9f2253e72502a83e785bb13dbb213c8fd677bfab7c2b136a32f45ae22a35942e'),
 ('Telemetry',
  'inc_sorafs_reserve_service_rate_limit',
  'atomic_if',
  'forward',
  '6fed42812f599b91387791b6c053b11d9f07ec2c028e4c190e92b9ba5762c0b5'),
 ('Telemetry',
  'inc_sorafs_disputes',
  'atomic_if',
  'forward',
  'a4b90c3cbb7d14ccd67af30df487adba9b359ac6132c0d9d9db680ddbc59aae0'),
 ('Telemetry',
  'record_sorafs_proof_stream_event',
  'atomic_if',
  'forward',
  '5480a2b7ab5aa1be02656353f6f4dc11b87f62bed50c22e9297abd5d5fde636e'),
 ('Telemetry',
  'start_sorafs_gateway_request',
  'atomic_if',
  'forward',
  'd242d927ba1de0719a636e6a083ae45d721e5ce015f0363ea772559be83257c1'),
 ('Telemetry',
  'finish_sorafs_gateway_request',
  'atomic_if',
  'explicit',
  '160f3af1b024704a300780d7f70c1d68c7320d3f76da048f85a5bd2dbe6251d0'),
 ('Telemetry',
  'record_sorafs_gateway_proof_verification',
  'atomic_if',
  'explicit',
  'ac6482955f2129104377ad81452680fa07ec390bf9192fa8338e7257f97c4674'),
 ('Telemetry',
  'record_sorafs_chunk_range',
  'atomic_if',
  'forward',
  '2b1c01c2602d20fdc4c9f53bea13f7bf521b0437094b9c272ff1424ae5962a20'),
 ('Telemetry',
  'set_sorafs_provider_range_capability',
  'atomic_if',
  'forward',
  '344b292eeb362183f7c5267d5caa5ed3c1926a3386e61d100dacc4ec35106c0d'),
 ('Telemetry',
  'inc_sorafs_routing_authority_cache',
  'atomic_if',
  'forward',
  '7ae4e51c78cdc507d5b8f29b11636c39f8ee5694e04ed5a057e54505d88e39a0'),
 ('Telemetry',
  'inc_sorafs_range_fetch_throttle',
  'atomic_if',
  'forward',
  '9c8b007610f685883fc208448e26224e701212182243fa0ff980c3456132a502'),
 ('Telemetry',
  'inc_sorafs_range_fetch_concurrency',
  'atomic_if',
  'forward',
  '7edbc9d5f9610db23d2411678842d79c06901241b6f9f3dca409111219669a41'),
 ('Telemetry',
  'dec_sorafs_range_fetch_concurrency',
  'atomic_if',
  'forward',
  'a30d08bd7677bedd9f5dc6dd721a3a8cb5a24a948d66fdbffa71a3deaea26a95'),
 ('Telemetry',
  'record_sorafs_gar_violation',
  'atomic_if',
  'forward',
  'a53d86ced2d6b408e7dac1d7e6665e8a987ab3326504db8a09cf10100e85037a'),
 ('Telemetry',
  'record_sorafs_gateway_refusal',
  'atomic_if',
  'forward',
  '0a2f108ac83e2596731bed49595a2f93dcc023a077d34420c8db35611ce0bb4f'),
 ('Telemetry',
  'set_sorafs_gateway_fixture_metadata',
  'atomic_if',
  'forward',
  '3ddd1f1ec7667e0fd2dee3efd784dd738600ec4b26ef29fecb63f4c40f4ff8e0'),
 ('Telemetry',
  'observe_taikai_ingest_latency',
  'atomic_if',
  'forward',
  'ea8bcb1cdd7a20bf5702223fe6fce50cb2b31b3c6b3be4c286d503387d38c2dd'),
 ('Telemetry',
  'observe_taikai_live_edge_drift',
  'atomic_if',
  'forward',
  '149b499b33bf9e550a934020e7a5b593a27d331a51dd4a9c11e8c4875ae3fb0e'),
 ('Telemetry',
  'inc_taikai_ingest_error',
  'atomic_if',
  'forward',
  '2cb86abdf7ec2b07f08b0f96c0bdc0dce2875893d2b59cbb7b2546201d814569'),
 ('Telemetry',
  'record_taikai_alias_rotation',
  'atomic_if',
  'forward',
  'd4d8ba05cc5adf294d85f0aadd20b5c3431ed9a9542a25b99266eb13e73faec8'),
 ('Telemetry',
  'inc_torii_active_conn',
  'atomic_if',
  'explicit',
  'e67407aae3c2903d98251c2652b6a44e2c0795430b1e2972994a6f406f30ccf6'),
 ('Telemetry',
  'dec_torii_active_conn',
  'atomic_if',
  'explicit',
  '83a660becb72a8843e6bd85c8aea4fb1ff5e5ddf46e57b22778f7e7dfa62e246'),
 ('Telemetry',
  'inc_bg_post_overflow',
  'atomic_if',
  'explicit',
  '8c18233d88918c8f51003aa3be82ade7a95261a5e31a77243102297f12cd3556'),
 ('Telemetry',
  'inc_bg_post_drop',
  'atomic_if',
  'explicit',
  'cbcd0aa0d6446d335e6d57fdf9576bb53c2dd927ceaa3497c1dd7e053331b8ea'),
 ('Telemetry',
  'observe_bg_post_age_ms',
  'atomic_if',
  'explicit',
  'c3d0c86db07d38d773841b02ccd39605ef998431c4ed8710607b7011894af7fd'),
 ('Telemetry',
  'set_rbc_sessions_active',
  'atomic_if',
  'explicit',
  '3def90f5b6e5971ce4869f3d0232cbe71df771f02a00f264948484afbe6ec611'),
 ('Telemetry',
  'inc_rbc_init_requests',
  'atomic_if',
  'explicit',
  'f6cc4f884fb00f2ae3901820181972f62159e6465bf0dc3f590f44ba620621a4'),
 ('Telemetry',
  'inc_rbc_chunk_requests',
  'atomic_if',
  'explicit',
  '81f453184e74c3db4c3657c1fee18c1a67d1daa42e79bfdd923748533c0c638d'),
 ('Telemetry',
  'add_rbc_requested_chunks',
  'atomic_if',
  'explicit',
  '2ee0e01519bfecc106576d4930b0d464c528898525bcfbb8b8b7625636807640'),
 ('Telemetry',
  'inc_rbc_repair_fallback',
  'atomic_if',
  'explicit',
  '4f55e67cefc458e8d5391a600ab5487b784c0ed7149040b6bdfcebfc72a7157b'),
 ('Telemetry',
  'inc_rbc_ready_broadcasts',
  'atomic_if',
  'explicit',
  '7833419377140de4a9f22d2cecbaf6772bebcdaf2ba716f41bcec5d1dcb243f0'),
 ('Telemetry',
  'inc_rbc_rebroadcast_skipped',
  'atomic_if',
  'explicit',
  '0b7b6d8607a578b666c1750053e2e6512fab2aecaaa05c6d2926e015e81ed058'),
 ('Telemetry',
  'inc_rbc_deliver_broadcasts',
  'atomic_if',
  'explicit',
  '467c69066a589ba59ea0b56b7c15593ed0399bdee5e267282bdd7025bc068348'),
 ('Telemetry',
  'add_rbc_reconstructed_stripes',
  'atomic_if',
  'explicit',
  '52fcb72a73034ca315c1f392e5a363e07bccf0327504ff6739fbcc4d6829a8f1'),
 ('Telemetry',
  'observe_rbc_seed_latency',
  'atomic_if',
  'explicit',
  'c126fcb48a346310333e026097574cec3e7e307526e22d20217311e6fffc74cd'),
 ('Telemetry',
  'inc_rbc_deliver_defer_ready',
  'atomic_if',
  'explicit',
  '8e91f775af6c51b7237554061644b3e63ba9f03b64fec35e3f4ae5f169ce8f1c'),
 ('Telemetry',
  'inc_rbc_deliver_defer_chunks',
  'atomic_if',
  'explicit',
  '4ac5c6c1de2ebbc9b3dcc57e4b268bb134fdbdf0cdeb8eefc6d3a3d9eed6bdad'),
 ('Telemetry',
  'inc_da_vote_ingested',
  'atomic_if',
  'explicit',
  '4d7df38fa1bee6e305b4c77feeb7a62f489856037c5b064892662dabb1863713'),
 ('Telemetry',
  'inc_kura_store_failure',
  'atomic_if',
  'explicit',
  '4eb21d919ee02f87241b944a8f4622bdcc4b65f83de315e467f0c27741119b50'),
 ('Telemetry',
  'inc_pacemaker_backpressure_deferrals',
  'atomic_if',
  'explicit',
  '2a7c7c363ad89a33e1e20433c778b580fda449c51a2e4059272a3445d6f88b88'),
 ('Telemetry',
  'inc_pacemaker_backpressure_deferral_reason',
  'atomic_if',
  'explicit',
  'a49ffdd90eac5250aed5ea8d571da3560b79325dfa1af5b6afc2ec7f846893f0'),
 ('Telemetry',
  'set_pacemaker_backpressure_deferral_active',
  'atomic_if',
  'explicit',
  '833d5e1932c445bf99d4fa0d391c264f83d7782294a4b3bc616c4bd0ad580d22'),
 ('Telemetry',
  'set_pacemaker_backpressure_deferral_age_ms',
  'atomic_if',
  'explicit',
  'fc35e3e163f948db709cf3e73b5d1f694ed4cc2684bd89250a3cd45b706ed284'),
 ('Telemetry',
  'record_sorafs_metering',
  'atomic_if',
  'forward',
  '200f80c83817f6c4faf3c2d2c27670b6fa3a8ea790e1e59a6c843395c32573e1'),
 ('Telemetry',
  'record_sorafs_storage',
  'atomic_if',
  'forward',
  '9ce4c39ac9849b36ad31e6d3c0d307406bf4e221b8d5659981217695ec1b2531'),
 ('Telemetry',
  'record_sorafs_pin_resource_usage',
  'atomic_if',
  'forward',
  '649dafd34cb94a367f2ab4edce7bc2d6f5f0cf7bb25b2eff83d70164eea0b8e8'),
 ('Telemetry',
  'record_sorafs_alias_cache',
  'atomic_if',
  'forward',
  '32b65b2d27f72b14c2887d64c26f63949bde28482faafd0afb7a59bcb7cf7180'),
 ('Telemetry',
  'observe_torii_route_stage_latency',
  'atomic_if',
  'explicit',
  'd6ff2ea8a83a42f79f3de0289c52d2e4150c22fa5cd241e7675d8a5cd531f658'),
 ('Telemetry',
  'inc_torii_api_token_hit',
  'atomic_if',
  'forward',
  '703f55d11b66e6ff5f8171297a3791f318f94b0e691c6b6de7bf9646bc157541'),
 ('Telemetry',
  'observe_torii_proof_request',
  'atomic_if',
  'explicit',
  'ccef9289ffd9038fe64c61f7a926357afec15661b7c864c39eebdb242a2754b3'),
 ('Telemetry',
  'record_torii_explorer_request',
  'atomic_if',
  'forward',
  '33ccd12606609c6d565de179f1a8cc6a97ea000e67e042f2ae9ca7dc3974ee09'),
 ('Telemetry',
  'inc_torii_proof_cache_hit',
  'atomic_if',
  'forward',
  '44af1badb062dba30d6b474c73bae154388deb0d11bc4f76a3243560a31afbd4'),
 ('Telemetry',
  'inc_torii_attachment_reject',
  'atomic_if',
  'forward',
  '652e43083ecfde368b947c4c61086e724c4a497205214398f88959f7998c94bf'),
 ('Telemetry',
  'observe_torii_attachment_sanitize_ms',
  'atomic_if',
  'forward',
  '06eb2d85ed397b52788608669f7ee9916ed112b629abb9cf6c25fd23df0d462e'),
 ('Telemetry',
  'inc_torii_zk_prover_gc',
  'atomic_if',
  'explicit',
  '9344754d9ada7c25fca7df65dc929c3e190db795d0347a94e31ef03bd31721b0'),
 ('Telemetry',
  'set_torii_zk_prover_inflight',
  'atomic_if',
  'explicit',
  '3627f9b3068633b301be51c0b4f9bc9ae65b425f428e2c4c64d0840ee1d76507'),
 ('Telemetry',
  'set_torii_zk_prover_pending',
  'atomic_if',
  'explicit',
  'c4ac978f10747f02696fcd2e1ae95560f9bd9652f8be1bba1236aeb2db3e52a0'),
 ('Telemetry',
  'set_torii_zk_ivm_prove_inflight',
  'atomic_if',
  'explicit',
  'd0c30bdc405f1ac82e341438cf94d7ee8bda538f2662947e7e70d72604479646'),
 ('Telemetry',
  'set_torii_zk_ivm_prove_queued',
  'atomic_if',
  'explicit',
  'd1c9a19f3f557b7bd6bd59ffb6a8c8cb7f79670023731e33b4f04a531bbf96a7'),
 ('Telemetry',
  'inc_torii_zk_prover_budget_exhausted',
  'atomic_if',
  'explicit',
  '4a522d5abfd94d492eeb88c514d561a2f168247b17ea3e24e84eaf82abc21653'),
 ('Telemetry',
  'inc_sorafs_proof_stream_inflight',
  'atomic_if',
  'forward',
  '9970c7f26e601c4a939026bbe247c2e10f079ba3ff64e02ec9cc62deb8c539ad'),
 ('Telemetry',
  'dec_sorafs_proof_stream_inflight',
  'atomic_if',
  'forward',
  '5e0bf2c2c59a9402963b75097af20f87844f72ea3f8a74f9b4bf6a99361a0522'),
 ('Telemetry',
  'set_sorafs_tls_state',
  'atomic_if',
  'forward',
  'b99f0b8e861765bdc7c21b99978d6213be77b750b99c8b7b6ed2083900d6516b'),
 ('Telemetry',
  'record_sorafs_tls_renewal',
  'atomic_if',
  'forward',
  '134a59a4be7ccf76ecb702a8b7a9d29bd0cb7ea7e6c091387544481f8cf925e9'),
 ('Telemetry',
  'set_sorafs_gateway_fixture_version',
  'atomic_if',
  'forward',
  'ac110791ed0504e7f634f0e5a1d2a53cae54b0fc79a536a8185ea98160795333'),
 ('Telemetry',
  'inc_torii_contract_error',
  'atomic_if',
  'explicit',
  'ddc71272e4399725183ff6ffc87ae61ec4930d3dc5209fef7228e29698321964'),
 ('Telemetry',
  'inc_torii_contract_throttle',
  'atomic_if',
  'explicit',
  'b537d0c312bbef843b83ee7ab9086330cd9918371928a882037b646b71a50d82'),
 ('Telemetry',
  'inc_torii_proof_throttle',
  'atomic_if',
  'explicit',
  '694f24a9b84a4db0604675decfce44ccbfce39b6f49b0a43831bfdb57a556c87'),
 ('Telemetry',
  'set_pacemaker_backoff_ms',
  'atomic_if',
  'explicit',
  'c3729d4181844f39033e55d1fdaff80bbce37de2ef7e700a668b4c8a1756053d'),
 ('Telemetry',
  'set_pacemaker_rtt_floor_ms',
  'atomic_if',
  'explicit',
  '2c6c8d7ef2a9ba772a1bbe708360308c15c06d7ba7bafb082ed26d1e3a56ec8d'),
 ('Telemetry',
  'set_pacemaker_jitter_permille',
  'atomic_if',
  'explicit',
  'ca0f373b303cd66af41f71c9dfb10e6ea488877a712108a7ea2c71c0917277ef'),
 ('Telemetry',
  'set_pacemaker_jitter_ms',
  'atomic_if',
  'explicit',
  'c16ef06c5d0e7fd12b2d0e6802e4daf323119aaf9d24762a375782481a3a8a39'),
 ('Telemetry',
  'observe_phase_latency_ms',
  'atomic_if',
  'explicit',
  'f91179c48ed5fd72dad1c6df3075821f69cc816dfc2bdb55c6989288eda179de'),
 ('Telemetry',
  'set_pacemaker_round_elapsed_ms',
  'atomic_if',
  'explicit',
  'c30ec919259c606fa3a0b6b6672b578f9a8470b0391f5f20c308001c8e69d549'),
 ('Telemetry',
  'set_pacemaker_view_timeout_target_ms',
  'atomic_if',
  'explicit',
  'ffd206012061ca42611f15ae25e1e6fdec39e64eea78c6e38775e409247d560f'),
 ('Telemetry',
  'set_pacemaker_view_timeout_remaining_ms',
  'atomic_if',
  'explicit',
  '04cbfd236dc60e7f3af9b0e51f783ce5301cacc29b2984c6590f6045fe742088'),
 ('Telemetry',
  'inc_dropped_messages',
  'atomic_if',
  'explicit',
  'fed3aaa636b04d341fbe05a026dacf2d930a783bcede5c6d752944d8a30d06dd'),
 ('Telemetry',
  'set_view_changes',
  'atomic_if',
  'explicit',
  'a78f84130e6a0bd3500340fc0aff3d860281b3c5a8b1097b980a3a9f33410e91'),
 ('Telemetry',
  'inc_tail_vote',
  'atomic_if',
  'explicit',
  '5f65ea671989962df93bc7dbb6e997a257331fe5f028a832559f47e485df714c'),
 ('Telemetry',
  'inc_widen_before_rotate',
  'atomic_if',
  'explicit',
  '33bc0f16cd6f63f359332becc473779100643a71219b15e7b1e90286bebb20ef'),
 ('Telemetry',
  'inc_view_change_suggest',
  'atomic_if',
  'explicit',
  '21cee3cc1891bfc51398df453f654143de908e7b4e36e72e276f983036a9c23e'),
 ('Telemetry',
  'inc_view_change_install',
  'atomic_if',
  'explicit',
  '7d0eb56c4fdd58c16924b63a260bf1d625840234097671c3280a93ac9bb76a59'),
 ('Telemetry',
  'inc_proposal_gap',
  'atomic_if',
  'explicit',
  'd5d08c25bcf24de814fc118130150bb26e586e975590a90e10fd2a1b54c6ca16'),
 ('Telemetry',
  'inc_gossip_fallback',
  'atomic_if',
  'explicit',
  '5f7d7442ab944de52ce59054b8d6a4ac4698d0156482e1ae44b3724ff8e4333d'),
 ('Telemetry',
  'inc_block_created_dropped_by_lock',
  'atomic_if',
  'explicit',
  '075eab66a6caa32925d014f34cab2a3494c57b8a3031dadd9461dfc62dd55f21'),
 ('Telemetry',
  'inc_block_created_hint_mismatch',
  'atomic_if',
  'explicit',
  '54114244c455191a7d1b25a1c6e1019ff395f0d9d68f0a9bf5a57e9b3242f18d'),
 ('Telemetry',
  'inc_block_created_proposal_mismatch',
  'atomic_if',
  'explicit',
  'fd0544cac9aeac7ecd61e9970424944a8043b7518ebd03cd8aa73f75fb3176c3'),
 ('Telemetry',
  'note_consensus_message_handling',
  'atomic_if',
  'explicit',
  '60b769852c7601e837c137aa0f9c72b59acf81b8ca078be992818ba5f4e2e5a4'),
 ('Telemetry',
  'observe_cert_size',
  'atomic_if',
  'explicit',
  '109b6ba3b68c7824434adcf5fcbf1822c543ab2ac7a3c7aeb6285b44afb16941'))

EXPECTED_PUBLIC_METHODS = {'StateTelemetry': (144, 'd9491b7c0737bc149ae40fac99491d74a75e5db2c944af1cd6a276a2c176f16c'),
 'StreamingTelemetry': (21, '12cb286b009d23632d1d5cb6bb3a2416ea2e0e02fef8aecf629619befd78d747'),
 'Telemetry': (274, 'f085cfc045e56faffb3e5a2393eb118f1519b453c6d21580708491c8779c3b56')}

EXPECTED_MACRO_HASHES = {'state_telemetry_enabled_metric_methods': '82bed9160ac70eee38046fc0e6b3f82af00dbe4391436eb5ac318a84b90833b2',
 'state_telemetry_enabled_metric_methods_early_return': '24eee9e3e063884f4ace2adf22a0f4893958f7d0e81ee089ce36dbc6ebe7238d',
 'streaming_telemetry_enabled_metric_methods': '63841349c861ee1d455d509e2c2544eb9a77957addc2bb174434d01e007ce0fb',
 'streaming_telemetry_enabled_metric_methods_early_return': '4ee8f5c3dfbb16f82fd859c418d55092604d6d884daac8c61545909b48d58974',
 'telemetry_atomic_enabled_metric_methods': '9debbf2b08302df31aaea677ddd3cb925de03a0096eee4ab36321bada0fc8d11',
 'telemetry_atomic_enabled_metric_methods_early_return': 'f3c510bd9c07ebf08374755bf72ba37f9059819578c6b92367cc527badeff9f8'}

MACRO_SPECS = {
    "state_telemetry_enabled_metric_methods": ("StateTelemetry", "is_enabled_if", True),
    "state_telemetry_enabled_metric_methods_early_return": (
        "StateTelemetry",
        "is_enabled_early_return",
        True,
    ),
    "streaming_telemetry_enabled_metric_methods": (
        "StreamingTelemetry",
        "is_enabled_if",
        True,
    ),
    "streaming_telemetry_enabled_metric_methods_early_return": (
        "StreamingTelemetry",
        "is_enabled_early_return",
        True,
    ),
    "telemetry_atomic_enabled_metric_methods": ("Telemetry", "atomic_if", True),
    "telemetry_atomic_enabled_metric_methods_early_return": (
        "Telemetry",
        "atomic_early_return",
        True,
    ),
}

EXCLUDED_DIRECT_METHODS = {
    "StateTelemetry": {
        "record_lane_lifecycle_outcome",
        "record_sorafs_fee_projection",
        "inc_sorafs_disputes",
        "record_musubi_governance_rejection",
        "record_musubi_integrity_failure",
        "record_musubi_cursor_failure",
    },
    "StreamingTelemetry": {"record_content_key_update"},
    "Telemetry": set(),
}


class GuardFailure(AssertionError):
    """Raised when the source no longer expands to the reviewed inventory."""


@dataclass(frozen=True)
class Row:
    """One typed metric-method row reconstructed as a public method."""

    owner: str
    name: str
    style: str
    form: str
    metadata: tuple[str, ...]
    args: tuple[str, ...]
    operation: str
    position: int

    @property
    def case_id(self) -> str:
        return f"{self.owner}::{self.name}"

    @property
    def digest(self) -> str:
        signature = "pub fn " + self.name + "(&self" + "".join(
            ", " + arg for arg in self.args
        ) + ")"
        payload = [
            self.case_id,
            self.style,
            self.form,
            list(self.metadata),
            _rust_tokens(signature),
            _rust_tokens(self.operation),
        ]
        return hashlib.sha256(
            json.dumps(payload, separators=(",", ":")).encode()
        ).hexdigest()


def _mask_non_code(source: str) -> str:
    masked = list(source)
    index = 0
    while index < len(source):
        if source.startswith("//", index):
            end = source.find("\n", index)
            end = len(source) if end < 0 else end
            for offset in range(index, end):
                masked[offset] = " "
            index = end
            continue
        if source.startswith("/*", index):
            depth = 1
            end = index + 2
            while end < len(source) and depth:
                if source.startswith("/*", end):
                    depth += 1
                    end += 2
                elif source.startswith("*/", end):
                    depth -= 1
                    end += 2
                else:
                    end += 1
            if depth:
                raise GuardFailure("unterminated block comment")
            for offset in range(index, end):
                if source[offset] != "\n":
                    masked[offset] = " "
            index = end
            continue
        raw = re.match(r'(?:br|rb|r)(#{0,32})"', source[index:])
        if raw:
            hashes = raw.group(1)
            end_marker = '"' + hashes
            end = source.find(end_marker, index + raw.end())
            if end < 0:
                raise GuardFailure("unterminated raw string")
            end += len(end_marker)
            for offset in range(index, end):
                if source[offset] != "\n":
                    masked[offset] = " "
            index = end
            continue
        if source[index] == '"':
            end = index + 1
            while end < len(source):
                if source[end] == "\\":
                    end += 2
                    continue
                if source[end] == '"':
                    end += 1
                    break
                end += 1
            for offset in range(index, min(end, len(source))):
                if source[offset] != "\n":
                    masked[offset] = " "
            index = end
            continue
        if source[index] == "'":
            end = index + 1
            if end < len(source) and source[end] == "\\":
                end += 2
            else:
                end += 1
            if end < len(source) and source[end] == "'":
                end += 1
                for offset in range(index, end):
                    masked[offset] = " "
                index = end
                continue
        index += 1
    return "".join(masked)


def _matching(
    masked: str,
    opening_index: int,
    opening: str = "{",
    closing: str = "}",
) -> int:
    depth = 0
    for index in range(opening_index, len(masked)):
        if masked[index] == opening:
            depth += 1
        elif masked[index] == closing:
            depth -= 1
            if depth == 0:
                return index
    raise GuardFailure(f"unbalanced {opening}{closing} delimiter")


def _split_top_level(text: str) -> list[str]:
    parts: list[str] = []
    start = 0
    stack: list[str] = []
    pairs = {")": "(", "]": "[", "}": "{", ">": "<"}
    for index, char in enumerate(text):
        if char in "([{<":
            stack.append(char)
        elif char in pairs:
            if stack:
                stack.pop()
        elif char == "," and not stack:
            item = " ".join(text[start:index].split())
            if item:
                parts.append(item)
            start = index + 1
    item = " ".join(text[start:].split())
    if item:
        parts.append(item)
    return parts


def _rust_tokens(text: str) -> tuple[str, ...]:
    tokens: list[str] = []
    index = 0
    while index < len(text):
        if text[index].isspace():
            index += 1
            continue
        if text.startswith("//", index):
            end = text.find("\n", index)
            index = len(text) if end < 0 else end
            continue
        if text.startswith("/*", index):
            depth = 1
            end = index + 2
            while end < len(text) and depth:
                if text.startswith("/*", end):
                    depth += 1
                    end += 2
                elif text.startswith("*/", end):
                    depth -= 1
                    end += 2
                else:
                    end += 1
            index = end
            continue
        raw = re.match(r'(?:br|rb|r)(#{0,32})"', text[index:])
        if raw:
            hashes = raw.group(1)
            marker = '"' + hashes
            end = text.find(marker, index + raw.end())
            if end < 0:
                raise GuardFailure("unterminated raw literal")
            end += len(marker)
            tokens.append("literal:" + text[index:end])
            index = end
            continue
        if text[index] == '"':
            end = index + 1
            while end < len(text):
                if text[end] == "\\":
                    end += 2
                    continue
                if text[end] == '"':
                    end += 1
                    break
                end += 1
            tokens.append("literal:" + text[index:end])
            index = end
            continue
        if text[index] == "'":
            end = index + 1
            if end < len(text) and text[end] == "\\":
                end += 2
            else:
                end += 1
            if end < len(text) and text[end] == "'":
                end += 1
                tokens.append("literal:" + text[index:end])
                index = end
                continue
            tokens.append("punct:'")
            index += 1
            continue
        identifier = re.match(r"[A-Za-z_][A-Za-z0-9_]*", text[index:])
        if identifier:
            value = identifier.group(0)
            tokens.append("ident:" + value)
            index += len(value)
            continue
        number = re.match(r"[0-9][A-Za-z0-9_.]*", text[index:])
        if number:
            value = number.group(0)
            tokens.append("number:" + value)
            index += len(value)
            continue
        tokens.append("punct:" + text[index])
        index += 1
    return tuple(tokens)


def _impl_span(source: str, masked: str, owner: str) -> tuple[int, int]:
    matches = list(re.finditer(r"(?m)^impl " + re.escape(owner) + r"\s*\{", masked))
    if len(matches) != 1:
        raise GuardFailure(f"{owner}: expected one inherent impl, found {len(matches)}")
    opening = masked.find("{", matches[0].start())
    return opening, _matching(masked, opening)


def _direct_public_methods(
    source: str,
    masked: str,
    owner: str,
) -> list[tuple[int, str]]:
    opening, ending = _impl_span(source, masked, owner)
    depth = 1
    cursor = opening + 1
    methods: list[tuple[int, str]] = []
    pattern = re.compile(r"(?m)^[ \t]+pub fn\s+([A-Za-z_][A-Za-z0-9_]*)")
    for match in pattern.finditer(masked, opening + 1, ending):
        segment = masked[cursor : match.start()]
        depth += segment.count("{") - segment.count("}")
        if depth == 1:
            methods.append((match.start(), match.group(1)))
        cursor = match.start()
    return methods


def _parse_row(
    source: str,
    bracket: int,
    closing: int,
    owner: str,
    style: str,
    metadata: tuple[str, ...],
) -> Row:
    inner = source[bracket + 1 : closing].strip()
    name_match = re.match(r"([A-Za-z_][A-Za-z0-9_]*)\s*\(", inner)
    if not name_match:
        raise GuardFailure(f"{owner}: malformed table row at byte {bracket}")
    name = name_match.group(1)
    local_mask = _mask_non_code(inner)
    opening = local_mask.find("(", name_match.start())
    ending = _matching(local_mask, opening, "(", ")")
    args_source = inner[opening + 1 : ending]
    trailing_arg_comma = args_source.rstrip().endswith(",")
    args = tuple(_split_top_level(args_source))
    for arg in args:
        if not re.fullmatch(r"[A-Za-z_][A-Za-z0-9_]*\s*:[\s\S]+", arg):
            raise GuardFailure(f"{owner}::{name}: malformed typed argument {arg!r}")
    suffix = inner[ending + 1 :].strip()
    if suffix == ";":
        form = "forward"
        arg_names = [arg.split(":", 1)[0].strip() for arg in args]
        trailing = "," if trailing_arg_comma and arg_names else ""
        operation = f"self.metrics.{name}(" + ", ".join(arg_names) + trailing + ");"
    elif suffix.startswith("=>"):
        form = "explicit"
        tail = suffix[2:].strip()
        if not tail.startswith(".") or not tail.endswith(";"):
            raise GuardFailure(f"{owner}::{name}: malformed explicit metric operation")
        operation = "self.metrics" + tail
    else:
        raise GuardFailure(f"{owner}::{name}: unknown table row form")
    return Row(owner, name, style, form, metadata, args, operation, bracket)


def _rows(source: str, masked: str) -> list[Row]:
    rows: list[Row] = []
    for macro_name, (owner, style, _) in MACRO_SPECS.items():
        impl_open, impl_close = _impl_span(source, masked, owner)
        pattern = re.compile(re.escape(macro_name) + r"!\s*\{")
        for invocation in pattern.finditer(masked, impl_open + 1, impl_close):
            opening = masked.find("{", invocation.start(), invocation.end())
            closing = _matching(masked, opening)
            position = opening + 1
            while position < closing:
                line_end = source.find("\n", position, closing)
                if line_end < 0:
                    line_end = closing
                line = source[position:line_end]
                stripped = line.strip()
                if not stripped:
                    position = min(line_end + 1, closing)
                    continue
                metadata: list[str] = []
                while stripped.startswith("///") or stripped.startswith("#["):
                    metadata.append(stripped)
                    position = min(line_end + 1, closing)
                    line_end = source.find("\n", position, closing)
                    if line_end < 0:
                        line_end = closing
                    line = source[position:line_end]
                    stripped = line.strip()
                if not stripped.startswith("["):
                    raise GuardFailure(
                        f"{macro_name}: unexpected table content at byte {position}: {stripped!r}"
                    )
                bracket = source.find("[", position, line_end + 1)
                row_close = _matching(masked, bracket, "[", "]")
                rows.append(
                    _parse_row(
                        source,
                        bracket,
                        row_close,
                        owner,
                        style,
                        tuple(metadata),
                    )
                )
                position = row_close + 1
    rows.sort(key=lambda row: row.position)
    return rows


def _macro_digest(source: str, masked: str, name: str) -> str:
    matches = list(
        re.finditer(r"(?m)^macro_rules! " + re.escape(name) + r"\s*\{", masked)
    )
    if len(matches) != 1:
        raise GuardFailure(
            f"expected one macro definition {name}, found {len(matches)}"
        )
    match = matches[0]
    opening = masked.find("{", match.start())
    ending = _matching(masked, opening)
    start = match.start()
    if name.startswith("streaming_"):
        previous = source.rfind("\n", 0, start - 1) + 1
        if source[previous:start].strip() != '#[cfg(feature = "telemetry")]':
            raise GuardFailure(f"{name}: missing telemetry cfg")
        start = previous
    digest_input = "\0".join(_rust_tokens(source[start : ending + 1]))
    return hashlib.sha256(digest_input.encode()).hexdigest()


def _bind_macro_provider(source: str, provider: str) -> str:
    provider_sha256 = hashlib.sha256(provider.encode()).hexdigest()
    if provider_sha256 != EXPECTED_MACRO_PROVIDER_SHA256:
        raise GuardFailure("enabled-metric macro provider bytes drifted")
    if source.count(MACRO_PROVIDER_INCLUDE) != 1:
        raise GuardFailure("expected one canonical enabled-metric macro provider include")
    if source.count(MACRO_PROVIDER_BINDING) != 1:
        raise GuardFailure(
            "enabled-metric macro provider is not bound immediately before StateTelemetry"
        )
    include_position = source.index(MACRO_PROVIDER_INCLUDE)
    masked = _mask_non_code(source)
    if not masked.startswith("include!(", include_position):
        raise GuardFailure("enabled-metric macro provider include is not active Rust code")
    return source.replace(MACRO_PROVIDER_INCLUDE, provider, 1)


def validate_source(source: str, provider: str) -> dict[str, int]:
    source_lines = source.count("\n")
    provider_lines = provider.count("\n")
    governed_lines = source_lines + provider_lines
    if governed_lines > MAX_GOVERNED_LINES:
        raise GuardFailure(
            "telemetry source-bundle line ceiling exceeded: "
            f"{governed_lines} > {MAX_GOVERNED_LINES}"
        )
    if PREIMAGE_LINES - governed_lines < MINIMUM_NET_REDUCTION:
        raise GuardFailure("minimum governed Rust-line reduction was lost")
    expanded_source = _bind_macro_provider(source, provider)
    masked = _mask_non_code(expanded_source)
    for name, expected in EXPECTED_MACRO_HASHES.items():
        actual = _macro_digest(expanded_source, masked, name)
        if actual != expected:
            raise GuardFailure(f"{name}: emitter guard/signature semantics drifted")
    rows = _rows(expanded_source, masked)
    observed = tuple(
        (row.owner, row.name, row.style, row.form, row.digest) for row in rows
    )
    if len(observed) != len(EXPECTED_ROWS):
        raise GuardFailure(
            f"typed row inventory changed: {len(observed)} != {len(EXPECTED_ROWS)}"
        )
    seen: set[str] = set()
    for index, (actual, expected) in enumerate(zip(observed, EXPECTED_ROWS)):
        case_id = actual[0] + "::" + actual[1]
        if case_id in seen:
            raise GuardFailure(f"duplicate typed row case ID: {case_id}")
        seen.add(case_id)
        if actual[:2] != expected[:2]:
            wanted = expected[0] + "::" + expected[1]
            raise GuardFailure(
                f"typed row order changed at {index}: {case_id} != {wanted}"
            )
        if actual != expected:
            raise GuardFailure(f"{case_id}: docs/signature/guard/operation drifted")
    forward_count = sum(row.form == "forward" for row in rows)
    if forward_count != EXPECTED_FORWARD_ROWS:
        raise GuardFailure(
            f"forward row inventory changed: {forward_count} != {EXPECTED_FORWARD_ROWS}"
        )
    row_events: dict[str, list[tuple[int, str]]] = {
        owner: [] for owner in EXPECTED_PUBLIC_METHODS
    }
    for row in rows:
        row_events[row.owner].append((row.position, row.name))
    for owner, (expected_count, expected_digest) in EXPECTED_PUBLIC_METHODS.items():
        direct = _direct_public_methods(expanded_source, masked, owner)
        direct_names = {name for _, name in direct}
        missing_direct = EXCLUDED_DIRECT_METHODS[owner] - direct_names
        if missing_direct:
            raise GuardFailure(
                f"{owner}: excluded direct methods moved or removed: {sorted(missing_direct)}"
            )
        events = sorted(direct + row_events[owner])
        names = [name for _, name in events]
        digest = hashlib.sha256("\0".join(names).encode()).hexdigest()
        if len(names) != expected_count or digest != expected_digest:
            raise GuardFailure(
                f"{owner}: public logical method inventory/order drifted "
                f"({len(names)} != {expected_count})"
            )
    return {
        "rows": len(rows),
        "forward_rows": forward_count,
        "source_lines": source_lines,
        "provider_lines": provider_lines,
        "governed_lines": governed_lines,
        "net_reduction": PREIMAGE_LINES - governed_lines,
    }


def _replace_once(source: str, old: str, new: str) -> str:
    if source.count(old) != 1:
        raise AssertionError(f"mutation anchor is not unique: {old!r}")
    return source.replace(old, new, 1)


def _mutate_macro(source: str, name: str, old: str, new: str) -> str:
    masked = _mask_non_code(source)
    match = re.search(r"(?m)^macro_rules! " + re.escape(name) + r"\s*\{", masked)
    if not match:
        raise AssertionError(f"missing mutation macro {name}")
    opening = masked.find("{", match.start())
    ending = _matching(masked, opening)
    body = source[match.start() : ending + 1]
    if body.count(old) != 1:
        raise AssertionError(f"macro mutation anchor is not unique: {old!r}")
    return source[: match.start()] + body.replace(old, new, 1) + source[ending + 1 :]


class TelemetryEnabledMetricMethodsGuardTest(unittest.TestCase):
    """Exercise the source seal and representative weakening mutations."""

    @classmethod
    def setUpClass(cls) -> None:
        cls.source = SOURCE_PATH.read_text(encoding="utf-8")
        cls.provider = MACRO_PROVIDER_PATH.read_text(encoding="utf-8")

    def assert_rejected(self, source: str) -> None:
        with self.assertRaises(GuardFailure):
            validate_source(source, self.provider)

    def assert_provider_rejected(self, provider: str) -> None:
        with self.assertRaises(GuardFailure):
            validate_source(self.source, provider)

    def test_reviewed_inventory_and_line_budget(self) -> None:
        self.assertEqual(
            validate_source(self.source, self.provider),
            {
                "rows": 192,
                "forward_rows": 64,
                "source_lines": 12_857,
                "provider_lines": 26,
                "governed_lines": 12_883,
                "net_reduction": 1_375,
            },
        )
        self.assertEqual(PREIMAGE_LINES - MAX_GOVERNED_LINES, 951)
        self.assertGreaterEqual(951, MINIMUM_NET_REDUCTION)

    def test_detached_macro_provider_is_rejected(self) -> None:
        self.assert_rejected(
            _replace_once(
                self.source,
                MACRO_PROVIDER_INCLUDE,
                "// enabled-metric macro provider detached",
            )
        )

    def test_macro_provider_mutation_is_rejected(self) -> None:
        self.assert_provider_rejected(
            _replace_once(
                self.provider,
                "if self.is_enabled() {",
                "if !self.is_enabled() {",
            )
        )

    def test_doc_mutation_is_rejected(self) -> None:
        self.assert_rejected(
            _replace_once(
                self.source,
                "/// Record a feedback timeout event.",
                "/// Record a feedback deadline event.",
            )
        )

    def test_signature_mutation_is_rejected(self) -> None:
        self.assert_rejected(
            _replace_once(
                self.source,
                "[inc_quic_datagrams_sent(delta: u64)",
                "[inc_quic_datagrams_sent(delta: u32)",
            )
        )

    def test_metric_operation_mutation_is_rejected(self) -> None:
        self.assert_rejected(
            _replace_once(
                self.source,
                ".streaming_feedback_timeout_total.inc();]",
                ".streaming_feedback_timeout_total.reset();]",
            )
        )

    def test_forwarder_mutation_is_rejected(self) -> None:
        self.assert_rejected(
            _replace_once(
                self.source,
                "[record_sorafs_fee_projection(provider: &str, fee: &Quantity);]",
                "[record_sorafs_fee_projection(provider: &str, value: &Quantity);]",
            )
        )

    def test_row_deletion_is_rejected(self) -> None:
        block = (
            "    /// Record a feedback timeout event.\n"
            "    [inc_feedback_timeout() => .streaming_feedback_timeout_total.inc();]\n"
        )
        self.assert_rejected(_replace_once(self.source, block, ""))

    def test_row_order_mutation_is_rejected(self) -> None:
        first = (
            "    /// Increment the sent datagram counter by the provided delta.\n"
            "    [inc_quic_datagrams_sent(delta: u64) => "
            ".streaming_quic_datagrams_sent_total.inc_by(delta);]\n"
        )
        second = (
            "    /// Increment the dropped datagram counter by the provided delta.\n"
            "    [inc_quic_datagrams_dropped(delta: u64) =>\n"
            "        .streaming_quic_datagrams_dropped_total.inc_by(delta);]\n"
        )
        joined = first + second
        self.assert_rejected(_replace_once(self.source, joined, second + first))

    def test_guard_style_mutation_is_rejected(self) -> None:
        self.assert_rejected(
            _mutate_macro(
                self.source,
                "telemetry_atomic_enabled_metric_methods",
                "if self.enabled.load(Ordering::Relaxed) {\n"
                "                self.metrics $($op)*",
                "if !self.enabled.load(Ordering::Relaxed) {\n"
                "                self.metrics $($op)*",
            )
        )

    def test_line_ceiling_mutation_is_rejected(self) -> None:
        governed_lines = self.source.count("\n") + self.provider.count("\n")
        excess = MAX_GOVERNED_LINES - governed_lines + 1
        self.assert_rejected(self.source + "\n" * excess)


if __name__ == "__main__":
    unittest.main()
