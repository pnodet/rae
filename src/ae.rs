/* A simple event-driven programming library. Originally I wrote this code
 * for the Jim's event-loop (Jim is a Tcl interpreter) but later translated
 * it in form of a library for easy reuse.
 *
 * Copyright (c) 2006-Present, Redis Ltd.
 * All rights reserved.
 *
 * Rust port of Redis ae.c
 */

use crate::ae_select;
use crate::ae_select::FiredEvent;
use crate::constants::*;
use crate::traits::*;
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct AeFileEvent {
    pub mask: i32,
    pub rfile_proc: Option<FileProc>,
    pub wfile_proc: Option<FileProc>,
    pub client_data: *mut std::ffi::c_void,
}

impl Default for AeFileEvent {
    fn default() -> Self {
        Self {
            mask: AE_NONE,
            rfile_proc: None,
            wfile_proc: None,
            client_data: std::ptr::null_mut(),
        }
    }
}

impl AeFileEvent {
    pub fn new() -> Self {
        Self::default()
    }
}

/* Uses a linked list structure with reference counting for safety */
#[derive(Debug)]
pub struct AeTimeEvent {
    pub id: i64,
    pub when: u64,
    pub time_proc: Option<TimeProc>,
    pub finalizer_proc: Option<EventFinalizerProc>,
    pub client_data: *mut std::ffi::c_void,
    pub refcount: i32,
}

impl AeTimeEvent {
    pub fn new(
        id: i64,
        when: u64,
        time_proc: Option<TimeProc>,
        finalizer_proc: Option<EventFinalizerProc>,
        client_data: *mut std::ffi::c_void,
    ) -> Self {
        Self {
            id,
            when,
            time_proc,
            finalizer_proc,
            client_data,
            refcount: 0,
        }
    }
}

#[derive(Debug)]
pub struct TimeEventNode {
    pub event: AeTimeEvent,
    pub next: Option<Box<TimeEventNode>>,
}

impl TimeEventNode {
    pub fn new(event: AeTimeEvent) -> Self {
        Self { event, next: None }
    }
}

/* Fields ordered for optimal memory alignment */
pub struct AeEventLoop {
    pub time_event_next_id: i64,
    pub apidata: Box<dyn EventBackend>,
    pub events: Vec<AeFileEvent>,
    pub fired: Vec<FiredEvent>,
    pub time_event_head: Option<Box<TimeEventNode>>,
    pub beforesleep: Option<BeforeSleepProc>,
    pub aftersleep: Option<AfterSleepProc>,
    pub privdata: [*mut std::ffi::c_void; 2],
    pub maxfd: i32,
    pub setsize: i32,
    pub nevents: u32,
    pub flags: i32,
    pub stop: bool,
}

impl AeEventLoop {
    fn new(setsize: i32, backend: Box<dyn EventBackend>) -> Self {
        let nevents = if setsize < INITIAL_EVENT as i32 {
            setsize as u32
        } else {
            INITIAL_EVENT as u32
        };

        let mut events = Vec::with_capacity(nevents as usize);
        let mut fired = Vec::with_capacity(nevents as usize);

        /* Events with mask == AE_NONE are not set. So let's initialize the
         * vector with it. */
        for _ in 0..nevents {
            events.push(AeFileEvent::new());
        }

        for _ in 0..nevents {
            fired.push(FiredEvent { fd: 0, mask: 0 });
        }

        Self {
            time_event_next_id: 0,
            apidata: backend,
            events,
            fired,
            time_event_head: None,
            beforesleep: None,
            aftersleep: None,
            privdata: [std::ptr::null_mut(); 2],
            maxfd: -1,
            setsize,
            nevents,
            flags: 0,
            stop: false,
        }
    }
}

unsafe impl Send for AeEventLoop {}

fn get_monotonic_us() -> u64 {
    static START_TIME: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
    let start = START_TIME.get_or_init(Instant::now);
    start.elapsed().as_micros() as u64
}

