# Supernote Exporter Tool (GUI)

This project is a Rust-based GUI application designed to work with Supernote A5X files. It provides a streamlined way to render and export Supernote files, adding features such as a Table of Contents (ToC) and link support. Additionally, it converts bitmap traces into vector graphics for smoother rendering and exporting. The project builds upon the foundational work of [jya-dev's supernote-tool](https://github.com/jya-dev/supernote-tool) while delivering a notable speed improvement (over 10x).

## Features

- **Graphical User Interface (GUI)**: A user-friendly interface for easy file handling and export configuration.
- **Supernote A5X File Parsing**: Efficiently decode and parse Supernote A5X files.
- **Bitmap to Vector Conversion**: Converts bitmap traces to vector graphics using the Potrace library.
- **Table of Contents (ToC) Generation**: Generate and add a ToC for easy navigation. Currently typed by hand, OCR planned.
- **Link Support**: Add support for clickable links within the exported files.
- **File Merge**: Merge multiple files into a single PDF with working inter-file links.
- **Color Remap**: Change from grayscale to colorfull images. Right now it only has default remaping:
  - Light Gray goes to yellow (`#fdfa75`)
  - Dark Gray goes to blue (`#4669d6`)
- **Optimized for MacOS (Apple Silicon)**: Built and optimized for machines running on Apple Silicon chips.

## Requirements

- **Rust 1.54** or later
- **Cargo** (Rust’s package manager)
- **Potrace C library**: This project requires the Potrace C library to be installed. 
- Additional dependencies as listed in `Cargo.toml`.

## Installation

1. Clone the repository to your local machine:

    ```bash
    git clone https://github.com/your-username/supernote-exporter.git
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

Launch the application by running the following command:

```bash
cargo run --release
```

The graphical interface will open, allowing you to load Supernote A5X files and configure export settings, such as adding a ToC or enabling clickable links in the output.

### Title Implementation

The titles are automatically grouped in the following way:

![Black, Light Gray, Dark Gray, Striped](./examples/Test%20Doc_Page_3.png)

However, there's no Optical Character Recognition (OCR) so they are first loaded with empty text fields. You can manually edit them and even save/load those title transcriptions with the Load/Save Cache buttons. When exporting, these settings will be automatically saved to the last selected settings file.

## GUI Features

- **File Import**: Load your `.note` files from Supernote devices.
- **Export Options**: Choose between exporting various PDF files or merge.
- **ToC**: Easily edit the Table of Contents with the pre-rendered titles.

## Contributions

Contributions are welcome! Feel free to open issues for bugs, feature requests, or submit pull requests. Check out `CONTRIBUTING.md` for more details on how to contribute.

## Acknowledgments

This project is based on the excellent work by [jya-dev's supernote-tool](https://github.com/jya-dev/supernote-tool). Special thanks to the open-source community for making tools like this possible.

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.