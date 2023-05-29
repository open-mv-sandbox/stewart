use std::{
    rc::Rc,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
};

use anyhow::Error;
use stewart::{Actor, Context, Schedule, Sender, State, World};

pub fn given_parent_child(ctx: &mut Context) -> Result<(ActorInfo, ActorInfo), Error> {
    let (mut ctx, parent) = given_actor(ctx)?;
    let (_, child) = given_actor(&mut ctx)?;

    Ok((parent, child))
}

pub fn given_actor<'a>(ctx: &'a mut Context) -> Result<(Context<'a>, ActorInfo), Error> {
    let (mut ctx, sender) = ctx.create("mock-actor")?;

    let instance = MockActor::default();
    let count = instance.count.clone();
    let dropped = instance.dropped.clone();
    ctx.start(instance)?;

    let info = ActorInfo {
        sender,
        count,
        dropped,
    };

    Ok((ctx, info))
}

pub fn when_sent_message_to(
    world: &mut World,
    schedule: &mut Schedule,
    sender: Sender<()>,
) -> Result<(), Error> {
    let mut ctx = Context::root(world, schedule);
    sender.send(&mut ctx, ());

    schedule.run_until_idle(world)?;

    Ok(())
}

pub struct ActorInfo {
    pub sender: Sender<()>,
    pub count: Rc<AtomicUsize>,
    pub dropped: Rc<AtomicBool>,
}

#[derive(Default)]
struct MockActor {
    count: Rc<AtomicUsize>,
    dropped: Rc<AtomicBool>,
}

impl Actor for MockActor {
    type Message = ();

    fn process(&mut self, _ctx: &mut Context, state: &mut State<Self>) -> Result<(), Error> {
        while let Some(_) = state.next() {
            self.count.fetch_add(1, Ordering::SeqCst);
        }

        state.stop();

        Ok(())
    }
}

impl Drop for MockActor {
    fn drop(&mut self) {
        self.dropped.store(true, Ordering::SeqCst);
    }
}
