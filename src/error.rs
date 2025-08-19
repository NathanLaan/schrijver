use std::fmt;
use std::io;
use thiserror::Error;

#[derive(Error, Debug, Clone)]
pub enum WriterError {
    #[error("ISO file not found: {0}")]
    IsoNotFound(String),

    #[error("USB device not found: {0}")]
    DeviceNotFound(String),

    #[error("Device is currently mounted: {0}")]
    DeviceMounted(String),

    #[error("Permission denied. Root privileges may be required.")]
    PermissionDenied,

    #[error("Insufficient space on device")]
    InsufficientSpace,

    #[error("Write verification failed")]
    VerificationFailed,

    #[error("IO error: {0}")]
    IoError(String),

    #[error("Device is busy or in use")]
    DeviceBusy,

    #[error("Invalid ISO file format")]
    InvalidIsoFormat,

    #[error("Operation was cancelled")]
    Cancelled,

    #[error("Unknown error: {0}")]
    Unknown(String),
}

impl WriterError {
    pub fn is_recoverable(&self) -> bool {
        match self {
            WriterError::DeviceMounted(_) => true,
            WriterError::DeviceBusy => true,
            WriterError::Cancelled => true,
            _ => false,
        }
    }

    pub fn user_friendly_message(&self) -> String {
        match self {
            WriterError::IsoNotFound(path) => {
                format!("The ISO file '{}' could not be found. Please check if the file exists and try again.", path)
            }
            WriterError::DeviceNotFound(device) => {
                format!("The USB device '{}' could not be found. Please ensure the device is connected and try refreshing the device list.", device)
            }
            WriterError::DeviceMounted(device) => {
                format!("The device '{}' is currently mounted. Please unmount all partitions on this device before writing.", device)
            }
            WriterError::PermissionDenied => {
                "Permission denied. You may need to run this application with administrator/root privileges to write to USB devices.".to_string()
            }
            WriterError::InsufficientSpace => {
                "The USB device does not have enough space for this ISO file. Please use a larger USB device.".to_string()
            }
            WriterError::VerificationFailed => {
                "The write operation completed, but verification failed. The data on the USB device may be corrupted. Please try again.".to_string()
            }
            WriterError::DeviceBusy => {
                "The USB device is currently busy. Please wait a moment and try again.".to_string()
            }
            WriterError::InvalidIsoFormat => {
                "The selected file does not appear to be a valid ISO file. Please select a proper ISO image.".to_string()
            }
            WriterError::Cancelled => {
                "The operation was cancelled by the user.".to_string()
            }
            WriterError::IoError(err) => {
                format!("An I/O error occurred: {}. Please check your system and device connections.", err)
            }
            WriterError::Unknown(msg) => {
                format!("An unexpected error occurred: {}", msg)
            }
        }
    }
}

// Convert from io::Error to WriterError for common error cases
impl From<io::Error> for WriterError {
    fn from(error: io::Error) -> Self {
        match error.kind() {
            io::ErrorKind::NotFound => WriterError::DeviceNotFound("Device not found".to_string()),
            io::ErrorKind::PermissionDenied => WriterError::PermissionDenied,
            io::ErrorKind::InvalidData => WriterError::InvalidIsoFormat,
            _ => WriterError::IoError(error.to_string()),
        }
    }
}