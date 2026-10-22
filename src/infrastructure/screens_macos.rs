//! AppKit screen coordinates converted to the top-left logical coordinates Iced uses.

use crate::application::positioning::Screen;
use objc2::MainThreadMarker;
use objc2_app_kit::NSScreen;
use objc2_core_foundation::{CFRetained, CFUUID};
use objc2_foundation::{NSNumber, NSString};
use std::ptr::NonNull;

#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn CGDisplayCreateUUIDFromDisplayID(display: u32) -> Option<NonNull<CFUUID>>;
}

fn display_uuid(screen: &NSScreen) -> Option<String> {
    let description = screen.deviceDescription();
    let number = description.objectForKey(&NSString::from_str("NSScreenNumber"))?;
    let display = number.downcast_ref::<NSNumber>()?.as_u32();
    // SAFETY: ColorSync accepts a numeric display ID and returns an optional owned CFUUID (Create rule).
    let raw = unsafe { CGDisplayCreateUUIDFromDisplayID(display) }?;
    // SAFETY: The non-null Create-rule return owns one reference, released by CFRetained.
    let uuid = unsafe { CFRetained::from_raw(raw) };
    CFUUID::new_string(None, Some(&uuid)).map(|text| text.to_string())
}

pub fn screens() -> Vec<Screen> {
    let Some(main_thread) = MainThreadMarker::new() else {
        return Vec::new();
    };
    let screens = NSScreen::screens(main_thread);
    let Some(primary) = screens.firstObject() else {
        return Vec::new();
    };
    let desktop_top = primary.frame().origin.y + primary.frame().size.height;
    let mut result = Vec::new();
    for screen in screens.iter() {
        let frame = screen.visibleFrame();
        let name = screen.localizedName().to_string();
        let Some(id) = display_uuid(&screen) else {
            continue;
        };
        result.push(Screen {
            id,
            name,
            x: frame.origin.x as f32,
            y: (desktop_top - frame.origin.y - frame.size.height) as f32,
            width: frame.size.width as f32,
            height: frame.size.height as f32,
        });
    }
    result
}
