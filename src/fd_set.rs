//! Safe Rust wrapper around libc's fd_set

use libc::{FD_CLR, FD_ISSET, FD_SET, FD_SETSIZE, FD_ZERO, fd_set};

/// Safe wrapper around libc's fd_set
#[derive(Clone)]
pub struct FdSet {
    inner: fd_set,
}

impl std::fmt::Debug for FdSet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FdSet").finish_non_exhaustive()
    }
}

impl FdSet {
    pub fn zero() -> Self {
        let mut fdset = FdSet {
            inner: unsafe { std::mem::zeroed() },
        };
        unsafe {
            FD_ZERO(&mut fdset.inner);
        }
        fdset
    }

    pub fn set(&mut self, fd: i32) {
        if fd >= 0 && (fd as usize) < FD_SETSIZE {
            unsafe {
                FD_SET(fd, &mut self.inner);
            }
        }
    }

    pub fn clr(&mut self, fd: i32) {
        if fd >= 0 && (fd as usize) < FD_SETSIZE {
            unsafe {
                FD_CLR(fd, &mut self.inner);
            }
        }
    }

    pub fn isset(&self, fd: i32) -> bool {
        if fd >= 0 && (fd as usize) < FD_SETSIZE {
            unsafe { FD_ISSET(fd, &self.inner) }
        } else {
            false
        }
    }

    /// Get mutable pointer to inner fd_set for select() FFI calls
    pub fn as_mut_ptr(&mut self) -> *mut fd_set {
        &mut self.inner
    }
}
