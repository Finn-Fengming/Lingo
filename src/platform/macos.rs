use super::{ErrorKind, PlatformError, TextRange, Trigger, replace_utf16, selected_utf16};
use std::{
    ffi::c_void,
    fmt, ptr,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc::{self, Sender},
    },
    thread,
    time::{Duration, Instant},
};

type CFRef = *const c_void;
type AXError = i32;
const UTF8: u32 = 0x0800_0100;
const AX_RANGE: u32 = 4;
const MAX_LOCAL_TEXT_UNITS: isize = 2 * 1024 * 1024;
const SHIFT: u64 = 1 << 17;
const CONTROL: u64 = 1 << 18;
const OPTION: u64 = 1 << 19;
const COMMAND: u64 = 1 << 20;

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct Point {
    x: f64,
    y: f64,
}

#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn AXIsProcessTrusted() -> u8;
    fn AXIsProcessTrustedWithOptions(options: CFRef) -> u8;
    static kAXTrustedCheckOptionPrompt: CFRef;
    fn AXUIElementCreateSystemWide() -> CFRef;
    fn AXUIElementGetTypeID() -> usize;
    fn AXUIElementGetPid(element: CFRef, pid: *mut i32) -> AXError;
    fn AXUIElementCopyAttributeValue(
        element: CFRef,
        attribute: CFRef,
        value: *mut CFRef,
    ) -> AXError;
    fn AXUIElementIsAttributeSettable(
        element: CFRef,
        attribute: CFRef,
        settable: *mut u8,
    ) -> AXError;
    fn AXUIElementSetAttributeValue(element: CFRef, attribute: CFRef, value: CFRef) -> AXError;
    fn AXUIElementSetMessagingTimeout(element: CFRef, timeout: f32) -> AXError;
    fn AXValueGetTypeID() -> usize;
    fn AXValueGetValue(value: CFRef, kind: u32, output: *mut c_void) -> u8;
    fn AXValueCreate(kind: u32, value: *const c_void) -> CFRef;
}

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFRetain(value: CFRef) -> CFRef;
    fn CFRelease(value: CFRef);
    fn CFEqual(a: CFRef, b: CFRef) -> u8;
    fn CFGetTypeID(value: CFRef) -> usize;
    fn CFStringGetTypeID() -> usize;
    fn CFBooleanGetTypeID() -> usize;
    fn CFBooleanGetValue(value: CFRef) -> u8;
    fn CFArrayGetTypeID() -> usize;
    fn CFArrayGetCount(value: CFRef) -> isize;
    fn CFStringCreateWithBytes(
        allocator: CFRef,
        bytes: *const u8,
        count: isize,
        encoding: u32,
        external: u8,
    ) -> CFRef;
    fn CFStringGetLength(value: CFRef) -> isize;
    fn CFStringGetCharacters(value: CFRef, range: TextRange, buffer: *mut u16);
    fn CFDictionaryCreate(
        allocator: CFRef,
        keys: *const CFRef,
        values: *const CFRef,
        count: isize,
        key_callbacks: CFRef,
        value_callbacks: CFRef,
    ) -> CFRef;
    static kCFBooleanTrue: CFRef;
    static kCFRunLoopDefaultMode: CFRef;
    fn CFRunLoopGetCurrent() -> CFRef;
    fn CFRunLoopAddSource(run_loop: CFRef, source: CFRef, mode: CFRef);
    fn CFRunLoopRemoveSource(run_loop: CFRef, source: CFRef, mode: CFRef);
    fn CFRunLoopRunInMode(mode: CFRef, seconds: f64, return_after_source: u8) -> i32;
    fn CFRunLoopStop(run_loop: CFRef);
    fn CFMachPortCreateRunLoopSource(allocator: CFRef, port: CFRef, order: isize) -> CFRef;
    fn CFMachPortInvalidate(port: CFRef);
}

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGPreflightListenEventAccess() -> bool;
    fn CGRequestListenEventAccess() -> bool;
    fn CGEventTapCreate(
        location: u32,
        place: u32,
        options: u32,
        mask: u64,
        callback: extern "C" fn(*mut c_void, u32, *mut c_void, *mut c_void) -> *mut c_void,
        user: *mut c_void,
    ) -> CFRef;
    fn CGEventTapEnable(tap: CFRef, enabled: bool);
    fn CGEventGetFlags(event: CFRef) -> u64;
    fn CGEventGetLocation(event: CFRef) -> Point;
    fn CGEventGetIntegerValueField(event: CFRef, field: u32) -> i64;
}