impl Drop for AeEventLoop {
    fn drop(&mut self) {
        /* Free the time events list. */
        while let Some(mut node) = self.time_event_head.take() {
            if let Some(finalizer) = node.event.finalizer_proc {
                finalizer(self, node.event.client_data);
            }
            self.time_event_head = node.next.take();
        }
    }
}

pub fn ae_create_event_loop(setsize: i32) -> Option<Box<AeEventLoop>> {
    // Create the platform-specific backend
    let backend = match create_select_backend() {
        Ok(backend) => backend,
        Err(_) => return None,
    };

    let event_loop = AeEventLoop::new(setsize, backend);
    Some(Box::new(event_loop))
}

/* Return the current set size. */
pub fn ae_get_set_size(event_loop: &AeEventLoop) -> i32 {
    event_loop.setsize
}

/*
 * Tell the event processing to change the wait timeout as soon as possible.
 *
 * Note: it just means you turn on/off the global AE_DONT_WAIT.
 */
pub fn ae_set_dont_wait(event_loop: &mut AeEventLoop, no_wait: bool) {
    if no_wait {
        event_loop.flags |= AE_DONT_WAIT;
    } else {
        event_loop.flags &= !AE_DONT_WAIT;
    }
}

/* Resize the maximum set size of the event loop.
 * If the requested set size is smaller than the current set size, but
 * there is already a file descriptor in use that is >= the requested
 * set size minus one, AE_ERR is returned and the operation is not
 * performed at all.
 *
 * Otherwise AE_OK is returned and the operation is successful. */
pub fn ae_resize_set_size(event_loop: &mut AeEventLoop, setsize: i32) -> i32 {
    if setsize == event_loop.setsize {
        return AE_OK;
    }

    if event_loop.maxfd >= setsize {
        return AE_ERR;
    }

    if event_loop.apidata.resize(setsize) == -1 {
        return AE_ERR;
    }

    event_loop.setsize = setsize;

    /* If the current allocated space is larger than the requested size,
     * we need to shrink it to the requested size. */
    if (setsize as u32) < event_loop.nevents {
        event_loop.events.truncate(setsize as usize);
        event_loop.fired.truncate(setsize as usize);
        event_loop.nevents = setsize as u32;
    }

    AE_OK
}

pub fn ae_delete_event_loop(event_loop: Box<AeEventLoop>) {
    // Drop will handle cleanup automatically
    drop(event_loop);
}

pub fn ae_stop(event_loop: &mut AeEventLoop) {
    event_loop.stop = true;
}

pub fn ae_set_before_sleep_proc(
    event_loop: &mut AeEventLoop,
    beforesleep: Option<BeforeSleepProc>,
) {
    event_loop.beforesleep = beforesleep;
}

pub fn ae_set_after_sleep_proc(event_loop: &mut AeEventLoop, aftersleep: Option<AfterSleepProc>) {
    event_loop.aftersleep = aftersleep;
}

pub fn ae_get_api_name() -> &'static str {
    ae_select::ae_api_name()
}

pub fn ae_main(event_loop: &mut AeEventLoop) {
    event_loop.stop = false;
    while !event_loop.stop {
        ae_process_events(
            event_loop,
            AE_ALL_EVENTS | AE_CALL_BEFORE_SLEEP | AE_CALL_AFTER_SLEEP,
        );
    }
}

fn create_select_backend() -> Result<Box<dyn EventBackend>, i32> {
    SelectBackend::create().map(|backend| backend as Box<dyn EventBackend>)
}

struct SelectBackend {
    state: Box<ae_select::aeApiState>,
}

impl EventBackend for SelectBackend {
    fn create() -> Result<Box<Self>, i32> {
        match ae_select::ae_api_create() {
            Ok(state) => Ok(Box::new(SelectBackend { state })),
            Err(e) => Err(e),
        }
    }

    fn free(self: Box<Self>) {
        ae_select::ae_api_free(self.state);
    }

    fn resize(&mut self, setsize: i32) -> i32 {
        ae_select::ae_api_resize(setsize)
    }

