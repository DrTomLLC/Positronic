use positronic_bridge::biolink::{
    AccessibilityConfig, BioLink, BioLinkEvent,
};
use positronic_bridge::hardware::{
    DeviceStatus, HardwarePanel, SensorStats, WaveformBuffer,
};
use positronic_bridge::input::InputEditor;

// ============================================================================
// BioLink - AccessibilityConfig Tests
// ============================================================================

#[test]
fn test_accessibility_config_default() {
    let config = AccessibilityConfig::default();
    assert!(!config.screen_reader_enabled);
    assert!(!config.tts_enabled);
    assert!(!config.high_contrast);
    assert!(!config.dyslexia_font);
    assert!((config.font_scale - 1.0).abs() < f32::EPSILON);
}

#[test]
fn test_accessibility_config_clone() {
    let config = AccessibilityConfig {
        screen_reader_enabled: true,
        tts_enabled: true,
        high_contrast: false,
        dyslexia_font: true,
        font_scale: 1.5,
    };
    let cloned = config.clone();
    assert!(cloned.screen_reader_enabled);
    assert!(cloned.dyslexia_font);
    assert!((cloned.font_scale - 1.5).abs() < f32::EPSILON);
}

// ============================================================================
// BioLink - BioLinkEvent Tests
// ============================================================================

#[test]
fn test_biolink_event_command_complete_success() {
    let event = BioLinkEvent::CommandComplete {
        command: "cargo build".to_string(),
        exit_code: 0,
    };
    let text = event.to_screen_reader_text();
    assert!(text.contains("succeeded"));
    assert!(text.contains("cargo build"));
}

#[test]
fn test_biolink_event_command_complete_failure() {
    let event = BioLinkEvent::CommandComplete {
        command: "make".to_string(),
        exit_code: 2,
    };
    let text = event.to_screen_reader_text();
    assert!(text.contains("failed"));
    assert!(text.contains("code 2"));
}

#[test]
fn test_biolink_event_job_finished() {
    let event = BioLinkEvent::JobFinished {
        description: "Compilation".to_string(),
        success: true,
    };
    let text = event.to_screen_reader_text();
    assert!(text.contains("complete"));
    assert!(text.contains("Compilation"));
}

#[test]
fn test_biolink_event_error() {
    let event = BioLinkEvent::ErrorOccurred("segfault".to_string());
    let text = event.to_screen_reader_text();
    assert!(text.contains("Error"));
    assert!(text.contains("segfault"));
}

#[test]
fn test_biolink_event_priority_ordering() {
    let error = BioLinkEvent::ErrorOccurred("x".to_string());
    let cmd = BioLinkEvent::CommandComplete {
        command: "x".to_string(),
        exit_code: 0,
    };
    let announce = BioLinkEvent::Announcement("x".to_string());
    assert!(error.priority() < cmd.priority());
    assert!(cmd.priority() < announce.priority());
}

#[test]
fn test_biolink_event_clone_eq() {
    let event = BioLinkEvent::Announcement("test".to_string());
    let cloned = event.clone();
    assert_eq!(event, cloned);
}

// ============================================================================
// BioLink - Controller Tests
// ============================================================================

#[test]
fn test_biolink_new() {
    let biolink = BioLink::new();
    assert!(!biolink.config.screen_reader_enabled);
    assert_eq!(biolink.pending_count(), 0);
}

#[test]
fn test_biolink_default() {
    let biolink = BioLink::default();
    assert_eq!(biolink.pending_count(), 0);
}

#[test]
fn test_biolink_announce_disabled() {
    let mut biolink = BioLink::new();
    // Both screen_reader and tts are disabled by default
    biolink.announce(BioLinkEvent::Announcement("test".to_string()));
    assert_eq!(biolink.pending_count(), 0);
}

#[test]
fn test_biolink_announce_enabled() {
    let config = AccessibilityConfig {
        screen_reader_enabled: true,
        ..Default::default()
    };
    let mut biolink = BioLink::with_config(config);
    biolink.announce(BioLinkEvent::Announcement("hello".to_string()));
    assert_eq!(biolink.pending_count(), 1);
}

#[test]
fn test_biolink_next_announcement() {
    let config = AccessibilityConfig {
        tts_enabled: true,
        ..Default::default()
    };
    let mut biolink = BioLink::with_config(config);
    biolink.announce(BioLinkEvent::Announcement("first".to_string()));
    biolink.announce(BioLinkEvent::Announcement("second".to_string()));

    let first = biolink.next_announcement().unwrap();
    assert!(first.contains("first"));
    let second = biolink.next_announcement().unwrap();
    assert!(second.contains("second"));
    assert!(biolink.next_announcement().is_none());
}

