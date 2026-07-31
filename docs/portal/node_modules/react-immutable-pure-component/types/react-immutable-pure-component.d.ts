export as namespace ImmutablePureComponent;

import { Component, ComponentType, NamedExoticComponent, MemoExoticComponent, FC } from 'react';

type UpdateOn<T> = Array<keyof T | any[]>

export function immutableMemo<P extends object>( Component: FC<P>, updateOnProps?: UpdateOn<P>): NamedExoticComponent<P>;
export function immutableMemo<T extends ComponentType<any>>( Component: T, updateOnProps?: UpdateOn<T>): MemoExoticComponent<T>;

export class ImmutablePureComponent<P = {}, S = {}, SS = any> extends Component<P, S, SS> {
      updateOnProps: UpdateOn<P>;
      updateOnStates: UpdateOn<S>;
}

export default ImmutablePureComponent;
