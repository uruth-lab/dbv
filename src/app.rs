use std::{fmt::Display, path::PathBuf};

use anyhow::Context;
use ecolor::Color32;
use egui::{Button, Checkbox, Label, Sense, Widget};
use egui_extras::{Column, TableBuilder};
use egui_plot::{Legend, MarkerShape, Plot, PlotBounds, PlotResponse, Points};
use log::{debug, info};

#[cfg(not(target_arch = "wasm32"))]
use crate::app::py_experiment::PyExperiment;

use self::{
    data_conversion::ConvertToSeries as _,
    data_definition::{Data, DataLabel, DataPoint, DistanceCalculation, PointArray, Save as _},
    local_experiments::{
        LocalExperiment, ModelInference, ModelInferenceConfig as _, ModelTrain as _,
        ProximityScore, TrainResults, Trained, UnTrained,
    },
    operational_state::{OperationOutcome, OperationalState, Payload},
    plot_zoom_reset::StatePlotResetZoom,
    prediction_classification::{prediction_classification, Classification},
    status_msg::StatusMsg,
    ui_blocks::OptionEditNumeric,
};

mod data_conversion;
mod data_definition;
mod display_slice;
mod local_experiments;
mod operational_state;
mod plot_zoom_reset;
mod prediction_classification;
#[cfg(not(target_arch = "wasm32"))]
mod py_experiment;
mod status_msg;
mod ui_blocks;

// TODO 3: Add support for adding notes to plot (Separate save button for annotations or save only depending on if we can integrate them, easy to do on file for matlab but csv?)
// TODO 3: Investigate supporting bounding boxes
// TODO 3: Add Ctrl + Z undo and Ctrl + Y redo

/// We derive Deserialize/Serialize so we can persist app state on shutdown.
#[derive(serde::Deserialize, serde::Serialize, PartialEq)]
#[serde(default)] // if we add new fields, give them default values when deserializing old state
pub struct DBV {
    /// Controls the size of the points
    marker_radius: f32,
    color_normal: Color32,
    color_anom: Color32,
    color_results_false_negatives: Color32,
    color_results_false_positives: Color32,
    color_results_true_negatives: Color32,
    color_results_true_positives: Color32,
    data: Data,
    click_mode: ClickMode,
    primary_click_label: DataLabel,
    allow_boxed_zoom: bool,
    show_data_only: bool,
    display_mode: DisplayMode,
    on_load_reset_plot_zoom: bool,
    show_plot_bounds: bool,
    #[cfg(not(target_arch = "wasm32"))]
    py_experiment: PyExperiment,
    loc_experiment: LocalExperiment,
    #[serde(skip)]
    should_show_reset_all_button: bool,
    #[serde(skip)]
    should_show_clear_history: bool,
    #[serde(skip)]
    edit_history: OptionEditNumeric<u16>,
    #[serde(skip)]
    plot_bounds: Option<PlotBounds>,
    #[serde(skip)]
    last_cursor_pos: Option<egui_plot::PlotPoint>,
    #[serde(skip)]
    state_reset_plot_zoom: StatePlotResetZoom,
    #[serde(skip)]
    status_msg: StatusMsg,
    #[serde(skip)]
    op_state: OperationalState,
    #[serde(skip)]
    edit_point: Option<DuringEditPoint>,
}

#[derive(serde::Deserialize, serde::Serialize, PartialEq, Debug)]
struct DuringEditPoint {
    index: usize,
    point: DataPoint,
}

#[derive(serde::Deserialize, serde::Serialize, PartialEq)]
enum ClickMode {
    AddPoints,
    DeletePoints,
}

#[derive(serde::Deserialize, serde::Serialize, PartialEq)]
enum DisplayMode {
    Plot,
    Table,
}

impl ClickMode {
    /// Returns `true` if the click mode is [`DeletePoints`].
    ///
    /// [`DeletePoints`]: ClickMode::DeletePoints
    #[must_use]
    fn is_delete_points(&self) -> bool {
        matches!(self, Self::DeletePoints)
    }
}

impl Default for DBV {
    fn default() -> Self {
        Self {
            marker_radius: 8.0,
            color_normal: Color32::from_rgb(100, 150, 230),
            color_anom: Color32::from_rgb(200, 150, 70),
            color_results_false_negatives: Color32::from_rgb(255, 119, 0),
            color_results_false_positives: Color32::from_rgb(255, 165, 0),
            color_results_true_negatives: Color32::from_rgb(136, 136, 255),
            color_results_true_positives: Color32::from_rgb(0, 0, 255),
            data: Default::default(),
            click_mode: ClickMode::AddPoints,
            primary_click_label: DataLabel::Normal,
            allow_boxed_zoom: false,
            show_data_only: false,
            display_mode: DisplayMode::Plot,
            #[cfg(not(target_arch = "wasm32"))]
            py_experiment: Default::default(),
            loc_experiment: Default::default(),
            should_show_reset_all_button: false,
            should_show_clear_history: false,
            edit_history: OptionEditNumeric::new(
                "Change Max History Size",
                Data::DEFAULT_MAX_HISTORY,
                1,
                0..=u16::MAX,
                "Max History Size: ",
            ),
            plot_bounds: Default::default(),
            last_cursor_pos: Default::default(),
            state_reset_plot_zoom: Default::default(),
            status_msg: Default::default(),
            op_state: Default::default(),
            on_load_reset_plot_zoom: true,
            edit_point: Default::default(),
            show_plot_bounds: false,
        }
    }
}

