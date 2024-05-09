use std::fmt::Display;

use anyhow::{bail, Context};
use log::info;
use rfd::FileHandle;
use serde_repr::{Deserialize_repr, Serialize_repr};

use self::undo_manager::{
    AddEventData, ClearEventData, DeleteEventData, EditEventData, Event, LoadEventData, UndoManager,
};

use super::{plot_zoom_reset::MinMaxPair, status_msg::StatusMsg};
pub use undo_manager::DataTimestamp;

#[cfg(not(target_arch = "wasm32"))]
mod matlab;
mod undo_manager;

pub type DataPoints = Vec<DataPoint>;

/// Represents the main data stored by the application (The points and related info)
/// It MUST ensure that all public functions manage the undo stack by pushing on an
/// event if applicable that stores the information required to undo said event.
/// Public methods mush also ensure the invalidate the cache if needed
#[derive(serde::Deserialize, serde::Serialize, Default, PartialEq)]
pub struct Data {
    points: DataPoints,
    /// Controls if / how many decimal places new points are rounded to
    pub rounding_decimal_places: Option<u8>,
    undo_manager: UndoManager,
    /// Caches the value from `self.points`
    cached_points_min_max: Option<MinMaxPair>,
}

pub trait Save {
    /// Saves the data to the file given
    ///
    /// ASSUMPTION: The parent folder of the file exists
    async fn save_to_file(&self, file: &FileHandle) -> anyhow::Result<()>;
}

pub type PointArray = [f64; 2];

pub trait DistanceCalculation {
    fn calculate_distance(p1: PointArray, p2: PointArray) -> f64 {
        let diff0 = p1[0] - p2[0];
        let diff1 = p1[1] - p2[1];
        ((diff0 * diff0) + (diff1 * diff1)).sqrt()
    }

    fn distance_to(&self, other: PointArray) -> f64 {
        Self::calculate_distance(self.to_array(), other)
    }

    fn to_array(&self) -> PointArray;
}

pub trait DistanceCalculations {
    /// Returns a vec with each index containing a vec of the pairwise distances for that point
    /// The index into the inner vec will match the index of the other point
    fn pairwise_distances(&self) -> Vec<Vec<f64>>;
}

impl Data {
    const BOUNDARY_MARGIN: f64 = 1.1; //10% increase
    const DEFAULT_DECIMAL_PLACES_FOR_ROUNDING: u8 = 0;
    pub const MAX_DECIMAL_PLACES: u8 = 10;
    pub const DEFAULT_MAX_HISTORY: u16 = UndoManager::DEFAULT_MAX_HISTORY;

    pub fn points(&self) -> &[DataPoint] {
        &self.points
    }

    /// Creates a new copy of all the points
    pub fn clone_points(&self) -> DataPoints {
        self.points.clone()
    }

    /// Returns if rounding is enabled
    pub fn is_rounding_enabled(&self) -> bool {
        self.rounding_decimal_places.is_some()
    }

    /// Turns rounding on or off
    pub fn set_rounding_enabled(&mut self, value: bool) {
        match (self.rounding_decimal_places, value) {
            (None, true) => {
                self.rounding_decimal_places = Some(Self::DEFAULT_DECIMAL_PLACES_FOR_ROUNDING)
            }
            (Some(_), false) => self.rounding_decimal_places = None,
            (None, false) | (Some(_), true) => (), // Do nothing already in correct state
        }
    }

    /// Returns a reference to the value inside of the option. It will set it to default if it is none
    pub fn rounding_decimal_places_mut(&mut self) -> &mut u8 {
        self.rounding_decimal_places
            .get_or_insert(Self::DEFAULT_DECIMAL_PLACES_FOR_ROUNDING)
    }

    fn invalidate_cache(&mut self) {
        self.cached_points_min_max = None;
    }

    fn get_closest_point(
        &self,
        target_coord: egui_plot::PlotPoint,
        label: Option<DataLabel>,
    ) -> Option<usize> {
        let mut result = None;
        let mut min_distance = f64::INFINITY;
        for (i, data_point) in self
            .points
            .iter()
            .enumerate()
            .filter(|(_, p)| label.is_none() || p.label == *label.as_ref().unwrap())
        {
            let distance = target_coord.distance_to(data_point.to_array());
            if distance < min_distance {
                result = Some(i);
                min_distance = distance;
            }
        }
        result
    }

