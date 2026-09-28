//! macOS-specific CEF initialization.

use std::ffi::CString;
use std::os::unix::ffi::OsStrExt;
use std::sync::{Arc, OnceLock};

use kurogane_layout::detect_cef_root_with_version;
use objc2::{
    ClassType, MainThreadMarker, MainThreadOnly, define_class, msg_send,
    rc::Retained,
    runtime::{AnyObject, Bool, NSObject, NSObjectProtocol, ProtocolObject},
};
use objc2_app_kit::{
    NSApp, NSApplication, NSApplicationActivationPolicy, NSApplicationDelegate,
    NSApplicationTerminateReply,
};

use crate::error::RuntimeError;
use crate::platform::macos::application::SimpleApplication;
use crate::runtime::RuntimeServices;

/// Runtime services used by the Objective-C `terminate:` override.
static SERVICES: OnceLock<Arc<RuntimeServices>> = OnceLock::new();

/// Registers runtime services for the application `terminate:` override.
pub fn set_services(services: Arc<RuntimeServices>) {
    let _ = SERVICES.set(services);
}

/// Loads CEF and, in the browser process, installs the required
/// `NSApplication` subclass and makes an unbundled process a regular
/// foreground app.
///
/// Uses the runtime-resolved CEF root rather than the app-bundle-only loader
/// path used by `cef::library_loader`.
///
/// CEF's subprocesses (`--type=renderer`, `gpu-process`, `utility`) get the
/// library alone: an `NSApplication` registers its process with LaunchServices
/// as an application, and inside a bundle every subprocess would then own a
/// Dock tile of its own. CEF's helper executables install none.
///
/// Under [`SandboxMode::Chromium`](crate::SandboxMode::Chromium) subprocesses
/// enter the seatbelt sandbox before the framework is loaded.
///
/// Must run on the main thread before CEF initialization.
pub fn init_ns_app(sandbox: crate::SandboxMode) -> Result<(), RuntimeError> {
    let browser = crate::runtime::is_browser_process();

    if !browser && matches!(sandbox, crate::SandboxMode::Chromium) {
        crate::sandbox::macos::initialize_helper()?;
    }

    load_framework()?;

    if !browser {
        return Ok(());
    }

    let mtm = MainThreadMarker::new().expect("init_ns_app must run on the main thread");

    unsafe {
        let _: Retained<AnyObject> = msg_send![SimpleApplication::class(), sharedApplication];
    }

    let app = NSApp(mtm);
    assert!(app.isKindOfClass(SimpleApplication::class()));

    promote_unbundled(&app);

    Ok(())
}

/// Ensures unbundled browser processes use a foreground activation policy.
///
/// Bundled applications retain the policy declared by their `Info.plist`.
fn promote_unbundled(app: &NSApplication) {
    if app.activationPolicy() == NSApplicationActivationPolicy::Prohibited {
        app.setActivationPolicy(NSApplicationActivationPolicy::Regular);
    }
}

/// Loads the Chromium Embedded Framework from the resolved CEF root.
fn load_framework() -> Result<(), RuntimeError> {
    // The library loader assumes an app-bundle layout
    // (<exe>/../Frameworks/...), which is unavailable in non-bundled dev runs
    let detected = detect_cef_root_with_version(None).map_err(|_| RuntimeError::CefNotInstalled)?;

    let framework = detected
        .root
        .join(cef::sys::FRAMEWORK_PATH)
        .canonicalize()
        .map_err(|_| RuntimeError::CefNotInstalled)?;

    let framework = CString::new(framework.as_os_str().as_bytes())
        .map_err(|_| RuntimeError::InvalidCefInstallation("invalid framework path".into()))?;

    let loaded = unsafe { cef::sys::cef_load_library(framework.as_ptr()) };
    if loaded != 1 {
        return Err(RuntimeError::InvalidCefInstallation(
            "failed to load Chromium Embedded Framework".into(),
        ));
    }

    Ok(())
}

/// Installs the application delegate for the process lifetime.
///
/// The delegate must be installed on the main thread after CEF initialization.
pub fn setup_app_delegate() {
    let mtm = MainThreadMarker::new().expect("Not running on the main thread");
    let app = NSApp(mtm);
    assert!(app.isKindOfClass(SimpleApplication::class()));

    let delegate = SimpleAppDelegate::new(mtm);
    let delegate_proto =
        ProtocolObject::<dyn NSApplicationDelegate>::from_retained(delegate.clone());
    app.setDelegate(Some(&delegate_proto));

    assert!(
        app.delegate()
            .unwrap()
            .isKindOfClass(SimpleAppDelegate::class())
    );

    // NSApplication does not retain its delegate. Keep the retained handle alive
    // until process exit so it outlives CEF initialization
    std::mem::forget(delegate);
}

