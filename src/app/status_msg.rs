use std::{
    fmt::{Debug, Display},
    sync::{Arc, Mutex},
};

use log::{debug, error};

/// Encapsulates the message to show in the status bar
///
/// Provides a way to ensure the correct API is used and the string is not randomly edited
///
/// ASSUMES: Mutex will never be poisoned and just unwraps
#[derive(Debug, Clone)]
pub struct StatusMsg {
    msg: Arc<Mutex<String>>,
    // TODO 3: Change to making these rendered by the struct and add colors and fade out over time
}

impl PartialEq for StatusMsg {
    fn eq(&self, other: &Self) -> bool {
        // WARNING: Possible performance issues as this could get called a lot
        self.msg() == other.msg()
    }
}

impl Default for StatusMsg {
    fn default() -> Self {
        Self {
            msg: Arc::new(Mutex::new(Self::starter_msg())),
        }
    }
}

impl StatusMsg {
    fn msg_time() -> String {
        use chrono::TimeZone as _;
        let time_stamp = web_time::SystemTime::UNIX_EPOCH
            .elapsed()
            .expect("expected date on system to be after the epoch")
            .as_secs();

        let dt = chrono::NaiveDateTime::from_timestamp_opt(time_stamp as i64, 0).unwrap();
        let dt = chrono::Local::from_utc_datetime(&chrono::Local, &dt);
        dt.format("%H:%M:%S").to_string()
    }

    fn add_msg<S: Display>(&mut self, msg: S) {
        // TODO 3: Add caching for display purposes and store message separately so they can be removed (not remove errors?)
        self.msg
            .lock()
            .unwrap()
            .push_str(&format!("\n------\n{msg}"));
    }

    pub fn info<S: Display>(&mut self, msg: S) {
        let msg = format!("[{} INFO ] {msg}", Self::msg_time());
        debug!("{msg}");
        self.add_msg(msg)
    }

    pub fn error_debug<S: Debug>(&mut self, msg: S) {
        let msg = format!("[{} ERROR] {msg:?}", Self::msg_time());
        error!("{msg}");
        self.add_msg(msg);
    }

    pub fn error_display<S: Display>(&mut self, msg: S) {
        let msg = format!("[{} ERROR] {msg}", Self::msg_time());
        error!("{msg}");
        self.add_msg(msg);
    }

    /// Returns a String to avoid keeping the lock
    ///
    /// Not sure if this is a good idea but will revisit if having performance issues
    pub fn msg(&self) -> String {
        self.msg.lock().unwrap().clone()
    }

    pub fn clear(&mut self) {
        *self = Default::default()
    }

    pub fn is_empty(&self) -> bool {
        self.msg() == Self::starter_msg()
    }

    fn starter_msg() -> String {
        "Status Messages".to_string()
    }
}
