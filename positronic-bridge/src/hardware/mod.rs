//! # Hardware Status Display
//!
//! Bridge-side hardware status tracking and display data.
//! Receives events from positronic-io and maintains UI-friendly state
//! for rendering device lists, connection status, and sensor data summaries.

use std::collections::HashMap;

/// Status of a hardware device connection
#[derive(Debug, Clone, PartialEq)]
pub enum DeviceStatus {
    /// Device detected but not connected
    Available,
    /// Actively connected and streaming
    Connected,
    /// Previously connected, now lost
    Disconnected,
    /// Connection attempt failed
    Error(String),
}

/// A hardware device visible to the Bridge UI
#[derive(Debug, Clone)]
pub struct DeviceInfo {
    pub port_name: String,
    pub status: DeviceStatus,
    pub baud_rate: Option<u32>,
    /// Rolling statistics for display
    pub stats: SensorStats,
}

/// Rolling statistics for a sensor data stream
#[derive(Debug, Clone, Default)]
pub struct SensorStats {
    /// Number of samples received
    pub sample_count: u64,
    /// Most recent value
    pub last_value: Option<f32>,
    /// Minimum value seen
    pub min_value: Option<f32>,
    /// Maximum value seen
    pub max_value: Option<f32>,
    /// Running average
    pub avg_value: Option<f64>,
    /// Sum for computing average
    sum: f64,
}

impl SensorStats {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a new sample and update rolling stats.
    pub fn record(&mut self, value: f32) {
        self.sample_count += 1;
        self.last_value = Some(value);
        self.sum += value as f64;
        self.avg_value = Some(self.sum / self.sample_count as f64);

        self.min_value = Some(match self.min_value {
            Some(min) => min.min(value),
            None => value,
        });

        self.max_value = Some(match self.max_value {
            Some(max) => max.max(value),
            None => value,
        });
    }

    /// Reset all statistics.
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

/// Ring buffer for oscilloscope-style display.
/// Stores the most recent N samples for waveform rendering.
#[derive(Debug, Clone)]
pub struct WaveformBuffer {
    /// Circular buffer of (timestamp, value) pairs
    buffer: Vec<(f64, f32)>,
    /// Write position in the ring
    write_pos: usize,
    /// Total capacity
    capacity: usize,
    /// Number of valid samples (up to capacity)
    len: usize,
}

impl WaveformBuffer {
    pub fn new(capacity: usize) -> Self {
        Self {
            buffer: vec![(0.0, 0.0); capacity],
            write_pos: 0,
            capacity,
            len: 0,
        }
    }

    /// Push a new sample into the ring buffer.
    pub fn push(&mut self, timestamp: f64, value: f32) {
        self.buffer[self.write_pos] = (timestamp, value);
        self.write_pos = (self.write_pos + 1) % self.capacity;
        if self.len < self.capacity {
            self.len += 1;
        }
    }

    /// Get all valid samples in chronological order.
    pub fn samples(&self) -> Vec<(f64, f32)> {
        if self.len < self.capacity {
            self.buffer[..self.len].to_vec()
        } else {
            let mut result = Vec::with_capacity(self.capacity);
            result.extend_from_slice(&self.buffer[self.write_pos..]);
            result.extend_from_slice(&self.buffer[..self.write_pos]);
            result
        }
    }

    /// Number of valid samples.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Whether the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Total capacity of the buffer.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Clear all samples.
    pub fn clear(&mut self) {
        self.write_pos = 0;
        self.len = 0;
    }
}

/// The hardware status panel state, maintained by the Bridge.
pub struct HardwarePanel {
    /// Known devices keyed by port name
    pub devices: HashMap<String, DeviceInfo>,
    /// Waveform buffers per port for oscilloscope rendering
    pub waveforms: HashMap<String, WaveformBuffer>,
    /// Default waveform buffer size
    waveform_capacity: usize,
}

impl HardwarePanel {
    pub fn new() -> Self {
        Self {
            devices: HashMap::new(),
            waveforms: HashMap::new(),
            waveform_capacity: 2048,
        }
    }

    /// Register a device as available (discovered via scan).
    pub fn device_discovered(&mut self, port_name: &str) {
        self.devices
            .entry(port_name.to_string())
            .or_insert_with(|| DeviceInfo {
                port_name: port_name.to_string(),
                status: DeviceStatus::Available,
                baud_rate: None,
                stats: SensorStats::new(),
            });
    }

    /// Mark a device as connected.
    pub fn device_connected(&mut self, port_name: &str, baud_rate: u32) {
        let device = self
            .devices
            .entry(port_name.to_string())
            .or_insert_with(|| DeviceInfo {
                port_name: port_name.to_string(),
                status: DeviceStatus::Available,
                baud_rate: None,
                stats: SensorStats::new(),
            });
        device.status = DeviceStatus::Connected;
        device.baud_rate = Some(baud_rate);
        device.stats.reset();

        self.waveforms
            .entry(port_name.to_string())
            .or_insert_with(|| WaveformBuffer::new(self.waveform_capacity));
    }

    /// Mark a device as disconnected.
    pub fn device_disconnected(&mut self, port_name: &str) {
        if let Some(device) = self.devices.get_mut(port_name) {
            device.status = DeviceStatus::Disconnected;
        }
    }

    /// Record a device error.
    pub fn device_error(&mut self, port_name: &str, error: String) {
        if let Some(device) = self.devices.get_mut(port_name) {
            device.status = DeviceStatus::Error(error);
        }
    }

    /// Record a sensor sample for a device.
    pub fn record_sample(&mut self, port_name: &str, timestamp: f64, value: f32) {
        if let Some(device) = self.devices.get_mut(port_name) {
            device.stats.record(value);
        }
        if let Some(waveform) = self.waveforms.get_mut(port_name) {
            waveform.push(timestamp, value);
        }
    }

    /// Get the list of all known devices.
    pub fn device_list(&self) -> Vec<&DeviceInfo> {
        self.devices.values().collect()
    }

    /// Get connected device count.
    pub fn connected_count(&self) -> usize {
        self.devices
            .values()
            .filter(|d| d.status == DeviceStatus::Connected)
            .count()
    }
}

impl Default for HardwarePanel {
    fn default() -> Self {
        Self::new()
    }
}