/// Owns one Create/Copy retain. Immutable AX references can cross threads; AX
/// operations use IPC and CoreFoundation ownership is independent of the caller.
/// Mutating a target is only performed by replace_selection / undo_replacement.
struct OwnedCF(CFRef);
unsafe impl Send for OwnedCF {}
unsafe impl Sync for OwnedCF {}
impl Drop for OwnedCF {
    fn drop(&mut self) {
        unsafe { CFRelease(self.0) }
    }
}
impl Clone for OwnedCF {
    fn clone(&self) -> Self {
        Self(unsafe { CFRetain(self.0) })
    }
}
impl OwnedCF {
    fn from_raw(raw: CFRef) -> Result<Self, PlatformError> {
        if raw.is_null() {
            Err(PlatformError::new(
                ErrorKind::System,
                "macOS returned an empty Accessibility object.",
            ))
        } else {
            Ok(Self(raw))
        }
    }
}

/// The full value is private, never formatted in diagnostics, and never sent to
/// a provider. It is used solely as a local concurrency/overwrite guard.
#[derive(Clone)]
pub struct SelectionSnapshot {
    pub selected_text: String,
    pub pid: i32,
    element: OwnedCF,
    value: String,
    range: TextRange,
}
impl fmt::Debug for SelectionSnapshot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SelectionSnapshot")
            .field("pid", &self.pid)
            .field("selected_utf16_units", &self.range.length)
            .finish_non_exhaustive()
    }
}

pub struct Replacement {
    original: SelectionSnapshot,
    after_value: String,
    after_range: TextRange,
    inserted_range: TextRange,
}
impl fmt::Debug for Replacement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Replacement")
            .field("pid", &self.original.pid)
            .finish_non_exhaustive()
    }
}

fn cf_string(value: &str) -> Result<OwnedCF, PlatformError> {
    OwnedCF::from_raw(unsafe {
        CFStringCreateWithBytes(ptr::null(), value.as_ptr(), value.len() as isize, UTF8, 0)
    })
}

fn ax_error(code: AXError, operation: &str) -> PlatformError {
    let (kind, reason) = match code {
        -25211 => (
            ErrorKind::AccessibilityPermission,
            "Accessibility access is disabled. Enable Lingo in System Settings → Privacy & Security → Accessibility.",
        ),
        -25204 => (
            ErrorKind::Unsupported,
            "The selected app did not respond to Accessibility. Try its standard text editor or another app.",
        ),
        -25205 | -25206 | -25208 | -25212 => (
            ErrorKind::Unsupported,
            "This input does not expose editable selected text to macOS Accessibility. Use Lingo's text window instead.",
        ),
        -25202 => (
            ErrorKind::Changed,
            "The original input is no longer available. Select the text again.",
        ),
        _ => (ErrorKind::System, "macOS could not complete the operation."),
    };
    PlatformError::new(kind, format!("{reason} ({operation}, AX {code})"))
}

fn attribute(element: CFRef, name: &str) -> Result<OwnedCF, PlatformError> {
    let name_cf = cf_string(name)?;
    let mut result = ptr::null();
    let code = unsafe { AXUIElementCopyAttributeValue(element, name_cf.0, &mut result) };
    if code != 0 {
        return Err(ax_error(code, name));
    }
    OwnedCF::from_raw(result)
}

fn text_attribute(element: CFRef, name: &str) -> Result<String, PlatformError> {
    let value = attribute(element, name)?;
    if unsafe { CFGetTypeID(value.0) != CFStringGetTypeID() } {
        return Err(PlatformError::new(
            ErrorKind::Unsupported,
            "This input does not expose a plain text value.",
        ));
    }
    let length = unsafe { CFStringGetLength(value.0) };
    if !(0..=MAX_LOCAL_TEXT_UNITS).contains(&length) {
        return Err(PlatformError::new(
            ErrorKind::Unsupported,
            "This input is too large to safely verify. Select text in a smaller input.",
        ));
    }
    let mut units = vec![0_u16; length as usize];
    unsafe {
        CFStringGetCharacters(
            value.0,
            TextRange {
                location: 0,
                length,
            },
            units.as_mut_ptr(),
        )
    };
    String::from_utf16(&units).map_err(|_| {
        PlatformError::new(
            ErrorKind::Unsupported,
            "The app exposed invalid Unicode text; Lingo left the input untouched.",
        )
    })
}

