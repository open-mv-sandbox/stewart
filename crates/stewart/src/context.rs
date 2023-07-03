use crate::Id;

/// Bundle of contextual information.
///
/// Currently tracks:
/// - Current actor
#[derive(Default)]
pub struct Context {
    current: Option<Id>,
}

impl Context {
    /// Create a new root context, not associated with an actor.
    pub fn new() -> Self {
        Self { current: None }
    }

    pub(crate) fn with_current(&self, id: Id) -> Self {
        Self { current: Some(id) }
    }

    pub(crate) fn current(&self) -> Option<Id> {
        self.current
    }
}
