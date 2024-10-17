# Supernote Exporter Tool (GUI)

This project is a Rust-based GUI application designed to work with [Supernote](https://supernote.com) A5X files. It provides a streamlined way to render and export Supernote files, adding features such as a Table of Contents (ToC) and link support. Additionally, it converts bitmap traces into vector graphics for smoother rendering and exporting. The project builds upon the foundational work of [jya-dev's supernote-tool](https://github.com/jya-dev/supernote-tool) while delivering a notable speed improvement (over `10x`).

## Features

- **Graphical User Interface (GUI)**: A user-friendly interface for easy file handling and export configuration.
- **Supernote A5X File Parsing**: Efficiently decode and parse Supernote files (of the A5X's dimensions).
- **Bitmap to Vector Conversion**: Converts bitmap traces to vector graphics using the Potrace library.
- **Table of Contents (ToC) Generation**: Generate and add a ToC for easy navigation. Currently typed by hand, OCR planned.
- **Link Support**: Add support for clickable links within the exported files.
- **File Merge**: Merge multiple files into a single PDF with working inter-file links.
- **Color Remap**: Change from grayscale to colorfull images. Right now it only has default remaping:
  - Light Gray goes to yellow (`#fdfa75`)
  - Dark Gray goes to blue (`#4669d6`)
- **Optimized for MacOS (Apple Silicon)**: Built and optimized for machines running on Apple Silicon chips. Needs work to integrate potrace's C library when compiling on Windows.

## Requirements
- **Potrace C library**: This project requires the Potrace C library to be installed.
  - On MacOS `brew install potrace`

For development only (not needed if using a pre-compiled binary)
- **Rust 1.54** or later
- **Cargo** (Rust’s package manager)
- Additional dependencies as listed in `Cargo.toml`.

## Installation

### Binary

Download releases tab for your platform.
- MacOS: Move the downloaded `.app` file to the Applications folder.
- Windows: Not yet built.

### From source

1. Clone the repository to your local machine (with GitHub CLI):

    ```bash
    gh repo clone mateo0023/supernote-tool-rs
    cd supernote-exporter
    ```

2. Install the required libraries:

    ```bash
    brew install potrace
    ```

3. Build the project:

    ```bash
    cargo build --release
    ```

## Usage

Either download your binary from the Releases page or launch the application by running the following command:

```bash
cargo run --release
```

The graphical interface will open, allowing you to load Supernote A5X files and configure export settings, such as adding a ToC or enabling clickable links in the output.

## GUI Features

- **File Import**: Load your `.note` files from Supernote devices.
- **Export Options**: Choose between exporting various PDF files or merge. File-file links will work only if merging into a single PDF.
- **ToC**: Easily edit the Table of Contents with the pre-rendered titles.
- **Save ToC Transcriptions**: You can load-save transcriptions to permanent storage. Great when exporting the same file over and over.

### Title Implementation

The titles are automatically grouped in the following way:

![Black, Light Gray, Dark Gray, Striped](./examples/Test%20Doc_Page_3.png)

However, there's no Optical Character Recognition (OCR) so they are first loaded with empty text fields. You can manually edit them and even save/load those title transcriptions with the Load/Save Cache buttons. When exporting, these settings will be automatically saved to the last selected settings file.

## Contributions

Contributions are welcome! Feel free to open issues for bugs, feature requests, or submit pull requests.

## Acknowledgments

This project is based on the excellent work decoding the file structure by [jya-dev's supernote-tool](https://github.com/jya-dev/supernote-tool). Special thanks to the open-source community for making tools like this possible.

## License

This project is licensed under the GNU GPLv3 License - see the [LICENSE](LICENSE) file for details.