    pub fn add(
        &mut self,
        pointer_coordinate: Option<egui_plot::PlotPoint>,
        label: DataLabel,
        status_msg: &mut StatusMsg,
    ) {
        if let Some(pointer_coord) = pointer_coordinate {
            self.invalidate_cache();
            let mut x = pointer_coord.x;
            let mut y = pointer_coord.y;
            if let Some(desired_decimal_places) = self.rounding_decimal_places {
                let ten_pow = 10f64.powi(desired_decimal_places as _);
                x = (x * ten_pow).round() / ten_pow;
                y = (y * ten_pow).round() / ten_pow;
            }
            let new_point = DataPoint::new(x, y, label);
            let event = Event::Add(AddEventData::new(new_point));
            self.undo_manager.add_undo(event);
            self.points.push(new_point); // Actual add action
        } else {
            status_msg.error_display("Unable to add point. Cursor not detected over the plot");
        }
    }

    pub fn edit(&mut self, index: usize, new_point: DataPoint) {
        self.invalidate_cache();
        let old_point = self
            .points
            .get_mut(index)
            .expect("requires a valid point index");
        let event = Event::Edit(EditEventData::new(new_point, *old_point, index));
        self.undo_manager.add_undo(event);
        *old_point = new_point; // Actual replacement action
    }

    pub fn delete(
        &mut self,
        pointer_coordinate: Option<egui_plot::PlotPoint>,
        label: DataLabel,
        status_msg: &mut StatusMsg,
    ) {
        let Some(pointer_coord) = pointer_coordinate else {
            status_msg.error_display("Unable to delete point. Cursor not detected over the plot");
            return;
        };
        let index_closest_point = self.get_closest_point(pointer_coord, Some(label));

        if let Some(index) = index_closest_point {
            self.delete_by_index(index);
        } else {
            status_msg.info("No suitable point available for deleting");
        }
    }

    pub fn is_empty(&self) -> bool {
        self.points.is_empty()
    }

    pub fn clear_points(&mut self) {
        self.invalidate_cache();
        let mut event_data = ClearEventData::new(vec![]);
        std::mem::swap(&mut self.points, &mut event_data.points); // Move points into event_data for possible restoration
        self.undo_manager.add_undo(Event::Clear(event_data));
    }

    pub fn clear_history(&mut self, status_msg: &mut StatusMsg) {
        if self.undo_manager.is_empty() {
            status_msg.info("No History to clear");
        } else {
            self.undo_manager.clear_all();
            status_msg.info("Data History Cleared")
        }
    }

    pub fn get_points_min_max_w_margin(&mut self) -> MinMaxPair {
        if let Some(result) = self.cached_points_min_max {
            result
        } else {
            let first_point = self.points.first();
            let mut min_x0 = first_point.map(|x| x.x0).unwrap_or(-1.);
            let mut max_x0 = first_point.map(|x| x.x0).unwrap_or(1.);
            let mut min_x1 = first_point.map(|x| x.x1).unwrap_or(-1.);
            let mut max_x1 = first_point.map(|x| x.x1).unwrap_or(1.);
            for point in self.points.iter() {
                min_x0 = point.x0.min(min_x0);
                max_x0 = point.x0.max(max_x0);
                min_x1 = point.x1.min(min_x1);
                max_x1 = point.x1.max(max_x1);
            }

            // Handle case where there is no diff on a dimension
            if (min_x0 - max_x0).abs() < f64::EPSILON {
                min_x0 -= 1.;
                max_x0 += 1.;
            }
            if (min_x1 - max_x1).abs() < f64::EPSILON {
                min_x1 -= 1.;
                max_x1 += 1.;
            }

            // Add Margin
            (min_x0, max_x0) = Self::add_margin(min_x0, max_x0);
            (min_x1, max_x1) = Self::add_margin(min_x1, max_x1);

            let result = MinMaxPair {
                min: [min_x0, min_x1],
                max: [max_x0, max_x1],
            };
            self.cached_points_min_max = Some(result); // Store in cache
            info!("Points MinMax Calculated:  {result:?}");
            result
        }
    }

