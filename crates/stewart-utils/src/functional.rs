use std::{marker::PhantomData, sync::atomic::AtomicPtr};

use anyhow::{Context as _, Error};
use stewart::{Actor, Id, Addr, Options, State, World};

pub fn when<F, M>(
    world: &mut World,
    parent: Option<Id>,
    options: Options,
    function: F,
) -> Result<Addr<M>, Error>
where
    F: FnMut(&mut World, M) -> Result<bool, Error> + 'static,
    M: 'static,
{
    let id = world.create(parent, options)?;

    // Start the actor
    let actor = When::<F, M> {
        id,
        function,
        _a: PhantomData,
    };
    world.start(id, actor)?;

    Ok(Addr::new(id))
}

pub fn map<F, I, O>(
    world: &mut World,
    parent: Option<Id>,
    target: Addr<O>,
    mut function: F,
) -> Result<Addr<I>, Error>
where
    F: FnMut(I) -> O + 'static,
    I: 'static,
    O: 'static,
{
    let addr = when(
        world,
        parent,
        Options::high_priority(),
        move |world, message| {
            let message = (function)(message);
            world.send(target, message);
            Ok(true)
        },
    )?;

    Ok(addr)
}

pub fn map_once<F, I, O>(
    world: &mut World,
    parent: Option<Id>,
    target: Addr<O>,
    function: F,
) -> Result<Addr<I>, Error>
where
    F: FnOnce(I) -> O + 'static,
    I: 'static,
    O: 'static,
{
    let mut function = Some(function);
    let addr = when(
        world,
        parent,
        Options::high_priority(),
        move |world, message| {
            let function = function
                .take()
                .context("map_once actor called more than once")?;
            let message = (function)(message);
            world.send(target, message);
            Ok(false)
        },
    )?;

    Ok(addr)
}

struct When<F, M> {
    id: Id,
    function: F,
    _a: PhantomData<AtomicPtr<M>>,
}

impl<F, M> Actor for When<F, M>
where
    F: FnMut(&mut World, M) -> Result<bool, Error> + 'static,
    M: 'static,
{
    type Message = M;

    fn process(&mut self, world: &mut World, state: &mut State<Self>) -> Result<(), Error> {
        while let Some(message) = state.next() {
            let result = (self.function)(world, message)?;

            if !result {
                world.stop(self.id)?;
            }
        }

        Ok(())
    }
}
