Config parameter ID, which is a path in config file, e.g. `foo.bar`.

```
use iroha_config_base::ParameterId;

let id = ParameterId::from(["foo", "bar"]);

assert_eq!(format!("{id}"), "foo.bar");
```