    fn add_event(&mut self, fd: i32, mask: i32) -> i32 {
        ae_select::ae_api_add_event(&mut self.state, fd, mask)
    }

    fn del_event(&mut self, fd: i32, mask: i32) {
        ae_select::ae_api_del_event(&mut self.state, fd, mask);
    }

    fn poll(
        &mut self,
        events: &[AeFileEvent],
        fired: &mut [FiredEvent],
        maxfd: i32,
        timeout: Option<Duration>,
    ) -> Result<i32, i32> {
        ae_select::ae_api_poll(&mut self.state, events, fired, maxfd, timeout)
    }

    fn name(&self) -> &'static str {
        ae_select::ae_api_name()
    }
}

pub fn ae_create_file_event(
    event_loop: &mut AeEventLoop,
    fd: i32,
    mask: i32,
    proc: FileProc,
    client_data: *mut std::ffi::c_void,
) -> i32 {
    if fd >= event_loop.setsize {
        return AE_ERR;
    }

    /* Resize the events and fired arrays if the file
     * descriptor exceeds the current number of events. */
    if (fd as u32) >= event_loop.nevents {
        let mut new_nevents = event_loop.nevents * 2;
        let fd_plus_one = (fd as u32) + 1;

        if new_nevents < fd_plus_one {
            new_nevents = fd_plus_one;
        }

        if new_nevents > (event_loop.setsize as u32) {
            new_nevents = event_loop.setsize as u32;
        }

        while event_loop.events.len() < (new_nevents as usize) {
            event_loop.events.push(AeFileEvent::new());
        }

        while event_loop.fired.len() < (new_nevents as usize) {
            event_loop.fired.push(FiredEvent { fd: 0, mask: 0 });
        }

        event_loop.nevents = new_nevents;
    }

    if event_loop.apidata.add_event(fd, mask) == -1 {
        return AE_ERR;
    }
    let fe = &mut event_loop.events[fd as usize];
    fe.mask |= mask;

    if mask & AE_READABLE != 0 {
        fe.rfile_proc = Some(proc);
    }
    if mask & AE_WRITABLE != 0 {
        fe.wfile_proc = Some(proc);
    }

    fe.client_data = client_data;

    if fd > event_loop.maxfd {
        event_loop.maxfd = fd;
    }

    AE_OK
}

pub fn ae_delete_file_event(event_loop: &mut AeEventLoop, fd: i32, mask: i32) {
    if fd >= event_loop.setsize {
        return;
    }

    let fe = &mut event_loop.events[fd as usize];
    if fe.mask == AE_NONE {
        return;
    }

    /* We want to always remove AE_BARRIER if set when AE_WRITABLE
     * is removed. */
    let mut mask_to_remove = mask;
    if mask & AE_WRITABLE != 0 {
        mask_to_remove |= AE_BARRIER;
    }

    event_loop.apidata.del_event(fd, mask_to_remove);
    fe.mask &= !mask_to_remove;

    if mask_to_remove & AE_READABLE != 0 {
        fe.rfile_proc = None;
    }
    if mask_to_remove & AE_WRITABLE != 0 {
        fe.wfile_proc = None;
    }

    if fd == event_loop.maxfd && fe.mask == AE_NONE {
        /* Update the max fd */
        let mut j = event_loop.maxfd - 1;
        while j >= 0 {
            if event_loop.events[j as usize].mask != AE_NONE {
                break;
            }
            j -= 1;
        }
        event_loop.maxfd = j;
    }
}

pub fn ae_get_file_client_data(event_loop: &AeEventLoop, fd: i32) -> *mut std::ffi::c_void {
    if fd >= event_loop.setsize {
        return std::ptr::null_mut();
    }

    let fe = &event_loop.events[fd as usize];
    if fe.mask == AE_NONE {
        return std::ptr::null_mut();
    }

    fe.client_data
}

pub fn ae_get_file_events(event_loop: &AeEventLoop, fd: i32) -> i32 {
    if fd >= event_loop.setsize {
        return 0;
    }

    event_loop.events[fd as usize].mask
}

