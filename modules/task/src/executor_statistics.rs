#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ExecutorStatisticsSnapshot {
    pub busy_ticks: u64,
    pub idle_ticks: u64,
}

impl ExecutorStatisticsSnapshot {
    pub const fn new(busy_ticks: u64, idle_ticks: u64) -> Self {
        Self {
            busy_ticks,
            idle_ticks,
        }
    }

    pub fn idle_ratio_basis_points(&self) -> u16 {
        let total = self.busy_ticks.saturating_add(self.idle_ticks);

        if total == 0 {
            return 0;
        }

        let ratio = self.idle_ticks.saturating_mul(10_000) / total;

        ratio.min(10_000) as u16
    }
}

pub trait ExecutorWithStatistics {
    fn spawner(&'static self) -> embassy_executor::Spawner;

    fn statistics_snapshot(&self) -> Option<ExecutorStatisticsSnapshot>;
}
