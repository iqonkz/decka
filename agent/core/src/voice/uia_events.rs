//! "Something changed" from UI Automation (TASKS.md T30.5): the focus moved, the tree of the window in front changed, a
//! window opened. The guide looks at the window then, instead of reading it over and over while the person thinks.
//! The events only say *that* something happened; what, is read from a fresh snapshot.

use tokio::sync::mpsc::UnboundedSender;

#[cfg(windows)]
mod win {
    use tokio::sync::mpsc::UnboundedSender;
    use windows::core::{implement, Ref, Result};
    use windows::Win32::Foundation::HWND;
    use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, SAFEARRAY};
    use windows::Win32::UI::Accessibility::{
        CUIAutomation, IUIAutomation, IUIAutomationElement, IUIAutomationEventHandler, IUIAutomationEventHandler_Impl, IUIAutomationFocusChangedEventHandler,
        IUIAutomationFocusChangedEventHandler_Impl, IUIAutomationStructureChangedEventHandler, IUIAutomationStructureChangedEventHandler_Impl, StructureChangeType,
        TreeScope_Subtree, UIA_Window_WindowOpenedEventId, UIA_EVENT_ID,
    };
    use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;

    #[implement(IUIAutomationFocusChangedEventHandler, IUIAutomationStructureChangedEventHandler, IUIAutomationEventHandler)]
    struct Tell(UnboundedSender<()>);

    impl Tell {
        fn tell(&self) -> Result<()> {
            // Nobody listening any more: the subscription is about to be removed.
            let _ = self.0.send(());
            Ok(())
        }
    }

    impl IUIAutomationFocusChangedEventHandler_Impl for Tell_Impl {
        fn HandleFocusChangedEvent(&self, _: Ref<'_, IUIAutomationElement>) -> Result<()> {
            self.tell()
        }
    }

    impl IUIAutomationStructureChangedEventHandler_Impl for Tell_Impl {
        fn HandleStructureChangedEvent(&self, _: Ref<'_, IUIAutomationElement>, _: StructureChangeType, _: *const SAFEARRAY) -> Result<()> {
            self.tell()
        }
    }

    impl IUIAutomationEventHandler_Impl for Tell_Impl {
        fn HandleAutomationEvent(&self, _: Ref<'_, IUIAutomationElement>, _: UIA_EVENT_ID) -> Result<()> {
            self.tell()
        }
    }

    /// Subscribes on this thread, waits for `stop`, unsubscribes.
    pub fn watch(handle: Option<isize>, events: UnboundedSender<()>, stop: std::sync::mpsc::Receiver<()>) {
        // SAFETY: COM on a thread of its own; every COM object is dropped inside `run` before `CoUninitialize`.
        unsafe {
            let initialised = CoInitializeEx(None, COINIT_MULTITHREADED).is_ok();
            let run = || -> Result<()> {
                let automation: IUIAutomation = CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER)?;
                let window = handle.map_or_else(|| GetForegroundWindow(), |h| HWND(h as *mut core::ffi::c_void));
                let focus: IUIAutomationFocusChangedEventHandler = Tell(events.clone()).into();
                let structure: IUIAutomationStructureChangedEventHandler = Tell(events.clone()).into();
                let opened: IUIAutomationEventHandler = Tell(events).into();
                // A program may refuse one kind: the others still count, and the slow look of the guide covers the rest.
                if let Err(e) = automation.AddFocusChangedEventHandler(None, &focus) {
                    tracing::debug!("focus events: {e}");
                }
                if !window.is_invalid() {
                    if let Err(e) = automation.ElementFromHandle(window).and_then(|root| automation.AddStructureChangedEventHandler(&root, TreeScope_Subtree, None, &structure)) {
                        tracing::debug!("structure events: {e}");
                    }
                }
                if let Err(e) = automation.GetRootElement().and_then(|desktop| automation.AddAutomationEventHandler(UIA_Window_WindowOpenedEventId, &desktop, TreeScope_Subtree, None, &opened)) {
                    tracing::debug!("window events: {e}");
                }
                // An error here means the watch was dropped: the same as a stop.
                let _ = stop.recv();
                automation.RemoveAllEventHandlers()
            };
            if let Err(e) = run() {
                tracing::debug!("UI Automation events: {e}");
            }
            if initialised {
                CoUninitialize();
            }
        }
    }
}

/// A running subscription; dropping it ends it (the thread unsubscribes by itself, nobody waits for it).
pub struct Watch {
    _stop: std::sync::mpsc::Sender<()>,
}

/// Subscribes to the changes of the window `handle` (`None`: the one in front), of the focus and of new windows.
pub fn watch(handle: Option<isize>, events: UnboundedSender<()>) -> Option<Watch> {
    let (stop, stopped) = std::sync::mpsc::channel();
    #[cfg(windows)]
    {
        std::thread::Builder::new().name("decka-uia-events".into()).spawn(move || win::watch(handle, events, stopped)).ok()?;
        Some(Watch { _stop: stop })
    }
    #[cfg(not(windows))]
    {
        let _ = (handle, events, stopped, stop);
        None
    }
}
