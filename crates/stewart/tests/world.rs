use std::{
    rc::Rc,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
};

use anyhow::{Context as _, Error};
use stewart::{Actor, Context, Schedule, Sender, State, World};
use tracing_test::traced_test;

#[test]
#[traced_test]
fn send_message_to_actor() -> Result<(), Error> {
    let mut world = World::default();
    let mut schedule = Schedule::default();

    let mut ctx = Context::root(&mut world, &mut schedule);
    let (parent, _child) = given_parent_child(&mut ctx)?;

    // Regular send
    when_sent_message_to(&mut world, &mut schedule, parent.sender.clone())?;
    assert_eq!(parent.count.load(Ordering::SeqCst), 1);

    // Actor should now be stopped, can't send to stopped
    when_sent_message_to(&mut world, &mut schedule, parent.sender)?;
    assert_eq!(parent.count.load(Ordering::SeqCst), 1);

    Ok(())
}

#[test]
#[traced_test]
fn stop_actors() -> Result<(), Error> {
    let mut world = World::default();
    let mut schedule = Schedule::default();

    let mut ctx = Context::root(&mut world, &mut schedule);
    let (parent, child) = given_parent_child(&mut ctx)?;

    // Stop parent
    parent.sender.send(&mut ctx, ());
    schedule.run_until_idle(&mut world)?;

    // Can't send message to child as it should be stopped too
    when_sent_message_to(&mut world, &mut schedule, child.sender)
        .context("test: failed to send message")?;
    assert_eq!(child.count.load(Ordering::SeqCst), 0);

    Ok(())
}

#[test]
#[traced_test]
fn not_started_removed() -> Result<(), Error> {
    let mut world = World::default();
    let mut schedule = Schedule::default();

    let mut ctx = Context::root(&mut world, &mut schedule);
    let (mut ctx, _) = ctx.create::<()>()?;

    // Create the child we use as a remove probe
    let (_, child) = given_actor(&mut ctx)?;

    // Process, this should remove the stale actor
    schedule.run_until_idle(&mut world)?;

    // Check drop happened, using the child actor
    assert!(child.dropped.load(Ordering::SeqCst));

    Ok(())
}

fn given_parent_child(ctx: &mut Context) -> Result<(ActorInfo, ActorInfo), Error> {
    let (mut ctx, parent) = given_actor(ctx)?;
    let (_, child) = given_actor(&mut ctx)?;

    Ok((parent, child))
}

fn given_actor<'a>(ctx: &'a mut Context) -> Result<(Context<'a>, ActorInfo), Error> {
    let (mut ctx, sender) = ctx.create()?;

    let instance = TestActor::default();
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

fn when_sent_message_to(
    world: &mut World,
    schedule: &mut Schedule,
    sender: Sender<()>,
) -> Result<(), Error> {
    let mut ctx = Context::root(world, schedule);
    sender.send(&mut ctx, ());

    schedule.run_until_idle(world)?;

    Ok(())
}

struct ActorInfo {
    sender: Sender<()>,
    count: Rc<AtomicUsize>,
    dropped: Rc<AtomicBool>,
}

#[derive(Default)]
struct TestActor {
    count: Rc<AtomicUsize>,
    dropped: Rc<AtomicBool>,
}

impl Actor for TestActor {
    type Message = ();

    fn process(&mut self, ctx: &mut Context, state: &mut State<Self>) -> Result<(), Error> {
        while let Some(_) = state.next() {
            self.count.fetch_add(1, Ordering::SeqCst);
        }

        ctx.stop()?;

        Ok(())
    }
}

impl Drop for TestActor {
    fn drop(&mut self) {
        self.dropped.store(true, Ordering::SeqCst);
    }
}
