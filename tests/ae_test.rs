use rae::AE_READABLE;
use rae::AE_WRITABLE;
use rae::AeFileEvent;
use rae::ae_select::*;

// Helper to create mock events array for testing
fn create_events_array(size: usize) -> Vec<AeFileEvent> {
    vec![AeFileEvent::new(); size]
}

#[test]
fn test_create() {
    assert!(ae_api_create().is_ok());
}

#[test]
fn test_free() {
    let state = ae_api_create().unwrap();
    ae_api_free(state); // Should not panic
}

#[test]
fn test_resize() {
    assert_eq!(ae_api_resize(512), 0);
    assert_eq!(ae_api_resize(1024), -1);
}

#[test]
fn test_add_del_event() {
    let mut state = ae_api_create().unwrap();

    assert_eq!(ae_api_add_event(&mut state, 5, AE_READABLE), 0);
    assert!(state.rfds.isset(5));

    ae_api_del_event(&mut state, 5, AE_READABLE);
    assert!(!state.rfds.isset(5));
}

#[test]
fn test_poll() {
    let mut state = ae_api_create().unwrap();
    let events = create_events_array(11);
    let mut fired = [FiredEvent { fd: 0, mask: 0 }; 64];
    let timeout = Some(std::time::Duration::from_millis(1));
    let numevents = ae_api_poll(&mut state, &events, &mut fired, 10, timeout).unwrap();
    assert_eq!(numevents, 0);
}

#[test]
fn test_api_name() {
    assert_eq!(ae_api_name(), "select");
}

#[test]
fn test_c_api_compatibility() {
    // Test exact same behavior as C version
    let mut state = ae_api_create().unwrap();

    // Should return 0 for valid setsize, -1 for invalid
    assert_eq!(ae_api_resize(512), 0);
    assert_eq!(ae_api_resize(1024), -1);

    // Poll with empty fd_sets should return 0 events (timeout)
    let mut fired = [FiredEvent { fd: 0, mask: 0 }; 64];
    let timeout = Some(std::time::Duration::from_millis(1));
    let events = create_events_array(6);
    let numevents = ae_api_poll(&mut state, &events, &mut fired, 5, timeout).unwrap();
    assert_eq!(numevents, 0);

    // Test add/del operations (API compatibility)
    assert_eq!(ae_api_add_event(&mut state, 3, AE_READABLE), 0);
    assert_eq!(ae_api_add_event(&mut state, 3, AE_WRITABLE), 0);
    ae_api_del_event(&mut state, 3, AE_READABLE | AE_WRITABLE);
}

#[test]
fn test_poll_behavior_matching_c() {
    let mut state = ae_api_create().unwrap();

    // Test empty state - should return 0 events like C select() returning 0
    let mut fired = [FiredEvent { fd: 0, mask: 0 }; 64];
    let timeout = Some(std::time::Duration::from_millis(1));
    let events = create_events_array(11);
    let numevents = ae_api_poll(&mut state, &events, &mut fired, 10, timeout).unwrap();
    assert_eq!(numevents, 0);

    // Test add/del operations
    assert_eq!(ae_api_add_event(&mut state, 5, AE_READABLE), 0);
    assert_eq!(ae_api_add_event(&mut state, 7, AE_WRITABLE), 0);
    assert_eq!(
        ae_api_add_event(&mut state, 10, AE_READABLE | AE_WRITABLE),
        0
    );

    // Remove the events before polling to avoid EBADF
    ae_api_del_event(&mut state, 5, AE_READABLE);
    ae_api_del_event(&mut state, 7, AE_WRITABLE);
    ae_api_del_event(&mut state, 10, AE_READABLE | AE_WRITABLE);

    // Poll with empty fd_sets should return 0 events (timeout)
    let mut fired = [FiredEvent { fd: 0, mask: 0 }; 64];
    let timeout = Some(std::time::Duration::from_millis(1));
    let events = create_events_array(11);
    let numevents = ae_api_poll(&mut state, &events, &mut fired, 10, timeout).unwrap();
    assert_eq!(numevents, 0);
}

#[test]
fn test_poll_edge_cases() {
    let mut state = ae_api_create().unwrap();

    // Test negative maxfd
    let mut fired = [FiredEvent { fd: 0, mask: 0 }; 64];
    let timeout = Some(std::time::Duration::from_millis(1));
    let events = create_events_array(0);
    let numevents = ae_api_poll(&mut state, &events, &mut fired, -1, timeout).unwrap();
    assert_eq!(numevents, 0);

    // Test zero maxfd
    let events = create_events_array(1);
    let numevents = ae_api_poll(&mut state, &events, &mut fired, 0, timeout).unwrap();
    assert_eq!(numevents, 0);

    // Test large maxfd
    let events = create_events_array(1001);
    let numevents = ae_api_poll(&mut state, &events, &mut fired, 1000, timeout).unwrap();
    assert_eq!(numevents, 0);
}

#[test]
fn test_event_mask_combinations() {
    let mut state = ae_api_create().unwrap();

    // Test different mask combinations
    assert_eq!(ae_api_add_event(&mut state, 1, AE_READABLE), 0);
    assert_eq!(ae_api_add_event(&mut state, 2, AE_WRITABLE), 0);
    assert_eq!(
        ae_api_add_event(&mut state, 3, AE_READABLE | AE_WRITABLE),
        0
    );

    // Verify fd_set contains the fds with correct masks
    assert!(state.rfds.isset(1));
    assert!(!state.wfds.isset(1));
    assert!(!state.rfds.isset(2));
    assert!(state.wfds.isset(2));
    assert!(state.rfds.isset(3));
    assert!(state.wfds.isset(3));

    // Remove partial mask
    ae_api_del_event(&mut state, 3, AE_READABLE);
    assert!(!state.rfds.isset(3));
    assert!(state.wfds.isset(3));

    // Remove remaining mask
    ae_api_del_event(&mut state, 3, AE_WRITABLE);
    assert!(!state.rfds.isset(3));
    assert!(!state.wfds.isset(3));
}
