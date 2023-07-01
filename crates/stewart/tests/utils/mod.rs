mod mock;

use std::sync::atomic::Ordering;

use anyhow::{Context as _, Error};
use stewart::{Context, Sender, World};

pub use mock::{given_fail_actor, given_mock_actor};

use self::mock::ActorInfo;

pub fn given_parent_child(cx: &mut Context) -> Result<(ActorInfo, ActorInfo), Error> {
    let (mut cx, parent) = given_mock_actor(cx)?;
    let (_, child) = given_mock_actor(&mut cx)?;

    Ok((parent, child))
}

pub fn when_sent_message_to(world: &mut World, sender: Sender<()>) -> Result<(), Error> {
    let mut cx = Context::root(world);
    sender.send(&mut cx, ());

    world
        .run_until_idle()
        .context("failed to process after sending")?;

    Ok(())
}

pub fn then_actor_dropped(actor: &ActorInfo) {
    assert!(actor.dropped.load(Ordering::SeqCst), "actor not dropped");
}
