use std::fmt;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub struct UsbDevice {
    pub name: String,
    pub device_path: String,
    pub size: u64,
    pub vendor: String,
    pub model: String,
    pub is_removable: bool,
}

impl fmt::Display for UsbDevice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({:.1} GB) - {}",
               self.name,
               self.size as f64 / (1024.0 * 1024.0 * 1024.0),
               self.device_path
        )
    }
}

pub async fn detect_usb_devices() -> Vec<UsbDevice> {
    #[cfg(target_os = "linux")]
    {
        detect_linux_usb_devices().await
    }

    #[cfg(not(target_os = "linux"))]
    {
        // Fallback for non-Linux systems (for development/testing)
        vec![
            UsbDevice {
                name: "Mock USB Drive".to_string(),
                device_path: "/dev/mock".to_string(),
                size: 8 * 1024 * 1024 * 1024, // 8GB
                vendor: "Mock".to_string(),
                model: "Test Drive".to_string(),
                is_removable: true,
            }
        ]
    }
}

#[cfg(target_os = "linux")]
async fn detect_linux_usb_devices() -> Vec<UsbDevice> {
    use std::fs;
    use std::path::Path;

    let mut devices = Vec::new();

    // Read /proc/partitions to find block devices
    if let Ok(partitions) = fs::read_to_string("/proc/partitions") {
        for line in partitions.lines().skip(2) { // Skip header lines
            let fields: Vec<&str> = line.split_whitespace().collect();
            if fields.len() >= 4 {
                let device_name = fields[3];

                // FIXSkip devices that DO end with numbers (partitions)
                if device_name.chars().last().unwrap_or('0').is_ascii_digit() {
                    continue;
                }

                let device_path = format!("/dev/{}", device_name);

                // Check if device is removable
                let removable_path = format!("/sys/block/{}/removable", device_name);
                let is_removable = fs::read_to_string(&removable_path)
                    .map(|content| content.trim() == "1")
                    .unwrap_or(false);

                if is_removable {
                    // Get device size
                    let size_path = format!("/sys/block/{}/size", device_name);
                    let size_sectors = fs::read_to_string(&size_path)
                        .ok()
                        .and_then(|s| s.trim().parse::<u64>().ok())
                        .unwrap_or(0);

                    let size_bytes = size_sectors * 512; // Sector size is typically 512 bytes

                    // Get vendor and model information
                    let (vendor, model) = get_device_info(&device_name).await;

                    let device = UsbDevice {
                        name: format!("{} {}", vendor, model),
                        device_path,
                        size: size_bytes,
                        vendor,
                        model,
                        is_removable: true,
                    };

                    devices.push(device);
                }
            }
        }
    }

    devices
}

#[cfg(target_os = "linux")]
async fn get_device_info(device_name: &str) -> (String, String) {
    use std::fs;

    let vendor_path = format!("/sys/block/{}/device/vendor", device_name);
    let model_path = format!("/sys/block/{}/device/model", device_name);

    let vendor = fs::read_to_string(&vendor_path)
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|_| "Unknown".to_string());

    let model = fs::read_to_string(&model_path)
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|_| "Device".to_string());

    (vendor, model)
}

pub fn is_device_mounted(device_path: &str) -> bool {
    #[cfg(target_os = "linux")]
    {
        use std::fs;

        // Check /proc/mounts
        if let Ok(mounts) = fs::read_to_string("/proc/mounts") {
            for line in mounts.lines() {
                let fields: Vec<&str> = line.split_whitespace().collect();
                if !fields.is_empty() && fields[0].starts_with(device_path) {
                    return true;
                }
            }
        }
        false
    }

    #[cfg(not(target_os = "linux"))]
    false
}

pub async fn validate_device_for_writing(device: &UsbDevice) -> Result<(), crate::error::WriterError> {
    use crate::error::WriterError;

    // Check if device exists
    if !std::path::Path::new(&device.device_path).exists() {
        return Err(WriterError::DeviceNotFound(device.device_path.clone()));
    }

    // Check if device is mounted
    if is_device_mounted(&device.device_path) {
        return Err(WriterError::DeviceMounted(device.device_path.clone()));
    }

    // TODO: Check write permissions

    Ok(())
}