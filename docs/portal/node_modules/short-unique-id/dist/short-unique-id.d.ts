import type {
  DEFAULT_OPTIONS,
  DEFAULT_UUID_LENGTH,
  default as ShortUniqueIdCore,
  ShortUniqueIdDefaultDictionaries,
  ShortUniqueIdOptions,
  ShortUniqueIdRanges,
  ShortUniqueIdRangesMap,
} from './short-unique-id-core.d.ts';

declare namespace ShortUniqueId {
  export type {
    DEFAULT_OPTIONS,
    DEFAULT_UUID_LENGTH,
    ShortUniqueIdDefaultDictionaries,
    ShortUniqueIdOptions,
    ShortUniqueIdRanges,
    ShortUniqueIdRangesMap,
  };
}
declare class ShortUniqueId extends ShortUniqueIdCore {}
export = ShortUniqueId;
