// Spawner module - handles spawner registration and selection

use super::*;

use alloc::collections::BTreeMap;
use embassy_executor::Spawner;

pub type SpawnerIdentifier = usize;

const IDLE_RATIO_GAP_THRESHOLD_BASIS_POINTS: u16 = 1_000;

#[derive(Clone, Copy, Debug)]
pub(crate) struct SpawnerCandidate {
    pub(crate) identifier: SpawnerIdentifier,
    pub(crate) telemetry: Option<ExecutorStatisticsSnapshot>,
    pub(crate) task_count: usize,
}

impl Manager {
    pub fn register_executor<ExecutorType: ExecutorWithStatistics>(
        &'static self,
        executor: &'static ExecutorType,
    ) -> Result<SpawnerIdentifier> {
        self.register_spawner_with_executor(executor.spawner(), Some(executor))
    }

    pub fn register_spawner(&'static self, spawner: Spawner) -> Result<SpawnerIdentifier> {
        self.register_spawner_with_executor(spawner, None)
    }

    pub fn register_spawner_with_executor(
        &'static self,
        spawner: Spawner,
        executor: Option<&'static dyn ExecutorWithStatistics>,
    ) -> Result<SpawnerIdentifier> {
        let mut inner = embassy_futures::block_on(self.0.write());

        let identifier = Self::find_first_available_identifier(
            &inner.spawners,
            (usize::MIN..usize::MAX).step_by(1),
        )
        .ok_or(Error::TooManySpawners)?;

        if inner.spawners.insert(identifier, spawner).is_some() {
            unreachable!("Spawner identifier already exists");
        }

        inner.executors.insert(identifier, executor);

        Ok(identifier)
    }

    pub fn unregister_spawner(&'static self, identifier: SpawnerIdentifier) -> Result<()> {
        let mut inner = embassy_futures::block_on(self.0.write());

        inner
            .spawners
            .remove(&identifier)
            .ok_or(Error::NoSpawnerAvailable)?;

        inner.executors.remove(&identifier);

        Ok(())
    }

    /// Select the best spawner for a new task using load balancing algorithm
    pub(crate) fn select_best_spawner(inner: &Inner) -> Result<SpawnerIdentifier> {
        if inner.spawners.is_empty() {
            return Err(Error::NoSpawnerAvailable);
        }

        let mut task_count_per_spawner = BTreeMap::new();

        for identifier in inner.spawners.keys() {
            task_count_per_spawner.insert(*identifier, 0);
        }

        for metadata in inner.tasks.values() {
            if let Some(load) = task_count_per_spawner.get_mut(&metadata.spawner_identifier) {
                *load += 1;
            }
        }

        let mut candidates = alloc::vec::Vec::new();

        for (identifier, task_count) in task_count_per_spawner {
            let telemetry = inner
                .executors
                .get(&identifier)
                .and_then(|executor| executor.and_then(|executor| executor.statistics_snapshot()));

            candidates.push(SpawnerCandidate {
                identifier,
                telemetry,
                task_count,
            });
        }

        Self::choose_spawner_from_candidates(&candidates)
    }

    pub(crate) fn choose_spawner_from_candidates(
        candidates: &[SpawnerCandidate],
    ) -> Result<SpawnerIdentifier> {
        if candidates.is_empty() {
            return Err(Error::NoSpawnerAvailable);
        }

        let mut best_telemetry_candidate = None;
        let mut second_best_idle_ratio = 0u16;

        for candidate in candidates {
            let Some(snapshot) = candidate.telemetry else {
                continue;
            };

            let idle_ratio = snapshot.idle_ratio_basis_points();

            if let Some((_, best_idle_ratio)) = best_telemetry_candidate {
                if idle_ratio > best_idle_ratio {
                    second_best_idle_ratio = best_idle_ratio;
                    best_telemetry_candidate = Some((candidate.identifier, idle_ratio));
                } else if idle_ratio > second_best_idle_ratio {
                    second_best_idle_ratio = idle_ratio;
                }
            } else {
                best_telemetry_candidate = Some((candidate.identifier, idle_ratio));
            }
        }

        if let Some((best_identifier, best_idle_ratio)) = best_telemetry_candidate {
            let idle_gap = best_idle_ratio.saturating_sub(second_best_idle_ratio);

            if idle_gap >= IDLE_RATIO_GAP_THRESHOLD_BASIS_POINTS {
                return Ok(best_identifier);
            }
        }

        let mut best_identifier = candidates[0].identifier;
        let mut best_task_count = candidates[0].task_count;

        for candidate in candidates.iter().skip(1) {
            if candidate.task_count < best_task_count
                || (candidate.task_count == best_task_count
                    && candidate.identifier < best_identifier)
            {
                best_task_count = candidate.task_count;
                best_identifier = candidate.identifier;
            }
        }

        Ok(best_identifier)
    }

    pub async fn get_spawner(&self, task: TaskIdentifier) -> Result<SpawnerIdentifier> {
        Self::get_task(&*self.0.read().await, task)
            .map(|task| task.spawner_identifier)
            .map_err(|_| Error::InvalidTaskIdentifier)
    }
}
