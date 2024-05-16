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
    outlier_index: usize,
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
        let outlier_index = calculate_outlier_index(&results);
        let train_data = TrainingInfo {
            results,
            outlier_index,
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
        let scores = pairwise_distances
            .into_iter()
            .enumerate()
            .map(|(score_for_index, distances)| {
                distances
                    .into_iter()
                    .enumerate()
                    .fold(f64::INFINITY, |acc, (other_index, elem)| {
                        if score_for_index == other_index {
                            // Skip distance to itself when getting minimum
                            acc
                        } else {
                            acc.min(elem)
                        }
                    })
            })
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

        if index == training_info.outlier_index {
            DataLabel::Anomaly
        } else {
            DataLabel::Normal
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
    type PredictConfig = ();

    fn predict_config_mut(&mut self) -> &mut Self::PredictConfig {
        unimplemented!("there isn't a suitable implementation for this")
    }
}

/// Get the index of the maximum score (break ties with lower index)
fn calculate_outlier_index(results: &TrainResults) -> usize {
    assert!(
        !results.scores.is_empty(),
        "requires at least one point for training"
    );
    results
        .scores
        .iter()
        .enumerate()
        .fold(
            (0, -f64::INFINITY),
            |acc, elem| match PartialOrd::partial_cmp(&acc.1, elem.1)
                .expect("distances should not be NAN")
            {
                std::cmp::Ordering::Less => (elem.0, *elem.1), // Need new tuple to remove reference
                _ => acc, // if Equal we want the previous value as well so that we get the lower index
            },
        )
        .0
}
