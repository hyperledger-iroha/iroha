[![npm version](https://badge.fury.io/js/react-immutable-pure-component.svg)](https://badge.fury.io/js/react-immutable-pure-component)

# ImmutablePureComponent

Unfortunately `React.PureComponent` is not embracing `Immutable.js` to it full
potential. While `Immutable.js` provides [hash value](https://facebook.github.io/immutable-js/docs/#/ValueObject/hashCode),
witch allows for fast comparison of two different instances
`React.PureComonent` is only comparing addresses of those instances.

The `ImmutablePureComponent` uses [is](https://facebook.github.io/immutable-js/docs/#/is) to compare values and
extends component functionality by introducing:
* `updateOnProps`
* `updateOnStates`

With those properties you can specify list of props or states that will be
checked for changes. If value is `undefined` (default) then all `props` and
`state` will be checked, otherwise array of keys or paths is expected. The path
is an `Array` of keys like in the example below. Path values are working for
any mix of supported collection as long as given key exists, otherwise checked
value is `undefined`. `Immutable.Collection`, plain Objects, Arrays, es6 Map
and any collection providing `get` and `has` functionality are all supported.

```
type UpdateOn<T> = Array<$Keys<T> | any[]>;

export class ImmutablePureComponent<
  Props,
  State = void,
> extends React$Component<Props, State> {

  updateOnProps: UpdateOn<Props>;
  updateOnStates: UpdateOn<State>;
}

export default ImmutablePureComponent;
```

# immutableMemo

With React `16.6.0` we ware introduced to `React.memo` a `React.PureComponent`
equivalent for functional components. And the same story goes here,
unfortunately `React.memo` is not fully embracing `Immutable` potential. That
is where `immutableMemo` steps in. This is wrapper over `React.memo` with
custom comparison function. `immutableMemo` accepts component as first argument
and optionally array of property keys or paths the same way as `updateOnProps`
is working for `ImmutablePureComponent`.

```
export function immutableMemo<Props>(
  component: React$ComponentType<Props>,
  updateOnProps?: UpdateOn<Props>,
): React$ComponentType<Props>;
```

### Example
In this example component will update when value of `me` is change and will
ignore changes of `data`, `check` or any other property. Component will also
update on change of first element of `buzz` or change to `type` and will ignore
changes to the rest of the state. 

```js
class Example extends ImmutablePureComponent {
  state = {
    fis: { 
      buzz: Immutable.List([10, 11])
      ignore: 'this',
    },
    type: undefined,
  };

  updateOnStates = [
    ['fis', 'buzz', 0],
    'type',
  ];

  updateOnProps = [
    ['data', 'check', 'me'],
  ];

  render() {...}
}

let data = Immutable.Map({ check: new Map([['me', true]]) }) 

ReactDOM.render(<Example data={data} onChange={() => {}}, root);
```

To check what its all about checkout the interactive example :D
### [Interactive example](https://codesandbox.io/s/github/Monar/react-immutable-pure-component/tree/master/example).
