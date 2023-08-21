# Actors Theory

Stewart is built around the "actor" pattern.
This pattern is implemented in different ways at different levels.
It's good to understand the theoretical concept of "actors", to understand how to use them.
This theory primer gives a quick *heavily generalized* explanations of what actors are.

Actors are a method of organizing "asynchronous" components of software.
Actor systems typically, but not always, have the following traits:

- Maintain a persistent state.
- Communicate with other actors, and the outside world, through messages.
- 'Suspended' execution, until new messages arrive.

This is very similar to the idea of "futures" or "promises".
This "async-await" model of futures is used to 'suspend' a function until the response it's waiting
for is available.

In contrast to futures, actors are an ongoing 'process'.
Actors don't suspend on another function's result, rather they *reactively* respond to incoming
messages.

In Rust, futures are implemented very similar to the actor model, even if typically you use them
with the `async` and `await` keywords.