    fn add_margin(min: f64, max: f64) -> (f64, f64) {
        let range = max - min;
        let new_range = range * Self::BOUNDARY_MARGIN;
        let half_diff = (new_range - range) / 2.0;
        (min - half_diff, max + half_diff)
    }

    pub fn has_undo(&self) -> bool {
        !self.undo_manager.is_undo_empty()
    }

    pub fn has_redo(&self) -> bool {
        !self.undo_manager.is_redo_empty()
    }

    /// Undoes the last change to the data or nothing if no changes
    pub fn undo(&mut self, status_msg: &mut StatusMsg) {
        if self.undo_manager.is_undo_empty() {
            status_msg.info("No history available to undo");
        } else {
            self.invalidate_cache();
            let event = self.undo_manager.undo();
            match event {
                Event::Add(event_data) => {
                    debug_assert_eq!(
                        *self
                            .points
                            .last()
                            .expect("should have a point if we are going to remove it"),
                        event_data.point,
                        "should be the last point added"
                    );
                    self.points.pop().expect("should not be None");
                }
                Event::Edit(event_data) => {
                    debug_assert_eq!(
                        *self
                            .points
                            .get(event_data.index)
                            .expect("should have a point if we are going to replace it"),
                        event_data.new_point,
                        "current state should have the new_point in at the index specified"
                    );
                    *self.points.get_mut(event_data.index).unwrap() = event_data.old_point;
                }
                Event::Delete(event_data) => {
                    debug_assert!(self.points.len() >= event_data.index, "index should be less than or equal to points length because it is supposed to be able to be inserted where it came from");
                    self.points.insert(event_data.index, event_data.point);
                }
                Event::Clear(event_data) => {
                    debug_assert!(
                        self.points.is_empty(),
                        "should not have any points when undoing a clear"
                    );
                    std::mem::swap(&mut self.points, &mut event_data.points);
                }
                Event::Load(event_data) => {
                    std::mem::swap(&mut self.points, &mut event_data.points);
                }
            }
            // status_msg.add_msg(&format!("Undo: {event}")); // TODO 3: Decide if auto removal of status_msgs is worth implementing (leaving this off pending that)
        }
    }

    /// Redoes the last change undone or nothing of no redo available
    pub fn redo(&mut self, status_msg: &mut StatusMsg) {
        if self.undo_manager.is_redo_empty() {
            status_msg.info("No history available to undo");
        } else {
            self.invalidate_cache();
            let event = self.undo_manager.redo();
            match event {
                Event::Add(event_data) => self.points.push(event_data.point),
                Event::Edit(event_data) => {
                    debug_assert_eq!(
                        *self
                            .points
                            .get(event_data.index)
                            .expect("should have a point if we are going to replace it"),
                        event_data.old_point,
                        "current state should have the old_point in at the index specified"
                    );
                    *self.points.get_mut(event_data.index).unwrap() = event_data.new_point;
                }
                Event::Delete(event_data) => {
                    debug_assert_eq!(
                        self.points[event_data.index], event_data.point,
                        "redoing a delete but point is not the same"
                    );
                    self.points.remove(event_data.index);
                }
                Event::Clear(event_data) => {
                    debug_assert!(
                        event_data.points.is_empty(),
                        "should not have any points when redoing a clear"
                    );
                    std::mem::swap(&mut self.points, &mut event_data.points);
                }
                Event::Load(event_data) => {
                    std::mem::swap(&mut self.points, &mut event_data.points);
                }
            }
            // status_msg.add_msg(&format!("Redo: {event}")); // TODO 3: Decide if auto removal of status_msgs is worth implementing (leaving this off pending that)
        }
    }

    pub fn has_history(&self) -> bool {
        !self.undo_manager.is_empty()
    }

    pub fn set_history_size(&mut self, value: Option<u16>) {
        self.undo_manager.set_max_history_size(value);
    }

    pub fn max_history_size(&self) -> Option<u16> {
        self.undo_manager.max_history_size()
    }

    /// Function replaces the data with the data passed in (also handles the history as needed)
    pub fn replace_with_loaded_data(&mut self, points: DataPoints) {
        self.invalidate_cache();
        let mut event_data = LoadEventData::new(points);
        std::mem::swap(&mut self.points, &mut event_data.points); // Move points into event_data for possible restoration
        self.undo_manager.add_undo(Event::Load(event_data));
    }

