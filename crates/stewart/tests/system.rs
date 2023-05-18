use std::{
    rc::Rc,
    sync::atomic::{AtomicUsize, Ordering},
};

use anyhow::{Context, Error};
use stewart::{ActorId, Addr, StartError, State, System, SystemOptions, World};
use tracing::{event, Level};
use tracing_test::traced_test;

#[test]
#[traced_test]
fn send_message_to_actor() -> Result<(), Error> {
    let mut world = World::new();
    let (parent, _child) = given_parent_child(&mut world)?;

    // Regular send
    when_sent_message_to(&mut world, parent.addr)?;
    assert_eq!(parent.count.load(Ordering::SeqCst), 1);

    // Can't send to stopped
    world.stop(parent.id)?;
    when_sent_message_to(&mut world, parent.addr)?;
    assert_eq!(parent.count.load(Ordering::SeqCst), 1);

    Ok(())
}

#[test]
#[traced_test]
fn stop_actors() -> Result<(), Error> {
    let mut world = World::new();
    let (parent, child) = given_parent_child(&mut world)?;

    world.stop(parent.id)?;

    // Can't send message to child as it should be stopped too
    when_sent_message_to(&mut world, child.addr).context("test: failed to send message")?;
    assert_eq!(child.count.load(Ordering::SeqCst), 0);

    Ok(())
}

#[test]
#[traced_test]
fn not_started_removed() -> Result<(), Error> {
    let mut world = World::new();

    let actor = world.create(None)?;
    let system = world.register(TestActorSystem, actor, SystemOptions::default());

    // Process, this should remove the stale actor
    world.run_until_idle()?;

    // Make sure we can't start
    let result = world.start(actor, system, TestActor::default());
    if let Err(StartError::ActorNotFound) = result {
        event!(Level::INFO, "correct result");
    } else {
        assert!(false, "incorret result: {:?}", result);
    }

    Ok(())
}

fn given_parent_child(world: &mut World) -> Result<(ActorInfo, ActorInfo), Error> {
    let parent = given_actor(world, None)?;
    let child = given_actor(world, Some(parent.id))?;

    Ok((parent, child))
}

fn given_actor<'a>(world: &mut World, parent: Option<ActorId>) -> Result<ActorInfo, Error> {
    let actor = world.create(parent)?;
    let system = world.register(TestActorSystem, actor, SystemOptions::default());

    let instance = TestActor::default();
    let count = instance.count.clone();
    world.start(actor, system, instance)?;

    let info = ActorInfo {
        id: actor,
        addr: Addr::new(actor),
        count,
    };

    Ok(info)
}

fn when_sent_message_to(world: &mut World, addr: Addr<()>) -> Result<(), Error> {
    world.send(addr, ());
    world.run_until_idle()?;
    Ok(())
}

struct ActorInfo {
    id: ActorId,
    addr: Addr<()>,
    count: Rc<AtomicUsize>,
}

struct TestActorSystem;

impl System for TestActorSystem {
    type Instance = TestActor;
    type Message = ();

    fn process(&mut self, _world: &mut World, state: &mut State<Self>) -> Result<(), Error> {
        while let Some((id, _)) = state.next() {
            let instance = state.get_mut(id).context("failed to get instance")?;
            instance.count.fetch_add(1, Ordering::SeqCst);
        }

        Ok(())
    }
}

#[derive(Default)]
struct TestActor {
    count: Rc<AtomicUsize>,
}
