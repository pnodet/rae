/* File Event Tests
 *
 * Tests for file event functionality including creation, deletion,
 * event masks, and callback execution for file descriptor events.
 */

use rae::{
    AE_DONT_WAIT, AE_FILE_EVENTS, AE_OK, AE_READABLE, AE_WRITABLE, ae_create_event_loop,
    ae_create_file_event, ae_delete_event_loop, ae_delete_file_event, ae_get_file_client_data,
    ae_get_file_events, ae_process_events,
};
use std::ffi::c_void;
use std::sync::atomic::{AtomicI32, Ordering};

// Test callbacks
static READ_CALLBACK_COUNTER: AtomicI32 = AtomicI32::new(0);
static WRITE_CALLBACK_COUNTER: AtomicI32 = AtomicI32::new(0);

fn read_callback(
    _event_loop: &mut rae::AeEventLoop,
    _fd: i32,
    _client_data: *mut c_void,
    _mask: i32,
) {
    READ_CALLBACK_COUNTER.fetch_add(1, Ordering::SeqCst);
}

fn write_callback(
    _event_loop: &mut rae::AeEventLoop,
    _fd: i32,
    _client_data: *mut c_void,
    _mask: i32,
) {
    WRITE_CALLBACK_COUNTER.fetch_add(1, Ordering::SeqCst);
}

fn combined_callback(
    _event_loop: &mut rae::AeEventLoop,
    fd: i32,
    client_data: *mut c_void,
    mask: i32,
) {
    // Record which events were received
    if !client_data.is_null() {
        unsafe {
            let counters = client_data as *mut (i32, i32, i32); // (fd, read_count, write_count)
            (*counters).0 = fd;
            if mask & AE_READABLE != 0 {
                (*counters).1 += 1;
            }
            if mask & AE_WRITABLE != 0 {
                (*counters).2 += 1;
            }
        }
    }
}

mod basic_functionality {
    use super::*;

    #[test]
    fn test_create_file_event_read() {
        let mut event_loop = ae_create_event_loop(64).expect("Failed to create event loop");

        // Create a readable file event
        let result = ae_create_file_event(
            &mut event_loop,
            5, // fd
            AE_READABLE,
            read_callback,
            std::ptr::null_mut(),
        );

        assert_eq!(result, AE_OK, "Creating readable file event should succeed");

        // Verify event was registered
        let mask = ae_get_file_events(&event_loop, 5);
        assert_eq!(
            mask, AE_READABLE,
            "File descriptor should be registered for reading"
        );

        ae_delete_event_loop(event_loop);
    }

    #[test]
    fn test_create_file_event_write() {
        let mut event_loop = ae_create_event_loop(64).expect("Failed to create event loop");

        // Create a writable file event
        let result = ae_create_file_event(
            &mut event_loop,
            7,
            AE_WRITABLE,
            write_callback,
            std::ptr::null_mut(),
        );

        assert_eq!(result, AE_OK, "Creating writable file event should succeed");

        // Verify event was registered
        let mask = ae_get_file_events(&event_loop, 7);
        assert_eq!(
            mask, AE_WRITABLE,
            "File descriptor should be registered for writing"
        );

        ae_delete_event_loop(event_loop);
    }

    #[test]
    fn test_create_file_event_read_write() {
        let mut event_loop = ae_create_event_loop(64).expect("Failed to create event loop");

        // Create a file event for both reading and writing
        let result = ae_create_file_event(
            &mut event_loop,
            3,
            AE_READABLE | AE_WRITABLE,
            combined_callback,
            std::ptr::null_mut(),
        );

        assert_eq!(
            result, AE_OK,
            "Creating read/write file event should succeed"
        );

        // Verify event was registered
        let mask = ae_get_file_events(&event_loop, 3);
        assert_eq!(
            mask,
            AE_READABLE | AE_WRITABLE,
            "File descriptor should be registered for both"
        );

        ae_delete_event_loop(event_loop);
    }

    #[test]
    fn test_delete_file_event() {
        let mut event_loop = ae_create_event_loop(64).expect("Failed to create event loop");

        // Create file event
        let result = ae_create_file_event(
            &mut event_loop,
            10,
            AE_READABLE,
            read_callback,
            std::ptr::null_mut(),
        );
        assert_eq!(result, AE_OK);

        // Verify it exists
        let mask = ae_get_file_events(&event_loop, 10);
        assert_eq!(mask, AE_READABLE);

        // Delete the event
        ae_delete_file_event(&mut event_loop, 10, AE_READABLE);

        // Verify it's gone
        let mask = ae_get_file_events(&event_loop, 10);
        assert_eq!(mask, 0, "File event should be deleted");

        ae_delete_event_loop(event_loop);
    }

