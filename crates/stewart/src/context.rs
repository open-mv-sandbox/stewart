use thunderdome::Index;

use crate::Handle;

/// Bundle of contextual information for operations.
///
/// Currently tracks:
/// - Current actor, for creation of child actors.
///
/// This can in the future contain more information.
pub struct Context {
    current: Option<Index>,
}

impl Context {
    /// Create a 'root' context, not associated with an actor.
    pub fn root() -> Self {
        Self { current: None }
    }

    /// TODO: Shouldn't be public
    pub fn with<A>(&self, hnd: Handle<A>) -> Self {
        Self {
            current: Some(hnd.index),
        }
    }

    pub(crate) fn current(&self) -> Option<Index> {
        self.current
    }
}
