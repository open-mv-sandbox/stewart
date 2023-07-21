# Stewart

The core stewart library is a small *single-threaded* actor system.

## Signal

Stewart's core library does not include an implementation of sending messages to actors.
Instead, you can use `Signal` to schedule an actor to be run.
Then, during a `process` step, the actor can use a messaging implementation to poll for pending
messages.

If you have typical requirements for your messaging, you can use `stewart-message`.
This crate uses `Signal` internally.
