mod utils;

use anyhow::Error;
use stewart::World;
use tracing_test::traced_test;

use crate::utils::{
    given_fail_actor, given_mock_actor, then_actor_dropped, then_messages_received,
    when_sent_message_to,
};

#[test]
#[traced_test]
fn send_message_to_actor() -> Result<(), Error> {
    let mut world = World::default();

    let actor = given_mock_actor(&mut world)?;

    when_sent_message_to(&mut world, &actor.sender)?;

    then_messages_received(&actor, 1);

    Ok(())
}

#[test]
#[traced_test]
fn stop_actor() -> Result<(), Error> {
    let mut world = World::default();

    let actor = given_mock_actor(&mut world)?;

    // This will stop the actor
    when_sent_message_to(&mut world, &actor.sender)?;

    then_actor_dropped(&actor);

    Ok(())
}

#[test]
#[traced_test]
fn failed_stopped() -> Result<(), Error> {
    let mut world = World::default();

    let actor = given_fail_actor(&mut world)?;

    when_sent_message_to(&mut world, &actor.sender)?;

    then_actor_dropped(&actor);

    Ok(())
}
