//! Browser client implementation.

use cef::*;
use crate::debug;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use crate::runtime::RuntimeServices;
use crate::browser_registry::{BrowserRegistry, BrowserType};
use crate::chrome_commands::KuroganeCommandHandler;
use crate::ipc::{FrameId, IpcRouter};

//
// LifeSpanHandler
//
wrap_life_span_handler! {
    pub struct KuroganeLifeSpanHandler {
        browser_registry: Arc<Mutex<BrowserRegistry>>,
        is_closing: Arc<AtomicBool>,
        router: Arc<IpcRouter>,
        // What the browsers of this client are, popups aside
        browser_type: BrowserType,
        // The application's new-window policy, if it set one
        new_window: Option<crate::new_window::NewWindowHandler>,
    }

    impl LifeSpanHandler {
        fn on_after_created(&self, browser: Option<&mut Browser>) {
            let Some(browser) = browser else {
                return;
            };
            debug!("on_after_created cef_id={}", browser.identifier());

            let mut reg = self.browser_registry.lock().unwrap();

            // A popup shares its opener's client. The BrowserView delegate
            // classifies a Views popup exactly; CEF does not promise which of
            // the two sees it first, and the first registers it
            let (browser_type, opener) = match self.browser_type {
                // Whatever Chromium opens on its own stays that kind
                BrowserType::ChromeUi => (BrowserType::ChromeUi, None),
                _ if browser.is_popup() != 0 => {
                    let opener = browser
                        .host()
                        .and_then(|host| reg.find_id_by_cef_id(host.opener_identifier()));
                    (BrowserType::Popup, opener)
                }
                browser_type => (browser_type, None),
            };

            reg.ensure_registered(browser, browser_type, opener);
        }

        #[allow(clippy::too_many_arguments)]
        fn on_before_popup(
            &self,
            _browser: Option<&mut Browser>,
            frame: Option<&mut Frame>,
            _popup_id: ::std::os::raw::c_int,
            target_url: Option<&CefString>,
            _target_frame_name: Option<&CefString>,
            target_disposition: WindowOpenDisposition,
            user_gesture: ::std::os::raw::c_int,
            _popup_features: Option<&PopupFeatures>,
            _window_info: Option<&mut WindowInfo>,
            _client: Option<&mut Option<Client>>,
            _settings: Option<&mut BrowserSettings>,
            _extra_info: Option<&mut Option<DictionaryValue>>,
            _no_javascript_access: Option<&mut ::std::os::raw::c_int>,
        ) -> ::std::os::raw::c_int {
            let Some(policy) = &self.new_window else {
                return 0;
            };
            let request = crate::NewWindowRequest {
                url: target_url.map(|url| url.to_string()).unwrap_or_default(),
                opener_url: frame
                    .map(|frame| CefString::from(&frame.url()).to_string())
                    .unwrap_or_default(),
                disposition: target_disposition.into(),
                user_gesture: user_gesture != 0,
            };
            let action = policy(&request);
            debug!("[LifeSpan] new window {} -> {:?}", request.url, action);

            let (cancel, external) = crate::new_window::popup_outcome(action);
            if external && let Err(err) = crate::open_external(&request.url) {
                eprintln!("kurogane: {err}");
            }
            cancel as ::std::os::raw::c_int
        }

        fn do_close(&self, _browser: Option<&mut Browser>) -> i32 {
            let reg = self.browser_registry.lock().unwrap();
            if reg.count() == 1 {
                self.is_closing.store(true, Ordering::Release);
            }
            0
        }

        fn on_before_close(&self, browser: Option<&mut Browser>) {
            let Some(browser) = browser else {
                return;
            };
            debug!("on_before_close cef_id={}", browser.identifier());

            let (browser_id, stragglers) = {
                let mut reg = self.browser_registry.lock().unwrap();
                let Some(id) = reg.find_id_by_browser(browser) else {
                    return;
                };
                let was_app_browser = reg
                    .get(id)
                    .is_some_and(|state| state.metadata.browser_type != BrowserType::ChromeUi);

                reg.unregister(id);
                debug!("Browser {} destroyed", id.as_u32());

                if reg.is_empty() {
                    debug!("[BrowserRegistry] last browser removed, quitting message loop");

                    // quit_message_loop() is only meaningful when CEF owns the main loop
                    // In embedded mode the host event loop owns shutdown and this call is effectively a no-op
                    // TODO: Move shutdown coordination behind a single runtime lifecycle abstraction instead of mixing quit_message_loop() and shutdown_signal

                    quit_message_loop();
                }

                // Windows Chromium opened on its own close with the
                // application's last one rather than keep the process running
                let stragglers = if was_app_browser && !reg.has_app_browsers() {
                    reg.chrome_ui_browsers()
                } else {
                    Vec::new()
                };

                (id, stragglers)
            };

            for straggler in stragglers {
                if let Some(host) = straggler.host() {
                    host.close_browser(1);
                }
            }

            // Cancel any pending async handlers for this browser
            self.router.cancel_all_for_browser(browser_id);
        }
    }
}

