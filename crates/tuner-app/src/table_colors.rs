use std::time::Duration;

use eframe::egui;

use super::{format_f64_with_precision, school_help, TableColorMode, TableColorSettings};

pub(super) fn color_range_for_iter(values: impl IntoIterator<Item = f64>) -> Option<(f64, f64)> {
    let mut finite = values.into_iter().filter(|value| value.is_finite());
    let first = finite.next()?;
    let mut minimum = first;
    let mut maximum = first;
    for value in finite {
        minimum = minimum.min(value);
        maximum = maximum.max(value);
    }
    Some((minimum, maximum))
}

pub fn color_range_for_values(values: &[f64]) -> Option<(f64, f64)> {
    color_range_for_iter(values.iter().copied())
}

pub fn table_cell_fill(
    settings: &TableColorSettings,
    value: f64,
    observed_range: Option<(f64, f64)>,
) -> Option<egui::Color32> {
    if settings.mode == TableColorMode::Off || !value.is_finite() {
        return None;
    }
    let (minimum, maximum) = match settings.mode {
        TableColorMode::AutoRange => observed_range?,
        TableColorMode::FixedRange => {
            let minimum = settings
                .fixed_min
                .filter(|value| value.is_finite())
                .or_else(|| observed_range.map(|range| range.0))?;
            let maximum = settings
                .fixed_max
                .filter(|value| value.is_finite())
                .or_else(|| observed_range.map(|range| range.1))?;
            (minimum, maximum)
        }
        TableColorMode::Off => return None,
    };
    let fraction = if minimum < maximum {
        ((value - minimum) / (maximum - minimum)).clamp(0.0, 1.0) as f32
    } else {
        0.5
    };
    Some(interpolate_table_color(settings, fraction))
}

fn interpolate_table_color(settings: &TableColorSettings, fraction: f32) -> egui::Color32 {
    let fraction = fraction.clamp(0.0, 1.0);
    if fraction <= 0.5 {
        blend_table_color(settings.low, settings.middle, fraction * 2.0)
    } else {
        blend_table_color(settings.middle, settings.high, (fraction - 0.5) * 2.0)
    }
}

pub(super) fn blend_table_color(start: [u8; 4], end: [u8; 4], fraction: f32) -> egui::Color32 {
    let fraction = fraction.clamp(0.0, 1.0);
    let channel = |start: u8, end: u8| {
        (f32::from(start) + (f32::from(end) - f32::from(start)) * fraction)
            .round()
            .clamp(0.0, 255.0) as u8
    };
    egui::Color32::from_rgba_unmultiplied(
        channel(start[0], end[0]),
        channel(start[1], end[1]),
        channel(start[2], end[2]),
        channel(start[3], end[3]),
    )
}

pub(super) fn selected_cell_sweep_progress(time_seconds: f64) -> f32 {
    if !time_seconds.is_finite() {
        return 0.0;
    }
    let phase = time_seconds.rem_euclid(2.0);
    if phase <= 1.0 {
        phase as f32
    } else {
        (2.0 - phase) as f32
    }
}

pub(super) fn selected_cell_sweep_intensity(
    column: usize,
    left: usize,
    right: usize,
    progress: f32,
) -> f32 {
    let progress = progress.clamp(0.0, 1.0);
    if right <= left {
        return 0.28 + 0.42 * (1.0 - (progress - 0.5).abs() * 2.0);
    }

    let span = (right - left) as f32;
    let position = column.saturating_sub(left).min(right - left) as f32;
    let radius = (span * 0.32).max(1.0);
    let falloff = (1.0 - (position - progress * span).abs() / radius).clamp(0.0, 1.0);
    falloff * falloff * (3.0 - 2.0 * falloff) * 0.78
}

pub(super) fn selected_cell_sweep_fill(base: egui::Color32, intensity: f32) -> egui::Color32 {
    const SWEEP_ACCENT: [u8; 4] = [120, 215, 255, 255];
    blend_table_color(
        base.to_array(),
        SWEEP_ACCENT,
        (intensity * 0.5).clamp(0.0, 0.39),
    )
}

