/* Integration Tests
 *
 * Tests for complete event loop integration including combinations of
 * file events, time events, and full event loop workflows.
 */

use rae::{
    AE_ALL_EVENTS, AE_DONT_WAIT, AE_FILE_EVENTS, AE_NOMORE, AE_READABLE, AE_TIME_EVENTS,
    AE_WRITABLE, ae_create_event_loop, ae_create_file_event, ae_create_time_event,
    ae_delete_event_loop, ae_delete_file_event, ae_delete_time_event, ae_process_events,
    ae_set_after_sleep_proc, ae_set_before_sleep_proc, ae_stop,
};
use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::time::Duration;

// Global counters for integration tests
static FILE_EVENT_COUNTER: AtomicI32 = AtomicI32::new(0);
static TIME_EVENT_COUNTER: AtomicI32 = AtomicI32::new(0);
static BEFORE_SLEEP_COUNTER: AtomicI32 = AtomicI32::new(0);
static AFTER_SLEEP_COUNTER: AtomicI32 = AtomicI32::new(0);
static SHOULD_STOP: AtomicBool = AtomicBool::new(false);

// Test callbacks
fn integration_file_callback(
    _event_loop: &mut rae::AeEventLoop,
    _fd: i32,
    _client_data: *mut c_void,
    _mask: i32,
) {
    FILE_EVENT_COUNTER.fetch_add(1, Ordering::SeqCst);

    // Stop after processing some events
    if FILE_EVENT_COUNTER.load(Ordering::SeqCst) >= 3 {
        SHOULD_STOP.store(true, Ordering::SeqCst);
    }
}

fn integration_time_callback(
    event_loop: &mut rae::AeEventLoop,
    _id: i64,
    _client_data: *mut c_void,
) -> i32 {
    TIME_EVENT_COUNTER.fetch_add(1, Ordering::SeqCst);

    // Stop the event loop after a few time events
    if TIME_EVENT_COUNTER.load(Ordering::SeqCst) >= 2 {
        ae_stop(event_loop);
    }
    AE_NOMORE
}

fn before_sleep_callback(event_loop: &mut rae::AeEventLoop) {
    BEFORE_SLEEP_COUNTER.fetch_add(1, Ordering::SeqCst);

    // Stop if we've seen too many cycles
    if BEFORE_SLEEP_COUNTER.load(Ordering::SeqCst) >= 5 {
        ae_stop(event_loop);
    }
}

fn after_sleep_callback(_event_loop: &mut rae::AeEventLoop) {
    AFTER_SLEEP_COUNTER.fetch_add(1, Ordering::SeqCst);
}

fn stopping_time_callback(
    event_loop: &mut rae::AeEventLoop,
    _id: i64,
    _client_data: *mut c_void,
) -> i32 {
    // Stop the event loop when this time event fires
    ae_stop(event_loop);
    AE_NOMORE
}

mod combined_events {
    use super::*;

    #[test]
    fn test_file_and_time_events() {
        let mut event_loop = ae_create_event_loop(64).expect("Failed to create event loop");

        FILE_EVENT_COUNTER.store(0, Ordering::SeqCst);
        TIME_EVENT_COUNTER.store(0, Ordering::SeqCst);

        // Create a time event for near-immediate execution
        let _time_id = ae_create_time_event(
            &mut event_loop,
            1, // 1ms
            integration_time_callback,
            std::ptr::null_mut(),
            None,
        );

        // Create a file event (won't actually fire without real fd)
        let _result = ae_create_file_event(
            &mut event_loop,
            5,
            AE_READABLE,
            integration_file_callback,
            std::ptr::null_mut(),
        );

        // Process events multiple times to see time events execute
        std::thread::sleep(Duration::from_millis(5));

        let processed = ae_process_events(&mut event_loop, AE_ALL_EVENTS | AE_DONT_WAIT);
        assert!(processed >= 1, "Should process at least time event");

        let time_count = TIME_EVENT_COUNTER.load(Ordering::SeqCst);
        assert!(time_count >= 1, "Time event should have executed");

        ae_delete_event_loop(event_loop);
    }

    #[test]
    fn test_multiple_time_events_with_files() {
        let mut event_loop = ae_create_event_loop(64).expect("Failed to create event loop");

        TIME_EVENT_COUNTER.store(0, Ordering::SeqCst);

        // Create multiple time events at different intervals
        for i in 1..=3 {
            let _time_id = ae_create_time_event(
                &mut event_loop,
                i, // Different delays
                integration_time_callback,
                std::ptr::null_mut(),
                None,
            );
        }

        // Create file events (won't fire without real fds)
        for fd in 10..=12 {
            let _result = ae_create_file_event(
                &mut event_loop,
                fd,
                AE_READABLE | AE_WRITABLE,
                integration_file_callback,
                std::ptr::null_mut(),
            );
        }

        std::thread::sleep(Duration::from_millis(10));

        // Process all events
        let processed = ae_process_events(&mut event_loop, AE_ALL_EVENTS | AE_DONT_WAIT);
        assert!(processed >= 3, "Should process multiple time events");

        ae_delete_event_loop(event_loop);
    }