fn selection_range(element: CFRef) -> Result<TextRange, PlatformError> {
    if attribute(element, "AXSelectedTextRanges").is_ok_and(|ranges| unsafe {
        CFGetTypeID(ranges.0) == CFArrayGetTypeID() && CFArrayGetCount(ranges.0) > 1
    }) {
        return Err(PlatformError::new(
            ErrorKind::Unsupported,
            "Select one continuous text range. Multiple selections are left untouched.",
        ));
    }
    let value = attribute(element, "AXSelectedTextRange")?;
    let mut range = TextRange {
        location: 0,
        length: 0,
    };
    if unsafe {
        CFGetTypeID(value.0) != AXValueGetTypeID()
            || AXValueGetValue(value.0, AX_RANGE, (&mut range as *mut TextRange).cast()) == 0
    } {
        return Err(PlatformError::new(
            ErrorKind::Unsupported,
            "This app does not expose a verifiable text selection.",
        ));
    }
    Ok(range)
}

fn is_settable(element: CFRef, name: &str) -> bool {
    let Ok(name) = cf_string(name) else {
        return false;
    };
    let mut settable = 0;
    unsafe { AXUIElementIsAttributeSettable(element, name.0, &mut settable) == 0 && settable != 0 }
}

fn set_attribute(element: CFRef, name: &str, value: CFRef) -> Result<(), PlatformError> {
    let name_cf = cf_string(name)?;
    let code = unsafe { AXUIElementSetAttributeValue(element, name_cf.0, value) };
    if code == 0 {
        Ok(())
    } else {
        Err(ax_error(code, name))
    }
}

fn focused_element() -> Result<OwnedCF, PlatformError> {
    if !is_accessibility_trusted() {
        return Err(PlatformError::new(
            ErrorKind::AccessibilityPermission,
            "Enable Lingo in System Settings → Privacy & Security → Accessibility, then try again.",
        ));
    }
    let system = OwnedCF::from_raw(unsafe { AXUIElementCreateSystemWide() })?;
    unsafe { AXUIElementSetMessagingTimeout(system.0, 0.6) };
    let focused = attribute(system.0, "AXFocusedUIElement")?;
    if unsafe { CFGetTypeID(focused.0) != AXUIElementGetTypeID() } {
        return Err(PlatformError::new(
            ErrorKind::Unsupported,
            "No editable input is focused.",
        ));
    }
    Ok(focused)
}

fn pid(element: CFRef) -> Result<i32, PlatformError> {
    let mut pid = 0;
    let code = unsafe { AXUIElementGetPid(element, &mut pid) };
    if code == 0 {
        Ok(pid)
    } else {
        Err(ax_error(code, "input process"))
    }
}

fn ensure_not_secure(element: CFRef) -> Result<(), PlatformError> {
    let role = text_attribute(element, "AXRole").unwrap_or_default();
    let subrole = text_attribute(element, "AXSubrole").unwrap_or_default();
    let protected = attribute(element, "AXContainsProtectedContent").is_ok_and(|value| unsafe {
        CFGetTypeID(value.0) == CFBooleanGetTypeID() && CFBooleanGetValue(value.0) != 0
    });
    if protected
        || role.contains("Secure")
        || subrole.contains("Secure")
        || subrole.contains("Password")
    {
        return Err(PlatformError::new(
            ErrorKind::SecureField,
            "Password and secure inputs are never read or translated.",
        ));
    }
    if !matches!(role.as_str(), "AXTextField" | "AXTextArea" | "AXComboBox") {
        return Err(PlatformError::new(
            ErrorKind::Unsupported,
            "Select text inside an editable text field. This control is not a supported input.",
        ));
    }
    Ok(())
}

pub fn capture_selection() -> Result<SelectionSnapshot, PlatformError> {
    capture_selection_for_process(None)
}

