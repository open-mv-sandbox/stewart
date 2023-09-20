# Your First Actor

You can create a new stewart actor behavior by implementing the `Actor` trait.

```rust
struct MyActor {
    message: String,
}

impl Actor for MyActor {
    fn process(&mut self, _world: &mut World, _meta: &mut Metadata) -> Result<(), Error> {
        println!("Woken up!");
        println!("Message: {}", self.message);

        Ok(())
    }
}
```

Actors in stewart are "suspended" using "cooperative multitasking".
When your actor is woken up, its `process` callback is called.

Your actor processing doesn't necessarily mean there are messages available to be processed.
The only guarantee is that after a "signal" is sent to your actor, your actor will be processed when
the world gets to it.
If the entire system doesn't get dropped before that, of course.

## Adding an actor to a world

Creating an instance of your actor works the same as any other Rust type.
A stewart world is simply a collection of actor instances.

To keep your concrete actor type private, you can create a function to start a new instance of your
actor.

```rust
fn start_my_actor(world: &mut World) -> Result<Signal, Error> {
    // Create and insert the actor
    let actor = MyActor {};
    let id = world.insert("my-actor", actor)?;

    // Return the signal to wake it up
    let signal = world.signal(id);
    Ok(signal)
}
```

This function returns a `Signal` to wake up the actor to keep things simple, but typically you would
return for example a `Sender` instead.
The `message` module that implements `Sender` internally uses `Signal`.

You can send a `Signal` from anywhere on the same thread, it does not have to happen during actor
processing.

## Creating and processing a world

To put it all together, you need to have a world to add your actor to.
After signalling your actor, it will then be processed when calling `process` on the world.

```rust
fn main() -> Result<(), Error> {
    // Create a world with an actor
    let mut world = World::default();
    let signal = start_my_actor(&mut world)?;

    // Wake up the actor
    signal.send();
    world.process()?;

    Ok(())
}
```

This is the most basic, and perfectly functional, way to create an actor runtime.
If all your actors block internally and never have to wait on external data this works perfectly
fine as `process` will keep running until no actors are waiting for processing.

If you do however have things your world may need to wait on while not processing an actor, you
can implement this here.
For example, `stewart-mio` implements a world processing loop based on a mio event loop.
