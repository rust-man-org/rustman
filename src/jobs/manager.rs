use tokio_util::sync::CancellationToken;

use super::kind::JobKind;

#[derive(Debug, Default)]
pub struct JobManager {
    slots: [Slot; JobKind::COUNT],
}

#[derive(Debug, Default)]
struct Slot {
    generation: u64,
    token: Option<CancellationToken>,
}

impl JobManager {
    pub fn start(&mut self, kind: JobKind) -> (u64, CancellationToken) {
        let slot = &mut self.slots[kind.index()];
        if let Some(previous) = slot.token.take() {
            previous.cancel();
        }
        slot.generation = slot.generation.wrapping_add(1);
        let token = CancellationToken::new();
        slot.token = Some(token.clone());
        (slot.generation, token)
    }

    /// True if no newer job of `kind` has started since `generation` was issued.
    pub fn is_current(&self, kind: JobKind, generation: u64) -> bool {
        self.slots[kind.index()].generation == generation
    }

    /// Cancels the running job of `kind`. Its generation goes stale too, so
    /// a result already on its way is dropped rather than applied.
    pub fn cancel(&mut self, kind: JobKind) {
        let slot = &mut self.slots[kind.index()];
        if let Some(token) = slot.token.take() {
            token.cancel();
        }
        slot.generation = slot.generation.wrapping_add(1);
    }

    pub fn cancel_all(&mut self) {
        for slot in &mut self.slots {
            if let Some(token) = slot.token.take() {
                token.cancel();
            }
        }
    }
}

impl Drop for JobManager {
    fn drop(&mut self) {
        self.cancel_all();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_cancelled_job_is_no_longer_current() {
        let mut jobs = JobManager::default();
        let (generation, token) = jobs.start(JobKind::Request);
        assert!(jobs.is_current(JobKind::Request, generation));
        jobs.cancel(JobKind::Request);
        assert!(token.is_cancelled());
        assert!(!jobs.is_current(JobKind::Request, generation), "a response already queued is dropped");
    }
}