fn capture_selection_for_process(
    expected_pid: Option<i64>,
) -> Result<SelectionSnapshot, PlatformError> {
    let element = focused_element()?;
    let pid = pid(element.0)?;
    if expected_pid.is_some_and(|expected| expected > 0 && expected != i64::from(pid)) {
        return Err(PlatformError::new(
            ErrorKind::Changed,
            "Focus changed after the translation gesture. Select the text again.",
        ));
    }
    ensure_not_secure(element.0)?;
    if pid == std::process::id() as i32 {
        return Err(PlatformError::new(
            ErrorKind::Unsupported,
            "Use the Translate button for text inside Lingo, or select text in another app.",
        ));
    }
    let range = selection_range(element.0)?;
    if range.length <= 0 {
        return Err(PlatformError::new(
            ErrorKind::NoSelection,
            "Select the words you want to translate first.",
        ));
    }
    if !is_settable(element.0, "AXSelectedText") {
        return Err(PlatformError::new(
            ErrorKind::Unsupported,
            "This app does not allow safe replacement of selected text. Use Lingo's text window instead.",
        ));
    }
    let selected_text = text_attribute(element.0, "AXSelectedText")?;
    let value = text_attribute(element.0, "AXValue")?;
    if selected_text.trim().is_empty() {
        return Err(PlatformError::new(
            ErrorKind::NoSelection,
            "Select some text to translate.",
        ));
    }
    if selected_utf16(&value, range).as_deref() != Some(&selected_text) {
        return Err(PlatformError::new(
            ErrorKind::Unsupported,
            "This app reports inconsistent selection offsets. Lingo left the input untouched.",
        ));
    }
    let snapshot = SelectionSnapshot {
        selected_text,
        pid,
        element,
        value,
        range,
    };
    // Catch edits made while the individual AX attributes were being read.
    verify_target(&snapshot, &snapshot.value, snapshot.range)?;
    Ok(snapshot)
}

fn verify_target(
    snapshot: &SelectionSnapshot,
    expected_value: &str,
    expected_range: TextRange,
) -> Result<(), PlatformError> {
    let current = focused_element()?;
    if pid(current.0)? != snapshot.pid || unsafe { CFEqual(current.0, snapshot.element.0) == 0 } {
        return Err(PlatformError::new(
            ErrorKind::Changed,
            "Focus moved to another input. Your translation is ready in Lingo; the original text was left untouched.",
        ));
    }
    ensure_not_secure(current.0)?;
    let range = selection_range(current.0)?;
    let value = text_attribute(current.0, "AXValue")?;
    if range != expected_range || value != expected_value {
        return Err(PlatformError::new(
            ErrorKind::Changed,
            "The text or selection changed while translating. Your translation is ready in Lingo; nothing was overwritten.",
        ));
    }
    Ok(())
}

pub fn replace_selection(
    snapshot: &SelectionSnapshot,
    replacement: &str,
) -> Result<Replacement, PlatformError> {
    if replacement.is_empty() {
        return Err(PlatformError::new(
            ErrorKind::System,
            "The translation was empty. Your text was left untouched.",
        ));
    }
    let after_value =
        replace_utf16(&snapshot.value, snapshot.range, replacement).ok_or_else(|| {
            PlatformError::new(
                ErrorKind::Unsupported,
                "The selected range cannot be safely replaced.",
            )
        })?;
    let translated = cf_string(replacement)?;
    verify_target(snapshot, &snapshot.value, snapshot.range)?;
    // SelectedText changes only the selected range and preserves the rest of the
    // input, including rich-text formatting. Never replace the entire AXValue.
    set_attribute(snapshot.element.0, "AXSelectedText", translated.0)?;
    verify_written_value(snapshot.element.0, &after_value)?;
    let after_range = selection_range(snapshot.element.0)?;
    let inserted_range = TextRange {
        location: snapshot.range.location,
        length: replacement.encode_utf16().count() as isize,
    };
    Ok(Replacement {
        original: snapshot.clone(),
        after_value,
        after_range,
        inserted_range,
    })
}

fn verify_written_value(element: CFRef, expected: &str) -> Result<(), PlatformError> {
    for attempt in 0..4 {
        if text_attribute(element, "AXValue")?.as_str() == expected {
            return Ok(());
        }
        if attempt < 3 {
            thread::sleep(Duration::from_millis(20));
        }
    }
    Err(PlatformError::new(
        ErrorKind::Changed,
        "The editor did not confirm the expected result. Check its current text before trying again; Lingo will not retry the write.",
    ))
}

