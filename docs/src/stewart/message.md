# Message

Stewart actors can wake eachother up within the same system by sending `Signal`.
A `Signal` however doesn't contain any data.
For this, `stewart-message` provides messaging primitives.

## Consistent State and Dropping `Mailbox`

To make sure state between actors is as consistent as possible, `Sender` will return an error if
the other side no longer can receive.
However, a `Mailbox` can still attempt to receive, even if all `Sender`s are dropped.

To avoid unintentionally crashing your actors, make sure you always process **all** incoming
messages from an actor, before sending any messages back.

If your actor maintains `Mailbox`es other actors send to, make sure they're notified of the
`Mailbox` closing.
Usually simply sending out an event to a receiving mailbox is enough, as it should be processed
first.

This pattern relies on the *single-threaded* nature of a stewart `World`.
Envoy actors relaying messages to external systems should still follow this pattern internally.