pub(super) fn selected_sweep_progress(
    context: &egui::Context,
    enabled: bool,
    has_selection: bool,
) -> Option<f32> {
    if !enabled || !has_selection {
        return None;
    }
    context.request_repaint_after(Duration::from_millis(33));
    Some(selected_cell_sweep_progress(
        context.input(|input| input.time),
    ))
}

pub(super) fn table_coloring_menu(
    ui: &mut egui::Ui,
    settings: &mut TableColorSettings,
    observed_range: Option<(f64, f64)>,
    decimal_places: u8,
    school_me_mode: bool,
    help_id: school_help::SchoolHelpId,
) {
    ui.label("Cell coloring");
    for (mode, label) in [
        (TableColorMode::Off, "Off"),
        (TableColorMode::AutoRange, "Auto range"),
        (TableColorMode::FixedRange, "Fixed range"),
    ] {
        let response = ui.selectable_label(settings.mode == mode, label);
        school_help::show_custom_for_response(ui.ctx(), &response, school_me_mode, help_id);
        if response.clicked() {
            settings.mode = mode;
        }
    }
    if settings.mode == TableColorMode::Off {
        return;
    }

    ui.separator();
    ui.label("Palette");
    ui.horizontal(|ui| {
        ui.label("Low");
        let response = ui.color_edit_button_srgba_unmultiplied(&mut settings.low);
        school_help::show_custom_for_response(ui.ctx(), &response, school_me_mode, help_id);
    });
    ui.horizontal(|ui| {
        ui.label("Middle");
        let response = ui.color_edit_button_srgba_unmultiplied(&mut settings.middle);
        school_help::show_custom_for_response(ui.ctx(), &response, school_me_mode, help_id);
    });
    ui.horizontal(|ui| {
        ui.label("High");
        let response = ui.color_edit_button_srgba_unmultiplied(&mut settings.high);
        school_help::show_custom_for_response(ui.ctx(), &response, school_me_mode, help_id);
    });
    let reset = ui.button("Reset thermal palette");
    school_help::show_custom_for_response(ui.ctx(), &reset, school_me_mode, help_id);
    if reset.clicked() {
        let mode = settings.mode;
        *settings = TableColorSettings::default();
        settings.mode = mode;
    }

    if settings.mode == TableColorMode::FixedRange {
        ui.separator();
        ui.label("Fixed thresholds");
        let observed_minimum = observed_range.map(|range| range.0).unwrap_or(0.0);
        let observed_maximum = observed_range.map(|range| range.1).unwrap_or(1.0);
        let mut minimum = settings.fixed_min.unwrap_or(observed_minimum);
        let mut maximum = settings.fixed_max.unwrap_or(observed_maximum);
        ui.horizontal(|ui| {
            ui.label("Min");
            let response = ui.add(egui::DragValue::new(&mut minimum).speed(0.1));
            school_help::show_custom_for_response(ui.ctx(), &response, school_me_mode, help_id);
            if response.changed() {
                settings.fixed_min = Some(minimum);
            }
        });
        ui.horizontal(|ui| {
            ui.label("Max");
            let response = ui.add(egui::DragValue::new(&mut maximum).speed(0.1));
            school_help::show_custom_for_response(ui.ctx(), &response, school_me_mode, help_id);
            if response.changed() {
                settings.fixed_max = Some(maximum);
            }
        });
    }

    if let Some((minimum, maximum)) = observed_range {
        ui.separator();
        ui.horizontal(|ui| {
            for (label, fraction) in [("Low", 0.0), ("Mid", 0.5), ("High", 1.0)] {
                let color = interpolate_table_color(settings, fraction);
                ui.label(egui::RichText::new(label).background_color(color));
            }
        });
        ui.small(format!(
            "Observed {}–{}",
            format_f64_with_precision(minimum, decimal_places),
            format_f64_with_precision(maximum, decimal_places)
        ));
    }
    settings.sanitize();
}
