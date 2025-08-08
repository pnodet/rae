/* AE Event Loop Constants */

pub const AE_OK: i32 = 0;
pub const AE_ERR: i32 = -1;

pub const AE_NONE: i32 = 0;
pub const AE_READABLE: i32 = 1;
pub const AE_WRITABLE: i32 = 2;

/* With WRITABLE, never fire the event if the READABLE event already fired
 * in the same event loop iteration. Useful when you want to persist things
 * to disk before sending replies, and want to do that in a group fashion. */
pub const AE_BARRIER: i32 = 4;

pub const AE_FILE_EVENTS: i32 = 1 << 0;
pub const AE_TIME_EVENTS: i32 = 1 << 1;
pub const AE_ALL_EVENTS: i32 = AE_FILE_EVENTS | AE_TIME_EVENTS;
pub const AE_DONT_WAIT: i32 = 1 << 2;
pub const AE_CALL_BEFORE_SLEEP: i32 = 1 << 3;
pub const AE_CALL_AFTER_SLEEP: i32 = 1 << 4;

pub const AE_NOMORE: i32 = -1;
pub const AE_DELETED_EVENT_ID: i64 = -1;

pub const INITIAL_EVENT: usize = 1024;