pub fn ae_create_time_event(
    event_loop: &mut AeEventLoop,
    milliseconds: i64,
    proc: TimeProc,
    client_data: *mut std::ffi::c_void,
    finalizer_proc: Option<EventFinalizerProc>,
) -> i64 {
    let id = event_loop.time_event_next_id;
    event_loop.time_event_next_id += 1;

    let when = get_monotonic_us() + (milliseconds * 1000) as u64;
    let time_event = AeTimeEvent::new(id, when, Some(proc), finalizer_proc, client_data);

    let mut new_node = Box::new(TimeEventNode::new(time_event));
    new_node.next = event_loop.time_event_head.take();
    event_loop.time_event_head = Some(new_node);

    id
}

pub fn ae_delete_time_event(event_loop: &mut AeEventLoop, id: i64) -> i32 {
    let mut current = &mut event_loop.time_event_head;

    while let Some(node) = current {
        if node.event.id == id {
            node.event.id = AE_DELETED_EVENT_ID;
            return AE_OK;
        }
        current = &mut node.next;
    }

    AE_ERR
}

/* How many microseconds until the first timer should fire.
 * If there are no timers, -1 is returned.
 */
fn us_until_earliest_timer(event_loop: &AeEventLoop) -> i64 {
    if event_loop.time_event_head.is_none() {
        return -1;
    }

    let mut earliest: Option<&AeTimeEvent> = None;
    let mut current = &event_loop.time_event_head;

    while let Some(node) = current {
        let te = &node.event;
        if te.id != AE_DELETED_EVENT_ID && (earliest.is_none() || te.when < earliest.unwrap().when)
        {
            earliest = Some(te);
        }
        current = &node.next;
    }

    if let Some(earliest_event) = earliest {
        let now = get_monotonic_us();
        if now >= earliest_event.when {
            0
        } else {
            (earliest_event.when - now) as i64
        }
    } else {
        -1
    }
}

/* Process time events */
fn process_time_events(event_loop: &mut AeEventLoop) -> i32 {
    let mut processed = 0;
    let max_id = event_loop.time_event_next_id - 1;
    let now = get_monotonic_us();

    /* First, collect events that need to be processed */
    let mut events_to_process = Vec::new();
    let mut events_to_remove = Vec::new();

    let mut current = &mut event_loop.time_event_head;
    while let Some(node) = current {
        let te = &mut node.event;

        /* Remove events scheduled for deletion. */
        if te.id == AE_DELETED_EVENT_ID {
            if te.refcount == 0 {
                events_to_remove.push(te.id);
            }
            current = &mut node.next;
            continue;
        }

        /* Skip events created during this iteration */
        if te.id > max_id {
            current = &mut node.next;
            continue;
        }

        if te.when <= now {
            events_to_process.push(te.id);
        }

        current = &mut node.next;
    }

    for event_id in events_to_process {
        let mut event_found = false;
        let mut time_proc_to_call: Option<TimeProc> = None;
        let mut client_data: *mut std::ffi::c_void = std::ptr::null_mut();

        let mut current = &mut event_loop.time_event_head;
        while let Some(node) = current {
            let te = &mut node.event;
            if te.id == event_id && te.id != AE_DELETED_EVENT_ID && te.when <= now {
                te.refcount += 1;
                time_proc_to_call = te.time_proc;
                client_data = te.client_data;
                event_found = true;
                break;
            }
            current = &mut node.next;
        }

        if event_found && let Some(time_proc) = time_proc_to_call {
            let retval = time_proc(event_loop, event_id, client_data);
            processed += 1;

            let updated_now = get_monotonic_us();

            let mut current = &mut event_loop.time_event_head;
            while let Some(node) = current {
                let te = &mut node.event;
                if te.id == event_id {
                    te.refcount -= 1;
                    if retval != AE_NOMORE {
                        te.when = updated_now + (retval * 1000) as u64;
                    } else {
                        te.id = AE_DELETED_EVENT_ID;
                    }
                    break;
                }
                current = &mut node.next;
            }
        }
    }

    cleanup_deleted_time_events(event_loop);

    processed
}

