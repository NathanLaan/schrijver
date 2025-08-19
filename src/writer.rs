use std::fs::{File, OpenOptions};
use std::io::{self, Read, Write, Seek, SeekFrom};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::task;
use tokio::sync::mpsc;
use crate::error::WriterError;

const BUFFER_SIZE: usize = 1024 * 1024; // 1MB buffer

pub struct UsbWriter {
    iso_path: String,
    device_path: String,
    buffer_size: usize,
}

#[derive(Debug, Clone)]
pub struct WriteProgress {
    pub bytes_written: u64,
    pub total_bytes: u64,
    pub progress_percent: f32,
    pub speed_mbps: f64,
}

impl UsbWriter {
    pub fn new(iso_path: String, device_path: String) -> Self {
        Self {
            iso_path,
            device_path,
            buffer_size: BUFFER_SIZE,
        }
    }

    pub async fn write_iso(&self) -> Result<(), WriterError> {
        let iso_path = self.iso_path.clone();
        let device_path = self.device_path.clone();
        let buffer_size = self.buffer_size;

        task::spawn_blocking(move || {
            Self::write_iso_sync(&iso_path, &device_path, buffer_size)
        })
            .await
            .map_err(|e| WriterError::IoError(io::Error::new(io::ErrorKind::Other, e).to_string()))?
    }

    pub async fn write_iso_with_progress<F>(&self, progress_callback: F) -> Result<(), WriterError>
    where
        F: Fn(WriteProgress) + Send + Sync + 'static,
    {
        let iso_path = self.iso_path.clone();
        let device_path = self.device_path.clone();
        let buffer_size = self.buffer_size;
        let callback = Arc::new(progress_callback);

        task::spawn_blocking(move || {
            Self::write_iso_with_progress_sync(&iso_path, &device_path, buffer_size, callback)
        })
            .await
            .map_err(|e| WriterError::IoError(io::Error::new(io::ErrorKind::Other, e).to_string()))?
    }

    fn write_iso_sync(
        iso_path: &str,
        device_path: &str,
        buffer_size: usize
    ) -> Result<(), WriterError> {
        Self::write_iso_with_progress_sync(
            iso_path,
            device_path,
            buffer_size,
            Arc::new(|_| {}) 
        )
    }

    fn write_iso_with_progress_sync<F>(
        iso_path: &str,
        device_path: &str,
        buffer_size: usize,
        progress_callback: Arc<F>
    ) -> Result<(), WriterError>
    where
        F: Fn(WriteProgress) + Send + Sync,
    {
        // Open ISO file for reading
        let mut iso_file = File::open(iso_path)
            .map_err(|e| {
                eprintln!("Failed to open ISO file: {}", e);
                WriterError::IoError(e.to_string())
            })?;

        // Open device file for writing (requires ROOT!))
        let mut device_file = OpenOptions::new()
            .write(true)
            .create(false)
            .truncate(false)
            .open(device_path)
            .map_err(|e| {
                eprintln!("Failed to open device {}: {}", device_path, e);
                match e.kind() {
                    io::ErrorKind::PermissionDenied => WriterError::PermissionDenied,
                    io::ErrorKind::NotFound => WriterError::DeviceNotFound(device_path.to_string()),
                    _ => WriterError::IoError(e.to_string()),
                }
            })?;

        // Get ISO file size
        let iso_size = iso_file.metadata()
            .map_err(|e| WriterError::IoError(e.to_string()))?
            .len();

        println!("Starting write: {} bytes to {}", iso_size, device_path);

        // Perform the actual writing with progress reporting
        Self::copy_with_progress(iso_file, device_file, buffer_size, iso_size, progress_callback)?;

        println!("Write completed successfully");
        Ok(())
    }

    fn copy_with_progress<R, W, F>(
        mut reader: R,
        mut writer: W,
        buffer_size: usize,
        total_size: u64,
        progress_callback: Arc<F>,
    ) -> Result<(), WriterError>
    where
        R: Read,
        W: Write,
        F: Fn(WriteProgress),
    {
        let mut buffer = vec![0u8; buffer_size];
        let mut bytes_written = 0u64;
        let start_time = std::time::Instant::now();
        let mut last_progress_time = start_time;

        loop {
            let bytes_read = reader.read(&mut buffer)
                .map_err(|e| WriterError::IoError(e.to_string()))?;

            if bytes_read == 0 {
                break; // EOF reached
            }

            // Write data to the device
            writer.write_all(&buffer[..bytes_read])
                .map_err(|e| {
                    eprintln!("Write error: {}", e);
                    WriterError::IoError(e.to_string())
                })?;

            bytes_written += bytes_read as u64;
            let now = std::time::Instant::now();

            // Report progress every 100ms
            if now.duration_since(last_progress_time).as_millis() > 100 {
                let elapsed = now.duration_since(start_time).as_secs_f64();
                let speed_mbps = if elapsed > 0.0 {
                    (bytes_written as f64) / (1024.0 * 1024.0) / elapsed
                } else {
                    0.0
                };

                let progress = WriteProgress {
                    bytes_written,
                    total_bytes: total_size,
                    progress_percent: (bytes_written as f32 / total_size as f32) * 100.0,
                    speed_mbps,
                };

                progress_callback(progress);
                last_progress_time = now;
            }
        }

        // Ensure all data is written to the device
        writer.flush().map_err(|e| WriterError::IoError(e.to_string()))?;

        // Final progress report
        let elapsed = std::time::Instant::now().duration_since(start_time).as_secs_f64();
        let speed_mbps = if elapsed > 0.0 {
            (bytes_written as f64) / (1024.0 * 1024.0) / elapsed
        } else {
            0.0
        };

        progress_callback(WriteProgress {
            bytes_written,
            total_bytes: total_size,
            progress_percent: 100.0,
            speed_mbps,
        });

        println!("Wrote {} bytes in {:.1} seconds ({:.1} MB/s)",
                 bytes_written, elapsed, speed_mbps);

        Ok(())
    }

