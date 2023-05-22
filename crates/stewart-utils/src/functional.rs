use std::{marker::PhantomData, sync::atomic::AtomicPtr};

use anyhow::{Context as _, Error};
use stewart::{Actor, Addr, Context, Options, State, World};

pub fn when<F, M>(ctx: &mut Context, options: Options, function: F) -> Result<Addr<M>, Error>
where
    F: FnMut(&mut World, M) -> Result<bool, Error> + 'static,
    M: 'static,
{
    let mut ctx = ctx.create(options)?;

    // Start the actor
    let actor = When::<F, M> {
        function,
        _a: PhantomData,
    };
    ctx.start(actor)?;

    Ok(ctx.addr()?)
}

pub fn map<F, I, O>(ctx: &mut Context, target: Addr<O>, mut function: F) -> Result<Addr<I>, Error>
where
    F: FnMut(I) -> O + 'static,
    I: 'static,
    O: 'static,
{
    let addr = when(ctx, Options::high_priority(), move |world, message| {
        let message = (function)(message);
        world.send(target, message);
        Ok(true)
    })?;

    Ok(addr)
}

pub fn map_once<F, I, O>(ctx: &mut Context, target: Addr<O>, function: F) -> Result<Addr<I>, Error>
where
    F: FnOnce(I) -> O + 'static,
    I: 'static,
    O: 'static,
{
    let mut function = Some(function);
    let addr = when(ctx, Options::high_priority(), move |world, message| {
        let function = function
            .take()
            .context("map_once actor called more than once")?;
        let message = (function)(message);
        world.send(target, message);
        Ok(false)
    })?;

    Ok(addr)
}

struct When<F, M> {
    function: F,
    _a: PhantomData<AtomicPtr<M>>,
}

impl<F, M> Actor for When<F, M>
where
    F: FnMut(&mut World, M) -> Result<bool, Error> + 'static,
    M: 'static,
{
    type Message = M;

    fn process(&mut self, ctx: &mut Context, state: &mut State<Self>) -> Result<(), Error> {
        while let Some(message) = state.next() {
            let result = (self.function)(ctx, message)?;

            if !result {
                ctx.stop()?;
            }
        }

        Ok(())
    }
}