impl DBV {
    /// Called once before the first frame.
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // This is also where you can customize the look and feel of egui using
        // `cc.egui_ctx.set_visuals` and `cc.egui_ctx.set_fonts`.

        // Load previous app state (if any).
        // Note that you must enable the `persistence` feature for this to work.
        if let Some(storage) = cc.storage {
            info!("Storage found, loading...");
            if let Some(result) = eframe::get_value(storage, eframe::APP_KEY) {
                info!("Loading app data succeeded");
                result
            } else {
                info!("Load failed");
                Default::default()
            }
        } else {
            info!("Storage not found");
            Default::default()
        }
    }

    fn panel_top(&mut self, ui: &mut egui::Ui) {
        self.ui_menu_main(ui);
        if !self.show_data_only {
            self.ui_instructions(ui);

            ui.separator();
            self.ui_run_loc_experiment(ui);
            #[cfg(not(target_arch = "wasm32"))]
            {
                ui.separator();
                self.ui_run_py_experiment(ui);
            }
            ui.separator();
            ui.horizontal(|ui| {
                self.ui_click_mode_display(ui);
                ui.separator();
                self.ui_display_mode(ui);
                ui.separator();
                self.ui_btn_undo_redo(ui);
            });
        }
    }

    fn ui_click_mode_display(&mut self, ui: &mut egui::Ui) {
        let display_text = format!(
            // TODO 3: Add colors for ADD and DELETE
            "Mode: Click to {} point {}",
            match self.click_mode {
                ClickMode::AddPoints => "ADD",
                ClickMode::DeletePoints => "DELETE",
            },
            if self.primary_click_label.is_normal() {
                ""
            } else {
                "(Primary and Secondary Click Swapped)"
            }
        );
        if ui
            .add(Label::new(display_text).sense(Sense::click()))
            .on_hover_text("Click to toggle mode")
            .clicked()
        {
            self.toggle_click_mode();
        }
    }

    fn ui_instructions(&mut self, ui: &mut egui::Ui) {
        ui.collapsing("Instructions", |ui| {
            ui.label("Primary click to add normal point (Usually left click)");
            ui.label("Secondary click to add anomaly point (Usually right click)");
            ui.label("Middle click to switch between adding and removing points");
            ui.label("Pan by dragging, or scroll (+ shift = horizontal).");
            if self.allow_boxed_zoom {
                ui.label("Box zooming: Right click to zoom in and zoom out using a selection.");
            }
            if cfg!(target_arch = "wasm32") {
                ui.label("Zoom with ctrl / ⌘ + pointer wheel, or with pinch gesture.");
            } else if cfg!(target_os = "macos") {
                ui.label("Zoom with ctrl / ⌘ + scroll.");
            } else {
                ui.label("Zoom with ctrl + scroll.");
            }
        });
    }

    fn ui_menu_options(&mut self, ui: &mut egui::Ui) {
        ui.menu_button("Options", |ui| {
            ui.add(
                egui::DragValue::new(&mut self.marker_radius)
                    .speed(0.1)
                    .clamp_range(0.0..=f64::INFINITY)
                    .prefix("Point Display Radius: "),
            );
            ui.menu_button("Points Colors", |ui| {
                ui.horizontal(|ui| {
                    ui.strong("Without Results");
                    ui.separator();
                    ui.label("Normal");
                    ui.color_edit_button_srgba(&mut self.color_normal);

                    ui.separator();
                    ui.label("Anomaly");
                    ui.color_edit_button_srgba(&mut self.color_anom);
                });
                ui.horizontal(|ui| {
                    ui.strong("With Results");

                    ui.separator();
                    ui.label("TP");
                    ui.color_edit_button_srgba(&mut self.color_results_true_positives);

                    ui.separator();
                    ui.label("FP");
                    ui.color_edit_button_srgba(&mut self.color_results_false_positives);

                    ui.separator();
                    ui.label("TN");
                    ui.color_edit_button_srgba(&mut self.color_results_true_negatives);

                    ui.separator();
                    ui.label("FN");
                    ui.color_edit_button_srgba(&mut self.color_results_false_negatives);
                });
            });
            ui.separator();
            let mut should_remove_on_click: bool = self.click_mode.is_delete_points();
            ui.checkbox(&mut should_remove_on_click, "Should remove point on click");
            self.click_mode = if should_remove_on_click {
                ClickMode::DeletePoints
            } else {
                ClickMode::AddPoints
            };

            let mut should_swap_normal_on_click = self.primary_click_label.is_anomaly();
            ui.checkbox(
                &mut should_swap_normal_on_click,
                "Swap Click for Normal and Anomaly",
            );
            self.primary_click_label = if should_swap_normal_on_click {
                DataLabel::Anomaly
            } else {
                DataLabel::Normal
            };

            // Handle setting rounding of new points
            ui.horizontal(|ui| {
                let mut is_rounding_new_points_enabled = self.data.is_rounding_enabled();
                ui.checkbox(
                    &mut is_rounding_new_points_enabled,
                    "Should round new points",
                );
                self.data
                    .set_rounding_enabled(is_rounding_new_points_enabled);
                if is_rounding_new_points_enabled {
                    ui.separator();
                    ui.label("Number of Decimal places: ");
                    ui.add(egui::Slider::new(
                        self.data.rounding_decimal_places_mut(),
                        0..=Data::MAX_DECIMAL_PLACES,
                    ));
                }
            });

            ui.checkbox(&mut self.allow_boxed_zoom, "Allow boxed zoom")
                .on_hover_text("When enabled, instructions include an explanation");

            ui.checkbox(&mut self.show_plot_bounds, "Show plot bounds");

            ui.checkbox(&mut self.on_load_reset_plot_zoom, "On load reset plot zoom");

            ui.horizontal(|ui| {
                ui.checkbox(
                    &mut self.should_show_reset_all_button,
                    "Show Reset ALL Button",
                )
                .on_hover_text("Does not reset the plot's zoom");
                if self.should_show_reset_all_button {
                    egui::reset_button(ui, self);
                }
            });
        });
    }

    fn ui_undo_redo_with_options(&mut self, ui: &mut egui::Ui) {
        ui.add_enabled_ui(self.op_state.is_normal(), |ui| {
            self.ui_btn_undo_redo(ui);
            ui.menu_button("History Options", |ui| {
                ui.menu_button("Clear History", |ui| {
                    ui.add_enabled(
                        self.data.has_history(),
                        Checkbox::new(&mut self.should_show_clear_history, "Clear Data History..."),
                    );
                    if self.should_show_clear_history && ui.button("Confirm Clear").clicked() {
                        self.data.clear_history(&mut self.status_msg);
                        self.should_show_clear_history = false;
                        ui.close_menu();
                    }
                });

                ui.horizontal(|ui| {
                    self.edit_history
                        .update(ui, self.data.max_history_size(), |value| {
                            self.data.set_history_size(value)
                        })
                })
            });
        });
    }

    fn ui_btn_undo_redo(&mut self, ui: &mut egui::Ui) {
        if ui
            .add_enabled(self.data.has_undo(), Button::new("Undo"))
            .clicked()
        {
            self.data.undo(&mut self.status_msg);
            ui.close_menu();
        }
        if ui
            .add_enabled(self.data.has_redo(), Button::new("Redo"))
            .clicked()
        {
            self.data.redo(&mut self.status_msg);
            ui.close_menu();
        }
    }

    fn panel_bottom(&mut self, ui: &mut egui::Ui) {
        ui.label(self.status_msg.msg());
        ui.horizontal(|ui| {
            self.ui_btn_clear_status_msgs(ui);
            self.ui_btn_delete_all_points(ui);
            self.ui_btn_reset_plot_zoom(ui);
            if let Some(pos) = self.last_cursor_pos.as_ref() {
                ui.label(format!("Last Pos: {:.3},{:.3}", pos.x, pos.y));
            }
            if self.show_plot_bounds {
                if let Some(bounds) = self.plot_bounds {
                    ui.label(format!(
                        "Plot bounds: min: {:.02?}, max: {:.02?}",
                        bounds.min(),
                        bounds.max()
                    ));
                }
            }
            match &self.state_reset_plot_zoom {
                StatePlotResetZoom::Set => {
                    ui.label("Plot reset: In Progress");
                }
                StatePlotResetZoom::Wait(_) => {
                    ui.label("Plot reset: Waiting for next step to verify");
                }
                StatePlotResetZoom::Verify(_) => {
                    ui.label("Plot reset: Verifying");
                }
                StatePlotResetZoom::NotRunning => (),
                StatePlotResetZoom::Error(msg) => {
                    ui.label(format!("Plot Reset Failed. Error: {msg}"));
                }
            }
        });
    }

    /// Creates a button to delete all the points and returns true if the button was clicked after doing the action
    fn ui_btn_delete_all_points(&mut self, ui: &mut egui::Ui) -> bool {
        if ui
            .add_enabled(!self.data.is_empty(), Button::new("Delete all points"))
            .clicked()
        {
            self.data.clear_points();
            true
        } else {
            false
        }
    }

    /// Creates a button to clear status messages and returns true if the button was clicked after doing the action of clearing
    fn ui_btn_clear_status_msgs(&mut self, ui: &mut egui::Ui) -> bool {
        if ui
            .add_enabled(
                !self.status_msg.is_empty(),
                Button::new("Clear Status Msgs"),
            )
            .clicked()
        {
            self.status_msg.clear();
            true
        } else {
            false
        }
    }

    fn panel_center(&mut self, ui: &mut egui::Ui) {
        match &self.display_mode {
            DisplayMode::Plot => self.ui_plot(ui),
            DisplayMode::Table => self.ui_table(ui),
        }
    }
    fn ui_plot(&mut self, ui: &mut egui::Ui) {
        let markers_plot = Plot::new("markers")
            .data_aspect(1.0)
            .legend(Legend::default())
            .min_size(egui::Vec2 { x: 100.0, y: 100.0 })
            .allow_boxed_zoom(self.allow_boxed_zoom)
            .allow_double_click_reset(false);

        let PlotResponse {
            response,
            inner: pointer_coordinate,
            ..
        } = markers_plot.show(ui, |plot_ui| {
            let markers = if let Some(model) = self.loc_inference_model() {
                self.markers_w_results(model)
            } else {
                self.markers_wo_results()
            };
            for marker in markers {
                plot_ui.points(marker);
            }
            if !self.state_reset_plot_zoom.is_stopped() {
                self.state_reset_plot_zoom
                    .step(plot_ui, self.data.get_points_min_max_w_margin())
            }
            self.plot_bounds = Some(plot_ui.plot_bounds());
            plot_ui.pointer_coordinate()
        });
        if pointer_coordinate.is_some() {
            self.last_cursor_pos = pointer_coordinate;
        }

        // Needs to have the option to use the last cursor position because on mobile the cursor position
        // doesn't persist after the finger is lifted which is when the click happens
        self.click_handler(&response, pointer_coordinate.or(self.last_cursor_pos));
    }

    fn ui_table(&mut self, ui: &mut egui::Ui) {
        let text_height = egui::TextStyle::Body
            .resolve(ui.style())
            .size
            .max(ui.spacing().interact_size.y);

        let has_inference_model = self.loc_inference_model().is_some();

        let mut table_builder = TableBuilder::new(ui)
            .striped(true)
            .resizable(true)
            .cell_layout(egui::Layout::left_to_right(egui::Align::LEFT))
            .column(Column::auto())
            .column(Column::auto())
            .column(Column::auto())
            .column(Column::auto())
            .column(Column::auto());

        if has_inference_model {
            // Add columns for inference results
            table_builder = table_builder.column(Column::auto()).column(Column::auto());
        }
        table_builder = table_builder.min_scrolled_height(0.0);

        let table = table_builder.header(20.0, |mut header| {
            header.col(|ui| {
                ui.strong("row #");
            });
            header.col(|ui| {
                ui.strong("x0");
            });
            header.col(|ui| {
                ui.strong("x1");
            });
            header.col(|ui| {
                ui.strong("label");
            });
            header.col(|ui| {
                ui.strong(""); // Empty column for buttons
            });
            if has_inference_model {
                header.col(|ui| {
                    ui.strong("prediction");
                });
                header.col(|ui| {
                    ui.strong("classification");
                });
            }
        });

        table.body(|body| {
            body.rows(text_height, self.data.points().len(), |mut row| {
                let row_index = row.index();
                if row_index >= self.data.points().len(){
                    // This should only happen if the delete button was clicked and the last row was visible 
                    // but don't have a good way to keep track if it was clicked so just check if we get an illegal index
                    debug!("Stopping rows from being output on the assumption that the delete button was clicked");
                    return;
                }
                let point = self.data.points()[row_index];
                let DataPoint { x0, x1, label } = point;
                row.col(|ui| {
                    ui.label(row_index.to_string());
                });
                match self.edit_point.as_mut() {
                    Some(x) if x.index == row_index => {
                        fn edit_num<Num: egui::emath::Numeric>(
                            ui: &mut egui::Ui,
                            value: &mut Num,
                            rounding_decimal_places: Option<u8>,
                        ) {
                            let mut drag_value = egui::DragValue::new(value);
                            if let Some(precision) = rounding_decimal_places {
                                drag_value = drag_value.speed(0.1f64.powi(precision as i32));
                            }
                            ui.add(drag_value);
                        }
                        row.col(|ui| {
                            edit_num(ui, &mut x.point.x0, self.data.rounding_decimal_places)
                        });
                        row.col(|ui| {
                            edit_num(ui, &mut x.point.x1, self.data.rounding_decimal_places)
                        });
                        row.col(|ui| {
                            egui::ComboBox::new("id-table-cell-label", "")
                                .selected_text(x.point.label.to_string())
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(
                                        &mut x.point.label,
                                        DataLabel::Normal,
                                        DataLabel::Normal.to_string(),
                                    );
                                    ui.selectable_value(
                                        &mut x.point.label,
                                        DataLabel::Anomaly,
                                        DataLabel::Anomaly.to_string(),
                                    );
                                });
                        });
                    }
                    _ => {
                        row.col(|ui| {
                            ui.label(x0.to_string());
                        });
                        row.col(|ui| {
                            ui.label(x1.to_string());
                        });
                        row.col(|ui| {
                            ui.label(label.to_string());
                        });
                    }
                }
                row.col(|ui| {
                    if let Some(x) = self.edit_point.as_ref() {
                        if x.index == row_index {
                            if ui.button("Save").clicked() {
                                self.data.edit(x.index, x.point);
                                self.edit_point = None;
                            }
                            if ui.button("Cancel").clicked() {
                                self.edit_point = None;
                            }
                        } else {
                            // No buttons if not on the row being edited
                        }
                    } else if ui.button("Edit").clicked() {
                        self.edit_point = Some(DuringEditPoint {
                            index: row_index,
                            point,
                        });
                    } else if ui.button("Delete").clicked() {
                        debug!("Delete clicked on row_index: {row_index}");
                        self.data.delete_by_index(row_index);
                    }
                });
                if has_inference_model {
                    if let Some(model) = self.loc_inference_model() {
                        let predicted = model.prediction_on_training_data(row_index);
                        row.col(|ui| {
                            ui.label(predicted.to_string());
                        });
                        row.col(|ui| {
                            ui.label(prediction_classification(label, predicted).to_string());
                        });
                    }
                }
            });
        });
    }

    fn click_handler(
        &mut self,
        response: &egui::Response,
        pointer_coordinate: Option<egui_plot::PlotPoint>,
    ) {
        if response.clicked() {
            match self.click_mode {
                ClickMode::AddPoints => self.data.add(
                    pointer_coordinate,
                    self.primary_click_label,
                    &mut self.status_msg,
                ),
                ClickMode::DeletePoints => self.data.delete(
                    pointer_coordinate,
                    self.primary_click_label,
                    &mut self.status_msg,
                ),
            }
        }
        if response.secondary_clicked() {
            match self.click_mode {
                ClickMode::AddPoints => self.data.add(
                    pointer_coordinate,
                    self.secondary_click_label(),
                    &mut self.status_msg,
                ),
                ClickMode::DeletePoints => self.data.delete(
                    pointer_coordinate,
                    self.secondary_click_label(),
                    &mut self.status_msg,
                ),
            }
        }
        if response.middle_clicked() {
            self.toggle_click_mode();
        }
    }

    fn toggle_click_mode(&mut self) {
        self.click_mode = match self.click_mode {
            ClickMode::AddPoints => ClickMode::DeletePoints,
            ClickMode::DeletePoints => ClickMode::AddPoints,
        }
    }

    fn secondary_click_label(&self) -> DataLabel {
        match self.primary_click_label {
            DataLabel::Normal => DataLabel::Anomaly,
            DataLabel::Anomaly => DataLabel::Normal,
        }
    }

    fn ui_persistence(&mut self, ui: &mut egui::Ui) {
        // TODO 4: Add support for drag and drop files (see example in egui)
        ui.add_enabled_ui(self.op_state.is_normal(), |ui| {
            if ui.button("Load...").clicked() {
                self.load_data(ui.ctx().clone());
                ui.close_menu();
            }
            if ui.button("Save as...").clicked() {
                self.save_data(ui.ctx().clone());
                ui.close_menu();
            }
        });
    }

    fn save_data(&mut self, ctx: egui::Context) {
        debug_assert!(self.op_state.is_normal());
        let points = self.data.clone_points(); // Cloning seemed to be the most practical way I could think of to get a new copy to send into the closure
        self.op_state = OperationalState::Saving(execute(async move {
            let dialog = rfd::AsyncFileDialog::new().set_title("Save as");
            #[cfg(not(target_arch = "wasm32"))]
            let dialog = dialog.set_directory(PyExperiment::DATA_DIR);
            #[cfg(target_arch = "wasm32")]
            let dialog = dialog.set_file_name("manual_data_creator.csv");
            let Some(file) = dialog.save_file().await else {
                // user canceled
                ctx.request_repaint();
                return OperationOutcome::Cancelled;
            };
            let path = file_handle_to_path(&file);
            let result = match points
                .save_to_file(&file)
                .await
                .context("failed to save file")
            {
                Ok(()) => OperationOutcome::Success(Payload::Save(path)),
                Err(e) => OperationOutcome::Failed(e),
            };

            ctx.request_repaint();

            result
        }));
    }

    fn load_data(&mut self, ctx: egui::Context) {
        debug_assert!(self.op_state.is_normal());
        let mut status_msg = self.status_msg.clone(); // Clone is cheap because type uses an arc internally
        self.op_state = OperationalState::Loading(execute(async move {
            let dialog = rfd::AsyncFileDialog::new().set_title("Load data");
            #[cfg(not(target_arch = "wasm32"))]
            let dialog = dialog.set_directory(PyExperiment::DATA_DIR);
            let Some(file) = dialog.pick_file().await else {
                // user canceled
                ctx.request_repaint();
                return OperationOutcome::Cancelled;
            };
            let path = file_handle_to_path(&file);
            let result = match Data::load_from_file(&file).await.context("failed to load") {
                Ok((loaded_data, load_msg)) => {
                    if let Some(msg) = load_msg {
                        status_msg.info(msg)
                    }
                    OperationOutcome::Success(Payload::Load { loaded_data, path })
                }
                Err(e) => OperationOutcome::Failed(e),
            };

            ctx.request_repaint();

            result
        }));
    }

    fn ui_menu_main(&mut self, ui: &mut egui::Ui) {
        egui::menu::bar(ui, |ui| {
            self.ui_menu_file(ui);
            self.ui_menu_edit(ui);
            self.ui_menu_view(ui);
            self.ui_menu_options(ui);

            ui.add_space(16.0);
            egui::widgets::global_dark_light_mode_buttons(ui);
        });
    }

    fn ui_menu_view(&mut self, ui: &mut egui::Ui) {
        ui.menu_button("View", |ui| {
            ui.checkbox(&mut self.show_data_only, "Show Data Only");
            self.ui_btn_reset_plot_zoom(ui);
        });
    }

    fn ui_btn_reset_plot_zoom(&mut self, ui: &mut egui::Ui) {
        if ui
            .add_enabled(
                self.state_reset_plot_zoom.is_stopped(),
                Button::new("Reset Plot Zoom"),
            )
            .clicked()
        {
            self.state_reset_plot_zoom.start_reset();
            ui.close_menu();
        }
    }

    fn ui_menu_edit(&mut self, ui: &mut egui::Ui) {
        ui.menu_button("Edit", |ui| {
            self.ui_undo_redo_with_options(ui);
            if self.ui_btn_clear_status_msgs(ui) {
                ui.close_menu();
            };
            if self.ui_btn_delete_all_points(ui) {
                ui.close_menu();
            };
        });
    }
    fn ui_generic_run_button(
        &mut self,
        ui: &mut egui::Ui,
        allowed_to_enable: bool,
        widget: impl Widget,
        f: impl FnOnce(&mut Self, egui::Context),
    ) {
        let is_normal_state = self.op_state.is_normal();
        if ui
            .add_enabled(allowed_to_enable && is_normal_state, widget)
            .clicked()
        {
            f(self, ui.ctx().clone());
        }
        if !is_normal_state {
            ui.label("Operation in Progress...");
            ui.spinner();
        }
    }

    fn markers_wo_results(&self) -> Vec<Points> {
        let normal_points = self.data_points_to_egui_points(
            self.data.points().array_of_normal(),
            "Normal",
            MarkerShape::Plus,
            self.color_normal,
        );

        let anom_points = self.data_points_to_egui_points(
            self.data.points().array_of_anom(),
            "Anomalies ",
            MarkerShape::Asterisk,
            self.color_anom,
        );

        vec![normal_points, anom_points]
    }

    fn data_points_to_egui_points<S: Display>(
        &self,
        point_arrays: Vec<PointArray>,
        name: S,
        shape: MarkerShape,
        color: Color32,
    ) -> Points {
        let len = point_arrays.len();
        Points::new(point_arrays)
            .name(format!("{name} ({len})"))
            .radius(self.marker_radius)
            .shape(shape)
            .color(color)
    }

    /// Monitors and updates any tasks that are in progress
    fn update_op_state(&mut self) {
        match &self.op_state {
            OperationalState::Normal => (), // All normal no action needed
            OperationalState::RunningPyExperiment(promise)
            | OperationalState::Saving(promise)
            | OperationalState::Loading(promise)
            | OperationalState::RunningLocExperiment(promise) => {
                if promise.ready().is_some() {
                    let mut temp = OperationalState::default();
                    std::mem::swap(&mut temp, &mut self.op_state);
                    let owned_promise = match temp {
                        OperationalState::RunningPyExperiment(x)
                        | OperationalState::Saving(x)
                        | OperationalState::Loading(x)
                        | OperationalState::RunningLocExperiment(x) => x,
                        OperationalState::Normal => unreachable!(
                            "we matched to get into this code block so should still match"
                        ),
                    };
                    // ASSUMPTION: The way the outcome got here doesn't matter only the value inside of it.
                    //             The outer wrapper is just for UI to update depending on type of operation.
                    let outcome = owned_promise.block_and_take(); // We know the promise is ready at this point
                    #[cfg_attr(target_arch = "wasm32", allow(unused))]
                    match outcome {
                        OperationOutcome::Cancelled => (), // Nothing to do already set back to default in swap (When written this wasn't an expected state)
                        OperationOutcome::Success(payload) => match payload {
                            Payload::PyRun => self.status_msg.info("Python Run succeeded"),
                            Payload::Load { loaded_data, path } => {
                                self.data.replace_with_loaded_data(loaded_data);
                                if self.on_load_reset_plot_zoom {
                                    info!("Resetting plot zoom on load");
                                    self.state_reset_plot_zoom.start_reset();
                                } else {
                                    info!(
                                        "NOT resetting plot zoom on load because configured not to."
                                    );
                                }

                                #[cfg(not(target_arch = "wasm32"))]
                                self.set_py_experiment_filename(path);
                            }
                            Payload::Save(path) => {
                                self.status_msg
                                    .info(format!("Save successfully to {path:?}"));
                                #[cfg(not(target_arch = "wasm32"))]
                                self.set_py_experiment_filename(path);
                            }
                            Payload::Train(results) => {
                                self.status_msg.info("Model training completed");
                                match &self.loc_experiment {
                                    LocalExperiment::None => self.status_msg.error_display(
                                        "failed to save training results. Type set to None",
                                    ),
                                    LocalExperiment::ProximityScoreUntrained(x) => {
                                        self.loc_experiment = LocalExperiment::ProximityScoreTrained(
                                            (&x).to_inference(results),
                                        )
                                    }
                                    LocalExperiment::ProximityScoreTrained(x) => {
                                        self.loc_experiment = LocalExperiment::ProximityScoreTrained(
                                            (&x).to_inference(results),
                                        )
                                    }
                                }
                            }
                        },
                        OperationOutcome::Failed(e) => self.status_msg.error_debug(e),
                    }
                }
            }
        }
    }

    fn ui_display_mode(&mut self, ui: &mut egui::Ui) {
        ui.label("Display Mode");
        ui.radio_value(&mut self.display_mode, DisplayMode::Plot, "Plot");
        ui.radio_value(&mut self.display_mode, DisplayMode::Table, "Table");
    }

    fn ui_run_loc_experiment(&mut self, ui: &mut egui::Ui) {
        ui.collapsing("Run Local Experiment", |ui| {
            if self.op_state.is_running_loc_experiment() {
                ui.spinner();
            } else {
                ui.horizontal(|ui| {
                    ui.label("Algorithm");
                    if ui
                        .add(egui::RadioButton::new(
                            self.loc_experiment.is_none(),
                            "None",
                        ))
                        .clicked()
                    {
                        self.loc_experiment = LocalExperiment::None;
                    }
                    if ui
                        .add(egui::RadioButton::new(
                            self.loc_experiment.is_proximity_score(),
                            "Proximity Score",
                        ))
                        .clicked()
                    {
                        self.loc_experiment =
                            LocalExperiment::ProximityScoreUntrained(ProximityScore::new());
                    }
                });

                // Show configuration options for experiment
                match &mut self.loc_experiment {
                    LocalExperiment::None => (), // Do nothing there are not settings if there is no algorithm
                    LocalExperiment::ProximityScoreUntrained(..) => (), // No training settings for now
                    LocalExperiment::ProximityScoreTrained(..) => (), // No training settings for now
                }

                // If not None show run button
                if !self.loc_experiment.is_none() {
                    ui.horizontal(|ui| {
                        self.ui_generic_run_button(
                            ui,
                            true,
                            Button::new("Train Model"),
                            Self::train_model_wrapper,
                        );
                        // TODO 3: Add colored background using predict_batch functionality (maybe use checkbox to control if it is enabled or not)
                        //    Might be able to use a picture behind the plot with the colors as needed
                        //    And just disable the plot background https://docs.rs/egui_plot/latest/egui_plot/struct.Plot.html#method.show_background
                        self.ui_loc_predict_config(ui);
                    });
                };
            }
        });
    }

    // Needed because error[E0562]: `impl Trait` only allowed in function and inherent method argument and return types, not in variable bindings
    fn train_model_do(
        &mut self,
        f: impl std::future::Future<Output = anyhow::Result<TrainResults>> + Send + 'static,
        ctx: egui::Context,
    ) {
        self.op_state = OperationalState::RunningLocExperiment(execute(async move {
            let result = match f.await.context("failed to train model") {
                Ok(x) => OperationOutcome::Success(Payload::Train(x)),
                Err(e) => OperationOutcome::Failed(e),
            };

            ctx.request_repaint();

            result
        }));
    }

    fn train_model_wrapper(&mut self, ctx: egui::Context) {
        debug_assert!(self.op_state.is_normal());
        let mut status_msg = self.status_msg.clone(); // Clone is cheap because type uses an arc internally
        let points = self.data.clone_points();
        let data_timestamp = self.data.timestamp();
        match &self.loc_experiment {
            LocalExperiment::None => unreachable!("We should never be trying to train None"),
            LocalExperiment::ProximityScoreTrained(x) => {
                // Allow unit binding so if we change the code later it will still work
                #[allow(clippy::let_unit_value)]
                let config_clone = x.train_config_clone();
                self.train_model_do(
                    async move {
                        ProximityScore::<Trained>::train(
                            config_clone,
                            points,
                            data_timestamp,
                            &mut status_msg,
                        )
                        .await
                    },
                    ctx,
                );
            }
            LocalExperiment::ProximityScoreUntrained(x) => {
                // Allow unit binding so if we change the code later it will still work
                #[allow(clippy::let_unit_value)]
                let config_clone = x.train_config_clone();
                self.train_model_do(
                    async move {
                        ProximityScore::<UnTrained>::train(
                            config_clone,
                            points,
                            data_timestamp,
                            &mut status_msg,
                        )
                        .await
                    },
                    ctx,
                );
            }
        }
    }

    fn markers_w_results(&self, model: &dyn ModelInference) -> Vec<Points> {
        let mut false_negatives = vec![];
        let mut false_positives = vec![];
        let mut true_negatives = vec![];
        let mut true_positives = vec![];

        // Sort each point into one of the categories
        for (i, point) in self.data.points().iter().enumerate() {
            let ground_truth = point.label;
            let predicted = model.prediction_on_training_data(i);
            let point_array = point.to_array();
            match prediction_classification(ground_truth, predicted) {
                prediction_classification::Classification::FalseNegative => {
                    false_negatives.push(point_array)
                }
                prediction_classification::Classification::FalsePositive => {
                    false_positives.push(point_array)
                }
                prediction_classification::Classification::TrueNegative => {
                    true_negatives.push(point_array)
                }
                prediction_classification::Classification::TruePositive => {
                    true_positives.push(point_array)
                }
            }
        }

        vec![
            self.data_points_to_egui_points(
                true_positives,
                Classification::TruePositive,
                MarkerShape::Asterisk,
                self.color_results_true_positives,
            ),
            self.data_points_to_egui_points(
                false_positives,
                Classification::FalsePositive,
                MarkerShape::Plus,
                self.color_results_false_positives,
            ),
            self.data_points_to_egui_points(
                true_negatives,
                Classification::TrueNegative,
                MarkerShape::Plus,
                self.color_results_true_negatives,
            ),
            self.data_points_to_egui_points(
                false_negatives,
                Classification::FalseNegative,
                MarkerShape::Asterisk,
                self.color_results_false_negatives,
            ),
        ]
    }

    fn ui_loc_predict_config(&mut self, ui: &mut egui::Ui) {
        if let Some(training_timestamp) = self.loc_experiment.data_timestamp_at_training() {
            ui.separator();
            match training_timestamp.cmp(&self.data.timestamp()) {
                std::cmp::Ordering::Less => {
                    ui.label("Trained for older version of data (It's possible data may no longer be in the history)");
                }
                std::cmp::Ordering::Greater => {
                    ui.label("Trained for newer version of data (It's possible data may no longer be in the history)");
                }
                std::cmp::Ordering::Equal => {
                    // Show Prediction Configuration Options
                    let pred_config = &mut self.loc_experiment;
                    match pred_config {
                        LocalExperiment::None => unreachable!("we can't train None"),
                        LocalExperiment::ProximityScoreUntrained(..) => (), // It has no setting before training
                        LocalExperiment::ProximityScoreTrained(model) => {
                            let config = model.predict_config_mut();
                            ui.horizontal(|ui| {
                                ui.label("Threshold: ");
                                ui.add(egui::Slider::new(
                                    &mut config.threshold,
                                    config.min_score..=config.max_score,
                                ));
                                // TODO 3: Add button to set threshold to best value based on F1
                            });
                        }
                    }
                }
            };
            // TODO 3: Add button to search the history and see if it exists (if it doesn't ask if model should be deleted)
            // It can be missing if it was in the redo list and then a change is made other than redo because then the redo list is lost
        }
    }

    fn loc_inference_model(&self) -> Option<&dyn ModelInference> {
        if !self.loc_experiment.is_at_timestamp(self.data.timestamp()) {
            return None;
        }
        if let Some(result) = self.loc_experiment.model_inference() {
            Some(result)
        } else {
            unreachable!("we just checked that the results exist")
        }
    }

    fn ui_menu_file(&mut self, ui: &mut egui::Ui) {
        ui.menu_button("File", |ui| {
            self.ui_persistence(ui);
            #[cfg(not(target_arch = "wasm32"))] // no File->Quit on web pages!
            if ui.button("Quit").clicked() {
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
            }
        });
    }
}

