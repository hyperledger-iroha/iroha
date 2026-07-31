import type { EvaluationRealm } from '../../index.ts';

declare class MapSetEvaluationRealm extends EvaluationRealm {
  public readonly name: 'map-set';

  public isArray(node: unknown): node is Set<unknown>;
  public isObject(node: unknown): node is Map<unknown, unknown>;
  public sizeOf(node: unknown): number;
  public has(node: unknown, referenceToken: string): boolean;
  public evaluate<T = unknown>(node: unknown, referenceToken: string): T;
}

export default MapSetEvaluationRealm;
