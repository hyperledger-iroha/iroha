import type { NetworkId } from './index.js';
export type RaceTrackV1 = { kind: 'neon_tokyo' | 'harbor' | 'sakura'; value: null };
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
export function encodeRaceValueV1(name: string, value: unknown): Uint8Array;
export function decodeRaceValueV1(name: string, bytes: Uint8Array): unknown;
export function raceHashLiteralV1(bytes: Uint8Array): string;
export function raceGameplayHashV1(network: NetworkId | string, domain: 'input-reveal' | 'input-commitment' | 'commitment-set' | 'checkpoint' | 'challenge' | 'input-transcript' | 'simulation-state', value: unknown): Uint8Array;

export interface RaceCarStateV1 { progress_mm: string | number | bigint; lateral_mm: number; speed_mm_per_tick: number; lateral_velocity_mm_per_tick: number; boost_energy: number; finish_tick: number | null; dnf_tick: number | null }
export interface RaceStateV1 { tick: number; track: RaceTrackV1; cars: RaceCarStateV1[] }
export interface RaceStandingV1 { slot: number; finish_tick: number | null; dnf_tick: number | null; progress_mm: string | number | bigint }
export interface RaceResultV1 { ticks: number; standings: RaceStandingV1[]; winners: number[] }
/** Canonical eligibility-first policy. Validates bounds, not reachability or authenticated ledger state. */
export function deriveRaceResultV1(state: RaceStateV1): RaceResultV1;
/** Canonical generic outcomes. The caller separately checks terminal admission. */
export function deriveRaceOutcomeV1(state: RaceStateV1): import('./game.js').GameOutcomeV1;
/** Frozen stock terminal boundaries, with no automatic termination of an unfinished solo practice car. */
export function isRaceTerminalStateV1(state: RaceStateV1): boolean;
export interface RacePublicInputsV1 { network_id: string; race_id: string; roster_hash: string; rules_hash: string; track: RaceTrackV1; transcript_root: string; dispute_root: string; result: RaceResultV1 }
export type { ExecutionProofEnvelopeV1 } from './game.js';
export function raceInputPayloadV1(controls: number[]): number[];
export function raceControlsFromPayloadV1(payload: number[]): number[];
export function raceGameTranscriptV1(replay: RaceReplayV1): import('./game.js').GameTranscriptV1;

export interface RaceProverRequestV1 { statement: import('./game.js').ExecutionPublicInputsV1; manifest: import('./game.js').GameManifestV1; admission: import('./game.js').GameAdmissionBodyV1; replay: RaceReplayV1; checkpoint_state: RaceStateV1 | null }
export interface RaceProofPayloadV1 { manifest: import('./game.js').GameManifestV1; admission: import('./game.js').GameAdmissionBodyV1; outcome: import('./game.js').GameOutcomeV1; relation_inputs: RacePublicInputsV1; replay: RaceReplayV1; final_state: RaceStateV1; checkpoint_state: RaceStateV1 | null; stark_bytes: number[] }
