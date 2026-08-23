Consumes itself to produce initial data to:

- Open WS connection;
- Send first message into it;
- Handle first message from Iroha with the next handler.

It doesn't return a `Result` because it doesn't accept any parameters except of itself.
