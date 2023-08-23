# Actors Theory

Functions are great!
When you need to "abstract" something you want your code to do, you simply wrap it in a name with
some parameters.
This makes it a lot easier to reason around what you're making, and what your code is doing.

Sometimes, however, you need to create a task that needs to 'wait' on something.
Maybe you have to wait for a network adapter to receive a packet, or wait for another task to be
done performing some intensive calculations.

While it's perfectly *possible* to use OS threads for this, and 'block' the current one until you
get what you need, this is very resource-intensive.
Additionally, OS threads usually don't expect to be rapidly switching between a lot of different
threads in more complex programs.

## `async-await`

One approach to solving this instead is using the "async-await" pattern.
With this pattern you can still write your functions as normal, but additionally you can 'await' on
something before continuing.

How this is implemented depends on the language.
Rust for example, will 'generate' a state machine for your function.
This is nice, as it can track a lot of guarantees about memory and ownership semantics.

There are a few downsides to this approach.
A lot of complexity happens behind the scenes, making it a lot harder to reason about your code.
As well, while this pattern is well suited to a linear set of steps, it doesn't model ongoing
processes as well.

## The Actor Pattern

Enter, the "Actor Pattern".
Conceptually, actors are cooperative multitasking tasks, that communicate through messages.
This is, of course, highly simplifying and generalizing the concept.
The computer science of actors is however outside the scope of this guide.

In contrast to `async-await`, actors hide very little in how they work.
An actor maintains its internal state manually.
Its handler function returns when it is done processing, yielding back to the runtime.
