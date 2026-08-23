`WebSocket` connection flow stages.

Flow consists of the following:

1. **Init stage**: establish `WebSocket` connection with Iroha
2. **Events stage**: wait for messages from Iroha. For each message, decode *some event* from it
   and send back *some "received"* message



This module has a set of abstraction to extract pure data logic from transportation logic. Following sections
describe how to use this module from both **flow implementation** (data side) and
**transport implementation** sides.

## Flow implementation

From data side, you should implement a state machine built on top of these traits:

- [Init][conn_flow::Init] it is designed to consume its impl struct and produce a tuple, that has 2 items:
  **initial data** to establish WS connection, and the **handler** of the next flow stage — **events**.
  Then, transportation side should open a connection, send first message into it, receive message from Iroha
  and pass it into the next handler.
- [Events][conn_flow::Events] handles incoming messages and returns a **binary reply** back Iroha and **some decoded event**.

Here is an example of how to implement flow in a transport-agnostic manner:

```rust
use eyre::{Result, eyre};
use iroha::http::{
    Method, RequestBuilder,
    ws::conn_flow::{Events as FlowEvents, Init as FlowInit, InitData},
};

struct Init;

impl<R: RequestBuilder> FlowInit<R> for Init {
    type Next = Events;

    fn init(self) -> InitData<R, Self::Next> {
        InitData::new(
            R::new(
                Method::GET,
                "http://localhost:3000".parse().expect(
                    "`localhost` is a valid URL, port `3000` is sensible, `http` is supported",
                ),
            ),
            vec![1, 2, 3],
            Events,
        )
    }
}

struct Events;

impl FlowEvents for Events {
    type Event = u8;

    fn message(&self, message: Vec<u8>) -> Result<Self::Event> {
        Ok(message[0])
    }
}
```

## Transport implementation

You are a library user and want to use Iroha Client with your own HTTP/WS implementation. For such a purpose
the client library should provide an API wrapped into the flow traits. Anyway, firstly you should implement
[`super::RequestBuilder`] trait for your transport.

Let's take Events API as an example. [`crate::client::Client::events_handler`] creates a struct of
initial WS flow stage - [`crate::client::events_api::flow::Init`].
Here is an example (oversimplified) of how you can use it:

```rust
use eyre::Result;
use iroha::{
    client::events_api::flow as events_api_flow,
    data_model::prelude::EventBox,
    http::{
        Method, RequestBuilder,
        ws::conn_flow::{Events, Init, InitData},
    },
};
use url::Url;

// Some request builder
struct MyBuilder;

impl RequestBuilder for MyBuilder {
    fn new(_: Method, url: Url) -> Self {
        Self
    }

    fn param<K: AsRef<str>, V: ?Sized + ToString>(self, _: K, _: &V) -> Self {
        self
    }

    fn header<N: AsRef<str>, V: ?Sized + ToString>(self, _: N, _: &V) -> Self {
        self
    }

    fn body(self, data: Vec<u8>) -> Self {
        let _ = data;
        self
    }
}

impl MyBuilder {
    fn connect(self) -> MyStream {
        /* ... */
        MyStream {}
    }
}

// Some `WebSocket` stream
struct MyStream;

impl MyStream {
    // Receive message
    fn get_next(&self) -> Vec<u8> {
        /* ... */
        Vec::new()
    }

    // Send message
    fn send(&self, msg: Vec<u8>) {
        /* ... */
    }
}

fn collect_5_events(flow: events_api_flow::Init) -> Result<Vec<EventBox>> {
    // Constructing initial flow data
    let InitData {
        next: flow,
        first_message,
        req,
    }: InitData<MyBuilder, _> = flow.init();

    // Firstly, sending the message
    let stream = req.connect();
    stream.send(first_message);

    // And now we are able to collect events
    let mut events: Vec<EventBox> = Vec::with_capacity(5);
    while events.len() < 5 {
        let msg = stream.get_next();
        let event = flow.message(msg)?;
        events.push(event);
    }

    Ok(events)
}
```