#[test]
fn test_biolink_drain_announcements() {
    let config = AccessibilityConfig {
        screen_reader_enabled: true,
        ..Default::default()
    };
    let mut biolink = BioLink::with_config(config);
    biolink.announce(BioLinkEvent::Announcement("a".to_string()));
    biolink.announce(BioLinkEvent::Announcement("b".to_string()));
    biolink.announce(BioLinkEvent::Announcement("c".to_string()));

    let all = biolink.drain_announcements();
    assert_eq!(all.len(), 3);
    assert_eq!(biolink.pending_count(), 0);
}

#[test]
fn test_biolink_block_label_success() {
    let label = BioLink::block_label("ls -la", "file.txt\n", Some(0));
    assert!(label.contains("succeeded"));
    assert!(label.contains("ls -la"));
}

#[test]
fn test_biolink_block_label_failure() {
    let label = BioLink::block_label("make", "Error: missing target", Some(2));
    assert!(label.contains("failed"));
    assert!(label.contains("exit code 2"));
}

#[test]
fn test_biolink_block_label_running() {
    let label = BioLink::block_label("sleep 60", "", None);
    assert!(label.contains("running"));
}

#[test]
fn test_biolink_describe_input_empty() {
    let desc = BioLink::describe_input("", 0);
    assert_eq!(desc, "Input empty");
}

#[test]
fn test_biolink_describe_input_with_content() {
    let desc = BioLink::describe_input("cargo build", 5);
    assert!(desc.contains("cargo build"));
    assert!(desc.contains("position 5"));
}

// ============================================================================
// Hardware - SensorStats Tests
// ============================================================================

#[test]
fn test_sensor_stats_new() {
    let stats = SensorStats::new();
    assert_eq!(stats.sample_count, 0);
    assert!(stats.last_value.is_none());
    assert!(stats.min_value.is_none());
    assert!(stats.max_value.is_none());
    assert!(stats.avg_value.is_none());
}

#[test]
fn test_sensor_stats_record_single() {
    let mut stats = SensorStats::new();
    stats.record(5.0);
    assert_eq!(stats.sample_count, 1);
    assert_eq!(stats.last_value, Some(5.0));
    assert_eq!(stats.min_value, Some(5.0));
    assert_eq!(stats.max_value, Some(5.0));
    assert!((stats.avg_value.unwrap() - 5.0).abs() < f64::EPSILON);
}

#[test]
fn test_sensor_stats_record_multiple() {
    let mut stats = SensorStats::new();
    stats.record(2.0);
    stats.record(4.0);
    stats.record(6.0);
    assert_eq!(stats.sample_count, 3);
    assert_eq!(stats.last_value, Some(6.0));
    assert_eq!(stats.min_value, Some(2.0));
    assert_eq!(stats.max_value, Some(6.0));
    assert!((stats.avg_value.unwrap() - 4.0).abs() < f64::EPSILON);
}

#[test]
fn test_sensor_stats_reset() {
    let mut stats = SensorStats::new();
    stats.record(100.0);
    stats.reset();
    assert_eq!(stats.sample_count, 0);
    assert!(stats.last_value.is_none());
}

#[test]
fn test_sensor_stats_negative_values() {
    let mut stats = SensorStats::new();
    stats.record(-10.0);
    stats.record(-5.0);
    stats.record(0.0);
    assert_eq!(stats.min_value, Some(-10.0));
    assert_eq!(stats.max_value, Some(0.0));
}

// ============================================================================
// Hardware - WaveformBuffer Tests
// ============================================================================

#[test]
fn test_waveform_buffer_new() {
    let buf = WaveformBuffer::new(100);
    assert_eq!(buf.capacity(), 100);
    assert_eq!(buf.len(), 0);
    assert!(buf.is_empty());
}

#[test]
fn test_waveform_buffer_push() {
    let mut buf = WaveformBuffer::new(5);
    buf.push(0.0, 1.0);
    buf.push(0.1, 2.0);
    assert_eq!(buf.len(), 2);
    assert!(!buf.is_empty());
}

#[test]
fn test_waveform_buffer_samples_order() {
    let mut buf = WaveformBuffer::new(10);
    buf.push(0.0, 1.0);
    buf.push(1.0, 2.0);
    buf.push(2.0, 3.0);

    let samples = buf.samples();
    assert_eq!(samples.len(), 3);
    assert!((samples[0].1 - 1.0).abs() < f32::EPSILON);
    assert!((samples[1].1 - 2.0).abs() < f32::EPSILON);
    assert!((samples[2].1 - 3.0).abs() < f32::EPSILON);
}

#[test]
fn test_waveform_buffer_wraparound() {
    let mut buf = WaveformBuffer::new(3);
    buf.push(0.0, 1.0);
    buf.push(1.0, 2.0);
    buf.push(2.0, 3.0);
    buf.push(3.0, 4.0); // Overwrites first

    assert_eq!(buf.len(), 3);
    let samples = buf.samples();
    // Should be [2.0, 3.0, 4.0] in chronological order
    assert!((samples[0].1 - 2.0).abs() < f32::EPSILON);
    assert!((samples[1].1 - 3.0).abs() < f32::EPSILON);
    assert!((samples[2].1 - 4.0).abs() < f32::EPSILON);
}

