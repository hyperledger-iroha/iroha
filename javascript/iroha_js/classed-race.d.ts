/** Canonical Touring application types; codecs do not prove simulation results or NFT admission. */
export type ClassedRaceClassV1 = { kind: "touring_s1"; value: null };
export type ClassedRaceTrackV1 = { kind: "neon_tokyo" | "harbor" | "sakura"; value: null };
/** Encoding accepts canonical integers; decoded native i64 fields are decimal strings. */
export type ClassedRaceProgressV1 = number | string | bigint;
export interface ClassedRaceInputFrameV1 { tick: number; controls: number[] }
export interface ClassedRaceDnfEventV1 { tick: number; slots: number[] }
export interface ClassedRaceReplayV1 {
  version: 1;
  class_id: ClassedRaceClassV1;
  track: ClassedRaceTrackV1;
  player_count: number;
  frames: ClassedRaceInputFrameV1[];
  dnf_events: ClassedRaceDnfEventV1[];
}
export interface ClassedRaceCarStateV1 {
  progress_mm: ClassedRaceProgressV1;
  lateral_mm: number;
  speed_mm_per_tick: number;
  lateral_velocity_mm_per_tick: number;
  boost_energy: number;
  finish_tick: number | null;
  dnf_tick: number | null;
}
export interface ClassedRaceStateV1 {
  tick: number;
  class_id: ClassedRaceClassV1;
  track: ClassedRaceTrackV1;
  cars: ClassedRaceCarStateV1[];
}
export interface ClassedRaceStandingV1 {
  slot: number;
  finish_tick: number | null;
  dnf_tick: number | null;
  progress_mm: ClassedRaceProgressV1;
}
export interface ClassedRaceResultV1 {
  class_id: ClassedRaceClassV1;
  track: ClassedRaceTrackV1;
  ticks: number;
  terminal: boolean;
  standings: ClassedRaceStandingV1[];
  winners: number[];
}
export interface ClassedRaceValuesV1 {
  ClassedRaceClassV1: ClassedRaceClassV1;
  ClassedRaceTrackV1: ClassedRaceTrackV1;
  ClassedRaceInputFrameV1: ClassedRaceInputFrameV1;
  ClassedRaceDnfEventV1: ClassedRaceDnfEventV1;
  ClassedRaceReplayV1: ClassedRaceReplayV1;
  ClassedRaceCarStateV1: ClassedRaceCarStateV1;
  ClassedRaceStateV1: ClassedRaceStateV1;
  ClassedRaceStandingV1: ClassedRaceStandingV1;
  ClassedRaceResultV1: ClassedRaceResultV1;
}
export type ClassedRaceValueNameV1 = keyof ClassedRaceValuesV1;
export const CLASSED_RACE_VALUE_NAMES_V1: readonly ClassedRaceValueNameV1[];
export const CLASSED_RACE_MAX_TICKS_V1: 5400;
export const CLASSED_RACE_MAX_PLAYERS_V1: 8;
export const CLASSED_RACE_MAX_VALUE_BYTES_V1: number;
export function encodeClassedRaceValueV1<Name extends ClassedRaceValueNameV1>(name: Name, value: ClassedRaceValuesV1[Name]): Uint8Array;
export function decodeClassedRaceValueV1<Name extends ClassedRaceValueNameV1>(name: Name, bytes: Uint8Array): ClassedRaceValuesV1[Name];
/** Require the exact top-level native type name, flags, checksum, length and zero padding. */
export function decodeClassedRaceFrameV1<Name extends ClassedRaceValueNameV1>(name: Name, bytes: Uint8Array): ClassedRaceValuesV1[Name];
