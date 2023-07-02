use std::{
    rc::Rc,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
};

use anyhow::{bail, Error};
use stewart::{Actor, Context, Handler, State, World};

pub fn given_mock_actor(world: &mut World, cx: &Context) -> Result<(Context, ActorInfo), Error> {
    let (cx, id) = world.create(cx, "mock-actor")?;

    let instance = MockActor::default();

    let count = instance.count.clone();
    let dropped = instance.dropped.clone();
    world.start(id, instance)?;

    let info = ActorInfo {
        sender: Handler::to(id),
        count,
        dropped,
    };

    Ok((cx, info))
}

pub fn given_fail_actor(world: &mut World, cx: &Context) -> Result<(Context, ActorInfo), Error> {
    let (cx, id) = world.create(cx, "fail-actor")?;

    let mut instance = MockActor::default();
    instance.fail = true;

    let count = instance.count.clone();
    let dropped = instance.dropped.clone();
    world.start(id, instance)?;

    let info = ActorInfo {
        sender: Handler::to(id),
        count,
        dropped,
    };

    Ok((cx, info))
}

pub struct ActorInfo {
    pub sender: Handler<()>,
    pub count: Rc<AtomicUsize>,
    pub dropped: Rc<AtomicBool>,
}

#[derive(Default)]
struct MockActor {
    count: Rc<AtomicUsize>,
    dropped: Rc<AtomicBool>,
    fail: bool,
}

impl Actor for MockActor {
    type Message = ();

    fn process(
        &mut self,
        _world: &mut World,
        _cx: &Context,
        state: &mut State<Self>,
    ) -> Result<(), Error> {
        if self.fail {
            bail!("mock intentional fail");
        }

        while let Some(_) = state.next() {
            self.count.fetch_add(1, Ordering::SeqCst);
        }

        // Stop after handling just one set of messages
        state.stop();

        Ok(())
    }
}

impl Drop for MockActor {
    fn drop(&mut self) {
        self.dropped.store(true, Ordering::SeqCst);
    }
}
