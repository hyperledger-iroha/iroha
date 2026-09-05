import type { NetworkId } from './index.js';
export type RaceInstructionNameV1 = 'OpenRaceV1' | 'JoinRaceV1' | 'StartRaceV1' | 'CommitRaceCheckpointV1' | 'ChallengeRaceV1' | 'CommitRaceInputsV1' | 'RevealRaceInputsV1' | 'AdvanceRaceDeadlineV1' | 'SubmitRaceProofV1' | 'ExpireRaceV1';
export type RaceTrackV1 = { kind: 'neon_tokyo' | 'harbor' | 'sakura' };
export interface RaceRulesV1 { version: 1; track: RaceTrackV1; max_racers: number }
export interface RaceCheckpointV1 { race_id: string; epoch: string | number | bigint; tick: number; transcript_root: string; state_root: string; terminal: boolean }
export interface RaceSlotSignatureV1 { slot: number; signature: string }
export interface SignedRaceCheckpointV1 { checkpoint: RaceCheckpointV1; signatures: RaceSlotSignatureV1[] }
export interface RaceCommitmentSetBodyV1 { race_id: string; epoch: string | number | bigint; start_tick: number; parent_transcript_root: string; commitments: string[] }
export interface RaceCommitmentSetV1 extends RaceCommitmentSetBodyV1 { signatures: RaceSlotSignatureV1[] }
export interface RaceInputRevealV1 { race_id: string; epoch: string | number | bigint; start_tick: number; slot: number; controls: number[]; salt: string }
export interface RaceInputCommitmentBodyV1 { race_id: string; epoch: string | number | bigint; start_tick: number; slot: number; commitment: string }
export interface RaceInputCommitmentV1 extends RaceInputCommitmentBodyV1 { signature: string }
export interface RaceReplayV1 { track: RaceTrackV1; player_count: number; frames: { tick: number; controls: number[] }[]; dnf_events: { tick: number; slots: number[] }[] }
export const RACE_INSTRUCTION_NAMES_V1: readonly RaceInstructionNameV1[];
export const RACE_INSTRUCTION_WIRE_IDS_V1: readonly string[];
export function encodeRaceValueV1(name: string, value: unknown): Uint8Array;
export function decodeRaceValueV1(name: string, bytes: Uint8Array): unknown;
export function raceHashLiteralV1(bytes: Uint8Array): string;
export function raceGameplayHashV1(network: NetworkId | string, domain: 'input-reveal' | 'input-commitment' | 'commitment-set' | 'checkpoint' | 'challenge' | 'input-transcript' | 'simulation-state', value: unknown): Uint8Array;
export function buildRaceInstructionV1(name: RaceInstructionNameV1, value: unknown): object;

export interface RaceCarStateV1 { progress_mm: string | number | bigint; lateral_mm: number; speed_mm_per_tick: number; lateral_velocity_mm_per_tick: number; boost_energy: number; finish_tick: number | null; dnf_tick: number | null }
export interface RaceStateV1 { tick: number; track: RaceTrackV1; cars: RaceCarStateV1[] }
export interface RaceStandingV1 { slot: number; finish_tick: number | null; dnf_tick: number | null; progress_mm: string | number | bigint }
export interface RaceResultV1 { ticks: number; standings: RaceStandingV1[]; winners: number[] }
export interface RacePublicInputsV1 { network_id: string; race_id: string; roster_hash: string; rules_hash: string; track: RaceTrackV1; transcript_root: string; dispute_root: string; result: RaceResultV1 }
export interface ExecutionProofEnvelopeV1 { version: 1; profile_id: string; statement: RacePublicInputsV1; proof_bytes: number[] }
