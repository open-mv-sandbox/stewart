mod utils;

use std::sync::atomic::Ordering;

use anyhow::Error;
use stewart::{Context, World};
use tracing_test::traced_test;

use crate::utils::{
    given_fail_actor, given_mock_actor, given_parent_child, then_actor_dropped,
    when_sent_message_to, MockActor,
};

#[test]
#[traced_test]
fn send_message_to_actor() -> Result<(), Error> {
    let mut world = World::default();
    let mut cx = Context::root(&mut world);

    let (_, actor) = given_mock_actor(&mut cx)?;

    when_sent_message_to(&mut world, actor.sender.clone())?;
    assert_eq!(actor.count.load(Ordering::SeqCst), 1);

    then_actor_dropped(&actor);

    Ok(())
}

#[test]
#[traced_test]
fn stop_actors() -> Result<(), Error> {
    let mut world = World::default();
    let mut cx = Context::root(&mut world);

    let (parent, child) = given_parent_child(&mut cx)?;

    // Stop parent
    parent.sender.send(&mut cx, ());
    world.run_until_idle()?;

    then_actor_dropped(&child);

    Ok(())
}

#[test]
#[traced_test]
fn not_started_removed() -> Result<(), Error> {
    let mut world = World::default();
    let mut cx = Context::root(&mut world);

    let hnd = cx.create::<MockActor>("mock-actor")?;
    let mut cx = cx.with(hnd);

    let (_, actor) = given_mock_actor(&mut cx)?;

    // Process, this should remove the stale actor
    world.run_until_idle()?;

    then_actor_dropped(&actor);

    Ok(())
}

#[test]
#[traced_test]
fn failed_stopped() -> Result<(), Error> {
    let mut world = World::default();
    let mut cx = Context::root(&mut world);

    let (_, actor) = given_fail_actor(&mut cx)?;

    when_sent_message_to(&mut world, actor.sender.clone())?;

    then_actor_dropped(&actor);

    Ok(())
}