/// Installs the default App, Edit and Window menus.
///
/// Must run on the main thread after CEF initialization, in the browser process.
pub fn install_default_menu() {
    use objc2::runtime::Sel;
    use objc2_app_kit::{NSEventModifierFlags, NSMenu, NSMenuItem, NSRunningApplication};
    use objc2_foundation::NSString;

    use crate::menu::{Item, default_menus};

    let mtm = MainThreadMarker::new().expect("install_default_menu must run on the main thread");
    let app = NSApp(mtm);

    let app_name = NSRunningApplication::currentApplication()
        .localizedName()
        .map(|name| name.to_string())
        .or_else(|| {
            std::env::current_exe().ok().and_then(|exe| {
                exe.file_stem()
                    .map(|stem| stem.to_string_lossy().into_owned())
            })
        })
        .unwrap_or_else(|| "Application".to_owned());

    let bar = NSMenu::new(mtm);
    for menu in default_menus(&app_name) {
        let submenu = NSMenu::initWithTitle(NSMenu::alloc(mtm), &NSString::from_str(&menu.title));
        for item in &menu.items {
            let entry = match item {
                Item::Separator => NSMenuItem::separatorItem(mtm),
                Item::Action {
                    title,
                    selector,
                    key,
                    modifiers,
                } => {
                    let selector =
                        Sel::register(&CString::new(*selector).expect("selector has no NUL"));
                    // SAFETY: the selector travels the responder chain, which
                    // ignores it when nothing implements it
                    let entry = unsafe {
                        NSMenuItem::initWithTitle_action_keyEquivalent(
                            NSMenuItem::alloc(mtm),
                            &NSString::from_str(title),
                            Some(selector),
                            &NSString::from_str(key),
                        )
                    };
                    let mut mask = NSEventModifierFlags::Command;
                    if modifiers.shift {
                        mask |= NSEventModifierFlags::Shift;
                    }
                    if modifiers.option {
                        mask |= NSEventModifierFlags::Option;
                    }
                    entry.setKeyEquivalentModifierMask(mask);
                    entry
                }
            };
            submenu.addItem(&entry);
        }

        let holder = NSMenuItem::new(mtm);
        holder.setSubmenu(Some(&submenu));
        bar.addItem(&holder);
        if menu.is_window_menu {
            app.setWindowsMenu(Some(&submenu));
        }
    }
    app.setMainMenu(Some(&bar));
}

define_class! {
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    pub struct SimpleAppDelegate;

    unsafe impl NSObjectProtocol for SimpleAppDelegate {}

    unsafe impl NSApplicationDelegate for SimpleAppDelegate {
        #[unsafe(method(applicationShouldTerminate:))]
        unsafe fn application_should_terminate(&self, _sender: &NSApplication) -> NSApplicationTerminateReply {
            NSApplicationTerminateReply::TerminateNow
        }

        /// Ignores dock reopen requests while the application is running.
        #[unsafe(method(applicationShouldHandleReopen:hasVisibleWindows:))]
        unsafe fn application_should_handle_reopen(&self, _sender: &NSApplication, _has_visible_windows: Bool) -> Bool {
            Bool::NO
        }

        /// Enables secure state restoration encoding.
        ///
        /// Prevents macOS from restoring stale windows after an unclean shutdown.
        #[unsafe(method(applicationSupportsSecureRestorableState:))]
        unsafe fn application_supports_secure_restorable_state(&self, _sender: &NSApplication) -> Bool {
            Bool::YES
        }
    }
}

impl SimpleAppDelegate {
    fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let this = SimpleAppDelegate::alloc(mtm).set_ivars(());
        unsafe { msg_send![super(this), init] }
    }
}

mod application {
    use std::cell::Cell;

    use cef::application_mac::{CefAppProtocol, CrAppControlProtocol, CrAppProtocol};
    use objc2::{
        DefinedClass, define_class, extern_methods, msg_send,
        runtime::{AnyObject, Bool},
    };
    use objc2_app_kit::{NSApplication, NSEvent};

    use super::SERVICES;
    use crate::runtime::close_all_browsers_and_windows;

    /// CEF-compatible `NSApplication` subclass.
    #[derive(Default)]
    pub struct SimpleApplicationIvars {
        handling_send_event: Cell<Bool>,
    }

    define_class! {
        #[unsafe(super(NSApplication))]
        #[ivars = SimpleApplicationIvars]
        pub struct SimpleApplication;

        impl SimpleApplication {
            #[unsafe(method(sendEvent:))]
            unsafe fn send_event(&self, event: &NSEvent) {
                let was_sending_event = self.is_handling_send_event();
                if !was_sending_event {
                    self.set_handling_send_event(true);
                }

                let _: () = msg_send![super(self), sendEvent:event];

                if !was_sending_event {
                    self.set_handling_send_event(false);
                }
            }

            /// Converts application termination into orderly browser shutdown.
            ///
            /// Cocoa's default `terminate:` implementation exits the process,
            /// which prevents CEF from leaving the run loop and completing shutdown.
            /// Closing all browsers instead lets the normal CEF shutdown path run.
            #[unsafe(method(terminate:))]
            unsafe fn terminate(&self, _sender: &AnyObject) {
                if let Some(services) = SERVICES.get() {
                    close_all_browsers_and_windows(
                        &services.browser_registry,
                        &services.window_registry,
                    );
                }
            }
        }

        unsafe impl CrAppControlProtocol for SimpleApplication {
            #[unsafe(method(setHandlingSendEvent:))]
            unsafe fn _set_handling_send_event(&self, value: Bool) {
                self.ivars().handling_send_event.set(value);
            }
        }

        unsafe impl CrAppProtocol for SimpleApplication {
            #[unsafe(method(isHandlingSendEvent))]
            unsafe fn _is_handling_send_event(&self) -> Bool {
                self.ivars().handling_send_event.get()
            }
        }

        unsafe impl CefAppProtocol for SimpleApplication {}
    }

    impl SimpleApplication {
        extern_methods! {
            #[unsafe(method(setHandlingSendEvent:))]
            fn set_handling_send_event(&self, handling_send_event: bool);

            #[unsafe(method(isHandlingSendEvent))]
            fn is_handling_send_event(&self) -> bool;
        }
    }
}
