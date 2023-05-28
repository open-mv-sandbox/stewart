use std::{
    rc::Rc,
    sync::atomic::{AtomicUsize, Ordering},
};

use anyhow::{Context as _, Error};
use stewart::{Actor, Context, Sender, StartError, State, World};
use tracing::{event, Level};
use tracing_test::traced_test;

#[test]
#[traced_test]
fn send_message_to_actor() -> Result<(), Error> {
    let mut world = World::new();
    let mut ctx = world.root();
    let (parent, _child) = given_parent_child(&mut ctx)?;

    // Regular send
    when_sent_message_to(&mut ctx, parent.sender.clone())?;
    assert_eq!(parent.count.load(Ordering::SeqCst), 1);

    // Actor should now be stopped, can't send to stopped
    when_sent_message_to(&mut ctx, parent.sender)?;
    assert_eq!(parent.count.load(Ordering::SeqCst), 1);

    Ok(())
}

#[test]
#[traced_test]
fn stop_actors() -> Result<(), Error> {
    let mut world = World::new();
    let mut ctx = world.root();
    let (parent, child) = given_parent_child(&mut ctx)?;

    // Stop parent
    parent.sender.send(&mut ctx, ());
    ctx.run_until_idle()?;

    // Can't send message to child as it should be stopped too
    when_sent_message_to(&mut ctx, child.sender).context("test: failed to send message")?;
    assert_eq!(child.count.load(Ordering::SeqCst), 0);

    Ok(())
}

#[test]
#[traced_test]
fn not_started_removed() -> Result<(), Error> {
    let mut world = World::new();
    let mut ctx = world.root();

    let (mut ctx, _) = ctx.create::<()>()?;

    // Process, this should remove the stale actor
    ctx.run_until_idle()?;

    // Make sure we can't start
    let result = ctx.start(TestActor::default());
    if let Err(StartError::ActorNotFound) = result {
        event!(Level::INFO, "correct result");
    } else {
        assert!(false, "incorret result: {:?}", result);
    }

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
    ctx.start(instance)?;

    let info = ActorInfo { sender, count };

    Ok((ctx, info))
}

fn when_sent_message_to(ctx: &mut Context, sender: Sender<()>) -> Result<(), Error> {
    sender.send(ctx, ());
    ctx.run_until_idle()?;
    Ok(())
}

struct ActorInfo {
    sender: Sender<()>,
    count: Rc<AtomicUsize>,
}

#[derive(Default)]
struct TestActor {
    count: Rc<AtomicUsize>,
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