    /// Returns the loaded data if loaded with an optional status message
    pub async fn load_from_file(
        file: &FileHandle,
    ) -> anyhow::Result<(DataPoints, Option<&'static str>)> {
        let mut load_msg = None;
        let filename = file.file_name();
        let loaded_data = match &filename {
            s if s.ends_with("mat") => Self::load_as_matlab(file)?,
            s if s.ends_with("csv") => Self::load_as_csv(file)
                .await
                .context("Failed to load from CSV")?,
            s => {
                load_msg = Some("Extension not recognized. Attempted to load as CSV");
                Self::load_as_csv(file).await.with_context(|| {
                    format!("failed to load unrecognized file type as CSV. Filename: {s:?}")
                })?
            }
        };

        Ok((loaded_data, load_msg))
    }

    #[cfg(target_arch = "wasm32")]
    fn save_as_matlab(_: &[DataPoint], _: &FileHandle) -> anyhow::Result<()> {
        bail!("Saving to Matlab files is not supported in WASM")
    }

    #[cfg(target_arch = "wasm32")]
    fn load_as_matlab(_: &FileHandle) -> anyhow::Result<DataPoints> {
        bail!("Loading from Matlab files is not supported in WASM")
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn save_as_matlab(points: &[DataPoint], file: &FileHandle) -> anyhow::Result<()> {
        use self::matlab::MatlabData;

        let data = MatlabData::from(points);
        data.save_to_file(file.path())
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn load_as_matlab(file: &FileHandle) -> anyhow::Result<DataPoints> {
        self::matlab::MatlabData::load_from_file(file.path())
    }

    async fn save_as_csv(points: &[DataPoint], file: &FileHandle) -> anyhow::Result<()> {
        let mut write_buffer = Vec::new();
        let mut wtr = csv::Writer::from_writer(&mut write_buffer);

        for point in points.iter() {
            wtr.serialize(point)?;
        }

        wtr.flush().context("failed flushing csv writer")?;
        drop(wtr); // I think this is needed because drop on this type has side effects so it cannot be just moved by the non lexical lifetimes upgrade
        file.write(&write_buffer)
            .await
            .context("failed to write to FileHandle")
    }

    async fn load_as_csv(file: &FileHandle) -> anyhow::Result<DataPoints> {
        let text = file.read().await;
        let mut reader = csv::Reader::from_reader(text.as_slice());
        let mut result = vec![];
        for record in reader.deserialize() {
            let point: DataPoint = record?;
            result.push(point);
        }
        Ok(result)
    }

    pub fn timestamp(&self) -> DataTimestamp {
        self.undo_manager.timestamp()
    }

    pub fn delete_by_index(&mut self, index: usize) {
        self.invalidate_cache();
        let removed_point = self.points.remove(index); // Actual delete action
        self.undo_manager
            .add_undo(Event::Delete(DeleteEventData::new(index, removed_point)));
    }
}

impl Save for Data {
    async fn save_to_file(&self, file: &FileHandle) -> anyhow::Result<()> {
        self.points().save_to_file(file).await
    }
}

impl<T: AsRef<[DataPoint]>> Save for T {
    async fn save_to_file(&self, file: &FileHandle) -> anyhow::Result<()> {
        let filename = file.file_name();
        match &filename {
            s if s.ends_with("mat") => Data::save_as_matlab(self.as_ref(), file),
            s if s.ends_with("csv") => Data::save_as_csv(self.as_ref(), file)
                .await
                .context("failed to save to CSV"),
            _ => bail!("extension not recognized. Please use .csv or .mat. Filename: {file:?}"),
        }
        .context("failed to save")
    }
}

impl<T: AsRef<[DataPoint]>> DistanceCalculations for T {
    fn pairwise_distances(&self) -> Vec<Vec<f64>> {
        let points = self.as_ref();
        let mut result = vec![vec![0.; points.len()]; points.len()];
        for first in 0..points.len() {
            for second in (first + 1)..points.len() {
                let distance = points[first].distance_to(points[second].to_array());
                result[first][second] = distance;
                result[second][first] = distance;
            }
        }
        result
    }
}

impl DistanceCalculation for &DataPoint {
    fn to_array(&self) -> PointArray {
        [self.x0, self.x1]
    }
}

impl DistanceCalculation for DataPoint {
    fn to_array(&self) -> PointArray {
        [self.x0, self.x1]
    }
}

impl DistanceCalculation for &egui_plot::PlotPoint {
    fn to_array(&self) -> PointArray {
        [self.x, self.y]
    }
}

impl DistanceCalculation for egui_plot::PlotPoint {
    fn to_array(&self) -> PointArray {
        [self.x, self.y]
    }
}

#[derive(Serialize_repr, Deserialize_repr, PartialEq, Clone, Copy, Debug)]
#[repr(u8)]
pub enum DataLabel {
    Normal,
    Anomaly,
}

impl DataLabel {
    /// Returns `true` if the data label is [`Normal`].
    ///
    /// [`Normal`]: DataLabel::Normal
    #[must_use]
    pub fn is_normal(&self) -> bool {
        matches!(self, Self::Normal)
    }

    /// Returns `true` if the data label is [`Anomaly`].
    ///
    /// [`Anomaly`]: DataLabel::Anomaly
    #[must_use]
    pub fn is_anomaly(&self) -> bool {
        matches!(self, Self::Anomaly)
    }

    fn as_int(&self) -> u8 {
        *self as u8
    }
}

impl Display for DataLabel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                DataLabel::Normal => "N",
                DataLabel::Anomaly => "A",
            }
        )
    }
}

