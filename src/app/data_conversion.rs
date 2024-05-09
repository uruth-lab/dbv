use super::data_definition::{DataPoint, DistanceCalculation, PointArray};

pub trait ConvertToSeries {
    fn array_of_normal(&self) -> Vec<PointArray>;
    fn array_of_anom(&self) -> Vec<PointArray>;
}

impl ConvertToSeries for &[DataPoint] {
    fn array_of_normal(&self) -> Vec<PointArray> {
        self.iter()
            .filter_map(|point| {
                if point.label.is_normal() {
                    Some(point.to_array())
                } else {
                    None
                }
            })
            .collect()
    }

    fn array_of_anom(&self) -> Vec<PointArray> {
        self.iter()
            .filter_map(|point| {
                if point.label.is_anomaly() {
                    Some(point.to_array())
                } else {
                    None
                }
            })
            .collect()
    }
}
