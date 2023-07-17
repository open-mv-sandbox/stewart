mod utils;

use std::sync::atomic::Ordering;

use anyhow::Error;
use stewart::World;
use tracing_test::traced_test;

use crate::utils::{given_fail_actor, given_mock_actor, then_actor_dropped, when_sent_message_to};

#[test]
#[traced_test]
fn send_message_to_actor() -> Result<(), Error> {
    let mut world = World::default();

    let actor = given_mock_actor(&mut world)?;

    when_sent_message_to(&mut world, actor.sender.clone())?;
    assert_eq!(actor.count.load(Ordering::SeqCst), 1);

    then_actor_dropped(&actor);

    Ok(())
}

#[test]
#[traced_test]
fn stop_actors() -> Result<(), Error> {
    let mut world = World::default();

    let actor = given_mock_actor(&mut world)?;

    // Stop actor
    actor.sender.send(&mut world, ())?;
    world.run_until_idle()?;

    then_actor_dropped(&actor);

    Ok(())
}

#[test]
#[traced_test]
fn failed_stopped() -> Result<(), Error> {
    let mut world = World::default();

    let actor = given_fail_actor(&mut world)?;

    when_sent_message_to(&mut world, actor.sender.clone())?;

    then_actor_dropped(&actor);

    Ok(())
}
