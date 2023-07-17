use std::{
    rc::Rc,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
};

use anyhow::{bail, Error};
use stewart::{Actor, Context, Id, Mailbox, Sender, World};

pub fn given_mock_actor(world: &mut World) -> Result<ActorInfo, Error> {
    let actor = MockActor::default();

    let mailbox = actor.mailbox.clone();
    let count = actor.count.clone();
    let dropped = actor.dropped.clone();

    let id = world.create("mock-actor", actor);
    mailbox.register(id);

    let info = ActorInfo {
        id,
        sender: mailbox.sender(),
        count,
        dropped,
    };

    Ok(info)
}

pub fn given_fail_actor(world: &mut World) -> Result<ActorInfo, Error> {
    let mut actor = MockActor::default();
    actor.fail = true;

    let mailbox = actor.mailbox.clone();
    let count = actor.count.clone();
    let dropped = actor.dropped.clone();

    let id = world.create("fail-actor", actor);
    mailbox.register(id);

    let info = ActorInfo {
        id,
        sender: mailbox.sender(),
        count,
        dropped,
    };

    Ok(info)
}

pub struct ActorInfo {
    pub id: Id,
    pub sender: Sender<()>,
    pub count: Rc<AtomicUsize>,
    pub dropped: Rc<AtomicBool>,
}

#[derive(Default)]
struct MockActor {
    fail: bool,
    mailbox: Mailbox<()>,
    count: Rc<AtomicUsize>,
    dropped: Rc<AtomicBool>,
}

impl Actor for MockActor {
    fn process(&mut self, ctx: &mut Context) -> Result<(), Error> {
        if self.fail {
            bail!("mock intentional fail");
        }

        while self.mailbox.next().is_some() {
            self.count.fetch_add(1, Ordering::SeqCst);
        }

        // Stop after handling just one set of messages
        ctx.stop();

        Ok(())
    }
}

impl Drop for MockActor {
    fn drop(&mut self) {
        self.dropped.store(true, Ordering::SeqCst);
    }
}
