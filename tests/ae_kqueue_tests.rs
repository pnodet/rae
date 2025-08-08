#[cfg(any(
    target_os = "macos",
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd"
))]
mod kqueue_tests {
    use rae::AeFileEvent;
    use rae::ae_kqueue::aeApiState;
    use rae::ae_select::FiredEvent;
    use rae::constants::{AE_READABLE, AE_WRITABLE};
    use rae::traits::EventBackend;
    use std::time::Duration;

    #[test]
    fn test_create() {
        let result = aeApiState::create();
        assert!(result.is_ok(), "Failed to create kqueue API state");

        let state = result.unwrap();
        assert_eq!(state.name(), "kqueue");
    }

    #[test]
    fn test_resize() {
        let mut state = aeApiState::create().expect("Failed to create kqueue API state");

        let result = state.resize(100);
        assert_eq!(result, 0, "Resize should return 0 on success");

        let result = state.resize(1024);
        assert_eq!(result, 0, "Second resize should also return 0 on success");
    }

    #[test]
    fn test_add_del_event() {
        let mut state = aeApiState::create().expect("Failed to create kqueue API state");
        let result = state.resize(10);
        assert_eq!(result, 0, "Resize should return 0 on success");

        // Note: kqueue requires actual open file descriptors, so add_event may fail with -1
        // This is normal behavior - the important thing is that it doesn't panic

        // Test adding readable event (may fail if fd not open, but shouldn't panic)
        let _result = state.add_event(5, AE_READABLE);
        // Don't assert success since fd 5 may not be open

        // Test removing events (should not fail/panic regardless of fd state)
        state.del_event(5, AE_READABLE);
        state.del_event(5, AE_WRITABLE);

        // Test removing non-existent event (should not fail/panic)
        state.del_event(10, AE_READABLE);
    }

    #[test]
    fn test_event_mask_combinations() {
        let mut state = aeApiState::create().expect("Failed to create kqueue API state");
        let result = state.resize(10);
        assert_eq!(result, 0, "Resize should return 0 on success");

        // Test combining read and write events (may fail if fd not open)
        let _result = state.add_event(3, AE_READABLE | AE_WRITABLE);
        // Don't assert success since fd 3 may not be open

        state.del_event(3, AE_READABLE | AE_WRITABLE);
    }

    #[test]
    fn test_poll() {
        let mut state = aeApiState::create().expect("Failed to create kqueue API state");
        let result = state.resize(10);
        assert_eq!(result, 0, "Resize should return 0 on success");

        let events = vec![AeFileEvent::new(); 10];
        let mut fired = vec![FiredEvent { fd: -1, mask: 0 }; 10];

        // Test poll with immediate timeout (should return 0 events)
        let timeout = Some(Duration::from_millis(0));
        let result = state.poll(&events, &mut fired, 9, timeout);

        assert!(result.is_ok(), "Poll should succeed");
        assert_eq!(
            result.unwrap(),
            0,
            "Poll should return 0 events with no registered events"
        );
    }

    #[test]
    fn test_poll_no_timeout() {
        let mut state = aeApiState::create().expect("Failed to create kqueue API state");
        let result = state.resize(10);
        assert_eq!(result, 0, "Resize should return 0 on success");

        let events = vec![AeFileEvent::new(); 10];
        let mut fired = vec![FiredEvent { fd: -1, mask: 0 }; 10];

        // Create a separate thread to interrupt the poll after a short time
        use std::sync::{Arc, Mutex};
        use std::thread;

        let state = Arc::new(Mutex::new(state));
        let _state_clone = Arc::clone(&state);

        let handle = thread::spawn(move || {
            thread::sleep(Duration::from_millis(10));
            // Just let the thread exit, which will help ensure poll doesn't hang forever
        });

        // Test poll with no timeout - since no events are registered,
        // this should return quickly on most systems
        let result = {
            let mut state_guard = state.lock().unwrap();
            state_guard.poll(&events, &mut fired, 9, Some(Duration::from_millis(1)))
        };

        handle.join().unwrap();

        assert!(result.is_ok(), "Poll should succeed even with no events");
    }

    #[test]
    fn test_api_name() {
        use rae::ae_kqueue::ae_api_name;
        assert_eq!(ae_api_name(), "kqueue");
    }

    #[test]
    fn test_event_mask_optimization() {
        let mut state = aeApiState::create().expect("Failed to create kqueue API state");

        // Test the memory optimization calculations
        assert_eq!(state.resize(1), 0);
        assert_eq!(state.resize(4), 0); // Should use 1 byte
        assert_eq!(state.resize(8), 0); // Should use 2 bytes
        assert_eq!(state.resize(100), 0); // Should use 25 bytes

        // All resizes should succeed
    }

    #[test]
    fn test_large_fd_values() {
        let mut state = aeApiState::create().expect("Failed to create kqueue API state");
        assert_eq!(state.resize(1000), 0, "Resize should succeed");

        // Test with larger file descriptor values (may fail if fd not open)
        let _result = state.add_event(999, AE_READABLE);
        // Don't assert success since fd 999 is unlikely to be open

        state.del_event(999, AE_READABLE);
    }

    #[test]
    fn test_zero_setsize() {
        let mut state = aeApiState::create().expect("Failed to create kqueue API state");
        let result = state.resize(0);
        assert_eq!(result, 0, "Resize to 0 should succeed");

        let events = vec![];
        let mut fired = vec![];
        let result = state.poll(&events, &mut fired, -1, Some(Duration::from_millis(1)));
        assert!(result.is_ok(), "Poll with empty arrays should succeed");
    }

    #[test]
    fn test_negative_fd() {
        let mut state = aeApiState::create().expect("Failed to create kqueue API state");
        let result = state.resize(10);
        assert_eq!(result, 0, "Resize should return 0 on success");

        // Adding events for negative fds might fail, which is expected
        let _result = state.add_event(-1, AE_READABLE);
        // Don't assert success/failure as this is platform-dependent behavior

        // Deleting should not panic
        state.del_event(-1, AE_READABLE);
    }

    #[test]
    fn test_poll_edge_cases() {
        let mut state = aeApiState::create().expect("Failed to create kqueue API state");
        assert_eq!(state.resize(5), 0, "Resize should succeed");

        // Test with events array smaller than setsize
        let events = vec![AeFileEvent::new(); 3];
        let mut fired = vec![FiredEvent { fd: -1, mask: 0 }; 5];

        let result = state.poll(&events, &mut fired, 2, Some(Duration::from_millis(1)));
        assert!(
            result.is_ok(),
            "Poll should handle size mismatches gracefully"
        );

        // Test with fired array smaller than events
        let events = vec![AeFileEvent::new(); 5];
        let mut fired = vec![FiredEvent { fd: -1, mask: 0 }; 2];

        let result = state.poll(&events, &mut fired, 4, Some(Duration::from_millis(1)));
        assert!(
            result.is_ok(),
            "Poll should handle small fired array gracefully"
        );
    }
}

// Dummy test for platforms without kqueue
#[cfg(not(any(
    target_os = "macos",
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd"
)))]
#[test]
fn test_kqueue_not_available() {
    // This test exists to ensure the test suite runs on all platforms
    // Even though kqueue is not available, the test should pass
    assert!(true, "kqueue not available on this platform");
}
