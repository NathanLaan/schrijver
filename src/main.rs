use iced::widget::{button, column, container, row, text, progress_bar, pick_list};
use iced::{Alignment, Application, Command, Element, Length, Settings, Theme};
use rfd::AsyncFileDialog;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;

mod writer;
mod device;
mod error;

use writer::UsbWriter;
use device::{UsbDevice, detect_usb_devices};
use error::WriterError;

pub fn main() -> iced::Result {
    SchrijverApplication::run(Settings {
        window: iced::window::Settings {
            size: iced::Size::new(800.0, 480.0),
            ..Default::default()
        },
        ..Default::default()
    })
}

#[derive(Debug, Clone)]
pub enum Message {
    SelectIsoFile,
    IsoFileSelected(Option<PathBuf>),
    RefreshDevices,
    DevicesDetected(Vec<UsbDevice>),
    DeviceSelected(UsbDevice),
    StartWriting,
    WriteProgress(f32),
    WriteCompleted(Result<(), WriterError>),
}

struct SchrijverApplication {
    iso_path: Option<PathBuf>,
    selected_device: Option<UsbDevice>,
    available_devices: Vec<UsbDevice>,
    writer: Arc<Mutex<Option<UsbWriter>>>,
    write_progress: f32,
    is_writing: bool,
    status_message: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AppState {
    Idle,
    SelectingFile,
    Writing,
    Completed,
    Error(String),
}

impl Default for SchrijverApplication {
    fn default() -> Self {
        Self {
            iso_path: None,
            selected_device: None,
            available_devices: Vec::new(),
            writer: Arc::new(Mutex::new(None)),
            write_progress: 0.0,
            is_writing: false,
            status_message: "Ready to write ISO to USB".to_string(),
        }
    }
}

impl Application for SchrijverApplication {
    type Executor = iced::executor::Default;
    type Message = Message;
    type Theme = Theme;
    type Flags = ();

    fn new(_flags: ()) -> (Self, Command<Message>) {
        let app = Self::default();
        (app, Command::perform(detect_usb_devices(), Message::DevicesDetected))
    }

    fn title(&self) -> String {
        String::from("ISO to USB Writer")
    }

    fn update(&mut self, message: Message) -> Command<Message> {
        match message {
            Message::SelectIsoFile => {
                return Command::perform(select_iso_file(), Message::IsoFileSelected);
            }
            Message::IsoFileSelected(path) => {
                self.iso_path = path;
                if self.iso_path.is_some() {
                    self.status_message = format!("ISO file selected: {}",
                                                  self.iso_path.as_ref().unwrap().display());
                }
            }
            Message::RefreshDevices => {
                return Command::perform(detect_usb_devices(), Message::DevicesDetected);
            }
            Message::DevicesDetected(devices) => {
                self.available_devices = devices;
                self.status_message = format!("Found {} USB devices", self.available_devices.len());
            }
            Message::DeviceSelected(device) => {
                self.selected_device = Some(device.clone());
                self.status_message = format!("Selected device: {}", device.name);
            }
            Message::StartWriting => {
                if let (Some(iso_path), Some(device)) = (&self.iso_path, &self.selected_device) {
                    self.is_writing = true;
                    self.write_progress = 0.0;
                    self.status_message = "Writing ISO to USB device...".to_string();

                    let iso_path = iso_path.clone();
                    let device_path = device.device_path.clone();

                    return Command::perform(
                        write_iso_to_usb(iso_path, device_path),
                        Message::WriteCompleted
                    );
                }
            }
            Message::WriteProgress(progress) => {
                self.write_progress = progress;
            }
            Message::WriteCompleted(result) => {
                self.is_writing = false;
                match result {
                    Ok(()) => {
                        self.status_message = "ISO successfully written to USB device!".to_string();
                        self.write_progress = 1.0;
                    }
                    Err(error) => {
                        self.status_message = format!("Error: {}", error);
                        self.write_progress = 0.0;
                    }
                }
            }
        }
        Command::none()
    }

    fn view(&self) -> Element<Message> {
        let iso_section = column![
            row![
                text("1. Select ISO File").size(16),
                button("Select ISO").on_press(Message::SelectIsoFile),
                text(
                    self.iso_path
                        .as_ref()
                        .map(|p| p.file_name().unwrap_or_default().to_string_lossy().to_string())
                        .unwrap_or_else(|| "No file selected".to_string())
                )
                .size(14)
            ]
            .spacing(10)
            .align_items(Alignment::Center),
        ]
            .spacing(10);

        let device_section = column![
            row![
                text("2. Select USB Device").size(16),
                pick_list(
                    self.available_devices.as_slice(),
                    self.selected_device.clone(),
                    Message::DeviceSelected
                )
                .placeholder("Select USB device..."),
                button("Refresh").on_press(Message::RefreshDevices)
            ]
            .spacing(10)
            .align_items(Alignment::Center),
        ]
            .spacing(10);

        let write_section = row![
            text("3. Write ISO").size(16),
            if self.can_write() {
                button("Write ISO to USB Device")
                    .on_press(Message::StartWriting)
                    .style(iced::theme::Button::Primary)
            } else {
                button("Write ISO to USB Device")
                    .style(iced::theme::Button::Secondary)
            }
        ]
            .spacing(10);

        let progress_section = if self.is_writing || self.write_progress > 0.0 {
            column![
                text("Progress").size(16),
                progress_bar(0.0..=1.0, self.write_progress),
                text(format!("{:.1}%", self.write_progress * 100.0))
            ]
                .spacing(5)
        } else {
            column![]
        };

        let status_section = column![
            text("Status").size(16),
            text(&self.status_message).size(12),
        ]
            .spacing(5);

        let content = column![
            iso_section,
            device_section,
            write_section,
            progress_section,
            status_section,
        ]
            .spacing(20)
            .padding(20);

        container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x()
            .center_y()
            .into()
    }
}

impl SchrijverApplication {
    fn can_write(&self) -> bool {
        self.iso_path.is_some() && self.selected_device.is_some() && !self.is_writing
    }
}

async fn select_iso_file() -> Option<PathBuf> {
    AsyncFileDialog::new()
        .add_filter("ISO Files", &["iso"])
        .set_title("Select ISO File")
        .pick_file()
        .await
        .map(|file| file.path().to_path_buf())
}

async fn write_iso_to_usb(iso_path: PathBuf, device_path: String) -> Result<(), WriterError> {
    //
    // TODO: Write the ISO to the USB device!
    //
    // 1. Validate the device is writable and not mounted
    // 2. Open both the ISO file and device for reading/writing
    // 3. Copy data in chunks while updating progress
    // 4. Verify the write was successful
    //
    use crate::device::validate_device_for_writing;
    use crate::writer::write_iso_to_device;
    use std::path::Path;

    // Validate...
    let device = crate::device::UsbDevice {
        name: "Selected Device".to_string(),
        device_path: device_path.clone(),
        size: 0, // do not need size for validation...?
        vendor: "".to_string(),
        model: "".to_string(),
        is_removable: true,
    };

    validate_device_for_writing(&device).await?;

    // Complete the write operation
    write_iso_to_device(Path::new(&iso_path), &device_path).await?;

    Ok(())
}