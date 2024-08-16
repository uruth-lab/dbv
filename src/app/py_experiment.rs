use std::{fmt::Display, path::Path, process::Command};

use anyhow::{bail, Context};
use lazy_static::lazy_static;
use log::{info, warn};
use regex::Regex;

use crate::{
    app::{
        data_definition::Save as _,
        display_slice::DisplaySlice,
        execute,
        operational_state::{OperationOutcome, OperationalState, Payload},
    },
    DBV,
};

use super::{data_definition::DataPoint, status_msg::StatusMsg};

#[derive(Debug, Default, serde::Serialize, serde::Deserialize, PartialEq, Eq, Clone)]
pub struct PyExperiment {
    pub selected_algorithms: SelectedAlgorithms,
    /// Data directory that is adjacent to the python scripts
    data_dir: Option<String>,
    /// Stores the file name to be appended to the data folder
    data_filename: Option<String>,
    pub venv_activate_filename: Option<String>,
}

impl PyExperiment {
    pub fn unset_filename(&mut self) {
        self.data_filename = None;
    }

    /// Sets the filename if the value is valid and returns a reference to the value, if invalid an error is returned
    pub fn set_filename<P: AsRef<Path>>(&mut self, value: P) -> anyhow::Result<&str> {
        let path = value.as_ref();
        // Ensure value ends in .mat
        if Some("mat") != path.extension().map(|x| x.to_string_lossy()).as_deref() {
            bail!("only '.mat' files are supported for experiments but found {path:?}")
        }

        let Some(data_dir) = self.data_dir() else {
            // TODO 4: Would it be better to just set to the incoming path in this case?
            bail!("unable to set filename as data_directory is not set")
        };

        // Ensure value file is in correct folder or no folder specified
        if let Some(parent) = path.parent() {
            if !same_file::is_same_file(data_dir, parent).is_ok_and(|x| x) {
                info!("path: {path:?}");
                bail!(
                    "only files in data folder ({:?}) are allowed but found {:?} as parent of file",
                    data_dir,
                    parent.display()
                );
            }
        }

        if let Some(filename) = path.file_name() {
            if let Some(filename) = filename.to_str() {
                self.data_filename = Some(filename.into());
                Ok(self
                    .data_filename
                    .get_or_insert_with(|| unreachable!("value was just inserted")))
            } else {
                bail!("unable to convert OsStr to str for {filename:?}");
            }
        } else {
            bail!("unable to get filename from {path:?}");
        }
    }

    pub fn data_filename(&self) -> Option<&String> {
        self.data_filename.as_ref()
    }

    pub fn not_ready_reasons(&self) -> Vec<NotReadyReason> {
        let mut result = vec![];

        if !self.selected_algorithms.has_at_least_one() {
            result.push(NotReadyReason::NoAlgorithmSelected)
        }
        if self.data_dir.is_none() {
            result.push(NotReadyReason::NoDataDirSet)
        }
        if self.data_filename.is_none() {
            result.push(NotReadyReason::NoFileSet)
        }

        result
    }

    pub async fn run(
        &self,
        points: &[DataPoint],
        status_msg: &mut StatusMsg,
    ) -> anyhow::Result<()> {
        // Check if everything is ready to run
        let reasons = self.not_ready_reasons();
        if !reasons.is_empty() {
            bail!("Not ready to run: {}", reasons.to_delimited_string())
        }

        let data_path = Path::new(self.data_dir().expect("required to be ready"));

        // Save File
        let path = data_path.join(self.data_filename().expect("required to be ready"));
        let file = rfd::FileHandle::from(path);
        points.save_to_file(&file).await.context("save failed")?;
        status_msg.info(format!("Saved data before calling script to {file:?}"));

        // Send Command
        let working_dir = match data_path.parent() {
            Some(x) => x,
            None => bail!("Failed to get parent directory of data directory"),
        };
        let working_dir = working_dir
            .canonicalize()
            .context("failed to canonicalize working directory")?;

        let mut cmd_str = String::new();
        if let Some(filename) = &self.venv_activate_filename {
            cmd_str.push_str(&format!("source {filename:?} && "));
        }
        cmd_str.push_str(&format!(
            "python src/sub_routine.py {} -a {}",
            self.data_filename().expect("required to be ready"),
            self.selected_algorithms.as_delimited_string()
        ));
        info!("Command String: {cmd_str:?}");
        let mut command = Command::new("bash");

        status_msg.info("Command created. Going to start python script");

        command
            .env("PYTHONPATH", working_dir.as_os_str())
            .current_dir(&working_dir)
            .arg("-c")
            .arg(cmd_str);
        let output = command.output().context("command execution failed")?;
        let stdout = String::from_utf8(output.stdout).context("stdout conversion failed")?;
        let stderr = String::from_utf8(output.stderr).context("stdout conversion failed")?;
        let exit_code = match output.status.code() {
            Some(code) => code,
            None => bail!("unable to get exit code"),
        };

        // Log stderr if not empty
        if !stderr.is_empty() {
            status_msg.error_display(format!("stderr: {stderr:?}"));
        }

        // Ensure exit code is 0
        if exit_code != 0 {
            warn!("stdout: {stdout:?}"); // Only logged as it may be very long and hopefully the error is in stderr
            if stderr.is_empty() {
                // If it's not empty should have been logged above
                status_msg.error_display("stderr was empty");
            }
            bail!("run exited with non 0 exit code of {exit_code}");
        }

        // Collect run info
        // TODO 5: Try using RegexSet to see if that helps with runtime (no likely to be a big bottle neck might not be worth it)
        let run_result = RunResult::from_stdout(&stdout);

        // Open output folder
        if let Some(output_folder) = run_result.output_folder {
            let output_folder = working_dir.join(output_folder);
            opener::reveal(&output_folder).context("open output folder")?;
            status_msg.info(format!("Opened output folder: {output_folder:?}"));
        } else {
            warn!("stdout: {stdout:?}");
            bail!("run seems to have succeeded but couldn't find output folder in stdout");
        }

        Ok(())
    }

