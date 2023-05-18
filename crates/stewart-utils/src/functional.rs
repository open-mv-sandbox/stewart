use std::{marker::PhantomData, sync::atomic::AtomicPtr};

use anyhow::{Context as _, Error};
use stewart::{ActorId, Addr, State, System, SystemOptions, World};

pub fn when<F, M>(
    world: &mut World,
    parent: Option<ActorId>,
    options: SystemOptions,
    function: F,
) -> Result<Addr<M>, Error>
where
    F: FnMut(&mut World, ActorId, M) -> Result<bool, Error> + 'static,
    M: 'static,
{
    let id = world.create(parent)?;

    // In-line create a new system
    let system: WhenSystem<F, M> = WhenSystem { _w: PhantomData };
    let system = world.register(system, id, options);

    // Start the actor
    let actor = When::<F, M> {
        function,
        _a: PhantomData,
    };
    world.start(id, system, actor)?;

    Ok(Addr::new(id))
}

pub fn map<F, I, O>(
    world: &mut World,
    parent: Option<ActorId>,
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
        SystemOptions::high_priority(),
        move |world, _id, message| {
            let message = (function)(message);
            world.send(target, message);
            Ok(true)
        },
    )?;

    Ok(addr)
}

pub fn map_once<F, I, O>(
    world: &mut World,
    parent: Option<ActorId>,
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
        SystemOptions::high_priority(),
        move |world, _id, message| {
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

struct WhenSystem<F, M> {
    _w: PhantomData<When<F, M>>,
}

impl<F, M> System for WhenSystem<F, M>
where
    F: FnMut(&mut World, ActorId, M) -> Result<bool, Error> + 'static,
    M: 'static,
{
    type Instance = When<F, M>;
    type Message = M;

    fn process(&mut self, world: &mut World, state: &mut State<Self>) -> Result<(), Error> {
        while let Some((id, message)) = state.next() {
            let instance = state.get_mut(id).context("failed to get instance")?;
            let result = (instance.function)(world, id, message)?;

            if !result {
                world.stop(id)?;
            }
        }

        Ok(())
    }
}

struct When<F, M> {
    function: F,
    _a: PhantomData<AtomicPtr<M>>,
}
