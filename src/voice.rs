#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VoiceEvent {
    Authorizing,
    Recording,
    Recognizing,
    Partial(String),
    Final(String),
    PermissionDenied(String),
    Unavailable(String),
    Error(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VoiceCommand {
    Start,
    Stop,
    Cancel,
}

#[cfg(target_os = "macos")]
mod platform {
    use super::{VoiceCommand, VoiceEvent};
    use std::ffi::{CStr, c_char, c_int, c_void};
    use std::sync::mpsc::{self, Receiver, Sender};

    const EVENT_AUTHORIZING: c_int = 0;
    const EVENT_RECORDING: c_int = 1;
    const EVENT_RECOGNIZING: c_int = 2;
    const EVENT_PARTIAL: c_int = 3;
    const EVENT_FINAL: c_int = 4;
    const EVENT_PERMISSION_DENIED: c_int = 5;
    const EVENT_UNAVAILABLE: c_int = 6;
    const EVENT_ERROR: c_int = 7;

    unsafe extern "C" {
        fn rem_speech_create(
            callback: extern "C" fn(c_int, *const c_char, *mut c_void),
            context: *mut c_void,
        ) -> *mut c_void;
        fn rem_speech_start(handle: *mut c_void);
        fn rem_speech_stop(handle: *mut c_void);
        fn rem_speech_cancel(handle: *mut c_void);
        fn rem_speech_destroy(handle: *mut c_void);
    }

    pub struct VoiceService {
        receiver: Receiver<VoiceEvent>,
        handle: *mut c_void,
    }

    impl VoiceService {
        pub fn new() -> Self {
            let (sender, receiver) = mpsc::channel();
            let context = Box::into_raw(Box::new(sender)).cast::<c_void>();
            let handle = unsafe { rem_speech_create(receive_event, context) };
            Self { receiver, handle }
        }

        pub fn execute(&self, command: VoiceCommand) {
            unsafe {
                match command {
                    VoiceCommand::Start => rem_speech_start(self.handle),
                    VoiceCommand::Stop => rem_speech_stop(self.handle),
                    VoiceCommand::Cancel => rem_speech_cancel(self.handle),
                }
            }
        }

        pub fn try_iter(&self) -> impl Iterator<Item = VoiceEvent> + '_ {
            self.receiver.try_iter()
        }
    }

    impl Drop for VoiceService {
        fn drop(&mut self) {
            unsafe {
                rem_speech_destroy(self.handle);
            }
        }
    }

    extern "C" fn receive_event(event: c_int, message: *const c_char, context: *mut c_void) {
        let sender = unsafe { &*context.cast::<Sender<VoiceEvent>>() };
        let message = if message.is_null() {
            String::new()
        } else {
            unsafe { CStr::from_ptr(message) }
                .to_string_lossy()
                .into_owned()
        };
        let voice_event = match event {
            EVENT_AUTHORIZING => VoiceEvent::Authorizing,
            EVENT_RECORDING => VoiceEvent::Recording,
            EVENT_RECOGNIZING => VoiceEvent::Recognizing,
            EVENT_PARTIAL => VoiceEvent::Partial(message),
            EVENT_FINAL => VoiceEvent::Final(message),
            EVENT_PERMISSION_DENIED => VoiceEvent::PermissionDenied(message),
            EVENT_UNAVAILABLE => VoiceEvent::Unavailable(message),
            _ if event == EVENT_ERROR => VoiceEvent::Error(message),
            _ => VoiceEvent::Error("Unknown speech recognition event".to_string()),
        };
        let _ = sender.send(voice_event);
    }
}

#[cfg(not(target_os = "macos"))]
mod platform {
    use super::{VoiceCommand, VoiceEvent};

    pub struct VoiceService;

    impl VoiceService {
        pub fn new() -> Self {
            Self
        }

        pub fn execute(&self, _command: VoiceCommand) {}

        pub fn try_iter(&self) -> impl Iterator<Item = VoiceEvent> {
            std::iter::empty()
        }
    }
}

pub use platform::VoiceService;

impl Default for VoiceService {
    fn default() -> Self {
        Self::new()
    }
}