pub fn undo_replacement(replacement: &Replacement) -> Result<(), PlatformError> {
    let snapshot = &replacement.original;
    verify_target(snapshot, &replacement.after_value, replacement.after_range)?;
    if !is_settable(snapshot.element.0, "AXSelectedTextRange") {
        return Err(PlatformError::new(
            ErrorKind::Unsupported,
            "This app does not allow Lingo to restore the selection. Use the editor's Undo command.",
        ));
    }
    let range = OwnedCF::from_raw(unsafe {
        AXValueCreate(
            AX_RANGE,
            (&replacement.inserted_range as *const TextRange).cast(),
        )
    })?;
    set_attribute(snapshot.element.0, "AXSelectedTextRange", range.0)?;
    // Check again after changing the range, so a concurrent edit cannot cause
    // restoration into a different value or input.
    verify_target(
        snapshot,
        &replacement.after_value,
        replacement.inserted_range,
    )?;
    let original = cf_string(&snapshot.selected_text)?;
    set_attribute(snapshot.element.0, "AXSelectedText", original.0)?;
    verify_written_value(snapshot.element.0, &snapshot.value)
}

pub fn is_accessibility_trusted() -> bool {
    unsafe { AXIsProcessTrusted() != 0 }
}
pub fn is_input_monitoring_allowed() -> bool {
    unsafe { CGPreflightListenEventAccess() }
}
pub fn request_input_monitoring_permission() -> bool {
    unsafe { CGRequestListenEventAccess() }
}
pub fn request_accessibility_permission() -> bool {
    // The dictionary has non-owning callbacks; these two system constants have
    // static lifetimes and remain valid for the synchronous trust check.
    unsafe {
        let keys = [kAXTrustedCheckOptionPrompt];
        let values = [kCFBooleanTrue];
        let Ok(options) = OwnedCF::from_raw(CFDictionaryCreate(
            ptr::null(),
            keys.as_ptr(),
            values.as_ptr(),
            1,
            ptr::null(),
            ptr::null(),
        )) else {
            return false;
        };
        AXIsProcessTrustedWithOptions(options.0) != 0
    }
}
fn open_settings(anchor: &str) -> Result<(), PlatformError> {
    std::process::Command::new("/usr/bin/open")
        .arg(format!(
            "x-apple.systempreferences:com.apple.preference.security?{anchor}"
        ))
        .spawn()
        .map(|_| ())
        .map_err(|_| {
            PlatformError::new(
                ErrorKind::System,
                "Could not open System Settings. Open Privacy & Security manually.",
            )
        })
}
pub fn open_accessibility_settings() -> Result<(), PlatformError> {
    open_settings("Privacy_Accessibility")
}
pub fn open_input_monitoring_settings() -> Result<(), PlatformError> {
    open_settings("Privacy_ListenEvent")
}

#[derive(Clone, Copy)]
enum TriggerKind {
    Drag,
    Shortcut,
}

struct ListenerGates {
    enabled: AtomicBool,
    option_drag: AtomicBool,
    generation: AtomicU64,
}

#[derive(Clone, Copy)]
struct PendingTrigger {
    kind: TriggerKind,
    generation: u64,
    observed: Instant,
    target_pid: i64,
}

impl ListenerGates {
    fn pending(&self, kind: TriggerKind, event: CFRef) -> PendingTrigger {
        PendingTrigger {
            kind,
            generation: self.generation.load(Ordering::Acquire),
            observed: Instant::now(),
            target_pid: unsafe { CGEventGetIntegerValueField(event, 40) },
        }
    }
    fn permits_pending(&self, pending: PendingTrigger) -> bool {
        self.permits(pending.kind) && pending.generation == self.generation.load(Ordering::Acquire)
    }
}
impl ListenerGates {
    fn permits(&self, kind: TriggerKind) -> bool {
        self.enabled.load(Ordering::Acquire)
            && (!matches!(kind, TriggerKind::Drag) || self.option_drag.load(Ordering::Acquire))
    }
}

struct EventContext {
    sender: Sender<PendingTrigger>,
    gates: Arc<ListenerGates>,
    tap: CFRef,
    drag_start: Option<Point>,
    dragged: bool,
}

