mod mock;

use std::sync::atomic::Ordering;

use anyhow::{Context as _, Error};
use stewart::{utils::Handler, Context, World};

pub use mock::{given_fail_actor, given_mock_actor};

use self::mock::ActorInfo;

pub fn given_parent_child(
    world: &mut World,
    cx: &Context,
) -> Result<(ActorInfo, ActorInfo), Error> {
    let (cx, parent) = given_mock_actor(world, cx)?;
    let (_, child) = given_mock_actor(world, &cx)?;

    Ok((parent, child))
}

pub fn when_sent_message_to(world: &mut World, sender: Handler<()>) -> Result<(), Error> {
    sender.handle(world, ());

    world
        .run_until_idle()
        .context("failed to process after sending")?;

    Ok(())
}

pub fn then_actor_dropped(actor: &ActorInfo) {
    assert!(actor.dropped.load(Ordering::SeqCst), "actor not dropped");
}
