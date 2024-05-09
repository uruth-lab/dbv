use std::path::PathBuf;

use super::{data_definition::DataPoints, local_experiments::TrainResults};

pub type AwaitingType = poll_promise::Promise<OperationOutcome>;

#[derive(Default)]
pub enum OperationalState {
    #[default]
    Normal,
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    RunningPyExperiment(AwaitingType),
    Saving(AwaitingType),
    Loading(AwaitingType),
    RunningLocExperiment(AwaitingType),
}

#[derive(Debug)]
pub enum OperationOutcome {
    Cancelled,
    Success(Payload),
    Failed(anyhow::Error),
}

#[derive(Debug)]
pub enum Payload {
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    PyRun,
    Load {
        loaded_data: DataPoints,
        path: PathBuf,
    },
    Save(PathBuf),
    Train(TrainResults),
}

impl PartialEq for OperationalState {
    fn eq(&self, other: &Self) -> bool {
        core::mem::discriminant(self) == core::mem::discriminant(other)
    }
}

impl OperationalState {
    /// Returns `true` if the operational state is [`Normal`].
    ///
    /// [`Normal`]: OperationalState::Normal
    #[must_use]
    pub fn is_normal(&self) -> bool {
        matches!(self, Self::Normal)
    }

    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    /// Returns `true` if the operational state is [`RunningExperiment`].
    ///
    /// [`RunningExperiment`]: OperationalState::RunningExperiment
    #[must_use]
    pub fn is_running_py_experiment(&self) -> bool {
        matches!(self, Self::RunningPyExperiment(..))
    }

    /// Returns `true` if the operational state is [`RunningLocExperiment`].
    ///
    /// [`RunningLocExperiment`]: OperationalState::RunningLocExperiment
    #[must_use]
    pub fn is_running_loc_experiment(&self) -> bool {
        matches!(self, Self::RunningLocExperiment(..))
    }
}
