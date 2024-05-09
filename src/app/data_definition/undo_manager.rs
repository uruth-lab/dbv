use std::fmt::Display;

use self::{dequeue::Deque, stack::Stack};

use super::{DataPoint, DataPoints};

mod dequeue;
mod stack;

#[derive(serde::Deserialize, serde::Serialize, PartialEq)]
pub struct UndoManager {
    max_history_size: Option<u16>,
    undo_events: Deque<Event>,
    redo_events: Stack<Event>,
}

#[derive(
    serde::Deserialize, serde::Serialize, PartialEq, Eq, PartialOrd, Ord, Debug, Clone, Copy,
)]
pub struct DataTimestamp(u128);

impl DataTimestamp {
    fn now() -> Self {
        Self(
            web_time::SystemTime::UNIX_EPOCH
                .elapsed()
                .expect("expected date on system to be after the epoch")
                .as_nanos(),
        )
    }

    fn epoch() -> DataTimestamp {
        Self(0)
    }
}

impl Default for UndoManager {
    fn default() -> Self {
        Self {
            max_history_size: Some(Self::DEFAULT_MAX_HISTORY),
            undo_events: Default::default(),
            redo_events: Default::default(),
        }
    }
}

#[derive(serde::Deserialize, serde::Serialize, PartialEq, Debug)]
pub enum Event {
    Add(AddEventData),
    Edit(EditEventData),
    Delete(DeleteEventData),
    Clear(ClearEventData),
    Load(LoadEventData),
}

impl Event {
    pub fn timestamp(&self) -> DataTimestamp {
        match self {
            Event::Add(x) => x.timestamp,
            Event::Edit(x) => x.timestamp,
            Event::Delete(x) => x.timestamp,
            Event::Clear(x) => x.timestamp,
            Event::Load(x) => x.timestamp,
        }
    }
}

#[derive(serde::Deserialize, serde::Serialize, PartialEq, Debug)]
pub struct AddEventData {
    pub point: DataPoint,
    timestamp: DataTimestamp,
}
impl AddEventData {
    pub(crate) fn new(new_point: DataPoint) -> Self {
        Self {
            point: new_point,
            timestamp: DataTimestamp::now(),
        }
    }
}

#[derive(serde::Deserialize, serde::Serialize, PartialEq, Debug)]
pub struct EditEventData {
    pub new_point: DataPoint,
    pub old_point: DataPoint,
    pub index: usize,
    timestamp: DataTimestamp,
}
impl EditEventData {
    pub(crate) fn new(new_point: DataPoint, old_point: DataPoint, index: usize) -> Self {
        Self {
            new_point,
            old_point,
            index,
            timestamp: DataTimestamp::now(),
        }
    }
}

#[derive(serde::Deserialize, serde::Serialize, PartialEq, Debug)]
pub struct DeleteEventData {
    pub index: usize,
    pub point: DataPoint,
    timestamp: DataTimestamp,
}
impl DeleteEventData {
    pub(crate) fn new(index: usize, removed_point: DataPoint) -> Self {
        Self {
            index,
            point: removed_point,
            timestamp: DataTimestamp::now(),
        }
    }
}

#[derive(serde::Deserialize, serde::Serialize, PartialEq, Debug)]
pub struct ClearEventData {
    pub points: DataPoints,
    timestamp: DataTimestamp,
}
impl ClearEventData {
    pub(crate) fn new(points: DataPoints) -> Self {
        Self {
            points,
            timestamp: DataTimestamp::now(),
        }
    }
}

#[derive(serde::Deserialize, serde::Serialize, PartialEq, Debug)]
pub struct LoadEventData {
    pub points: DataPoints,
    timestamp: DataTimestamp,
}
impl LoadEventData {
    pub(crate) fn new(points: Vec<DataPoint>) -> Self {
        Self {
            points,
            timestamp: DataTimestamp::now(),
        }
    }
}

impl Display for Event {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Event::Add(data) => data.fmt(f),
            Event::Edit(data) => data.fmt(f),
            Event::Delete(data) => data.fmt(f),
            Event::Clear(data) => data.fmt(f),
            Event::Load(data) => data.fmt(f),
        }
    }
}

impl Display for AddEventData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Add Point: {}", self.point)
    }
}

impl Display for EditEventData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Edit Point: Index: {} From: {} To: {}",
            self.index, self.old_point, self.new_point
        )
    }
}

impl Display for DeleteEventData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Delete Point: {} at index: {}", self.point, self.index)
    }
}

impl Display for ClearEventData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Clear of Points")
    }
}

impl Display for LoadEventData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Load of Points")
    }
}

impl UndoManager {
    pub const DEFAULT_MAX_HISTORY: u16 = 200;
    pub fn max_history_size(&self) -> Option<u16> {
        self.max_history_size
    }

    pub fn set_max_history_size(&mut self, value: Option<u16>) {
        self.max_history_size = value;
        if let Some(max_size) = self.max_history_size {
            while self.undo_events.len() > max_size as usize {
                self.undo_events.remove_oldest();
            }
        }
    }

    pub fn clear_all(&mut self) {
        self.undo_events.clear();
        self.redo_events.clear();
    }

    pub fn is_undo_empty(&self) -> bool {
        self.undo_events.is_empty()
    }

    pub fn is_redo_empty(&self) -> bool {
        self.redo_events.is_empty()
    }

    pub fn is_empty(&self) -> bool {
        self.is_undo_empty() && self.is_redo_empty()
    }

    pub fn add_undo(&mut self, event: Event) {
        self.redo_events.clear();
        self.undo_events.push(event);
        if let Some(max_size) = self.max_history_size {
            if self.undo_events.len() > max_size as usize {
                self.undo_events.remove_oldest();
            }
            debug_assert!(
                self.undo_events.len() <= max_size as usize,
                "at this point it should be withing the limit"
            );
        }
    }

    /// Moves the most recent item into redo and returns a reference to it
    ///
    /// PANICS: Panics if there is nothing to undo
    pub fn undo(&mut self) -> &mut Event {
        let event = self
            .undo_events
            .pop()
            .expect("should not be empty if called");
        self.redo_events.push(event);
        self.redo_events
            .peek()
            .unwrap_or_else(|| panic!("should not be empty we just put an item into it"))
    }

    /// Moves the most recent item into undo and returns a reference to it
    ///
    /// PANICS: Panics if there is nothing to redo
    pub fn redo(&mut self) -> &mut Event {
        let event = self
            .redo_events
            .pop()
            .expect("should not be empty if called");
        self.undo_events.push(event);
        self.undo_events
            .peek_mut()
            .unwrap_or_else(|| panic!("should not be empty we just put an item into it"))
    }

    pub(crate) fn timestamp(&self) -> DataTimestamp {
        if let Some(event) = self.undo_events.peek() {
            event.timestamp()
        } else {
            // Epoch used to ensure always earlier than any other timestamp to prevent undoing the only change
            // and it saying that it was trained on an older dataset
            DataTimestamp::epoch()
        }
    }
}
