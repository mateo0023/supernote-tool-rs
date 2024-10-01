const ICON_DATA: &[u8; 72702] = include_bytes!("../../icons/256x256@2x.png");

pub fn get_icon() -> egui::IconData {
    let img = image::load_from_memory(ICON_DATA).unwrap();
    egui::IconData {
        rgba: img.into_rgba8().to_vec(),
        width: 256,
        height: 256,
    }
}