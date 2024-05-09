use std::fmt::Display;

use super::data_definition::DataLabel;

pub enum Classification {
    FalseNegative,
    FalsePositive,
    TrueNegative,
    TruePositive,
}

/// This function exists to keep the logic for this in one place so it doesn't get mixed up
pub fn prediction_classification(ground_truth: DataLabel, predicted: DataLabel) -> Classification {
    match (ground_truth, predicted) {
        (DataLabel::Normal, DataLabel::Normal) => Classification::TrueNegative,
        (DataLabel::Normal, DataLabel::Anomaly) => Classification::FalsePositive,
        (DataLabel::Anomaly, DataLabel::Normal) => Classification::FalseNegative,
        (DataLabel::Anomaly, DataLabel::Anomaly) => Classification::TruePositive,
    }
}

impl Display for Classification {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Classification::FalseNegative => "FN",
                Classification::FalsePositive => "FP",
                Classification::TrueNegative => "TN",
                Classification::TruePositive => "TP",
            }
        )
    }
}
