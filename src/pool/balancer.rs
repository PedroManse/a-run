use super::*;

#[derive(Debug)]
pub struct PoolBalancer<const N: usize> {
    pub(crate) total: AtomicUsize,
    runners: [PoolAnaliticRunner; N],
}

#[derive(Default, Debug)]
struct PoolAnaliticRunner {
    running: AtomicUsize,
}

pub(crate) struct PoolRunnerRef {
    pub(crate) id: usize,
    pub(crate) running: usize,
}

impl<const N: usize> PoolBalancer<N> {
    fn get_by_id(&self, id: usize) -> &PoolAnaliticRunner {
        &self.runners[id]
    }
    pub(crate) fn new() -> Self {
        PoolBalancer {
            total: AtomicUsize::default(),
            runners: [(); N].map(|_| PoolAnaliticRunner::default()),
        }
    }
    #[must_use]
    pub(crate) fn send(&self) -> PoolRunnerRef {
        let mut min = PoolRunnerRef {
            id: 0,
            running: usize::MAX,
        };
        for id in 0..N {
            let running = self
                .get_by_id(id)
                .running
                .load(std::sync::atomic::Ordering::SeqCst);
            if running == 0 {
                //eprintln!("[ACQ] Found idle runner #{id} (0 -> 1)");
                self.get_by_id(id)
                    .running
                    .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                self.total
                    .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                return PoolRunnerRef { id, running };
            } else if running < min.running {
                min = PoolRunnerRef { id, running };
            }
        }
        let _old = self
            .get_by_id(min.id)
            .running
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        self.total
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        //eprintln!("[ACQ] Best runner #{} ({} -> {})", min.id, _old, _old + 1);
        min
    }
    pub(crate) fn done(&self, id: usize) {
        let _old = self
            .get_by_id(id)
            .running
            .fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
        self.total
            .fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
        //eprintln!("[REL] runner #{} ({} -> {})", id, _old, _old - 1);
    }
}
