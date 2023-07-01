use std::{
    rc::Rc,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
};

use anyhow::{bail, Error};
use stewart::{Actor, Context, Sender, State};

pub fn given_mock_actor<'a>(cx: &'a mut Context) -> Result<(Context<'a>, ActorInfo), Error> {
    let hnd = cx.create("mock-actor")?;

    let instance = MockActor::default();

    let count = instance.count.clone();
    let dropped = instance.dropped.clone();
    cx.start(hnd, instance)?;

    let info = ActorInfo {
        sender: hnd.sender(),
        count,
        dropped,
    };

    let cx = cx.with(hnd);
    Ok((cx, info))
}

pub fn given_fail_actor<'a>(cx: &'a mut Context) -> Result<(Context<'a>, ActorInfo), Error> {
    let hnd = cx.create("fail-actor")?;

    let mut instance = MockActor::default();
    instance.fail = true;

    let count = instance.count.clone();
    let dropped = instance.dropped.clone();
    cx.start(hnd, instance)?;

    let info = ActorInfo {
        sender: hnd.sender(),
        count,
        dropped,
    };

    let cx = cx.with(hnd);
    Ok((cx, info))
}

pub struct ActorInfo {
    pub sender: Sender<()>,
    pub count: Rc<AtomicUsize>,
    pub dropped: Rc<AtomicBool>,
}

#[derive(Default)]
pub struct MockActor {
    count: Rc<AtomicUsize>,
    dropped: Rc<AtomicBool>,
    fail: bool,
}

impl Actor for MockActor {
    type Message = ();

    fn process(&mut self, _cx: &mut Context, state: &mut State<Self>) -> Result<(), Error> {
        if self.fail {
            bail!("mock intentional fail");
        }

        while let Some(_) = state.next() {
            self.count.fetch_add(1, Ordering::SeqCst);
        }

        // Stop after handling just one set of messages
        state.stop();

        Ok(())
    }
}

impl Drop for MockActor {
    fn drop(&mut self) {
        self.dropped.store(true, Ordering::SeqCst);
    }
}
