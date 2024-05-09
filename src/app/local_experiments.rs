use super::{
    data_definition::{DataLabel, DataPoints, DataTimestamp},
    status_msg::StatusMsg,
};

mod proximity_score;

pub use proximity_score::ProximityScore;

pub type Scores = Vec<f64>;

#[derive(serde::Deserialize, serde::Serialize, PartialEq)]
pub enum LocalExperiment {
    None,
    ProximityScoreUntrained(ProximityScore<UnTrained>),
    ProximityScoreTrained(ProximityScore<Trained>),
}

#[derive(serde::Deserialize, serde::Serialize, PartialEq, Debug)]
pub struct TrainResults {
    scores: Scores,
    data_timestamp_at_start: DataTimestamp,
}

#[derive(Debug, PartialEq)]
pub struct UnTrained;
#[derive(Debug, PartialEq)]
pub struct Trained;

pub trait ModelTrain {
    type TrainConfig;

    /// Executes the algorithm and returns the results
    async fn train(
        train_config: Self::TrainConfig,
        points: DataPoints,
        data_timestamp: DataTimestamp,
        status_msg: &mut StatusMsg,
    ) -> anyhow::Result<TrainResults>;

    /// Provides a way to get the configuration required while training
    fn train_config_clone(&self) -> Self::TrainConfig;

    /// Copies the current model (less any current result info) and returns a new model that can be used to replace the previous
    ///
    /// Doesn't directly consume self because it will likely need to be wrapped in a new enum variant
    #[must_use]
    fn to_inference(&self, results: TrainResults) -> impl ModelInference;
}

pub trait ModelInference {
    /// Check the last known data state of the data
    fn data_timestamp_at_training(&self) -> DataTimestamp;

    /// Gives a prediction on a point that was in the training data
    ///
    /// # PANICS
    /// If index is not within the scores during training
    fn prediction_on_training_data(&self, index: usize) -> DataLabel;
}

pub trait ModelInferenceConfig: ModelInference {
    type PredictConfig: Clone;

    /// Provides a way to edit the configurations
    fn predict_config_mut(&mut self) -> &mut Self::PredictConfig;

    // TODO 3: Add way to get best F1 score threshold
}

impl LocalExperiment {
    /// Returns `true` if the local experiment is [`None`].
    ///
    /// [`None`]: LocalExperiment::None
    #[must_use]
    pub fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }

    /// Returns `true` if the local experiment is [`ProximityScore`].
    ///
    /// [`ProximityScore`]: LocalExperiment::ProximityScore
    #[must_use]
    pub fn is_proximity_score(&self) -> bool {
        matches!(self, Self::ProximityScoreUntrained(..))
            || matches!(self, Self::ProximityScoreTrained(..))
    }

    pub(crate) fn model_inference(&self) -> Option<&dyn ModelInference> {
        Some(match self {
            LocalExperiment::None | LocalExperiment::ProximityScoreUntrained(_) => return None,
            LocalExperiment::ProximityScoreTrained(x) => x,
        })
    }

    pub(crate) fn is_at_timestamp(&self, timestamp: DataTimestamp) -> bool {
        self.data_timestamp_at_training() == Some(timestamp)
    }

    pub(crate) fn data_timestamp_at_training(&self) -> Option<DataTimestamp> {
        match self {
            LocalExperiment::None | LocalExperiment::ProximityScoreUntrained(_) => None,
            LocalExperiment::ProximityScoreTrained(x) => Some(x.data_timestamp_at_training()),
        }
    }
}

impl Default for LocalExperiment {
    fn default() -> Self {
        Self::None
    }
}
