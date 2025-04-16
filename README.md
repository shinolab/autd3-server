# AUTD3 Server

# Installation

[Prebuilt binaries are available for Windows, macOS, and Linux.](https://github.com/shinolab/autd3-server/releases/latest)

## For macOS users

This program is not signed, so you need to allow it to run. To do this, follow these steps:
```
cd /Applications
xattr -d com.apple.quarantine AUTD3\ Server.app
```
Or, build from source as described below.

# Building from source

To build from source, you need to install the following dependencies:
- [Rust](https://www.rust-lang.org/)
- [Node.js](https://nodejs.org/)
- [cargo-make](https://github.com/sagiegurari/cargo-make)

To build the project, run the following commands:
```bash
git clone https://github.com/shinolab/autd3-server
cd autd3-server
cargo make build
```

# LICENSE

* See [LICENSE](./LICENSE) and [NOTICE](./NOTICE) for more information.

# Author

Shun Suzuki, 2023-2025