pub fn ae_process_events(event_loop: &mut AeEventLoop, flags: i32) -> i32 {
    let mut processed = 0;

    /* Nothing to do? return ASAP */
    if (flags & AE_TIME_EVENTS) == 0 && (flags & AE_FILE_EVENTS) == 0 {
        return 0;
    }

    /* Note that we want to call poll() even if there are no file events
     * to process as long as we want to process time events, in order to
     * sleep until the next time event is ready to fire. */
    if event_loop.maxfd != -1 || ((flags & AE_TIME_EVENTS) != 0 && (flags & AE_DONT_WAIT) == 0) {
        // Call beforesleep callback if present
        if let Some(beforesleep) = event_loop.beforesleep
            && (flags & AE_CALL_BEFORE_SLEEP) != 0
        {
            beforesleep(event_loop);
        }

        // Determine timeout based on flags and time events
        let timeout = if (flags & AE_DONT_WAIT) != 0 || (event_loop.flags & AE_DONT_WAIT) != 0 {
            Some(Duration::from_secs(0)) // No wait
        } else if (flags & AE_TIME_EVENTS) != 0 {
            let us_until_timer = us_until_earliest_timer(event_loop);
            if us_until_timer >= 0 {
                Some(Duration::from_micros(us_until_timer as u64))
            } else {
                None // Infinite wait
            }
        } else {
            None // Infinite wait
        };

        // Call the multiplexing API, will return only on timeout or when some event fires
        let numevents = event_loop
            .apidata
            .poll(
                &event_loop.events,
                &mut event_loop.fired,
                event_loop.maxfd,
                timeout,
            )
            .unwrap_or(0); // Error in polling, continue with 0 events

        // Don't process file events if not requested
        let numevents = if (flags & AE_FILE_EVENTS) != 0 {
            numevents
        } else {
            0
        };

        // Call aftersleep callback if present
        if let Some(aftersleep) = event_loop.aftersleep
            && (flags & AE_CALL_AFTER_SLEEP) != 0
        {
            aftersleep(event_loop);
        }

        // Process file events
        for j in 0..(numevents as usize) {
            if j >= event_loop.fired.len() {
                break;
            }

            let fd = event_loop.fired[j].fd;
            let mask = event_loop.fired[j].mask;

            if (fd as usize) >= event_loop.events.len() {
                continue;
            }

            // Extract event info to avoid borrowing issues during callbacks
            let fe_mask = event_loop.events[fd as usize].mask;
            let rfile_proc = event_loop.events[fd as usize].rfile_proc;
            let wfile_proc = event_loop.events[fd as usize].wfile_proc;
            let client_data = event_loop.events[fd as usize].client_data;

            let mut fired = 0; // Number of events fired for current fd

            // Check if we should invert the calls (AE_BARRIER flag)
            let invert = (fe_mask & AE_BARRIER) != 0;

            // Fire the readable event if the call sequence is not inverted
            if !invert
                && (fe_mask & mask & AE_READABLE) != 0
                && let Some(rfile_proc) = rfile_proc
            {
                rfile_proc(event_loop, fd, client_data, mask);
                fired += 1;
            }

            // Fire the writable event
            if (fe_mask & mask & AE_WRITABLE) != 0 {
                // Refresh event info in case of resize during callback
                let current_fe_mask = if (fd as usize) < event_loop.events.len() {
                    event_loop.events[fd as usize].mask
                } else {
                    0
                };

                let current_wfile_proc = if (fd as usize) < event_loop.events.len() {
                    event_loop.events[fd as usize].wfile_proc
                } else {
                    None
                };

                let should_fire_write = fired == 0
                    || current_wfile_proc.is_none()
                    || (wfile_proc.is_some()
                        && rfile_proc.is_some()
                        && wfile_proc.unwrap() as *const FileProc
                            != rfile_proc.unwrap() as *const FileProc);

                if should_fire_write
                    && (current_fe_mask & mask & AE_WRITABLE) != 0
                    && let Some(wfile_proc) = current_wfile_proc
                {
                    let current_client_data = if (fd as usize) < event_loop.events.len() {
                        event_loop.events[fd as usize].client_data
                    } else {
                        client_data
                    };
                    wfile_proc(event_loop, fd, current_client_data, mask);
                    fired += 1;
                }
            }

            // If we have to invert the call, fire the readable event now after the writable one
            if invert {
                // Refresh event info in case of resize during callback
                let current_fe_mask = if (fd as usize) < event_loop.events.len() {
                    event_loop.events[fd as usize].mask
                } else {
                    0
                };

                let current_rfile_proc = if (fd as usize) < event_loop.events.len() {
                    event_loop.events[fd as usize].rfile_proc
                } else {
                    None
                };

                let should_fire_read = (current_fe_mask & mask & AE_READABLE) != 0
                    && (fired == 0
                        || current_rfile_proc.is_none()
                        || (rfile_proc.is_some()
                            && wfile_proc.is_some()
                            && rfile_proc.unwrap() as *const FileProc
                                != wfile_proc.unwrap() as *const FileProc));

                if should_fire_read && let Some(rfile_proc) = current_rfile_proc {
                    let current_client_data = if (fd as usize) < event_loop.events.len() {
                        event_loop.events[fd as usize].client_data
                    } else {
                        client_data
                    };
                    rfile_proc(event_loop, fd, current_client_data, mask);
                }
            }

            processed += 1;
        }
    }

    /* Check time events */
    if (flags & AE_TIME_EVENTS) != 0 {
        processed += process_time_events(event_loop);
    }

    processed /* return the number of processed file/time events */
}