    #[test]
    fn test_partial_delete_file_event() {
        let mut event_loop = ae_create_event_loop(64).expect("Failed to create event loop");

        // Create read/write event
        let result = ae_create_file_event(
            &mut event_loop,
            12,
            AE_READABLE | AE_WRITABLE,
            combined_callback,
            std::ptr::null_mut(),
        );
        assert_eq!(result, AE_OK);

        // Verify both masks are set
        let mask = ae_get_file_events(&event_loop, 12);
        assert_eq!(mask, AE_READABLE | AE_WRITABLE);

        // Delete only the readable part
        ae_delete_file_event(&mut event_loop, 12, AE_READABLE);

        // Verify only writable remains
        let mask = ae_get_file_events(&event_loop, 12);
        assert_eq!(mask, AE_WRITABLE, "Only writable should remain");

        // Delete the remaining part
        ae_delete_file_event(&mut event_loop, 12, AE_WRITABLE);

        // Verify nothing remains
        let mask = ae_get_file_events(&event_loop, 12);
        assert_eq!(mask, 0, "No events should remain");

        ae_delete_event_loop(event_loop);
    }
}

mod event_masks {
    use super::*;

    #[test]
    fn test_multiple_file_descriptors() {
        let mut event_loop = ae_create_event_loop(64).expect("Failed to create event loop");

        // Create events for different fds
        let result1 = ae_create_file_event(
            &mut event_loop,
            1,
            AE_READABLE,
            read_callback,
            std::ptr::null_mut(),
        );
        let result2 = ae_create_file_event(
            &mut event_loop,
            2,
            AE_WRITABLE,
            write_callback,
            std::ptr::null_mut(),
        );
        let result3 = ae_create_file_event(
            &mut event_loop,
            3,
            AE_READABLE | AE_WRITABLE,
            combined_callback,
            std::ptr::null_mut(),
        );

        assert_eq!(result1, AE_OK);
        assert_eq!(result2, AE_OK);
        assert_eq!(result3, AE_OK);

        // Verify each fd has correct mask
        assert_eq!(ae_get_file_events(&event_loop, 1), AE_READABLE);
        assert_eq!(ae_get_file_events(&event_loop, 2), AE_WRITABLE);
        assert_eq!(
            ae_get_file_events(&event_loop, 3),
            AE_READABLE | AE_WRITABLE
        );

        // Verify non-registered fds return 0
        assert_eq!(ae_get_file_events(&event_loop, 4), 0);
        assert_eq!(ae_get_file_events(&event_loop, 100), 0);

        ae_delete_event_loop(event_loop);
    }

    #[test]
    fn test_override_file_event() {
        let mut event_loop = ae_create_event_loop(64).expect("Failed to create event loop");

        // Create readable event
        let result = ae_create_file_event(
            &mut event_loop,
            15,
            AE_READABLE,
            read_callback,
            std::ptr::null_mut(),
        );
        assert_eq!(result, AE_OK);
        assert_eq!(ae_get_file_events(&event_loop, 15), AE_READABLE);

        // Add writable to same fd
        let result = ae_create_file_event(
            &mut event_loop,
            15,
            AE_WRITABLE,
            write_callback,
            std::ptr::null_mut(),
        );
        assert_eq!(result, AE_OK);

        // Should have both masks
        let mask = ae_get_file_events(&event_loop, 15);
        assert_eq!(
            mask,
            AE_READABLE | AE_WRITABLE,
            "Should have both readable and writable"
        );

        ae_delete_event_loop(event_loop);
    }

    #[test]
    fn test_invalid_file_descriptors() {
        let mut event_loop = ae_create_event_loop(64).expect("Failed to create event loop");

        // Test negative fd
        let _result = ae_create_file_event(
            &mut event_loop,
            -1,
            AE_READABLE,
            read_callback,
            std::ptr::null_mut(),
        );
        // Should either succeed or fail gracefully
        // Don't assert specific behavior as it's platform dependent

        // Test very large fd
        let _result = ae_create_file_event(
            &mut event_loop,
            999999,
            AE_READABLE,
            read_callback,
            std::ptr::null_mut(),
        );
        // Should either succeed or fail gracefully

        ae_delete_event_loop(event_loop);
    }

    #[test]
    fn test_zero_mask() {
        let mut event_loop = ae_create_event_loop(64).expect("Failed to create event loop");

        // Try to create event with no mask
        let _result = ae_create_file_event(
            &mut event_loop,
            5,
            0, // No mask
            read_callback,
            std::ptr::null_mut(),
        );

        // Should fail or succeed gracefully
        // Implementation detail - don't assert specific behavior

        ae_delete_event_loop(event_loop);
    }
}

mod client_data {
    use super::*;

    #[test]
    fn test_client_data_storage() {
        let mut event_loop = ae_create_event_loop(64).expect("Failed to create event loop");

        let mut test_data = 42i32;
        let client_data = &mut test_data as *mut i32 as *mut c_void;

        // Create event with client data
        let result =
            ae_create_file_event(&mut event_loop, 20, AE_READABLE, read_callback, client_data);
        assert_eq!(result, AE_OK);

        // Retrieve client data
        let retrieved_data = ae_get_file_client_data(&event_loop, 20);
        assert_eq!(
            retrieved_data, client_data,
            "Client data should be preserved"
        );

        // Verify the actual value
        unsafe {
            let value_ptr = retrieved_data as *mut i32;
            assert_eq!(*value_ptr, 42, "Client data value should be preserved");
        }

        ae_delete_event_loop(event_loop);
    }

