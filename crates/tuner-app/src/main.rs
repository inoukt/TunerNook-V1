use eframe::egui;
use tuner_app::{window_geometry::fit_root_window, TunerApp, APP_TITLE};

fn main() -> eframe::Result<()> {
    let desired = egui::vec2(1200.0, 620.0);
    let minimum = egui::vec2(800.0, 480.0);
    let work_area = primary_work_area_points();
    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size(desired)
        .with_min_inner_size(minimum)
        .with_clamp_size_to_monitor_size(true);
    if let Some(work_area) = work_area {
        let fitted = fit_root_window(work_area, desired, minimum);
        viewport = viewport
            .with_position(fitted.outer_position)
            .with_inner_size(fitted.inner_size)
            .with_min_inner_size(fitted.minimum_inner_size);
    }
    let options = eframe::NativeOptions {
        viewport,
        centered: work_area.is_none(),
        ..Default::default()
    };
    eframe::run_native(
        APP_TITLE,
        options,
        Box::new(|creation| Ok(Box::new(TunerApp::new(creation)))),
    )
}

#[cfg(target_os = "windows")]
#[repr(C)]
struct WinRect {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

#[cfg(target_os = "windows")]
#[link(name = "user32")]
unsafe extern "system" {
    fn SystemParametersInfoW(
        action: u32,
        parameter: u32,
        output: *mut std::ffi::c_void,
        flags: u32,
    ) -> i32;
    fn GetDpiForSystem() -> u32;
}

#[cfg(target_os = "windows")]
fn primary_work_area_points() -> Option<egui::Rect> {
    const SPI_GETWORKAREA: u32 = 0x0030;

    let mut rect = WinRect {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };
    if unsafe { SystemParametersInfoW(SPI_GETWORKAREA, 0, &mut rect as *mut _ as *mut _, 0) } == 0 {
        return None;
    }
    let scale = 96.0 / unsafe { GetDpiForSystem() }.max(1) as f32;
    Some(egui::Rect::from_min_max(
        egui::pos2(rect.left as f32 * scale, rect.top as f32 * scale),
        egui::pos2(rect.right as f32 * scale, rect.bottom as f32 * scale),
    ))
}

#[cfg(not(target_os = "windows"))]
fn primary_work_area_points() -> Option<egui::Rect> {
    None
}
