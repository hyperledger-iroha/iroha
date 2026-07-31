/* @flow */

declare type UpdateOn<T> = Array<$Keys<T> | any[]>;

declare export function immutableMemo<Props>(
  component: React$ComponentType<Props>,
  updateOnProps?: UpdateOn<Props>,
): React$ComponentType<Props>;

declare export class ImmutablePureComponent<
  Props,
  State = void,
> extends React$Component<Props, State> {
  // TODO: Due to bugs in Flow's handling of React.createClass, some fields
  // already declared in the base class need to be redeclared below. Ideally
  // they should simply be inherited.

  updateOnProps: UpdateOn<Props>;
  updateOnStates: UpdateOn<State>;

  props: Props;
  state: State;
}

export default ImmutablePureComponent;