    #[test]
    fn test_null_client_data() {
        let mut event_loop = ae_create_event_loop(64).expect("Failed to create event loop");

        // Create event with null client data
        let result = ae_create_file_event(
            &mut event_loop,
            25,
            AE_READABLE,
            read_callback,
            std::ptr::null_mut(),
        );
        assert_eq!(result, AE_OK);

        // Retrieve client data - should be null
        let retrieved_data = ae_get_file_client_data(&event_loop, 25);
        assert!(retrieved_data.is_null(), "Client data should be null");

        ae_delete_event_loop(event_loop);
    }

    #[test]
    fn test_client_data_for_nonexistent_fd() {
        let event_loop = ae_create_event_loop(64).expect("Failed to create event loop");

        // Try to get client data for non-existent fd
        let retrieved_data = ae_get_file_client_data(&event_loop, 999);
        assert!(
            retrieved_data.is_null(),
            "Client data for non-existent fd should be null"
        );

        // Manual cleanup since we're using immutable reference
        drop(event_loop);
    }
}

mod edge_cases {
    use super::*;

    #[test]
    fn test_delete_nonexistent_event() {
        let mut event_loop = ae_create_event_loop(64).expect("Failed to create event loop");

        // Delete event that doesn't exist - should not panic
        ae_delete_file_event(&mut event_loop, 99, AE_READABLE);
        ae_delete_file_event(&mut event_loop, 99, AE_WRITABLE);
        ae_delete_file_event(&mut event_loop, 99, AE_READABLE | AE_WRITABLE);

        ae_delete_event_loop(event_loop);
    }

    #[test]
    fn test_file_events_with_small_setsize() {
        let mut event_loop = ae_create_event_loop(5).expect("Failed to create small event loop");

        // Create events within bounds
        let result1 = ae_create_file_event(
            &mut event_loop,
            0,
            AE_READABLE,
            read_callback,
            std::ptr::null_mut(),
        );
        let result2 = ae_create_file_event(
            &mut event_loop,
            4,
            AE_WRITABLE,
            write_callback,
            std::ptr::null_mut(),
        );

        assert_eq!(result1, AE_OK);
        assert_eq!(result2, AE_OK);

        // Try to create event outside bounds
        let result3 = ae_create_file_event(
            &mut event_loop,
            5,
            AE_READABLE,
            read_callback,
            std::ptr::null_mut(),
        );
        // Should fail gracefully or extend the array

        ae_delete_event_loop(event_loop);
    }

    #[test]
    fn test_process_file_events_no_events() {
        let mut event_loop = ae_create_event_loop(64).expect("Failed to create event loop");

        // Process file events when none are registered
        let processed = ae_process_events(&mut event_loop, AE_FILE_EVENTS | AE_DONT_WAIT);
        assert_eq!(
            processed, 0,
            "Should process 0 file events when none registered"
        );

        ae_delete_event_loop(event_loop);
    }

    #[test]
    fn test_large_file_descriptor() {
        let mut event_loop = ae_create_event_loop(1000).expect("Failed to create large event loop");

        // Test with large but valid fd
        let result = ae_create_file_event(
            &mut event_loop,
            999,
            AE_READABLE,
            read_callback,
            std::ptr::null_mut(),
        );

        // Should succeed
        assert_eq!(result, AE_OK, "Large fd should be supported within setsize");

        let mask = ae_get_file_events(&event_loop, 999);
        assert_eq!(mask, AE_READABLE, "Large fd event should be registered");

        ae_delete_event_loop(event_loop);
    }
}

mod callback_verification {
    use super::*;

    fn fd_recording_callback(
        _event_loop: &mut rae::AeEventLoop,
        fd: i32,
        client_data: *mut c_void,
        mask: i32,
    ) {
        if !client_data.is_null() {
            unsafe {
                let record = client_data as *mut (i32, i32); // (fd, mask)
                (*record).0 = fd;
                (*record).1 = mask;
            }
        }
    }

    #[test]
    fn test_callback_parameters() {
        let mut event_loop = ae_create_event_loop(64).expect("Failed to create event loop");

        let mut callback_record = (0i32, 0i32); // Will store (fd, mask)
        let client_data = &mut callback_record as *mut (i32, i32) as *mut c_void;

        // Create file event
        let result = ae_create_file_event(
            &mut event_loop,
            42,
            AE_READABLE,
            fd_recording_callback,
            client_data,
        );
        assert_eq!(result, AE_OK);

        // Note: We can't easily trigger the callback without real file descriptors
        // This test verifies the setup is correct

        // Verify event was registered correctly
        let mask = ae_get_file_events(&event_loop, 42);
        assert_eq!(
            mask, AE_READABLE,
            "Event should be registered with correct mask"
        );

        let retrieved_data = ae_get_file_client_data(&event_loop, 42);
        assert_eq!(
            retrieved_data, client_data,
            "Client data should be preserved"
        );

        ae_delete_event_loop(event_loop);
    }
}