extern "C" fn event_callback(
    _: *mut c_void,
    event_type: u32,
    event: *mut c_void,
    user: *mut c_void,
) -> *mut c_void {
    // No unwinding may cross this C callback. All normal operations below are
    // non-blocking and only the trigger kind (never typed keys) is retained.
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let context = unsafe { &mut *user.cast::<EventContext>() };
        if matches!(event_type, 0xffff_fffe | 0xffff_ffff) {
            unsafe { CGEventTapEnable(context.tap, true) };
            return;
        }
        if event.is_null() || !context.gates.enabled.load(Ordering::Acquire) {
            context.drag_start = None;
            context.dragged = false;
            return;
        }
        let flags = unsafe { CGEventGetFlags(event) };
        match event_type {
            1 => {
                // left mouse down
                context.drag_start = (flags & OPTION != 0
                    && context.gates.permits(TriggerKind::Drag))
                .then(|| unsafe { CGEventGetLocation(event) });
                context.dragged = false;
            }
            6 => {
                // left mouse dragged
                if let Some(start) = context.drag_start {
                    let point = unsafe { CGEventGetLocation(event) };
                    context.dragged |= (point.x - start.x).hypot(point.y - start.y) >= 4.0;
                }
            }
            2 => {
                // left mouse up
                if context.drag_start.take().is_some()
                    && context.dragged
                    && context.gates.permits(TriggerKind::Drag)
                {
                    let _ = context
                        .sender
                        .send(context.gates.pending(TriggerKind::Drag, event));
                }
                context.dragged = false;
            }
            10 => {
                // key down; macOS virtual key 37 is L
                let modifiers = flags & (SHIFT | CONTROL | OPTION | COMMAND);
                if modifiers == SHIFT | COMMAND
                    && unsafe { CGEventGetIntegerValueField(event, 9) } == 37
                    && unsafe { CGEventGetIntegerValueField(event, 8) } == 0
                {
                    let _ = context
                        .sender
                        .send(context.gates.pending(TriggerKind::Shortcut, event));
                }
            }
            _ => {}
        }
    }));
    // This is a listen-only tap. The original event always continues unchanged.
    event
}

pub struct ListenerHandle {
    run_loop: OwnedCF,
    stop: Arc<AtomicBool>,
    gates: Arc<ListenerGates>,
}
impl ListenerHandle {
    /// Prevents new selection reads while paused, including already queued events.
    pub fn set_enabled(&self, enabled: bool) {
        if self.gates.enabled.swap(enabled, Ordering::AcqRel) != enabled {
            self.gates.generation.fetch_add(1, Ordering::AcqRel);
        }
    }
    pub fn set_option_drag(&self, enabled: bool) {
        if self.gates.option_drag.swap(enabled, Ordering::AcqRel) != enabled {
            self.gates.generation.fetch_add(1, Ordering::AcqRel);
        }
    }
}
impl Drop for ListenerHandle {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        unsafe { CFRunLoopStop(self.run_loop.0) };
        // Thread cleanup happens on its own run loop; never block app shutdown.
    }
}