    pub fn data_dir(&self) -> Option<&String> {
        self.data_dir.as_ref()
    }

    pub fn set_data_dir(&mut self, value: Option<String>) -> anyhow::Result<()> {
        // TODO 2: Validate input is a directory and that the script can be found relative to this folder
        self.data_dir = value;
        Ok(())
    }
}

#[derive(Debug, Default, serde::Serialize, serde::Deserialize, PartialEq, Eq, Clone)]
pub struct SelectedAlgorithms {
    selected: [bool; Self::LEN],
}

impl SelectedAlgorithms {
    pub const LEN: usize = 7;

    const NAMES: [&'static str; Self::LEN] = [
        "IsolationForest",
        "LocalOutlierFactor",
        "OneClassSVM",
        "PIDForest",
        "EIF",
        "Custom1",
        "CustomIF",
    ];

    pub fn as_delimited_string(&self) -> String {
        let mut result = vec![];
        for i in 0..self.selected.len() {
            if self.selected[i] {
                result.push(Self::NAMES[i]);
            }
        }
        result.join(",")
    }

    pub fn get_mut(&mut self, index: usize) -> (&'static str, &mut bool) {
        (Self::NAMES[index], &mut self.selected[index])
    }

    pub fn has_at_least_one(&self) -> bool {
        self.selected.iter().any(|&x| x)
    }
}

#[allow(clippy::enum_variant_names)] // Allowed as other prefix may be added later and want to keep them readable
pub enum NotReadyReason {
    NoDataDirSet,
    NoFileSet,
    NoAlgorithmSelected,
}

impl Display for NotReadyReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let value = match self {
            NotReadyReason::NoDataDirSet => "No Data Directory set",
            NotReadyReason::NoFileSet => "No filename set",
            NotReadyReason::NoAlgorithmSelected => "No Algorithm Selected",
        };
        write!(f, "{value}")
    }
}

#[derive(Debug, Default)]
struct RunResult {
    output_folder: Option<String>,
    #[allow(unused)] // TODO 4: Make performance information available in UI
    performances: Option<String>,
}

