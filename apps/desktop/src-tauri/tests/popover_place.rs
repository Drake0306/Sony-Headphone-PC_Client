//! Where the Windows popover goes: by the taskbar, centred on the tray icon,
//! always inside the work area. Pure arithmetic, so it runs on every OS.

use shdc_lib::popover::{windows_position, Screen, POPOVER_SIZE};

/// A 2560×1440 monitor at 150 % with a 48 px taskbar at the bottom.
fn bottom_taskbar() -> Screen {
    Screen {
        bounds: (0, 0, 2560, 1440),
        work_area: (0, 0, 2560, 1392),
        scale: 1.5,
    }
}

fn size(scale: f64) -> (i32, i32) {
    (
        (POPOVER_SIZE.0 * scale).round() as i32,
        (POPOVER_SIZE.1 * scale).round() as i32,
    )
}

#[test]
fn sits_just_above_a_bottom_taskbar_centred_on_the_icon() {
    let screen = bottom_taskbar();
    let (w, h) = size(1.5);
    // An icon in the notification area, 1000 px from the right edge so the
    // popover can centre on it without clamping.
    let tray = Some((1560, 1396, 32, 32));
    let (x, y) = windows_position(screen, tray);
    assert_eq!(x, 1560 + 16 - w / 2, "centred on the icon");
    assert_eq!(y + h + 12, 1392, "its bottom is one gap above the taskbar");
}

#[test]
fn icon_near_the_right_edge_keeps_the_popover_on_screen() {
    let screen = bottom_taskbar();
    let (w, h) = size(1.5);
    let (x, y) = windows_position(screen, Some((2500, 1396, 32, 32)));
    assert_eq!(x + w + 12, 2560, "pulled in to the work area's right edge");
    assert_eq!(y + h + 12, 1392);
}

#[test]
fn without_an_icon_rectangle_it_goes_to_the_notification_area_corner() {
    let screen = bottom_taskbar();
    let (w, h) = size(1.5);
    let (x, y) = windows_position(screen, None);
    assert_eq!((x + w + 12, y + h + 12), (2560, 1392));
}

#[test]
fn the_icon_rectangle_never_moves_it_vertically() {
    // Whatever y the shell reports for the icon, a bottom taskbar keeps the
    // popover on the work area's bottom edge (the bug seen on 8 November was a
    // popover far above the taskbar).
    let screen = bottom_taskbar();
    let (_, h) = size(1.5);
    for icon_y in [0, 300, 900, 1396] {
        let (_, y) = windows_position(screen, Some((1560, icon_y, 32, 32)));
        assert_eq!(y + h + 12, 1392, "icon y {icon_y}");
    }
}

#[test]
fn taskbar_at_the_top_puts_it_just_below() {
    let screen = Screen {
        bounds: (0, 0, 1920, 1080),
        work_area: (0, 40, 1920, 1040),
        scale: 1.0,
    };
    let (x, y) = windows_position(screen, Some((1700, 4, 32, 32)));
    assert_eq!(y, 40 + 8);
    assert_eq!(x, 1700 + 16 - 170);
}

#[test]
fn taskbar_on_a_side_hugs_that_side_and_centres_on_the_icon_vertically() {
    let left = Screen {
        bounds: (0, 0, 1920, 1080),
        work_area: (62, 0, 1858, 1080),
        scale: 1.0,
    };
    let (x, y) = windows_position(left, Some((15, 800, 32, 32)));
    assert_eq!(x, 62 + 8);
    assert_eq!(y, 800 + 16 - 210);

    let right = Screen {
        bounds: (0, 0, 1920, 1080),
        work_area: (0, 0, 1858, 1080),
        scale: 1.0,
    };
    let (x, y) = windows_position(right, Some((1873, 800, 32, 32)));
    assert_eq!(x + 340 + 8, 1858);
    assert_eq!(y, 800 + 16 - 210);
}

#[test]
fn second_monitor_offsets_and_scale_are_respected() {
    // A 1080p monitor at 100 % to the left of the primary, negative x.
    let screen = Screen {
        bounds: (-1920, 200, 1920, 1080),
        work_area: (-1920, 200, 1920, 1040),
        scale: 1.0,
    };
    let (x, y) = windows_position(screen, Some((-300, 1250, 24, 24)));
    assert_eq!(x, -300 + 12 - 170);
    assert_eq!(y + 420 + 8, 1240);
    assert!(x >= -1920 + 8 && x + 340 <= -8);
}

#[test]
fn a_tiny_work_area_keeps_the_top_left_corner_visible() {
    let screen = Screen {
        bounds: (0, 0, 300, 300),
        work_area: (0, 0, 300, 260),
        scale: 1.0,
    };
    let (x, y) = windows_position(screen, Some((100, 270, 24, 24)));
    assert_eq!((x, y), (8, 8));
}