    #[test]
    fn test_event_deletion_during_processing() {
        let mut event_loop = ae_create_event_loop(64).expect("Failed to create event loop");

        // Create time event
        let time_id = ae_create_time_event(
            &mut event_loop,
            1000, // 1 second (future)
            integration_time_callback,
            std::ptr::null_mut(),
            None,
        );

        // Create file event
        let _result = ae_create_file_event(
            &mut event_loop,
            20,
            AE_READABLE,
            integration_file_callback,
            std::ptr::null_mut(),
        );

        // Delete the time event before processing
        let result = ae_delete_time_event(&mut event_loop, time_id);
        assert_eq!(result, 0, "Should successfully delete time event");

        // Delete the file event
        ae_delete_file_event(&mut event_loop, 20, AE_READABLE);

        // Process events - should be nothing to process
        let processed = ae_process_events(&mut event_loop, AE_ALL_EVENTS | AE_DONT_WAIT);
        assert_eq!(processed, 0, "Should process no events after deletion");

        ae_delete_event_loop(event_loop);
    }
}

mod sleep_callbacks {
    use super::*;

    #[test]
    fn test_before_after_sleep_callbacks() {
        let mut event_loop = ae_create_event_loop(64).expect("Failed to create event loop");

        unsafe {
            BEFORE_SLEEP_COUNTER.store(0, Ordering::SeqCst);
            AFTER_SLEEP_COUNTER.store(0, Ordering::SeqCst);
        }

        // Set sleep callbacks
        ae_set_before_sleep_proc(&mut event_loop, Some(before_sleep_callback));
        ae_set_after_sleep_proc(&mut event_loop, Some(after_sleep_callback));

        // Create a time event to trigger processing
        let _time_id = ae_create_time_event(
            &mut event_loop,
            1,
            integration_time_callback,
            std::ptr::null_mut(),
            None,
        );

        std::thread::sleep(Duration::from_millis(5));

        // Process events
        let _processed = ae_process_events(&mut event_loop, AE_ALL_EVENTS | AE_DONT_WAIT);

        // Note: Sleep callbacks might not fire in DONT_WAIT mode
        // This test mainly verifies that setting them doesn't crash

        ae_delete_event_loop(event_loop);
    }

    #[test]
    fn test_clear_sleep_callbacks() {
        let mut event_loop = ae_create_event_loop(64).expect("Failed to create event loop");

        // Set callbacks
        ae_set_before_sleep_proc(&mut event_loop, Some(before_sleep_callback));
        ae_set_after_sleep_proc(&mut event_loop, Some(after_sleep_callback));

        // Clear callbacks
        ae_set_before_sleep_proc(&mut event_loop, None);
        ae_set_after_sleep_proc(&mut event_loop, None);

        // Should not crash
        let _processed = ae_process_events(&mut event_loop, AE_DONT_WAIT);

        ae_delete_event_loop(event_loop);
    }
}

mod event_loop_control {
    use super::*;

    #[test]
    fn test_stop_event_loop_with_time_event() {
        let mut event_loop = ae_create_event_loop(64).expect("Failed to create event loop");

        // Create a time event that will stop the loop
        let _time_id = ae_create_time_event(
            &mut event_loop,
            1, // 1ms
            stopping_time_callback,
            std::ptr::null_mut(),
            None,
        );

        std::thread::sleep(Duration::from_millis(5));

        // Process events - the time event should stop the loop
        let processed = ae_process_events(&mut event_loop, AE_ALL_EVENTS | AE_DONT_WAIT);
        assert_eq!(processed, 1, "Should process the stopping time event");

        // Event loop should now be stopped
        let processed = ae_process_events(&mut event_loop, AE_DONT_WAIT);
        assert_eq!(processed, 0, "Stopped event loop should process no events");

        ae_delete_event_loop(event_loop);
    }

    #[test]
    fn test_manual_stop() {
        let mut event_loop = ae_create_event_loop(64).expect("Failed to create event loop");

        // Stop the event loop manually
        ae_stop(&mut event_loop);

        // Should process no events
        let processed = ae_process_events(&mut event_loop, AE_DONT_WAIT);
        assert_eq!(
            processed, 0,
            "Manually stopped event loop should process no events"
        );

        ae_delete_event_loop(event_loop);
    }
}

mod processing_flags {
    use super::*;

    #[test]
    fn test_file_events_only() {
        let mut event_loop = ae_create_event_loop(64).expect("Failed to create event loop");

        // Create both file and time events
        let _result = ae_create_file_event(
            &mut event_loop,
            30,
            AE_READABLE,
            integration_file_callback,
            std::ptr::null_mut(),
        );

        let _time_id = ae_create_time_event(
            &mut event_loop,
            1,
            integration_time_callback,
            std::ptr::null_mut(),
            None,
        );

        std::thread::sleep(Duration::from_millis(5));

        // Process only file events
        let processed = ae_process_events(&mut event_loop, AE_FILE_EVENTS | AE_DONT_WAIT);
        // Should not process time events
        assert_eq!(
            processed, 0,
            "Should not process time events with FILE_EVENTS flag"
        );

        ae_delete_event_loop(event_loop);
    }

