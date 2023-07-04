mod mock;

use std::sync::atomic::Ordering;

use anyhow::{Context as _, Error};
use stewart::{Handler, Id, World};

pub use mock::{given_fail_actor, given_mock_actor};

use self::mock::ActorInfo;

pub fn given_parent_child(world: &mut World) -> Result<(ActorInfo, ActorInfo), Error> {
    let parent = given_mock_actor(world, Id::none())?;
    let child = given_mock_actor(world, parent.id)?;

    Ok((parent, child))
}

pub fn when_sent_message_to(world: &mut World, handler: Handler<()>) -> Result<(), Error> {
    handler.handle(world, ())?;

    world
        .run_until_idle()
        .context("failed to process after sending")?;

    Ok(())
}

pub fn then_actor_dropped(actor: &ActorInfo) {
    assert!(actor.dropped.load(Ordering::SeqCst), "actor not dropped");
}
