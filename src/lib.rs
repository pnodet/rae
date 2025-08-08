//! RAE - Redis AE Event Loop in Rust
//!
//! Rust port of Redis's Asynchronous Event (AE) loop system.
//!
//! This library provides a Redis-compatible asynchronous event loop implementation
//! with support for file descriptor events and timer events.
//!
//! # Example
//!
//! ```no_run
//! use rae::{ae_create_event_loop, ae_create_file_event, ae_main, AE_READABLE, AeEventLoop, FileProc};
//! use std::ffi::c_void;
//!
//! // Example callback function
//! fn my_callback(event_loop: &mut AeEventLoop, fd: i32, client_data: *mut c_void, mask: i32) {
//!     // Handle the event
//! }
//!
//! let mut event_loop = ae_create_event_loop(1024).expect("Failed to create event loop");
//!
//! // Register a file descriptor for reading (fd would be a real socket/file descriptor)
//! let fd = 0; // Example file descriptor
//! let data: *mut c_void = std::ptr::null_mut(); // Example client data
//! ae_create_file_event(&mut event_loop, fd, AE_READABLE, my_callback, data);
//!
//! // Run the event loop
//! ae_main(&mut event_loop);
//! ```

pub mod ae;
pub mod constants;
pub mod fd_set;
pub mod traits;

#[cfg(any(unix, target_os = "linux", target_os = "macos", target_os = "freebsd"))]
pub mod ae_select;

pub use constants::{
    AE_ALL_EVENTS, AE_BARRIER, AE_CALL_AFTER_SLEEP, AE_CALL_BEFORE_SLEEP, AE_DONT_WAIT, AE_ERR,
    AE_FILE_EVENTS, AE_NOMORE, AE_OK, AE_TIME_EVENTS,
};

pub use traits::{
    AfterSleepProc, BeforeSleepProc, EventBackend, EventFinalizerProc, FileProc, TimeProc,
};

pub use ae::{
    AeEventLoop, AeFileEvent, AeTimeEvent, ae_create_event_loop, ae_create_file_event,
    ae_create_time_event, ae_delete_event_loop, ae_delete_file_event, ae_delete_time_event,
    ae_get_api_name, ae_get_file_client_data, ae_get_file_events, ae_get_set_size, ae_main,
    ae_process_events, ae_resize_set_size, ae_set_after_sleep_proc, ae_set_before_sleep_proc,
    ae_set_dont_wait, ae_stop, ae_wait,
};

#[cfg(any(unix, target_os = "linux", target_os = "macos", target_os = "freebsd"))]
pub use constants::{AE_NONE, AE_READABLE, AE_WRITABLE};

#[cfg(any(unix, target_os = "linux", target_os = "macos", target_os = "freebsd"))]
pub use ae_select::FiredEvent;

#[cfg(any(unix, target_os = "linux", target_os = "macos", target_os = "freebsd"))]
pub use ae_select::{
    ae_api_add_event, ae_api_create, ae_api_del_event, ae_api_free, ae_api_name, ae_api_poll,
    ae_api_resize, aeApiState,
};

#[cfg(any(unix, target_os = "linux", target_os = "macos", target_os = "freebsd"))]
pub type ApiState = aeApiState;
