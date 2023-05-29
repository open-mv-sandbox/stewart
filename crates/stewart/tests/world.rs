mod utils;

use std::sync::atomic::Ordering;

use anyhow::{Context as _, Error};
use stewart::{Context, Schedule, World};
use tracing_test::traced_test;

use crate::utils::{given_actor, given_parent_child, when_sent_message_to};

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
    let (mut ctx, _) = ctx.create::<()>("mock-actor")?;

    // Create the child we use as a remove probe
    let (_, child) = given_actor(&mut ctx)?;

    // Process, this should remove the stale actor
    schedule.run_until_idle(&mut world)?;

    // Check drop happened, using the child actor
    assert!(child.dropped.load(Ordering::SeqCst));

    Ok(())
}
