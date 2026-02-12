use positronic_io::{HardwareEvent, HardwareMonitor, SensorSample, SerialConfig};

// ============================================================================
// SensorSample Tests
// ============================================================================

#[test]
fn test_sensor_sample_creation() {
    let sample = SensorSample {
        timestamp: 1.0,
        value: 3.14,
        channel: 0,
    };
    assert_eq!(sample.timestamp, 1.0);
    assert!((sample.value - 3.14).abs() < f32::EPSILON);
    assert_eq!(sample.channel, 0);
}

#[test]
fn test_sensor_sample_clone() {
    let sample = SensorSample {
        timestamp: 2.5,
        value: -1.0,
        channel: 1,
    };
    let cloned = sample;
    assert_eq!(cloned.timestamp, 2.5);
    assert_eq!(cloned.channel, 1);
}

#[test]
fn test_sensor_sample_debug() {
    let sample = SensorSample {
        timestamp: 0.0,
        value: 0.0,
        channel: 0,
    };
    let debug = format!("{:?}", sample);
    assert!(debug.contains("SensorSample"));
}

#[test]
fn test_sensor_sample_multiple_channels() {
    let samples: Vec<SensorSample> = (0..4)
        .map(|ch| SensorSample {
            timestamp: ch as f64,
            value: ch as f32 * 0.5,
            channel: ch,
        })
        .collect();
    assert_eq!(samples.len(), 4);
    assert_eq!(samples[3].channel, 3);
}

// ============================================================================
// HardwareEvent Tests
// ============================================================================

#[test]
fn test_hardware_event_device_connected() {
    let event = HardwareEvent::DeviceConnected("COM3".to_string());
    match event {
        HardwareEvent::DeviceConnected(name) => assert_eq!(name, "COM3"),
        _ => panic!("Wrong variant"),
    }
}

#[test]
fn test_hardware_event_device_disconnected() {
    let event = HardwareEvent::DeviceDisconnected("/dev/ttyUSB0".to_string());
    match event {
        HardwareEvent::DeviceDisconnected(name) => assert_eq!(name, "/dev/ttyUSB0"),
        _ => panic!("Wrong variant"),
    }
}

#[test]
fn test_hardware_event_data_batch() {
    let samples = vec![
        SensorSample {
            timestamp: 0.0,
            value: 1.0,
            channel: 0,
        },
        SensorSample {
            timestamp: 0.001,
            value: 1.5,
            channel: 0,
        },
    ];
    let event = HardwareEvent::DataBatch(samples);
    match event {
        HardwareEvent::DataBatch(batch) => assert_eq!(batch.len(), 2),
        _ => panic!("Wrong variant"),
    }
}

#[test]
fn test_hardware_event_serial_output() {
    let event = HardwareEvent::SerialOutput("Hello from Arduino\n".to_string());
    match event {
        HardwareEvent::SerialOutput(s) => assert!(s.contains("Arduino")),
        _ => panic!("Wrong variant"),
    }
}

#[test]
fn test_hardware_event_error() {
    let event = HardwareEvent::Error("Port busy".to_string());
    match event {
        HardwareEvent::Error(msg) => assert_eq!(msg, "Port busy"),
        _ => panic!("Wrong variant"),
    }
}

#[test]
fn test_hardware_event_clone() {
    let event = HardwareEvent::DeviceConnected("COM1".to_string());
    let cloned = event.clone();
    assert!(matches!(cloned, HardwareEvent::DeviceConnected(_)));
}

// ============================================================================
// SerialConfig Tests
// ============================================================================

#[test]
fn test_serial_config_creation() {
    let config = SerialConfig {
        port_name: "COM3".to_string(),
        baud_rate: 115200,
        data_bits: 8,
        flow_control: false,
    };
    assert_eq!(config.port_name, "COM3");
    assert_eq!(config.baud_rate, 115200);
    assert_eq!(config.data_bits, 8);
    assert!(!config.flow_control);
}

#[test]
fn test_serial_config_clone() {
    let config = SerialConfig {
        port_name: "/dev/ttyACM0".to_string(),
        baud_rate: 9600,
        data_bits: 8,
        flow_control: true,
    };
    let cloned = config.clone();
    assert_eq!(cloned.port_name, "/dev/ttyACM0");
    assert_eq!(cloned.baud_rate, 9600);
    assert!(cloned.flow_control);
}

#[test]
fn test_serial_config_debug() {
    let config = SerialConfig {
        port_name: "COM1".to_string(),
        baud_rate: 57600,
        data_bits: 8,
        flow_control: false,
    };
    let debug = format!("{:?}", config);
    assert!(debug.contains("COM1"));
    assert!(debug.contains("57600"));
}

#[test]
fn test_serial_config_various_baud_rates() {
    let baud_rates = [
        300, 1200, 2400, 4800, 9600, 19200, 38400, 57600, 115200, 230400, 460800, 921600,
    ];
    for baud in baud_rates {
        let config = SerialConfig {
            port_name: "test".to_string(),
            baud_rate: baud,
            data_bits: 8,
            flow_control: false,
        };
        assert_eq!(config.baud_rate, baud);
    }
}

// ============================================================================
// HardwareMonitor Tests
// ============================================================================

#[tokio::test]
async fn test_hardware_monitor_start() {
    let (monitor, _rx) = HardwareMonitor::start();
    let debug = format!("{:?}", monitor);
    assert!(debug.contains("HardwareMonitor"));
}

#[tokio::test]
async fn test_hardware_monitor_scan_ports() {
    let (monitor, _rx) = HardwareMonitor::start();
    // scan_ports sends a command; it may succeed or return an error
    // depending on the system, but should not panic
    let _ = monitor.scan_ports().await;
}

#[tokio::test]
async fn test_hardware_monitor_connect_nonexistent() {
    let (monitor, mut rx) = HardwareMonitor::start();
    // Connecting to a nonexistent port should send an error event
    let _ = monitor
        .connect("/dev/nonexistent_positronic_port", 9600)
        .await;

    // Give the IO thread time to process
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    // Try to receive the error event
    if let Ok(event) = rx.try_recv() {
        match event {
            HardwareEvent::Error(msg) => {
                assert!(msg.contains("nonexistent_positronic_port") || msg.contains("Failed"));
            }
            HardwareEvent::DeviceConnected(_) => {
                // Unlikely but possible on some systems
            }
            _ => {}
        }
    }
}