impl TryFrom<u8> for DataLabel {
    type Error = anyhow::Error;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            x if DataLabel::Normal.as_int() == x => Ok(DataLabel::Normal),
            x if DataLabel::Anomaly.as_int() == x => Ok(DataLabel::Anomaly),
            _ => bail!("unexpected value for DataLabel of {value}"),
        }
    }
}

#[derive(serde::Deserialize, serde::Serialize, PartialEq, Clone, Copy, Debug)]
// TODO 4: Handle approximately equal (Use case prompting idea is for edits in the table)
//      Reference library: https://jtempest.github.io/float_eq-rs/book/tutorials/basic_usage.html
pub struct DataPoint {
    pub x0: f64,
    pub x1: f64,
    pub label: DataLabel,
}

impl Display for DataPoint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{:.2}, {:.2}, {}]", self.x0, self.x1, self.label)
    }
}

impl DataPoint {
    fn new(x0: f64, x1: f64, label: DataLabel) -> Self {
        Self { x0, x1, label }
    }
}

#[cfg(test)]
pub(crate) mod tests {

    use super::*;

    #[test]
    #[allow(clippy::assertions_on_constants)]
    fn margin_in_expected_range() {
        assert!(Data::BOUNDARY_MARGIN >= 1.0 && Data::BOUNDARY_MARGIN <= 2.0);
    }

    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    pub(super) fn generate_data_points() -> DataPoints {
        (0..10)
            .map(|i| {
                let i = i as f64;
                DataPoint {
                    x0: i,
                    x1: i * 3.0,
                    label: if i % 4.0 == 0.0 {
                        DataLabel::Normal
                    } else {
                        DataLabel::Anomaly
                    },
                }
            })
            .collect()
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[ignore = "Needs to write to disk and tests code that doesn't change often"]
    #[tokio::test]
    async fn save_load_from_disk_as_csv() {
        let expected = generate_data_points();
        let temp_file = tempfile::NamedTempFile::new().unwrap();
        let path = temp_file.path();
        println!("Using temp file at: {path:?}");
        let file = FileHandle::from(path.to_path_buf());

        Data::save_as_csv(&expected, &file).await.unwrap();
        let actual = Data::load_as_csv(&file).await.unwrap();
        assert_eq!(actual, expected);
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[ignore = "Needs to write to disk and tests code that doesn't change often"]
    #[test]
    fn save_load_from_disk_as_matlab() {
        let expected = generate_data_points();
        let temp_file = tempfile::NamedTempFile::new().unwrap();
        let path = temp_file.path();
        println!("Using temp file at: {path:?}");
        let file = FileHandle::from(path.to_path_buf());

        Data::save_as_matlab(&expected, &file).unwrap();
        let actual = Data::load_as_matlab(&file).unwrap();
        assert_eq!(actual, expected);
    }
}
