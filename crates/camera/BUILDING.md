## Building on Windows
Refer to the [opencv-rust documentation](https://github.com/twistedfall/opencv-rust/blob/master/INSTALL.md), alternatively follow the following guide:

- install vcpkg
    - clone it to any location for tooling (e.g. C:/dev/tools)
        - `git clone https://github.com/microsoft/vcpkg.git`
    - run its bootstrap scripts
        - `cd vcpkg`
        - `.\bootstrap-vcpkg.bat`
- install [cargo-vcpkg](https://github.com/mcgoo/cargo-vcpkg)
    - `cargo install cargo-vcpkg`
- install llvm
    - `winget install llvm`
- install opencv4 via vcpkg (see `Cargo.toml`)
    - `cargo vcpkg build`
- finally, build the crate
    - `cargo build`