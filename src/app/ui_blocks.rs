use std::ops::RangeInclusive;

use egui::Button;

#[derive(Debug, PartialEq, Eq)]
pub struct OptionEditNumeric<T>
where
    T: egui::emath::Numeric + std::convert::Into<f64> + Copy,
{
    checkbox_msg: &'static str,
    default_value: T,
    drag_clamp_range: RangeInclusive<T>,
    drag_speed: T,
    drag_value_prefix: &'static str,
    during_edit_value: Option<T>,
    is_editing: bool,
}

impl<T> OptionEditNumeric<T>
where
    T: egui::emath::Numeric + std::convert::Into<f64> + Copy,
{
    pub fn new(
        checkbox_msg: &'static str,
        default_value: T,
        drag_speed: T,
        drag_clamp_range: RangeInclusive<T>,
        drag_value_prefix: &'static str,
    ) -> Self {
        Self {
            is_editing: false,
            checkbox_msg,
            during_edit_value: None,
            default_value,
            drag_speed,
            drag_clamp_range,
            drag_value_prefix,
        }
    }

    pub fn update<F: FnOnce(Option<T>)>(
        &mut self,
        ui: &mut egui::Ui,
        value: Option<T>,
        set_value: F,
    ) {
        ui.checkbox(&mut self.is_editing, self.checkbox_msg);
        if !self.is_editing {
            // Not changing right now keep "starter" value current
            self.during_edit_value = value;
        } else {
            // In process of changing
            let mut enabled = self.during_edit_value.is_some();
            ui.checkbox(&mut enabled, "Set");
            if enabled && self.during_edit_value.is_none() {
                self.during_edit_value = Some(self.default_value);
            } else if !enabled && self.during_edit_value.is_some() {
                self.during_edit_value = None;
            }
            if enabled {
                ui.add(
                    egui::DragValue::new(
                        self.during_edit_value
                            .get_or_insert_with(|| unreachable!("value should be set above")),
                    )
                    .speed(self.drag_speed)
                    .clamp_range(self.drag_clamp_range.clone())
                    .prefix(self.drag_value_prefix),
                );
            }
            if ui
                .add_enabled(self.during_edit_value != value, Button::new("Save Changes"))
                .clicked()
            {
                set_value(self.during_edit_value);
                self.is_editing = false;
            }
            if ui.button("Cancel Changes").clicked() {
                self.is_editing = false;
            }
        }
    }
}