    pub async fn verify_write(&self) -> Result<bool, WriterError> {
        let iso_path = self.iso_path.clone();
        let device_path = self.device_path.clone();

        task::spawn_blocking(move || {
            Self::verify_write_sync(&iso_path, &device_path)
        })
            .await
            .map_err(|e| WriterError::IoError(io::Error::new(io::ErrorKind::Other, e).to_string()))?
    }

    fn verify_write_sync(iso_path: &str, device_path: &str) -> Result<bool, WriterError> {
        let mut iso_file = File::open(iso_path)
            .map_err(|e| WriterError::IoError(e.to_string()))?;

        let mut device_file = File::open(device_path)
            .map_err(|e| WriterError::IoError(e.to_string()))?;

        let iso_size = iso_file.metadata()
            .map_err(|e| WriterError::IoError(e.to_string()))?
            .len();

        let buffer_size = 64 * 1024; // 64KB for verification
        let mut iso_buffer = vec![0u8; buffer_size];
        let mut device_buffer = vec![0u8; buffer_size];
        let mut bytes_verified = 0u64;

        println!("Verifying write...");

        loop {
            let iso_bytes = iso_file.read(&mut iso_buffer)
                .map_err(|e| WriterError::IoError(e.to_string()))?;
            let device_bytes = device_file.read(&mut device_buffer)
                .map_err(|e| WriterError::IoError(e.to_string()))?;

            if iso_bytes == 0 {
                break; // EOF reached
            }

            if iso_bytes != device_bytes {
                eprintln!("Verification failed: byte count mismatch");
                return Ok(false);
            }

            if iso_buffer[..iso_bytes] != device_buffer[..device_bytes] {
                eprintln!("Verification failed: data mismatch at byte {}", bytes_verified);
                return Ok(false);
            }

            bytes_verified += iso_bytes as u64;
            let progress = bytes_verified as f64 / iso_size as f64;
            print!("\rVerification: {:.1}%", progress * 100.0);
            io::stdout().flush().unwrap();
        }

        println!(); // New line after progress
        println!("Verification successful: {} bytes verified", bytes_verified);
        Ok(true)
    }
}

pub async fn write_iso_to_device(
    iso_path: &Path,
    device_path: &str,
) -> Result<(), WriterError> {
    // Validate that ISO file exists and is readable
    if !iso_path.exists() {
        return Err(WriterError::IsoNotFound(iso_path.to_string_lossy().to_string()));
    }

    // Check if ISO file is actually an ISO (basic check)
    if let Some(extension) = iso_path.extension() {
        if extension.to_string_lossy().to_lowercase() != "iso" {
            eprintln!("Warning: File doesn't have .iso extension");
        }
    }

    // Validate that device exists
    if !Path::new(device_path).exists() {
        return Err(WriterError::DeviceNotFound(device_path.to_string()));
    }

    // Check device size vs ISO size
    let iso_size = std::fs::metadata(iso_path)
        .map_err(|e| WriterError::IoError(e.to_string()))?
        .len();

    // Try to get device size (this is Linux-specific)
    if let Ok(device_size) = get_device_size(device_path) {
        if iso_size > device_size {
            return Err(WriterError::InsufficientSpace);
        }
        println!("Device size: {} bytes, ISO size: {} bytes", device_size, iso_size);
    }

    let writer = UsbWriter::new(
        iso_path.to_string_lossy().to_string(),
        device_path.to_string(),
    );

    // Write the ISO
    writer.write_iso().await?;

    println!("Write completed, starting verification...");

    // Verify the write
    if writer.verify_write().await? {
        println!("Verification successful!");
    } else {
        return Err(WriterError::VerificationFailed);
    }

    Ok(())
}

#[cfg(target_os = "linux")]
fn get_device_size(device_path: &str) -> Result<u64, io::Error> {
    use std::fs::File;
    use std::os::unix::io::AsRawFd;

    // Define the BLKGETSIZE64 ioctl command (not provided by libc)
    const BLKGETSIZE64: libc::c_ulong = 0x80081272;

    let file = File::open(device_path)?;
    let fd = file.as_raw_fd();

    // Use ioctl to get device size
    unsafe {
        let mut size: u64 = 0;
        let result = libc::ioctl(fd, BLKGETSIZE64, &mut size);
        if result == -1 {
            return Err(io::Error::last_os_error());
        }
        Ok(size)
    }
}

#[cfg(not(target_os = "linux"))]
fn get_device_size(_device_path: &str) -> Result<u64, io::Error> {
    // Fallback for non-Linux systems
    Err(io::Error::new(io::ErrorKind::Unsupported, "Device size detection not supported on this platform"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_copy_with_progress() {
        let test_data = b"Hello, World! This is test data for USB writing.";
        let mut reader = Cursor::new(test_data);
        let mut writer = Vec::new();

        let callback = Arc::new(|progress: WriteProgress| {
            println!("Progress: {:.1}%", progress.progress_percent);
        });

        let result = UsbWriter::copy_with_progress(
            &mut reader,
            &mut writer,
            16,
            test_data.len() as u64,
            callback
        );

        assert!(result.is_ok());
        assert_eq!(writer, test_data);
    }
}