/* Wait for milliseconds until the given file descriptor becomes
 * writable/readable/exception */
pub fn ae_wait(fd: i32, mask: i32, milliseconds: i64) -> i32 {
    use libc::{POLLERR, POLLHUP, POLLIN, POLLOUT, poll, pollfd};

    let mut pfd = pollfd {
        fd,
        events: 0,
        revents: 0,
    };

    if mask & AE_READABLE != 0 {
        pfd.events |= POLLIN;
    }
    if mask & AE_WRITABLE != 0 {
        pfd.events |= POLLOUT;
    }

    let retval = unsafe { poll(&mut pfd, 1, milliseconds as i32) };

    if retval == 1 {
        let mut retmask = 0;
        if pfd.revents & POLLIN != 0 {
            retmask |= AE_READABLE;
        }
        if pfd.revents & POLLOUT != 0 {
            retmask |= AE_WRITABLE;
        }
        if pfd.revents & POLLERR != 0 {
            retmask |= AE_WRITABLE;
        }
        if pfd.revents & POLLHUP != 0 {
            retmask |= AE_WRITABLE;
        }
        return retmask;
    }

    retval
}

fn cleanup_deleted_time_events(event_loop: &mut AeEventLoop) {
    let mut nodes_to_remove = Vec::new();

    let mut current = &event_loop.time_event_head;
    while let Some(node) = current {
        if node.event.id == AE_DELETED_EVENT_ID && node.event.refcount == 0 {
            nodes_to_remove.push((
                node.event.id,
                node.event.finalizer_proc,
                node.event.client_data,
            ));
        }
        current = &node.next;
    }

    for (_id, finalizer_proc, client_data) in &nodes_to_remove {
        if let Some(finalizer) = finalizer_proc {
            finalizer(event_loop, *client_data);
        }
    }

    fn remove_deleted_nodes(current: &mut Option<Box<TimeEventNode>>) {
        if let Some(mut node) = current.take() {
            if node.event.id == AE_DELETED_EVENT_ID && node.event.refcount == 0 {
                *current = node.next.take();
                remove_deleted_nodes(current);
            } else {
                remove_deleted_nodes(&mut node.next);
                *current = Some(node);
            }
        }
    }

    remove_deleted_nodes(&mut event_loop.time_event_head);
}
