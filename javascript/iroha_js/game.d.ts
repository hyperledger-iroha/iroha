export * from "./game-resources.js";
import type { GameResourceReservationClauseV1, GameResourceReturnPolicyV1, GameResourceReservationRecordV1 } from "./game-resources.js";
import type { NetworkId } from './index.js';
export type GameInstructionNameV1 = 'OpenGameSessionV1' | 'JoinGameSessionV1' | 'StartGameSessionV1' | 'CommitGameCheckpointV1' | 'ChallengeGameSessionV1' | 'CommitGameInputsV1' | 'RevealGameInputsV1' | 'AdvanceGameDeadlineV1' | 'SettleGameSessionV1' | 'ExpireGameSessionV1' | 'ClaimGamePayoutV1' | 'StakeGameItemV1' | 'RegisterExecutionProofProfileV1' | 'VerifyExecutionProofV1';
export type GameAccessV1 = { kind: 'public'; public_key: null } | { kind: 'invite'; public_key: string };
export type GamePayoutPolicyV1 = { kind: 'no_payout' | 'equal_winners_or_refund'; value: null };
export interface GameManifestV1 { version: 1; application_id: string; profile_id: string; application_parameters: number[]; min_participants: number; max_participants: number; batch_ticks: number; max_ticks: number; max_input_bytes: number; max_participant_data_bytes: number; access: GameAccessV1; payout_policy: GamePayoutPolicyV1 }
export interface GameCheckpointV1 { session_id: string; epoch: string | number | bigint; tick: number; transcript_root: string; state_root: string; terminal: boolean }
export interface GameSlotSignatureV1 { slot: number; signature: string }
export interface SignedGameCheckpointV1 { checkpoint: GameCheckpointV1; signatures: GameSlotSignatureV1[] }
export interface GameCommitmentSetBodyV1 { session_id: string; epoch: string | number | bigint; start_tick: number; parent_transcript_root: string; commitments: string[] }
export interface GameCommitmentSetV1 { session_id: string; epoch: string | number | bigint; start_tick: number; parent_transcript_root: string; commitments: string[]; signatures: GameSlotSignatureV1[] }
export interface GameInputCommitmentV1 { session_id: string; epoch: string | number | bigint; start_tick: number; slot: number; commitment: string; signature: string }
export interface GameInputRevealV1 { session_id: string; epoch: string | number | bigint; start_tick: number; slot: number; payload: number[]; salt: string }
export interface GameTranscriptV1 { batches: { start_tick: number; inputs: number[][] }[]; dnf_events: { tick: number; slots: number[] }[] }
export interface GamePayoutClaimV1 { slot: number; amount: string; remaining: string }
export interface ClaimGamePayoutV1 { session_id: string; slot: number; destination: string; amount: string }
/** Mandatory wallet-approved debit and immutable game terms; no endpoint-derived defaults. */
export interface JoinGameSessionV1 { session_id: string; input_key: string; application_data: number[]; resources: GameResourceReservationClauseV1[]; invitation: string | null; expected_manifest_hash: string; expected_asset_definition: string; expected_stake: string }
export interface GameOutcomeV1 { terminal_tick: number; winner_slots: number[]; result: number[] }
export interface ExecutionPublicInputsV1 { network_id: string; session_id: string; manifest_hash: string; roster_hash: string; transcript_root: string; dispute_root: string; outcome_hash: string }
export interface ExecutionProofEnvelopeV1 { version: 1; profile_id: string; statement: ExecutionPublicInputsV1; proof_bytes: number[] }
export const GAME_INSTRUCTION_NAMES_V1: readonly GameInstructionNameV1[];
export const GAME_INSTRUCTION_WIRE_IDS_V1: readonly string[];
/** Complete canonical execution envelope and typed settlement payload bound; not a global transaction limit. */
export const EXECUTION_PROOF_MAX_ENVELOPE_BYTES_V1: number;
export function encodeGameValueV1(name: string, value: unknown): Uint8Array;
export function decodeGameValueV1(name: string, bytes: Uint8Array): unknown;
export function gameHashLiteralV1(bytes: Uint8Array): string;
export function gameMessageHashV1(network: NetworkId | string, domain: 'roster' | 'input-reveal' | 'input-commitment' | 'commitment-set' | 'checkpoint' | 'challenge' | 'invitation' | 'input-transcript' | 'simulation-state' | 'session-manifest' | 'session-outcome', value: unknown): Uint8Array;
export function buildGameInstructionV1(name: GameInstructionNameV1, value: unknown): object;
export function buildJoinGameSessionV1(value: JoinGameSessionV1): { JoinGameSessionV1: JoinGameSessionV1 };

export interface GameParticipantV1 { account: string; input_key: string; application_data: number[]; dnf_at_tick: number | null }
export interface GameItemStakeV1 { slot: number; nft_id: string; custody: string; metadata_hash: string; recipient: string | null; claimed: boolean }
export interface StakeGameItemV1 { session_id: string; nft_id: string; expected_manifest_hash: string }
export interface GameAdmissionParticipantV1 { account: string; input_key: string; application_data: number[] }
export interface GameAdmissionWagerV1 { slot: number; nft_id: string; metadata_hash: string }
export interface GameAdmissionResourceV1 { slot: number; nft_id: string; metadata_hash: string; role_id: string; policy: GameResourceReturnPolicyV1 }
export interface GameAdmissionBodyV1 { version: 1; participants: GameAdmissionParticipantV1[]; wagers: GameAdmissionWagerV1[]; resources: GameAdmissionResourceV1[] }
export function gameRosterHashV1(network: NetworkId | string, sessionId: string, admission: GameAdmissionBodyV1): string;
export function validateGameAdmissionBodyV1(value: GameAdmissionBodyV1): GameAdmissionBodyV1;

export interface GameSessionEventV1 { session_id: string; revision: string | number | bigint; phase: number; dispute_root: string; payout_claims: GamePayoutClaimV1[]; item_stakes: GameItemStakeV1[]; resources: GameResourceReservationRecordV1[]; terminal_at_height: string | number | bigint | null }

export const GAME_ADMISSION_MAX_ACCOUNT_ENCODED_BYTES_V1: 32768;
export const GAME_ADMISSION_MAX_NFT_ENCODED_BYTES_V1: 1024;
export const GAME_ADMISSION_MAX_BYTES_V1: number;
