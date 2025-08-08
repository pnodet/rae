/* Kqueue(2)-based ae.c module
 *
 * Copyright (C) 2009 Harish Mallipeddi - harish.mallipeddi@gmail.com
 * All rights reserved.
 *
 * Rust port of Redis ae_kqueue.c
 */

use crate::ae_select::FiredEvent;
use crate::constants::{AE_READABLE, AE_WRITABLE};
use crate::traits::EventBackend;
use libc::{EINTR, EV_ADD, EV_DELETE, EVFILT_READ, EVFILT_WRITE, close, kevent, kqueue, timespec};
use std::os::unix::io::RawFd;
use std::time::Duration;

/* Manual implementation of EV_SET macro since it's not available in libc */
#[inline]
unsafe fn ev_set(
    kev: &mut libc::kevent,
    ident: libc::uintptr_t,
    filter: libc::c_short,
    flags: libc::c_ushort,
    fflags: libc::c_uint,
    data: libc::intptr_t,
    udata: *mut libc::c_void,
) {
    kev.ident = ident;
    kev.filter = filter;
    kev.flags = flags;
    kev.fflags = fflags;
    kev.data = data;
    kev.udata = udata;
}

#[allow(non_camel_case_types)]
pub struct aeApiState {
    kqfd: RawFd,
    events: Vec<libc::kevent>,
    /* Events mask for merge read and write event.
     * To reduce memory consumption, we use 2 bits to store the mask
     * of an event, so that 1 byte will store the mask of 4 events. */
    eventsMask: Vec<u8>,
}

impl aeApiState {
    /* Calculate bytes needed to store event masks for setsize file descriptors */
    #[inline]
    fn event_mask_malloc_size(setsize: i32) -> usize {
        (setsize as usize).div_ceil(4)
    }

    /* Calculate bit offset within byte for fd's 2-bit mask */
    #[inline]
    fn event_mask_offset(fd: i32) -> usize {
        ((fd as usize) % 4) * 2
    }

    /* Encode 2-bit mask at fd's position */
    #[inline]
    fn event_mask_encode(fd: i32, mask: i32) -> u8 {
        ((mask & 0x3) as u8) << Self::event_mask_offset(fd)
    }

    /* Get event mask for fd */
    #[inline]
    fn get_event_mask(&self, fd: i32) -> i32 {
        let byte_index = (fd as usize) / 4;
        if byte_index >= self.eventsMask.len() {
            return 0;
        }
        ((self.eventsMask[byte_index] >> Self::event_mask_offset(fd)) & 0x3) as i32
    }

    /* Add mask bits to fd's event mask */
    #[inline]
    fn add_event_mask(&mut self, fd: i32, mask: i32) {
        let byte_index = (fd as usize) / 4;
        if byte_index < self.eventsMask.len() {
            self.eventsMask[byte_index] |= Self::event_mask_encode(fd, mask);
        }
    }

    /* Reset fd's event mask to 0 */
    #[inline]
    fn reset_event_mask(&mut self, fd: i32) {
        let byte_index = (fd as usize) / 4;
        if byte_index < self.eventsMask.len() {
            self.eventsMask[byte_index] &= !Self::event_mask_encode(fd, 0x3);
        }
    }

    /* Safe wrapper for kevent registration calls */
    #[inline]
    fn register_kevent(&self, fd: i32, filter: libc::c_short, flags: libc::c_ushort) -> i32 {
        let mut ke = unsafe { std::mem::zeroed::<libc::kevent>() };
        unsafe {
            ev_set(
                &mut ke,
                fd as libc::uintptr_t,
                filter,
                flags,
                0,
                0,
                std::ptr::null_mut(),
            );
            if kevent(self.kqfd, &ke, 1, std::ptr::null_mut(), 0, std::ptr::null()) == -1 {
                -1
            } else {
                0
            }
        }
    }
}

impl EventBackend for aeApiState {
    fn create() -> Result<Box<Self>, i32> {
        let kqfd = unsafe { kqueue() };
        if kqfd == -1 {
            return Err(-1);
        }

        // Set close-on-exec flag (equivalent to anetCloexec)
        if unsafe { libc::fcntl(kqfd, libc::F_SETFD, libc::FD_CLOEXEC) } == -1 {
            unsafe { close(kqfd) };
            return Err(-1);
        }

        Ok(Box::new(aeApiState {
            kqfd,
            events: Vec::new(),
            eventsMask: Vec::new(),
        }))
    }

