mod mock;

use std::sync::atomic::Ordering;

use anyhow::{Context as _, Error};
use stewart::{Sender, World};

pub use mock::{given_fail_actor, given_mock_actor};

use self::mock::ActorInfo;

pub fn when_sent_message_to(world: &mut World, sender: Sender<()>) -> Result<(), Error> {
    sender.send(world, ())?;

    world
        .run_until_idle()
        .context("failed to process after sending")?;

    Ok(())
}

pub fn then_actor_dropped(actor: &ActorInfo) {
    assert!(actor.dropped.load(Ordering::SeqCst), "actor not dropped");
}