impl RunResult {
    fn from_stdout(out: &str) -> Self {
        let mut result = Self::default();
        lazy_static! {
            static ref REGEX_OUTPUT_FOLDER: Regex = Regex::new(r#"Output folder: "(.+)""#).unwrap();
        }
        lazy_static! {
            static ref REGEX_PERFORMANCES: Regex =
                Regex::new(r#"Performances saved to: "(.+)""#).unwrap();
        }

        if let Some(capture) = REGEX_OUTPUT_FOLDER.captures(out) {
            if let Some(value) = capture.get(1) {
                result.output_folder = Some(value.as_str().to_string());
            }
        }

        if let Some(capture) = REGEX_PERFORMANCES.captures(out) {
            if let Some(value) = capture.get(1) {
                result.performances = Some(value.as_str().to_string());
            }
        }

        result
    }
}

impl DBV {
    pub(super) const NOT_SET: &'static str = "[not set]";

    pub(super) fn set_py_experiment_filename<P: AsRef<std::path::Path>>(&mut self, path: P) {
        if let Err(e) = self
            .py_experiment
            .set_filename(path)
            .context("failed to set filename for python experiments")
        {
            self.py_experiment.unset_filename(); // Unset because setting failed
            self.status_msg.error_debug(e);
        }
    }

    pub(super) fn ui_run_py_experiment(&mut self, ui: &mut egui::Ui) {
        ui.collapsing("Run Python Experiment", |ui| {
            ui.collapsing("Algorithms", |ui| {
                for i in 0..SelectedAlgorithms::LEN {
                    let (text, checked) = self.py_experiment.selected_algorithms.get_mut(i);
                    ui.checkbox(checked, text);
                }
            });

            ui.separator();
            let tip_on_how_to_set = "Use Save or Load to set filename";
            ui.horizontal(|ui| {
                ui.label("Data filename: ").on_hover_text(tip_on_how_to_set);
                if let Some(text) = self.py_experiment.data_filename() {
                    ui.label(text).on_hover_text(tip_on_how_to_set);
                    if ui.button("Unset Filename").clicked() {
                        self.py_experiment.unset_filename();
                    }
                } else {
                    ui.label(Self::NOT_SET).on_hover_text(tip_on_how_to_set);
                }
                if self.py_experiment.data_filename().is_some() {
                    ui.separator();
                    // TODO 3: Only save if needed and only show warning if save is pending (Can check undo to see if save is needed, will need to add an ID)
                    // TODO 3: Add colors to make this more visible
                    ui.strong("Warning: Overwrites data file on run");
                }
            });

            ui.separator();
            ui.horizontal(|ui| {
                ui.label("venv activation file:");
                if let Some(file_name) = &self.py_experiment.venv_activate_filename {
                    let mut should_use_venv = true;
                    ui.checkbox(&mut should_use_venv, "");
                    ui.label(file_name);

                    if !should_use_venv {
                        self.py_experiment.venv_activate_filename = None;
                    }
                } else {
                    ui.label(Self::NOT_SET);
                }
                ui.separator();
                if ui.button("Browse...").clicked() {
                    self.browse_for_activation_file();
                }
                ui.hyperlink_to(
                    "Python Docs",
                    "https://docs.python.org/3/library/venv.html#how-venvs-work",
                );
                ui.label("(Only bash/zsh supported for now)");
            });

            ui.separator();
            ui.horizontal(|ui| {
                ui.label("Data directory:");
                if let Some(folder_name) = self.py_experiment.data_dir() {
                    let mut should_use_folder = true;
                    ui.checkbox(&mut should_use_folder, "");
                    ui.label(folder_name);

                    if !should_use_folder {
                        self.py_experiment
                            .set_data_dir(None)
                            .expect("should always be able to set to None");
                    }
                } else {
                    ui.label(Self::NOT_SET);
                }
                ui.separator();
                if ui.button("Browse...").clicked() {
                    self.browse_for_data_dir();
                }
                ui.label("(Folder in experimental_framework for data)");
            });

            ui.separator();
            ui.horizontal(|ui| {
                let not_ready_reasons = self.py_experiment.not_ready_reasons();
                if self.op_state.is_running_py_experiment() {
                    ui.spinner();
                } else {
                    self.ui_run_py_button(ui, &not_ready_reasons);
                }

                if !not_ready_reasons.is_empty() {
                    ui.separator();
                    ui.label(format!(
                        "Unable to run because: {}",
                        not_ready_reasons.to_delimited_string()
                    ));
                }
            });
        });
    }

    pub(super) fn ui_run_py_button(
        &mut self,
        ui: &mut egui::Ui,
        not_ready_reasons: &[NotReadyReason],
    ) {
        self.ui_generic_run_button(
            ui,
            not_ready_reasons.is_empty(),
            egui::Button::new("Run Experiment"),
            Self::run_py_experiment,
        );
    }

    pub(super) fn run_py_experiment(&mut self, ctx: egui::Context) {
        debug_assert!(self.op_state.is_normal());
        let mut status_msg = self.status_msg.clone(); // Clone is cheap because type uses an arc internally
        let py_experiment = self.py_experiment.clone();
        let points = self.data.clone_points();
        self.op_state = OperationalState::RunningPyExperiment(execute(async move {
            let result = match py_experiment
                .run(&points, &mut status_msg)
                .await
                .context("python experiment run failed")
            {
                Ok(()) => OperationOutcome::Success(Payload::PyRun),
                Err(e) => OperationOutcome::Failed(e),
            };
            ctx.request_repaint();
            result
        }));
    }

    fn browse_for_activation_file(&mut self) {
        if let Some(file_name) =
            Self::browse_for_file("Select venv activation script", &mut self.status_msg)
        {
            self.py_experiment.venv_activate_filename = Some(file_name);
        }
    }

    fn browse_for_data_dir(&mut self) {
        if let Some(file_name) = Self::browse_for_folder("Select Data Folder", &mut self.status_msg)
        {
            if let Err(e) = self
                .py_experiment
                .set_data_dir(Some(file_name))
                .context("failed to set data_dir")
            {
                self.status_msg.error_display(e);
            };
        }
    }

    fn browse_for_file(title: &str, status_msg: &mut StatusMsg) -> Option<String> {
        let path = rfd::FileDialog::new().set_title(title).pick_file()?;
        match path.to_str() {
            Some(s) => Some(s.to_string()),
            None => {
                status_msg.error_display(
                    "Unable to convert selected filename to string. (Invalid UTF-8?)",
                );
                None
            }
        }
    }

    fn browse_for_folder(title: &str, status_msg: &mut StatusMsg) -> Option<String> {
        // TODO 4: Refactor to merge with `browse_for_file``
        let path = rfd::FileDialog::new().set_title(title).pick_folder()?;
        match path.to_str() {
            Some(s) => Some(s.to_string()),
            None => {
                status_msg.error_display(
                    "Unable to convert selected folder name to string. (Invalid UTF-8?)",
                );
                None
            }
        }
    }
}