    fn free(self: Box<Self>) {
        unsafe { close(self.kqfd) };
    }

    fn resize(&mut self, setsize: i32) -> i32 {
        let new_size = setsize as usize;

        // Only grow, don't shrink and reallocate unnecessarily
        if new_size > self.events.capacity() {
            self.events.reserve(new_size - self.events.len());
        }
        self.events.resize(new_size, unsafe { std::mem::zeroed() });

        let mask_size = Self::event_mask_malloc_size(setsize);
        if mask_size > self.eventsMask.len() {
            self.eventsMask.resize(mask_size, 0);
        } else {
            // Clear existing mask data for smaller sizes
            for byte in &mut self.eventsMask[..mask_size] {
                *byte = 0;
            }
        }
        0
    }

    fn add_event(&mut self, fd: i32, mask: i32) -> i32 {
        if mask & AE_READABLE != 0 && self.register_kevent(fd, EVFILT_READ, EV_ADD) == -1 {
            return -1;
        }

        if mask & AE_WRITABLE != 0 && self.register_kevent(fd, EVFILT_WRITE, EV_ADD) == -1 {
            return -1;
        }

        0
    }

    fn del_event(&mut self, fd: i32, mask: i32) {
        if mask & AE_READABLE != 0 {
            self.register_kevent(fd, EVFILT_READ, EV_DELETE);
        }

        if mask & AE_WRITABLE != 0 {
            self.register_kevent(fd, EVFILT_WRITE, EV_DELETE);
        }
    }

    fn poll(
        &mut self,
        _events: &[crate::ae::AeFileEvent],
        fired: &mut [FiredEvent],
        _maxfd: i32,
        timeout: Option<Duration>,
    ) -> Result<i32, i32> {
        let retval = if let Some(timeout_dur) = timeout {
            let timeout_spec = timespec {
                tv_sec: timeout_dur.as_secs() as libc::time_t,
                tv_nsec: timeout_dur.subsec_nanos() as libc::c_long,
            };
            unsafe {
                kevent(
                    self.kqfd,
                    std::ptr::null(),
                    0,
                    self.events.as_mut_ptr(),
                    self.events.len() as libc::c_int,
                    &timeout_spec,
                )
            }
        } else {
            unsafe {
                kevent(
                    self.kqfd,
                    std::ptr::null(),
                    0,
                    self.events.as_mut_ptr(),
                    self.events.len() as libc::c_int,
                    std::ptr::null(),
                )
            }
        };

        if retval > 0 {
            /* Normally we execute the read event first and then the write event.
             * When the barrier is set, we will do it reverse.
             *
             * However, under kqueue, read and write events would be separate
             * events, which would make it impossible to control the order of
             * reads and writes. So we store the event's mask we've got and merge
             * the same fd events later. */
            for j in 0..retval {
                let e = &self.events[j as usize];
                let fd = e.ident as i32;
                let mask = if e.filter == EVFILT_READ {
                    AE_READABLE
                } else if e.filter == EVFILT_WRITE {
                    AE_WRITABLE
                } else {
                    0
                };

                if mask != 0 {
                    self.add_event_mask(fd, mask);
                }
            }

            /* Re-traversal to merge read and write events, and set the fd's mask to
             * 0 so that events are not added again when the fd is encountered again. */
            let mut numevents = 0;
            for j in 0..retval {
                let e = &self.events[j as usize];
                let fd = e.ident as i32;
                let mask = self.get_event_mask(fd);

                if mask != 0 && (numevents as usize) < fired.len() {
                    fired[numevents as usize] = FiredEvent { fd, mask };
                    self.reset_event_mask(fd);
                    numevents += 1;
                }
            }

            Ok(numevents)
        } else if retval == -1 {
            let errno = unsafe { *libc::__error() };
            if errno == EINTR { Ok(0) } else { Err(errno) }
        } else {
            Ok(0)
        }
    }

    fn name(&self) -> &'static str {
        "kqueue"
    }
}

pub fn ae_api_name() -> &'static str {
    "kqueue"
}
