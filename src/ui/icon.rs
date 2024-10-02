const ICON_DATA: &[u8; 25161] = include_bytes!("../../icons/128x128.png");

pub fn get_icon() -> egui::IconData {
    let img = image::load_from_memory(ICON_DATA).unwrap();
    egui::IconData {
        rgba: img.into_rgba8().to_vec(),
        width: 128,
        height: 128,
    }
}