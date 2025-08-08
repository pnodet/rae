/* Select()-based ae.c module.
 *
 * Copyright (c) 2009-Present, Redis Ltd.
 * All rights reserved.
 *
 * Licensed under your choice of (a) the Redis Source Available License 2.0
 * (RSALv2); or (b) the Server Side Public License v1 (SSPLv1); or (c) the
 * GNU Affero General Public License v3 (AGPLv3).
 */

use crate::constants::{AE_NONE, AE_READABLE, AE_WRITABLE};
use crate::fd_set::FdSet;
use libc::{FD_SETSIZE, select, timeval};
use std::time::Duration;

#[derive(Debug, Clone, Copy)]
pub struct FiredEvent {
    pub fd: i32,
    pub mask: i32,
}

#[allow(non_camel_case_types)]
pub struct aeApiState {
    pub rfds: FdSet,
    pub wfds: FdSet,
    /* We need to have a copy of the fd sets as it's not safe to reuse
     * FD sets after select(). */
    pub _rfds: FdSet,
    pub _wfds: FdSet,
}

impl Default for aeApiState {
    fn default() -> Self {
        Self::new()
    }
}

impl aeApiState {
    pub fn new() -> Self {
        aeApiState {
            rfds: FdSet::zero(),
            wfds: FdSet::zero(),
            _rfds: FdSet::zero(),
            _wfds: FdSet::zero(),
        }
    }
}

pub fn ae_api_create() -> Result<Box<aeApiState>, i32> {
    Ok(Box::new(aeApiState::new()))
}

pub fn ae_api_resize(setsize: i32) -> i32 {
    /* Just ensure we have enough room in the fd_set type. */
    if (setsize as usize) >= FD_SETSIZE {
        return -1;
    }

    0
}

pub fn ae_api_free(state: Box<aeApiState>) {
    drop(state);
}

pub fn ae_api_add_event(state: &mut aeApiState, fd: i32, mask: i32) -> i32 {
    if mask & AE_READABLE != 0 {
        state.rfds.set(fd);
    }
    if mask & AE_WRITABLE != 0 {
        state.wfds.set(fd);
    }

    0
}

pub fn ae_api_del_event(state: &mut aeApiState, fd: i32, mask: i32) {
    if mask & AE_READABLE != 0 {
        state.rfds.clr(fd);
    }
    if mask & AE_WRITABLE != 0 {
        state.wfds.clr(fd);
    }
}

pub fn ae_api_poll(
    state: &mut aeApiState,
    events: &[crate::ae::AeFileEvent],
    fired: &mut [FiredEvent],
    maxfd: i32,
    tvp: Option<Duration>,
) -> Result<i32, i32> {
    state._rfds = state.rfds.clone();
    state._wfds = state.wfds.clone();

    let timeout_ptr = tvp
        .map(|d| timeval {
            tv_sec: d.as_secs() as libc::time_t,
            tv_usec: d.subsec_micros() as libc::suseconds_t,
        })
        .as_mut()
        .map_or(std::ptr::null_mut(), |t| t as *mut timeval);

    let retval = unsafe {
        select(
            maxfd + 1,
            state._rfds.as_mut_ptr(),
            state._wfds.as_mut_ptr(),
            std::ptr::null_mut(), // no exceptfds
            timeout_ptr,
        )
    };

    if retval < 0 {
        let errno = unsafe { *libc::__error() };
        /* Ignore EINTR (interrupted system call) like the C version */
        if errno == libc::EINTR {
            return Ok(0);
        }
        return Err(errno);
    }
    if retval == 0 {
        return Ok(0);
    }

    let mut numevents = 0;
    for fd in 0..=maxfd {
        if numevents >= fired.len() {
            break;
        }

        /* Critical: validate that this fd is actually registered for events
         * This matches the C version: if (fe->mask == AE_NONE) continue; */
        if fd as usize >= events.len() || events[fd as usize].mask == AE_NONE {
            continue;
        }

        let fe = &events[fd as usize];
        let mut mask = 0;

        /* Only report events that are both ready AND registered */
        if (fe.mask & AE_READABLE) != 0 && state._rfds.isset(fd) {
            mask |= AE_READABLE;
        }
        if (fe.mask & AE_WRITABLE) != 0 && state._wfds.isset(fd) {
            mask |= AE_WRITABLE;
        }

        if mask != 0 {
            fired[numevents] = FiredEvent { fd, mask };
            numevents += 1;
        }
    }

    Ok(numevents as i32)
}

pub fn ae_api_name() -> &'static str {
    "select"
}