pub fn start_global_listener(sender: Sender<Trigger>) -> Result<ListenerHandle, PlatformError> {
    if !is_accessibility_trusted() {
        return Err(PlatformError::new(
            ErrorKind::AccessibilityPermission,
            "Enable Accessibility for Lingo to use selection translation.",
        ));
    }
    if !is_input_monitoring_allowed() {
        return Err(PlatformError::new(
            ErrorKind::InputMonitoringPermission,
            "Enable Input Monitoring for Lingo to use Option-drag and ⌘⇧L, then restart Lingo.",
        ));
    }
    let stop = Arc::new(AtomicBool::new(false));
    let gates = Arc::new(ListenerGates {
        enabled: AtomicBool::new(true),
        option_drag: AtomicBool::new(true),
        generation: AtomicU64::new(0),
    });
    let worker_stop = Arc::clone(&stop);
    let worker_gates = Arc::clone(&gates);
    let (trigger_sender, trigger_receiver) = mpsc::channel::<PendingTrigger>();
    thread::Builder::new()
        .name("lingo-selection".into())
        .spawn(move || {
            while !worker_stop.load(Ordering::Acquire) {
                let pending = match trigger_receiver.recv_timeout(Duration::from_millis(100)) {
                    Ok(pending) => pending,
                    Err(mpsc::RecvTimeoutError::Timeout) => continue,
                    Err(_) => break,
                };
                // Old queued events must not accidentally read a later input.
                if pending.observed.elapsed() > Duration::from_millis(500)
                    || !worker_gates.permits_pending(pending)
                {
                    continue;
                }
                // A listen-only tap observes mouse-up before the app necessarily
                // finishes processing it. Allow the selected range to settle.
                thread::sleep(Duration::from_millis(45));
                if worker_stop.load(Ordering::Acquire) {
                    break;
                }
                if !worker_gates.permits_pending(pending) {
                    continue;
                }
                let selection = capture_selection_for_process(Some(pending.target_pid));
                if !worker_gates.permits_pending(pending) {
                    continue;
                }
                let message = match pending.kind {
                    TriggerKind::Drag => Trigger::SelectionDrag(selection),
                    TriggerKind::Shortcut => Trigger::Shortcut(selection),
                };
                if sender.send(message).is_err() {
                    break;
                }
            }
        })
        .map_err(|_| {
            PlatformError::new(
                ErrorKind::System,
                "Could not start the selection capture worker.",
            )
        })?;

    let (ready_sender, ready_receiver) = mpsc::sync_channel::<Result<OwnedCF, PlatformError>>(1);
    let listener_stop = Arc::clone(&stop);
    let listener_gates = Arc::clone(&gates);
    thread::Builder::new().name("lingo-events".into()).spawn(move || {
        let mut context = Box::new(EventContext { sender: trigger_sender, gates: listener_gates, tap: ptr::null(), drag_start: None, dragged: false });
        let mask = (1_u64 << 1) | (1 << 2) | (1 << 6) | (1 << 10);
        // Annotated session events include their destination process ID.
        let raw_tap = unsafe { CGEventTapCreate(2, 0, 1, mask, event_callback, (&mut *context as *mut EventContext).cast()) };
        let Ok(tap) = OwnedCF::from_raw(raw_tap) else {
            let _ = ready_sender.send(Err(PlatformError::new(ErrorKind::InputMonitoringPermission, "macOS could not start the global listener. Enable Input Monitoring and Accessibility for Lingo, then restart it.")));
            return;
        };
        context.tap = tap.0;
        let Ok(source) = OwnedCF::from_raw(unsafe { CFMachPortCreateRunLoopSource(ptr::null(), tap.0, 0) }) else {
            let _ = ready_sender.send(Err(PlatformError::new(ErrorKind::System, "Could not create the macOS event source.")));
            return;
        };
        let run_loop = unsafe { CFRunLoopGetCurrent() };
        unsafe {
            CFRunLoopAddSource(run_loop, source.0, kCFRunLoopDefaultMode);
            CGEventTapEnable(tap.0, true);
        }
        let retained_run_loop = OwnedCF(unsafe { CFRetain(run_loop) });
        if ready_sender.send(Ok(retained_run_loop)).is_ok() {
            while !listener_stop.load(Ordering::Acquire) {
                // A bounded run prevents a stop-before-run race from leaving a
                // listener alive after its handle has already been dropped.
                unsafe { CFRunLoopRunInMode(kCFRunLoopDefaultMode, 0.25, 0) };
            }
        }
        unsafe {
            CGEventTapEnable(tap.0, false);
            CFMachPortInvalidate(tap.0);
            CFRunLoopRemoveSource(run_loop, source.0, kCFRunLoopDefaultMode);
        }
        listener_stop.store(true, Ordering::Release);
        // Context remains alive until the source has been removed and disabled.
        drop(context);
    }).map_err(|_| { stop.store(true, Ordering::Release); PlatformError::new(ErrorKind::System, "Could not start the global event listener.") })?;
    match ready_receiver.recv_timeout(Duration::from_secs(5)) {
        Ok(Ok(run_loop)) => Ok(ListenerHandle {
            run_loop,
            stop,
            gates,
        }),
        Ok(Err(error)) => {
            stop.store(true, Ordering::Release);
            Err(error)
        }
        Err(_) => {
            stop.store(true, Ordering::Release);
            Err(PlatformError::new(
                ErrorKind::System,
                "The global listener did not start. Restart Lingo and check its permissions.",
            ))
        }
    }
}
