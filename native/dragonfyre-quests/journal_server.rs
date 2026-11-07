use super::{Game, Integrated};

impl Integrated {
    /// Run a quest task between server ticks without blocking the window.
    pub(in crate::game) fn dragonfyre_post(
        &self,
        work: impl FnOnce(&mut Game<'_>) + Send + 'static,
    ) -> bool {
        self.tasks.send(Box::new(work)).is_ok()
    }
}

