# Message

Stewart actors can wake eachother up within the same system by sending `Signal`.
A `Signal` however doesn't contain any data.
For this, `stewart-message` provides messaging primitives.

## Consistent State and Dropping `Mailbox`/`Sender`

When a `Sender` has no `Mailbox`, calling `send` returns an error.
When a `Maiblox` has no messages *and* no `Senders`, calling `recv` returns an error.
This is because this is very likely a mistake, and stewart will always prefer terminating over
running in an inconsistent state.

This may raise a concern, what if an actor stops but I don't know about this yet, and accidentally
send a message?
Luckily, actors in a stewart world always run *single-threaded*.
Because only one actor runs at a time, there will be no changes made to it inbetween polling
events, and sending messages.
As long as you maintain this order, you can catch normal close stop form other actor before sending
data to them.

Of course, in distributed systems this is likely not true.
An actor envoy should handle the complexities of talking to external systems internally.