    #[test]
    fn test_time_events_only() {
        let mut event_loop = ae_create_event_loop(64).expect("Failed to create event loop");

        TIME_EVENT_COUNTER.store(0, Ordering::SeqCst);

        // Create both file and time events
        let _result = ae_create_file_event(
            &mut event_loop,
            31,
            AE_READABLE,
            integration_file_callback,
            std::ptr::null_mut(),
        );

        let _time_id = ae_create_time_event(
            &mut event_loop,
            1,
            integration_time_callback,
            std::ptr::null_mut(),
            None,
        );

        std::thread::sleep(Duration::from_millis(5));

        // Process only time events
        let processed = ae_process_events(&mut event_loop, AE_TIME_EVENTS | AE_DONT_WAIT);
        assert!(processed >= 1, "Should process time events");

        let time_count = TIME_EVENT_COUNTER.load(Ordering::SeqCst);
        assert!(time_count >= 1, "Time event callback should have executed");

        ae_delete_event_loop(event_loop);
    }

    #[test]
    fn test_all_events_flag() {
        let mut event_loop = ae_create_event_loop(64).expect("Failed to create event loop");

        TIME_EVENT_COUNTER.store(0, Ordering::SeqCst);

        // Create time event
        let _time_id = ae_create_time_event(
            &mut event_loop,
            1,
            integration_time_callback,
            std::ptr::null_mut(),
            None,
        );

        std::thread::sleep(Duration::from_millis(5));

        // Process all events
        let processed = ae_process_events(&mut event_loop, AE_ALL_EVENTS | AE_DONT_WAIT);
        assert!(processed >= 1, "Should process available events");

        ae_delete_event_loop(event_loop);
    }

    #[test]
    fn test_no_flags() {
        let mut event_loop = ae_create_event_loop(64).expect("Failed to create event loop");

        // Process with no flags - should return 0
        let processed = ae_process_events(&mut event_loop, AE_DONT_WAIT);
        assert_eq!(processed, 0, "Processing with no flags should return 0");

        ae_delete_event_loop(event_loop);
    }
}

mod stress_tests {
    use super::*;

    #[test]
    fn test_many_time_events() {
        let mut event_loop = ae_create_event_loop(64).expect("Failed to create event loop");

        TIME_EVENT_COUNTER.store(0, Ordering::SeqCst);

        // Create many time events
        for i in 1..=10 {
            let _time_id = ae_create_time_event(
                &mut event_loop,
                i as i64,
                integration_time_callback,
                std::ptr::null_mut(),
                None,
            );
        }

        std::thread::sleep(Duration::from_millis(15));

        // Process all time events
        let processed = ae_process_events(&mut event_loop, AE_TIME_EVENTS | AE_DONT_WAIT);
        assert!(processed >= 5, "Should process multiple time events");

        ae_delete_event_loop(event_loop);
    }

    #[test]
    fn test_many_file_events() {
        let mut event_loop = ae_create_event_loop(128).expect("Failed to create event loop");

        // Create many file events
        for fd in 50..70 {
            let _result = ae_create_file_event(
                &mut event_loop,
                fd,
                AE_READABLE | AE_WRITABLE,
                integration_file_callback,
                std::ptr::null_mut(),
            );
        }

        // Process file events (won't actually fire without real fds)
        let processed = ae_process_events(&mut event_loop, AE_FILE_EVENTS | AE_DONT_WAIT);
        assert_eq!(
            processed, 0,
            "File events won't fire without real file descriptors"
        );

        ae_delete_event_loop(event_loop);
    }

    #[test]
    fn test_mixed_events_and_deletion() {
        let mut event_loop = ae_create_event_loop(64).expect("Failed to create event loop");

        // Create mixed events
        let mut time_ids = Vec::new();
        for i in 1..=5 {
            let time_id = ae_create_time_event(
                &mut event_loop,
                i * 10, // Different delays
                integration_time_callback,
                std::ptr::null_mut(),
                None,
            );
            time_ids.push(time_id);
        }

        for fd in 100..105 {
            let _result = ae_create_file_event(
                &mut event_loop,
                fd,
                AE_READABLE,
                integration_file_callback,
                std::ptr::null_mut(),
            );
        }

        // Delete some events
        for &time_id in &time_ids[0..2] {
            ae_delete_time_event(&mut event_loop, time_id);
        }

        for fd in 100..102 {
            ae_delete_file_event(&mut event_loop, fd, AE_READABLE);
        }

        std::thread::sleep(Duration::from_millis(60));

        // Process remaining events
        let processed = ae_process_events(&mut event_loop, AE_ALL_EVENTS | AE_DONT_WAIT);
        // Should process the remaining time events
        assert!(processed >= 1, "Should process remaining events");

        ae_delete_event_loop(event_loop);
    }
}
