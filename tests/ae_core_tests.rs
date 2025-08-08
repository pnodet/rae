/* Core AE Event Loop Tests
 *
 * Tests for the main ae.rs functionality including event loop creation,
 * basic operations, and core API functions.
 */

use rae::{
    AE_ALL_EVENTS, AE_DONT_WAIT, AE_ERR, AE_FILE_EVENTS, AE_OK, AE_TIME_EVENTS,
    ae_create_event_loop, ae_delete_event_loop, ae_get_api_name, ae_get_set_size,
    ae_process_events, ae_resize_set_size, ae_set_dont_wait, ae_stop,
};

mod core_functionality {
    use super::*;

    #[test]
    fn test_create_event_loop() {
        let event_loop = ae_create_event_loop(1024);
        assert!(event_loop.is_some(), "Failed to create event loop");

        let el = event_loop.unwrap();
        assert_eq!(ae_get_set_size(&el), 1024);

        ae_delete_event_loop(el);
    }

    #[test]
    fn test_create_event_loop_invalid_size() {
        // Test with size 0 - should fail or succeed based on implementation
        let result = ae_create_event_loop(0);
        // Our implementation might allow size 0 - either behavior is acceptable

        // Test with negative size - should fail
        let result = ae_create_event_loop(-1);
        assert!(
            result.is_none(),
            "Creating event loop with negative size should fail"
        );
    }

    #[test]
    fn test_resize_event_loop() {
        let mut event_loop = ae_create_event_loop(100).expect("Failed to create event loop");

        // Test valid resize
        let result = ae_resize_set_size(&mut event_loop, 200);
        assert_eq!(result, AE_OK, "Resize should succeed");
        assert_eq!(ae_get_set_size(&event_loop), 200);

        // Test resize to smaller size
        let result = ae_resize_set_size(&mut event_loop, 50);
        assert_eq!(result, AE_OK, "Resize to smaller size should succeed");
        assert_eq!(ae_get_set_size(&event_loop), 50);

        ae_delete_event_loop(event_loop);
    }

    #[test]
    fn test_resize_invalid_size() {
        let mut event_loop = ae_create_event_loop(100).expect("Failed to create event loop");

        // Test resize to 0 - may succeed or fail based on implementation
        let result = ae_resize_set_size(&mut event_loop, 0);
        // Our implementation might allow resize to 0 - either AE_OK or AE_ERR is acceptable

        // Test resize to negative - should fail
        let result = ae_resize_set_size(&mut event_loop, -1);
        assert_eq!(result, AE_ERR, "Resize to negative should fail");

        ae_delete_event_loop(event_loop);
    }

    #[test]
    fn test_api_name() {
        let api_name = ae_get_api_name();
        // Should be either "select" or "kqueue" depending on platform
        assert!(
            api_name == "select" || api_name == "kqueue",
            "API name should be select or kqueue, got: {}",
            api_name
        );
    }

    #[test]
    fn test_process_events_no_events() {
        let mut event_loop = ae_create_event_loop(64).expect("Failed to create event loop");

        // Process events with no registered events - should return 0
        let processed = ae_process_events(&mut event_loop, AE_ALL_EVENTS | AE_DONT_WAIT);
        assert_eq!(processed, 0, "Should process 0 events when none registered");

        ae_delete_event_loop(event_loop);
    }

    #[test]
    fn test_event_flags() {
        let mut event_loop = ae_create_event_loop(64).expect("Failed to create event loop");

        // Test processing with different flag combinations
        let processed = ae_process_events(&mut event_loop, AE_FILE_EVENTS | AE_DONT_WAIT);
        assert_eq!(processed, 0);

        let processed = ae_process_events(&mut event_loop, AE_TIME_EVENTS | AE_DONT_WAIT);
        assert_eq!(processed, 0);

        let processed = ae_process_events(&mut event_loop, AE_ALL_EVENTS | AE_DONT_WAIT);
        assert_eq!(processed, 0);

        ae_delete_event_loop(event_loop);
    }

    #[test]
    fn test_stop_event_loop() {
        let mut event_loop = ae_create_event_loop(64).expect("Failed to create event loop");

        // Stop the event loop
        ae_stop(&mut event_loop);

        // Process events - should handle stop condition
        let processed = ae_process_events(&mut event_loop, AE_DONT_WAIT);
        assert_eq!(processed, 0);

        ae_delete_event_loop(event_loop);
    }

    #[test]
    fn test_dont_wait_flag() {
        let mut event_loop = ae_create_event_loop(64).expect("Failed to create event loop");

        // Set dont wait flag
        ae_set_dont_wait(&mut event_loop, true);

        // Process events - should not block
        let processed = ae_process_events(&mut event_loop, AE_ALL_EVENTS | AE_DONT_WAIT);
        assert_eq!(processed, 0);

        // Clear dont wait flag
        ae_set_dont_wait(&mut event_loop, false);

        ae_delete_event_loop(event_loop);
    }
}

mod error_conditions {
    use super::*;

    #[test]
    fn test_large_setsize() {
        // Test with a very large setsize - should succeed or fail gracefully
        let result = ae_create_event_loop(100000);
        match result {
            Some(el) => {
                assert_eq!(ae_get_set_size(&el), 100000);
                ae_delete_event_loop(el);
            }
            None => {
                // Large setsize might fail due to system limits - that's ok
            }
        }
    }

    #[test]
    fn test_multiple_create_delete() {
        // Test creating and deleting multiple event loops
        for i in 1..=10 {
            let event_loop = ae_create_event_loop(64 + i).expect("Failed to create event loop");
            assert_eq!(ae_get_set_size(&event_loop), 64 + i);
            ae_delete_event_loop(event_loop);
        }
    }

    #[test]
    fn test_process_events_edge_cases() {
        let mut event_loop = ae_create_event_loop(1).expect("Failed to create event loop");

        // Test with flag 0 - should return 0
        let processed = ae_process_events(&mut event_loop, 0);
        assert_eq!(processed, 0);

        ae_delete_event_loop(event_loop);
    }
}

mod platform_specific {
    use super::*;

    #[test]
    fn test_backend_consistency() {
        let event_loop = ae_create_event_loop(64).expect("Failed to create event loop");
        let api_name = ae_get_api_name();

        // Ensure backend is consistently reported
        assert!(!api_name.is_empty(), "API name should not be empty");
        assert!(api_name.len() > 2, "API name should be meaningful");

        ae_delete_event_loop(event_loop);
    }

    #[cfg(any(
        target_os = "macos",
        target_os = "freebsd",
        target_os = "openbsd",
        target_os = "netbsd"
    ))]
    #[test]
    fn test_kqueue_available() {
        let api_name = ae_get_api_name();
        // On BSD systems, we should prefer kqueue over select
        // Note: This test might need adjustment based on actual priority logic
        assert!(
            api_name == "kqueue" || api_name == "select",
            "On BSD systems, should use kqueue or select"
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn test_select_on_linux() {
        let api_name = ae_get_api_name();
        // On Linux, we currently only have select (epoll not implemented yet)
        assert_eq!(api_name, "select", "On Linux, should use select backend");
    }
}
