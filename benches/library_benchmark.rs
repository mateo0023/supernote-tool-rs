fn main() {
    use supernote_tool_rs::*;
    let mut sch = Scheduler::new();
    sch.load_notebooks(
        vec!["./test/01. Asset Allocation.note".into()],
        ServerConfig::default()
    );
    let titles = loop {
        if let Some(msg) = sch.check_update() { match msg {
            scheduler::messages::SchedulerResponse::NoteMessage(note_msg) => match note_msg {
                scheduler::messages::NoteMsg::TitleLoaded(title_collection) => break title_collection,
                scheduler::messages::NoteMsg::FailedToLoad(e) => panic!("Failed to load {}", e),
                _ => (),
            },
            _ => panic!("Unexpected message while benchmarking")
        } }
    };
    let id = titles.note_id;
    sch.save_notebooks(
        vec![titles],
        scheduler::ExportSettings::Seprate(vec![(id, "./test.pdf".into())])
    );
    loop {
        if let Some(msg) = sch.check_update() {
            use scheduler::messages::SchedulerResponse;
            if let SchedulerResponse::ExportMessage(exp_msg) = msg { 
                match exp_msg {
                    scheduler::messages::ExpMsg::Complete => break,
                    scheduler::messages::ExpMsg::Error(e) => panic!("Export failed with {}", e),
                    _ => ()
                }
             }
        }
    }
}