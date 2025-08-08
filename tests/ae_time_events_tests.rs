/* Time Event Tests
 *
 * Tests for time event functionality including creation, deletion,
 * scheduling, and callback execution.
 */

use rae::{
    AE_DONT_WAIT, AE_NOMORE, AE_TIME_EVENTS, ae_create_event_loop, ae_create_time_event,
    ae_delete_event_loop, ae_delete_time_event, ae_process_events,
};
use std::ffi::c_void;
use std::sync::atomic::{AtomicI32, Ordering};
use std::time::Duration;

// Helper callback that increments a counter
static CALLBACK_COUNTER: AtomicI32 = AtomicI32::new(0);

fn test_time_callback(
    _event_loop: &mut rae::AeEventLoop,
    _id: i64,
    _client_data: *mut c_void,
) -> i32 {
    CALLBACK_COUNTER.fetch_add(1, Ordering::SeqCst);
    AE_NOMORE // Don't reschedule
}

// Callback that reschedules itself
fn reschedule_callback(
    _event_loop: &mut rae::AeEventLoop,
    _id: i64,
    _client_data: *mut c_void,
) -> i32 {
    CALLBACK_COUNTER.fetch_add(1, Ordering::SeqCst);
    100 // Reschedule in 100ms
}

mod basic_functionality {
    use super::*;

    #[test]
    fn test_create_time_event() {
        let mut event_loop = ae_create_event_loop(64).expect("Failed to create event loop");

        // Create a time event scheduled for immediate execution
        let event_id = ae_create_time_event(
            &mut event_loop,
            1, // 1ms from now
            test_time_callback,
            std::ptr::null_mut(),
            None,
        );

        assert!(event_id > 0, "Time event ID should be positive");

        ae_delete_event_loop(event_loop);
    }

    #[test]
    fn test_delete_time_event() {
        let mut event_loop = ae_create_event_loop(64).expect("Failed to create event loop");

        // Create a time event
        let event_id = ae_create_time_event(
            &mut event_loop,
            1000, // 1 second from now
            test_time_callback,
            std::ptr::null_mut(),
            None,
        );

        assert!(event_id > 0);

        // Delete the time event
        let result = ae_delete_time_event(&mut event_loop, event_id);
        assert_eq!(result, 0, "Deleting time event should succeed");

        // Try to delete again - should fail
        let result = ae_delete_time_event(&mut event_loop, event_id);
        assert_eq!(result, -1, "Deleting non-existent time event should fail");

        ae_delete_event_loop(event_loop);
    }

    #[test]
    fn test_invalid_time_event_operations() {
        let mut event_loop = ae_create_event_loop(64).expect("Failed to create event loop");

        // Try to delete non-existent event
        let result = ae_delete_time_event(&mut event_loop, 999999);
        assert_eq!(result, -1, "Deleting non-existent event should fail");

        // Try to delete with invalid ID
        let result = ae_delete_time_event(&mut event_loop, -1);
        assert_eq!(result, -1, "Deleting with invalid ID should fail");

        ae_delete_event_loop(event_loop);
    }
}

mod time_execution {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_time_event_execution() {
        let mut event_loop = ae_create_event_loop(64).expect("Failed to create event loop");

        // Reset counter
        CALLBACK_COUNTER.store(0, Ordering::SeqCst);

        // Create a time event for immediate execution
        let _event_id = ae_create_time_event(
            &mut event_loop,
            1, // 1ms from now
            test_time_callback,
            std::ptr::null_mut(),
            None,
        );

        // Sleep briefly to ensure the event time has passed
        std::thread::sleep(Duration::from_millis(5));

        // Process time events
        let processed = ae_process_events(&mut event_loop, AE_TIME_EVENTS | AE_DONT_WAIT);
        assert_eq!(processed, 1, "Should process 1 time event");

        // Check that callback was executed
        let counter = CALLBACK_COUNTER.load(Ordering::SeqCst);
        assert_eq!(counter, 1, "Callback should have been executed once");

        ae_delete_event_loop(event_loop);
    }

    #[test]
    fn test_multiple_time_events() {
        let mut event_loop = ae_create_event_loop(64).expect("Failed to create event loop");

        CALLBACK_COUNTER.store(0, Ordering::SeqCst);

        // Create multiple time events
        for i in 1..=3 {
            let _event_id = ae_create_time_event(
                &mut event_loop,
                i, // i ms from now
                test_time_callback,
                std::ptr::null_mut(),
                None,
            );
        }

        // Sleep to ensure all events have passed their time
        std::thread::sleep(Duration::from_millis(10));

        // Process time events
        let processed = ae_process_events(&mut event_loop, AE_TIME_EVENTS | AE_DONT_WAIT);
        assert_eq!(processed, 3, "Should process 3 time events");

        let counter = CALLBACK_COUNTER.load(Ordering::SeqCst);
        assert_eq!(counter, 3, "All 3 callbacks should have been executed");

        ae_delete_event_loop(event_loop);
    }

