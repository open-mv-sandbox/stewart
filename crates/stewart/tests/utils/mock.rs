use std::{
    rc::Rc,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
};

use anyhow::{bail, Error};
use stewart::{Actor, Context, Handler, Id, World};

pub fn given_mock_actor(world: &mut World) -> Result<ActorInfo, Error> {
    let id = world.create("mock-actor")?;

    let instance = MockActor::default();

    let count = instance.count.clone();
    let dropped = instance.dropped.clone();
    world.start(id, instance)?;

    let info = ActorInfo {
        id,
        handler: Handler::to(id),
        count,
        dropped,
    };

    Ok(info)
}

pub fn given_fail_actor(world: &mut World) -> Result<ActorInfo, Error> {
    let id = world.create("fail-actor")?;

    let mut instance = MockActor::default();
    instance.fail = true;

    let count = instance.count.clone();
    let dropped = instance.dropped.clone();
    world.start(id, instance)?;

    let info = ActorInfo {
        id,
        handler: Handler::to(id),
        count,
        dropped,
    };

    Ok(info)
}

pub struct ActorInfo {
    pub id: Id,
    pub handler: Handler<()>,
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

    fn process(&mut self, _world: &mut World, mut cx: Context<Self>) -> Result<(), Error> {
        if self.fail {
            bail!("mock intentional fail");
        }

        while cx.next_message().is_some() {
            self.count.fetch_add(1, Ordering::SeqCst);
        }

        // Stop after handling just one set of messages
        cx.stop();

        Ok(())
    }
}

impl Drop for MockActor {
    fn drop(&mut self) {
        self.dropped.store(true, Ordering::SeqCst);
    }
}