#[test]
fn test_waveform_buffer_clear() {
    let mut buf = WaveformBuffer::new(5);
    buf.push(0.0, 1.0);
    buf.push(1.0, 2.0);
    buf.clear();
    assert_eq!(buf.len(), 0);
    assert!(buf.is_empty());
}

#[test]
fn test_waveform_buffer_full_cycle() {
    let mut buf = WaveformBuffer::new(4);
    for i in 0..10 {
        buf.push(i as f64, i as f32);
    }
    assert_eq!(buf.len(), 4);
    let samples = buf.samples();
    // Last 4: 6, 7, 8, 9
    assert!((samples[0].1 - 6.0).abs() < f32::EPSILON);
    assert!((samples[3].1 - 9.0).abs() < f32::EPSILON);
}

// ============================================================================
// Hardware - HardwarePanel Tests
// ============================================================================

#[test]
fn test_hardware_panel_new() {
    let panel = HardwarePanel::new();
    assert!(panel.devices.is_empty());
    assert_eq!(panel.connected_count(), 0);
}

#[test]
fn test_hardware_panel_default() {
    let panel = HardwarePanel::default();
    assert!(panel.devices.is_empty());
}

#[test]
fn test_hardware_panel_device_discovered() {
    let mut panel = HardwarePanel::new();
    panel.device_discovered("COM3");
    assert_eq!(panel.devices.len(), 1);
    assert_eq!(panel.devices["COM3"].status, DeviceStatus::Available);
}

#[test]
fn test_hardware_panel_device_discovered_idempotent() {
    let mut panel = HardwarePanel::new();
    panel.device_discovered("COM3");
    panel.device_discovered("COM3");
    assert_eq!(panel.devices.len(), 1);
}

#[test]
fn test_hardware_panel_device_connected() {
    let mut panel = HardwarePanel::new();
    panel.device_connected("COM3", 115200);
    assert_eq!(panel.devices["COM3"].status, DeviceStatus::Connected);
    assert_eq!(panel.devices["COM3"].baud_rate, Some(115200));
    assert_eq!(panel.connected_count(), 1);
}

#[test]
fn test_hardware_panel_device_disconnected() {
    let mut panel = HardwarePanel::new();
    panel.device_connected("COM3", 9600);
    panel.device_disconnected("COM3");
    assert_eq!(panel.devices["COM3"].status, DeviceStatus::Disconnected);
    assert_eq!(panel.connected_count(), 0);
}

#[test]
fn test_hardware_panel_device_error() {
    let mut panel = HardwarePanel::new();
    panel.device_connected("COM3", 9600);
    panel.device_error("COM3", "Port busy".to_string());
    match &panel.devices["COM3"].status {
        DeviceStatus::Error(msg) => assert_eq!(msg, "Port busy"),
        _ => panic!("Expected Error status"),
    }
}

#[test]
fn test_hardware_panel_record_sample() {
    let mut panel = HardwarePanel::new();
    panel.device_connected("COM3", 9600);
    panel.record_sample("COM3", 0.0, 3.14);
    panel.record_sample("COM3", 0.1, 2.71);

    let stats = &panel.devices["COM3"].stats;
    assert_eq!(stats.sample_count, 2);
    assert_eq!(stats.last_value, Some(2.71));

    let waveform = &panel.waveforms["COM3"];
    assert_eq!(waveform.len(), 2);
}

#[test]
fn test_hardware_panel_device_list() {
    let mut panel = HardwarePanel::new();
    panel.device_discovered("COM1");
    panel.device_discovered("COM2");
    panel.device_discovered("COM3");
    assert_eq!(panel.device_list().len(), 3);
}

#[test]
fn test_hardware_panel_multiple_connections() {
    let mut panel = HardwarePanel::new();
    panel.device_connected("COM1", 9600);
    panel.device_connected("COM2", 115200);
    panel.device_discovered("COM3");
    assert_eq!(panel.connected_count(), 2);
}

// ============================================================================
// InputEditor Tests
// ============================================================================

#[test]
fn test_input_editor_new() {
    let editor = InputEditor::new();
    assert_eq!(editor.value, "");
}

#[test]
fn test_input_editor_value_mutation() {
    let mut editor = InputEditor::new();
    editor.value = "cargo build".to_string();
    assert_eq!(editor.value, "cargo build");
}

#[test]
fn test_input_editor_clear() {
    let mut editor = InputEditor::new();
    editor.value = "some command".to_string();
    editor.value.clear();
    assert!(editor.value.is_empty());
}

#[test]
fn test_input_editor_debug() {
    let editor = InputEditor::new();
    let debug = format!("{:?}", editor);
    assert!(debug.contains("InputEditor"));
}
