mod utils;

use std::sync::atomic::Ordering;

use anyhow::Error;
use stewart::{Id, World};
use tracing_test::traced_test;

use crate::utils::{
    given_fail_actor, given_mock_actor, given_parent_child, then_actor_dropped,
    when_sent_message_to,
};

#[test]
#[traced_test]
fn send_message_to_actor() -> Result<(), Error> {
    let mut world = World::default();

    let actor = given_mock_actor(&mut world, Id::none())?;

    when_sent_message_to(&mut world, actor.handler.clone())?;
    assert_eq!(actor.count.load(Ordering::SeqCst), 1);

    then_actor_dropped(&actor);

    Ok(())
}

#[test]
#[traced_test]
fn stop_actors() -> Result<(), Error> {
    let mut world = World::default();

    let (parent, child) = given_parent_child(&mut world)?;

    // Stop parent
    parent.handler.handle(&mut world, ())?;
    world.run_until_idle()?;

    then_actor_dropped(&child);

    Ok(())
}

#[test]
#[traced_test]
fn not_started_removed() -> Result<(), Error> {
    let mut world = World::default();

    let id = world.create(Id::none(), "mock-actor")?;

    let actor = given_mock_actor(&mut world, id)?;

    // Process, this should remove the stale actor
    world.run_until_idle()?;

    then_actor_dropped(&actor);

    Ok(())
}

#[test]
#[traced_test]
fn failed_stopped() -> Result<(), Error> {
    let mut world = World::default();

    let actor = given_fail_actor(&mut world, Id::none())?;

    when_sent_message_to(&mut world, actor.handler.clone())?;

    then_actor_dropped(&actor);

    Ok(())
}