/// Convenience method to convert FileHandle to PathBuf to keep same code between WASM and Native.
///
/// Paths are not used in the WASM code except for logging so set to the filename only
fn file_handle_to_path(file: &rfd::FileHandle) -> PathBuf {
    #[cfg(not(target_arch = "wasm32"))]
    return file.path().to_path_buf();
    #[cfg(target_arch = "wasm32")]
    return PathBuf::from(file.file_name());
}

impl eframe::App for DBV {
    /// Called by the frame work to save state before shutdown.
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        info!("Saving app data...");
        eframe::set_value(storage, eframe::APP_KEY, self);
    }

    /// Called each time the UI needs repainting, which may be many times per second.
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.update_op_state();

        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            self.panel_top(ui);
        });

        if !self.show_data_only {
            egui::TopBottomPanel::bottom("bottom_panel").show(ctx, |ui| {
                self.panel_bottom(ui);
            });
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            // The central panel the region left after adding TopPanel and BottomPanel
            self.panel_center(ui);
        });
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn execute(
    f: impl std::future::Future<Output = OperationOutcome> + 'static + Send,
) -> operational_state::AwaitingType {
    poll_promise::Promise::spawn_async(f)
}

#[cfg(target_arch = "wasm32")]
fn execute(
    f: impl std::future::Future<Output = OperationOutcome> + 'static,
) -> operational_state::AwaitingType {
    poll_promise::Promise::spawn_local(f)
}
