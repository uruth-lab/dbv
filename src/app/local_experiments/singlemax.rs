use std::marker::PhantomData;

use anyhow::bail;

use crate::app::{
    data_definition::{DataLabel, DataPoints, DataTimestamp, DistanceCalculations as _},
    status_msg::StatusMsg,
};

use super::{ModelInference, ModelInferenceConfig, ModelTrain, TrainResults, Trained, UnTrained};

#[derive(serde::Deserialize, serde::Serialize, PartialEq)]
pub struct SingleMax<State = UnTrained> {
    train_data: Option<TrainingInfo>,
    state: PhantomData<State>, // This doesn't take up space at runtime
}
impl SingleMax {
    pub(crate) fn new() -> SingleMax {
        SingleMax::<UnTrained> {
            train_data: None,
            state: PhantomData,
        }
    }
}

#[derive(serde::Deserialize, serde::Serialize, PartialEq)]
pub struct TrainingInfo {
    results: TrainResults,
    predict_config: PredictConfig,
}

#[derive(serde::Deserialize, serde::Serialize, PartialEq, Clone, Copy, Debug)]
pub struct PredictConfig {
    pub min_score: f64,
    pub max_score: f64,
    pub threshold: f64,
}

impl<T> ModelTrain for &SingleMax<T> {
    type TrainConfig = ();

    async fn train(
        train_config: Self::TrainConfig,
        points: DataPoints,
        data_timestamp: DataTimestamp,
        status_msg: &mut StatusMsg,
    ) -> anyhow::Result<TrainResults> {
        SingleMax::<T>::train(train_config, points, data_timestamp, status_msg).await
    }

    fn train_config_clone(&self) -> Self::TrainConfig {}

    fn to_inference(&self, results: TrainResults) -> SingleMax<Trained> {
        let predict_config = PredictConfig::from(&results);
        let train_data = TrainingInfo {
            results,
            predict_config,
        };
        SingleMax::<Trained> {
            train_data: Some(train_data),
            state: PhantomData,
        }
    }
}

impl<T> ModelTrain for SingleMax<T> {
    type TrainConfig = ();

    async fn train(
        _train_config: Self::TrainConfig,
        points: DataPoints,
        data_timestamp: DataTimestamp,
        _status_msg: &mut StatusMsg,
    ) -> anyhow::Result<TrainResults> {
        if points.is_empty() {
            bail!("no points found");
        }
        let pairwise_distances = points.pairwise_distances();
        let n = points.len() as f64;
        let scores = pairwise_distances
            .into_iter()
            .map(|distances| distances.into_iter().sum::<f64>() / n)
            .collect();
        Ok(TrainResults {
            scores,
            data_timestamp_at_start: data_timestamp,
        })
    }

    fn train_config_clone(&self) -> Self::TrainConfig {}

    fn to_inference(&self, results: TrainResults) -> SingleMax<Trained> {
        (&self).to_inference(results)
    }
}

impl ModelInference for &SingleMax<Trained> {
    fn data_timestamp_at_training(&self) -> DataTimestamp {
        self.train_data
            .as_ref()
            .expect("expected to only be called if this is set (checked by type)")
            .results
            .data_timestamp_at_start
    }

    fn prediction_on_training_data(&self, index: usize) -> DataLabel {
        let training_info = self
            .train_data
            .as_ref()
            .expect("expected to only be called if this is set (checked by type)");
        let scores = &training_info.results.scores;
        let threshold = training_info.predict_config.threshold;
        if scores[index] < threshold {
            DataLabel::Normal
        } else {
            DataLabel::Anomaly
        }
    }

    fn score_for_training_data(&self, index: usize) -> f64 {
        let training_info = self
            .train_data
            .as_ref()
            .expect("expected to only be called if this is set (checked by type)");
        training_info.results.scores[index]
    }
}

impl ModelInference for SingleMax<Trained> {
    fn data_timestamp_at_training(&self) -> DataTimestamp {
        (&self).data_timestamp_at_training()
    }

    fn prediction_on_training_data(&self, index: usize) -> DataLabel {
        (&self).prediction_on_training_data(index)
    }

    fn score_for_training_data(&self, index: usize) -> f64 {
        (&self).score_for_training_data(index)
    }
}

impl ModelInferenceConfig for SingleMax<Trained> {
    type PredictConfig = PredictConfig;

    fn predict_config_mut(&mut self) -> &mut Self::PredictConfig {
        &mut self
            .train_data
            .as_mut()
            .expect("expected to only be called if this is set (checked by type)")
            .predict_config
    }
}

impl From<&TrainResults> for PredictConfig {
    fn from(value: &TrainResults) -> Self {
        let scores = &value.scores;
        debug_assert!(
            !scores.is_empty(),
            "training should fail if there are no points"
        );
        let mut min_score = scores[0];
        let mut max_score = scores[0];
        for &score in scores {
            if min_score > score {
                min_score = score;
            }
            if max_score < score {
                max_score = score;
            }
        }
        let threshold =
            Self::THRESHOLD_RATIO * max_score + (1. - Self::THRESHOLD_RATIO) * min_score;
        Self {
            min_score,
            max_score,
            threshold,
        }
    }
}

impl PredictConfig {
    const THRESHOLD_RATIO: f64 = 3. / 4.; // Set to 75% NB: code assumes this is between 0 and 1
}
