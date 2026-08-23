General HTTP request builder.

To use custom builder with client, you need to implement this trait for some type and pass it
to the client that will fill it with data.

The order of builder methods invocation is not strict. There is no guarantee that builder user calls
all methods. Only [`RequestBuilder::new`] is the required one.
