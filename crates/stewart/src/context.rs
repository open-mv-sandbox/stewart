use crate::Id;

/// Bundle of contextual information for operations.
///
/// Currently tracks:
/// - Current actor, for creation of child actors.
///
/// This can in the future contain more information.
pub struct Context {
    current: Option<Id>,
}

impl Context {
    /// Create a 'root' context, not associated with an actor.
    pub fn root() -> Self {
        Self { current: None }
    }

    /// TODO: Maybe shouldn't be public?
    pub fn with(&self, id: Id) -> Self {
        Self { current: Some(id) }
    }

    pub(crate) fn current(&self) -> Option<Id> {
        self.current
    }
}
