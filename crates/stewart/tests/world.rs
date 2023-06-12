mod utils;

use std::sync::atomic::Ordering;

use anyhow::Error;
use stewart::{Context, Schedule, World};
use tracing_test::traced_test;

use crate::utils::{
    given_fail_actor, given_mock_actor, given_parent_child, then_actor_dropped,
    when_sent_message_to,
};

#[test]
#[traced_test]
fn send_message_to_actor() -> Result<(), Error> {
    let mut world = World::default();
    let mut schedule = Schedule::default();
    let mut ctx = Context::root(&mut world, &mut schedule);

    let (_, actor) = given_mock_actor(&mut ctx)?;

    when_sent_message_to(&mut world, &mut schedule, actor.sender.clone())?;
    assert_eq!(actor.count.load(Ordering::SeqCst), 1);

    then_actor_dropped(&actor);

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

    then_actor_dropped(&child);

    Ok(())
}

#[test]
#[traced_test]
fn not_started_removed() -> Result<(), Error> {
    let mut world = World::default();
    let mut schedule = Schedule::default();
    let mut ctx = Context::root(&mut world, &mut schedule);

    let (mut ctx, _) = ctx.create::<()>("mock-actor")?;

    let (_, actor) = given_mock_actor(&mut ctx)?;

    // Process, this should remove the stale actor
    schedule.run_until_idle(&mut world)?;

    then_actor_dropped(&actor);

    Ok(())
}

#[test]
#[traced_test]
fn failed_stopped() -> Result<(), Error> {
    let mut world = World::default();
    let mut schedule = Schedule::default();
    let mut ctx = Context::root(&mut world, &mut schedule);

    let (_, actor) = given_fail_actor(&mut ctx)?;

    when_sent_message_to(&mut world, &mut schedule, actor.sender.clone())?;

    then_actor_dropped(&actor);

    Ok(())
}
