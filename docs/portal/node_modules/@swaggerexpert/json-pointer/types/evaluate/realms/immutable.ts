import type { EvaluationRealm } from '../../index.ts';

declare class ImmutableEvaluationRealm extends EvaluationRealm {
  public readonly name: 'immutable';

  public isArray(node: unknown): boolean;
  public isObject(node: unknown): boolean;
  public sizeOf(node: unknown): number;
  public has(node: unknown, referenceToken: string): boolean;
  public evaluate<T = unknown>(node: unknown, referenceToken: string): T;
}

export default ImmutableEvaluationRealm;