//
// LOAD HANDLER
//
wrap_load_handler! {
    pub struct KuroganeLoadHandler {
        router: Arc<IpcRouter>,
    }

    impl LoadHandler {
        fn on_load_start(
            &self,
            _browser: Option<&mut Browser>,
            frame: Option<&mut Frame>,
            _transition_type: TransitionType,
        ) {
            let Some(frame) = frame else {
                return;
            };
            let u: CefString = (&frame.url()).into();
            debug!("[LoadHandler] START {}", u.to_string());
            // Reset state when the frame loads a new document
            self.router.clear_for_frame(&FrameId::of(frame));
        }

        fn on_load_end(
            &self,
            _browser: Option<&mut Browser>,
            frame: Option<&mut Frame>,
            http_status_code: i32,
        ) {
            if let Some(f) = frame {
                let u: CefString = (&f.url()).into();
                debug!("[LoadHandler] END {} status={}", u.to_string(), http_status_code);
            }
        }

        fn on_load_error(
            &self,
            _browser: Option<&mut Browser>,
            _frame: Option<&mut Frame>,
            error_code: Errorcode,
            error_text: Option<&CefString>,
            failed_url: Option<&CefString>,
        ) {
            let err = error_text.map(|s| s.to_string()).unwrap_or_default();
            let url = failed_url.map(|s| s.to_string()).unwrap_or_default();
            debug!("[LoadHandler] ERROR {:?} '{}' {}", error_code, err, url);
        }
    }
}

//
// CLIENT
//
wrap_client! {
    pub struct KuroganeClient {
        services: Arc<RuntimeServices>,
        is_closing: Arc<AtomicBool>,
        // What the browsers of this client are, popups aside
        browser_type: BrowserType,
    }

    impl Client {
        fn command_handler(&self) -> Option<CommandHandler> {
            Some(KuroganeCommandHandler::new(self.services.clone()))
        }

        fn load_handler(&self) -> Option<LoadHandler> {
            Some(KuroganeLoadHandler::new(self.services.router.clone()))
        }

        fn life_span_handler(&self) -> Option<LifeSpanHandler> {
            Some(KuroganeLifeSpanHandler::new(
                self.services.browser_registry.clone(),
                self.is_closing.clone(),
                self.services.router.clone(),
                self.browser_type,
                self.services.new_window.clone(),
            ))
        }

        fn download_handler(&self) -> Option<DownloadHandler> {
            crate::download::handler(self.services.downloads)
        }

        fn on_process_message_received(
            &self,
            browser: Option<&mut Browser>,
            frame: Option<&mut Frame>,
            source_process: ProcessId,
            message: Option<&mut ProcessMessage>,
        ) -> i32 {
            // Only handle messages from renderer
            if source_process != ProcessId::RENDERER {
                return 0;
            }

            let (Some(browser), Some(frame), Some(msg)) = (browser, frame, message) else {
                debug!("[IPC Browser] message without a browser, frame or body");
                return 0;
            };

            // Renderer-controlled bytes drive everything below; a panic must
            // not unwind across this CEF callback and abort the process
            let handled = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                // Resolve browser identity from the registry
                let browser_id = {
                    let reg = self.services.browser_registry.lock().unwrap();
                    reg.find_id_by_browser(browser)
                };
                crate::ipc::handle_ipc_message(browser, frame, msg, &self.services.router, browser_id)
            }));
            match handled {
                Ok(true) => 1,
                Ok(false) => 0,
                Err(_) => {
                    debug!("[IPC Browser] dispatch panicked; message dropped");
                    1
                }
            }
        }
    }
}

impl Drop for KuroganeClient {
    fn drop(&mut self) {
        debug!("KuroganeClient dropped");
    }
}
