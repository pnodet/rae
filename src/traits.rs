/* Traits and Type Aliases for AE Event Loop */

use crate::ae_select::FiredEvent;
use std::ffi::c_void;
use std::time::Duration;

/* Type aliases for callback functions (matching C API) */
pub type FileProc =
    fn(event_loop: &mut crate::ae::AeEventLoop, fd: i32, client_data: *mut c_void, mask: i32);
pub type TimeProc =
    fn(event_loop: &mut crate::ae::AeEventLoop, id: i64, client_data: *mut c_void) -> i32;
pub type EventFinalizerProc = fn(event_loop: &mut crate::ae::AeEventLoop, client_data: *mut c_void);
pub type BeforeSleepProc = fn(event_loop: &mut crate::ae::AeEventLoop);
pub type AfterSleepProc = fn(event_loop: &mut crate::ae::AeEventLoop);

/* Platform-specific event backend trait */
pub trait EventBackend {
    fn create() -> Result<Box<Self>, i32>
    where
        Self: Sized;
    fn free(self: Box<Self>);
    fn resize(&mut self, setsize: i32) -> i32;
    fn add_event(&mut self, fd: i32, mask: i32) -> i32;
    fn del_event(&mut self, fd: i32, mask: i32);
    fn poll(
        &mut self,
        events: &[crate::ae::AeFileEvent],
        fired: &mut [FiredEvent],
        maxfd: i32,
        timeout: Option<Duration>,
    ) -> Result<i32, i32>;
    fn name(&self) -> &'static str;
}
