//! Native selection integration. Text outside the selection remains on this device.
//!
//! macOS Accessibility gives us a retained reference to the original input. Every
//! write checks that reference, its process, its value, and its selection again.
//! We never synthesize copy/paste keystrokes or inspect the clipboard.

use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    AccessibilityPermission,
    InputMonitoringPermission,
    NoSelection,
    SecureField,
    Unsupported,
    Changed,
    System,
}

#[derive(Debug, Clone)]
pub struct PlatformError {
    pub kind: ErrorKind,
    message: String,
}

impl PlatformError {
    fn new(kind: ErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
}

impl fmt::Display for PlatformError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for PlatformError {}

/// Capture happens on a background worker before a UI notification is delivered.
/// The event tap itself only queues a trigger and never blocks on Accessibility.
#[derive(Debug)]
pub enum Trigger {
    SelectionDrag(Result<SelectionSnapshot, PlatformError>),
    Shortcut(Result<SelectionSnapshot, PlatformError>),
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg(any(target_os = "macos", test))]
struct TextRange {
    location: isize,
    length: isize,
}

/// AX ranges count UTF-16 code units, not Rust bytes or Unicode scalar values.
#[cfg(any(target_os = "macos", test))]
fn replace_utf16(value: &str, range: TextRange, replacement: &str) -> Option<String> {
    let start = usize::try_from(range.location).ok()?;
    let len = usize::try_from(range.length).ok()?;
    let end = start.checked_add(len)?;
    let units: Vec<u16> = value.encode_utf16().collect();
    if end > units.len() {
        return None;
    }
    // Decode the two sides independently, refusing ranges splitting a surrogate.
    let mut result = String::from_utf16(&units[..start]).ok()?;
    String::from_utf16(&units[start..end]).ok()?;
    result.push_str(replacement);
    result.push_str(&String::from_utf16(&units[end..]).ok()?);
    Some(result)
}

#[cfg(any(target_os = "macos", test))]
fn selected_utf16(value: &str, range: TextRange) -> Option<String> {
    let start = usize::try_from(range.location).ok()?;
    let end = start.checked_add(usize::try_from(range.length).ok()?)?;
    let units: Vec<u16> = value.encode_utf16().collect();
    String::from_utf16(units.get(start..end)?).ok()
}

#[cfg(target_os = "macos")]
#[path = "platform/macos.rs"]
mod macos;
#[cfg(target_os = "macos")]
pub use macos::*;

#[cfg(not(target_os = "macos"))]
mod unsupported {
    use super::*;
    use std::sync::mpsc::Sender;

    #[derive(Debug)]
    pub struct SelectionSnapshot {
        pub selected_text: String,
        pub pid: i32,
    }
    #[derive(Debug)]
    pub struct Replacement;
    pub struct ListenerHandle;
    impl ListenerHandle {
        pub fn set_enabled(&self, _: bool) {}
        pub fn set_option_drag(&self, _: bool) {}
    }
    fn unsupported() -> PlatformError {
        PlatformError::new(
            ErrorKind::Unsupported,
            "In-place selection translation currently requires macOS. You can still translate text in the Lingo window.",
        )
    }
    pub fn is_accessibility_trusted() -> bool {
        false
    }
    pub fn is_input_monitoring_allowed() -> bool {
        false
    }
    pub fn request_accessibility_permission() -> bool {
        false
    }
    pub fn request_input_monitoring_permission() -> bool {
        false
    }
    pub fn open_accessibility_settings() -> Result<(), PlatformError> {
        Err(unsupported())
    }
    pub fn open_input_monitoring_settings() -> Result<(), PlatformError> {
        Err(unsupported())
    }
    pub fn start_global_listener(_: Sender<Trigger>) -> Result<ListenerHandle, PlatformError> {
        Err(unsupported())
    }
    pub fn capture_selection() -> Result<SelectionSnapshot, PlatformError> {
        Err(unsupported())
    }
    pub fn replace_selection(_: &SelectionSnapshot, _: &str) -> Result<Replacement, PlatformError> {
        Err(unsupported())
    }
    pub fn undo_replacement(_: &Replacement) -> Result<(), PlatformError> {
        Err(unsupported())
    }
}
#[cfg(not(target_os = "macos"))]
pub use unsupported::*;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn utf16_replacement_preserves_surrounding_multilingual_text() {
        let value = "Hello 👋，世界！";
        let range = TextRange {
            location: 9,
            length: 2,
        };
        assert_eq!(selected_utf16(value, range).as_deref(), Some("世界"));
        assert_eq!(
            replace_utf16(value, range, "world").as_deref(),
            Some("Hello 👋，world！")
        );
    }
    #[test]
    fn malformed_or_split_surrogate_ranges_are_rejected() {
        for range in [
            TextRange {
                location: -1,
                length: 1,
            },
            TextRange {
                location: 0,
                length: -1,
            },
            TextRange {
                location: isize::MAX,
                length: isize::MAX,
            },
            TextRange {
                location: 1,
                length: 1,
            },
            TextRange {
                location: 0,
                length: 1,
            },
            TextRange {
                location: 5,
                length: 1,
            },
        ] {
            assert!(replace_utf16("👋x", range, "hello").is_none());
            assert!(selected_utf16("👋x", range).is_none());
        }
    }
    #[test]
    fn combining_marks_and_newlines_keep_their_exact_units() {
        let range = TextRange {
            location: 2,
            length: 2,
        };
        assert_eq!(
            replace_utf16("a\ne\u{301}!", range, "é").as_deref(),
            Some("a\né!")
        );
    }
}
