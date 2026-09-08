/** Explicit native return policy; resource authorization is separate from wagers. */
export interface GameResourceReturnPolicyV1 { kind: 'return_to_original_owner_at_terminal'; value: null }
/** Explicit temporary custody authorization; separate from a wager or opaque application data. */
export interface GameResourceReservationClauseV1 {
  nft_id: string;
  expected_metadata_hash: string;
  role_id: string;
  policy: GameResourceReturnPolicyV1;
}
/** A compiled adapter requirement does not supply wallet authorization. */
export interface GameResourceRequirementV1 {
  nft_id: string;
  expected_metadata_hash: string;
  role_id: string;
  policy: GameResourceReturnPolicyV1;
}
export interface GameResourceReservationRecordV1 {
  slot: number;
  nft_id: string;
  metadata_hash: string;
  role_id: string;
  policy: GameResourceReturnPolicyV1;
  original_owner: string;
  custody: string;
  reserved_at_height: string | number | bigint;
  released_at_height: string | number | bigint | null;
}
export interface GameResourceReservationSetV1 {
  version: 1;
  network_id: string;
  session_id: string;
  records: GameResourceReservationRecordV1[];
}
export interface GameResourceValuesV1 {
  GameResourceReturnPolicyV1: GameResourceReturnPolicyV1;
  GameResourceReservationClauseV1: GameResourceReservationClauseV1;
  GameResourceRequirementV1: GameResourceRequirementV1;
  GameResourceReservationRecordV1: GameResourceReservationRecordV1;
  GameResourceReservationSetV1: GameResourceReservationSetV1;
  clauses: GameResourceReservationClauseV1[];
  requirements: GameResourceRequirementV1[];
  records: GameResourceReservationRecordV1[];
}
export const GAME_RESOURCE_VALUE_NAMES_V1: readonly (keyof GameResourceValuesV1)[];
export const GAME_MAX_RESOURCES_PER_PARTICIPANT_V1: 4;
export const GAME_MAX_RESOURCE_PARTICIPANTS_V1: 32;
export const GAME_MAX_RESOURCE_RECORDS_V1: 128;
export const GAME_RESOURCE_MAX_NFT_ID_BYTES_V1: 512;
export const GAME_RESOURCE_MAX_ACCOUNT_ID_BYTES_V1: number;
export const GAME_RESOURCE_MAX_CLAUSE_BYTES_V1: 1024;
export const GAME_RESOURCE_MAX_SET_BYTES_V1: number;
/** Canonical bounded COMPACT_LEN bare bytes. */
export function encodeGameResourceValueV1<T extends keyof GameResourceValuesV1>(name: T, value: GameResourceValuesV1[T]): Uint8Array;
export function decodeGameResourceValueV1<T extends keyof GameResourceValuesV1>(name: T, bytes: Uint8Array): GameResourceValuesV1[T];
export function validateGameResourceClausesV1(value: readonly GameResourceReservationClauseV1[]): GameResourceReservationClauseV1[];
export function validateGameResourceRequirementsV1(value: readonly GameResourceRequirementV1[]): GameResourceRequirementV1[];
export function matchGameResourceRequirementsV1(clauses: readonly GameResourceReservationClauseV1[], requirements: readonly GameResourceRequirementV1[]): void;
/** Pure geometry/roster check. Does not authenticate WSV, custody derivation or a terminal transition. */
export function validateGameResourceReservationSetV1(value: GameResourceReservationSetV1, participantOwners?: readonly string[]): GameResourceReservationSetV1;

export function validateGameResourceRecordsV1(value: readonly GameResourceReservationRecordV1[], participantOwners?: readonly string[]): GameResourceReservationRecordV1[];
