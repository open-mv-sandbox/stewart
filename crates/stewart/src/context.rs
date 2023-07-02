use std::{
    any::{Any, TypeId},
    collections::HashMap,
    rc::Rc,
};

use crate::Id;

/// Bundle of contextual information.
///
/// Currently tracks:
/// - Current actor
/// - Blackboard of contextual data
#[derive(Default)]
pub struct Context {
    blackboard: Rc<Blackboard>,
    current: Option<Id>,
}

impl Context {
    /// Create a new root context, not associated with an actor.
    pub fn new(blackboard: Blackboard) -> Self {
        Self {
            blackboard: Rc::new(blackboard),
            current: None,
        }
    }

    pub(crate) fn with_current(&self, id: Id) -> Self {
        Self {
            blackboard: self.blackboard.clone(),
            current: Some(id),
        }
    }

    /// Get the blackboard of the current context.
    pub fn blackboard(&self) -> &Blackboard {
        &self.blackboard
    }

    pub(crate) fn current(&self) -> Option<Id> {
        self.current
    }
}

/// Map of values of types, used for contextual information.
#[derive(Default)]
pub struct Blackboard {
    values: HashMap<TypeId, Box<dyn Any>>,
}

impl Blackboard {
    /// Set a value.
    pub fn set<T>(&mut self, value: T)
    where
        T: 'static,
    {
        let type_id = TypeId::of::<T>();
        let value = Box::new(value);
        self.values.insert(type_id, value);
    }

    /// Get a value.
    pub fn get<T>(&self) -> Option<&T>
    where
        T: 'static,
    {
        let type_id = TypeId::of::<T>();
        let value = self.values.get(&type_id);
        value.and_then(|v| v.downcast_ref())
    }
}
