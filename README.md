# Schrijver ISO Writer

A Linux GUI application written in Rust using the ICED UI framework for writing ISO files to a USB drive.

![Schrijver ISO Writer](/schrijver-main-window.png "Schrijver ISO Writer Main Window UI")

Note: Root is required to write to USB devices.

## Features

- Automatically detect removable USB devices.
- Progress display during ISO writing.

## Development Environment Setup

Tested on Debian and Ubuntu.

```bash
sudo apt update
sudo apt install build-essential pkg-config libfontconfig1-dev libudev-dev
git clone git@github.com:NathanLaan/schrijver.git
cd schrijver
cargo build --release
sudo ./target/release/schrijver
```