    #[test]
    fn test_time_event_rescheduling() {
        let mut event_loop = ae_create_event_loop(64).expect("Failed to create event loop");

        CALLBACK_COUNTER.store(0, Ordering::SeqCst);

        // Create a rescheduling time event
        let _event_id = ae_create_time_event(
            &mut event_loop,
            1, // 1ms from now
            reschedule_callback,
            std::ptr::null_mut(),
            None,
        );

        std::thread::sleep(Duration::from_millis(5));

        // Process first execution
        let processed = ae_process_events(&mut event_loop, AE_TIME_EVENTS | AE_DONT_WAIT);
        assert_eq!(processed, 1, "Should process 1 time event");

        let counter = CALLBACK_COUNTER.load(Ordering::SeqCst);
        assert_eq!(counter, 1, "Callback should have been executed once");

        // The event should be rescheduled for 100ms later
        // Process again immediately - should be 0 events
        let processed = ae_process_events(&mut event_loop, AE_TIME_EVENTS | AE_DONT_WAIT);
        assert_eq!(
            processed, 0,
            "Should not process rescheduled event immediately"
        );

        ae_delete_event_loop(event_loop);
    }

    #[test]
    fn test_future_time_event() {
        let mut event_loop = ae_create_event_loop(64).expect("Failed to create event loop");

        CALLBACK_COUNTER.store(0, Ordering::SeqCst);

        // Create a time event for the future
        let _event_id = ae_create_time_event(
            &mut event_loop,
            5000, // 5 seconds from now
            test_time_callback,
            std::ptr::null_mut(),
            None,
        );

        // Process immediately - should not execute the event
        let processed = ae_process_events(&mut event_loop, AE_TIME_EVENTS | AE_DONT_WAIT);
        assert_eq!(processed, 0, "Should not process future time event");

        let counter = CALLBACK_COUNTER.load(Ordering::SeqCst);
        assert_eq!(counter, 0, "Future callback should not execute");

        ae_delete_event_loop(event_loop);
    }
}

mod edge_cases {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_zero_delay_time_event() {
        let mut event_loop = ae_create_event_loop(64).expect("Failed to create event loop");

        CALLBACK_COUNTER.store(0, Ordering::SeqCst);

        // Create a time event with 0 delay (immediate)
        let _event_id = ae_create_time_event(
            &mut event_loop,
            0, // immediate
            test_time_callback,
            std::ptr::null_mut(),
            None,
        );

        // Process immediately
        let processed = ae_process_events(&mut event_loop, AE_TIME_EVENTS | AE_DONT_WAIT);
        assert_eq!(processed, 1, "Should process immediate time event");

        let counter = CALLBACK_COUNTER.load(Ordering::SeqCst);
        assert_eq!(counter, 1, "Immediate callback should execute");

        ae_delete_event_loop(event_loop);
    }

    #[test]
    fn test_delete_during_processing() {
        let mut event_loop = ae_create_event_loop(64).expect("Failed to create event loop");

        // Create multiple time events
        let event_id1 = ae_create_time_event(
            &mut event_loop,
            1,
            test_time_callback,
            std::ptr::null_mut(),
            None,
        );

        let event_id2 = ae_create_time_event(
            &mut event_loop,
            2,
            test_time_callback,
            std::ptr::null_mut(),
            None,
        );

        // Delete one before processing
        let result = ae_delete_time_event(&mut event_loop, event_id1);
        assert_eq!(result, 0, "Deleting event should succeed");

        std::thread::sleep(Duration::from_millis(5));

        CALLBACK_COUNTER.store(0, Ordering::SeqCst);

        // Process - should only execute the remaining event
        let processed = ae_process_events(&mut event_loop, AE_TIME_EVENTS | AE_DONT_WAIT);
        assert_eq!(processed, 1, "Should process 1 remaining time event");

        let counter = CALLBACK_COUNTER.load(Ordering::SeqCst);
        assert_eq!(counter, 1, "Only remaining callback should execute");

        // Clean up remaining event
        ae_delete_time_event(&mut event_loop, event_id2);

        ae_delete_event_loop(event_loop);
    }

    #[test]
    fn test_large_delay() {
        let mut event_loop = ae_create_event_loop(64).expect("Failed to create event loop");

        // Create a time event with very large delay
        let _event_id = ae_create_time_event(
            &mut event_loop,
            i64::MAX, // Very large delay
            test_time_callback,
            std::ptr::null_mut(),
            None,
        );

        // Should not execute immediately
        let processed = ae_process_events(&mut event_loop, AE_TIME_EVENTS | AE_DONT_WAIT);
        assert_eq!(processed, 0, "Should not process event with large delay");

        ae_delete_event_loop(event_loop);
    }
}

mod callback_data {
    use super::*;
    use std::time::Duration;

    // Callback that uses client data
    fn client_data_callback(
        _event_loop: &mut rae::AeEventLoop,
        _id: i64,
        client_data: *mut c_void,
    ) -> i32 {
        if !client_data.is_null() {
            unsafe {
                let value = client_data as *mut i32;
                *value += 1;
            }
        }
        AE_NOMORE
    }

    #[test]
    fn test_client_data_passing() {
        let mut event_loop = ae_create_event_loop(64).expect("Failed to create event loop");

        let mut test_value = 42;
        let client_data = &mut test_value as *mut i32 as *mut c_void;

        // Create time event with client data
        let _event_id =
            ae_create_time_event(&mut event_loop, 1, client_data_callback, client_data, None);

        std::thread::sleep(Duration::from_millis(5));

        // Process the event
        let processed = ae_process_events(&mut event_loop, AE_TIME_EVENTS | AE_DONT_WAIT);
        assert_eq!(processed, 1, "Should process time event");

        // Check that client data was modified
        assert_eq!(test_value, 43, "Client data should have been incremented");

        ae_delete_event_loop(event_loop);
    }
}
