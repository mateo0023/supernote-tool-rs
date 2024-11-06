fn async_bench() {
    use supernote_tool_rs::*;
    let mut sch = Scheduler::new();
    sch.load_notebooks(
        vec!["./test/01. Asset Allocation.note".into()],
        ServerConfig::default()
    );
    let titles = loop {
        if let Some(msg) = sch.check_update() { match msg {
            messages::SchedulerResponse::NoteMessage(note_msg) => match note_msg {
                messages::NoteMsg::TitleLoaded(title_collection) => break title_collection,
                messages::NoteMsg::FailedToLoad(e) => panic!("Failed to load {}", e),
                _ => (),
            },
            _ => panic!("Unexpected message while benchmarking")
        } }
    };
    let id = titles.note_id;
    sch.save_notebooks(
        vec![titles],
        ExportSettings::Seprate(vec![(id, "./test/test.pdf".into())])
    );
    loop {
        if let Some(msg) = sch.check_update() {
            use messages::SchedulerResponse;
            if let SchedulerResponse::ExportMessage(exp_msg) = msg { 
                match exp_msg {
                    messages::ExpMsg::Complete => break,
                    messages::ExpMsg::Error(e) => panic!("Export failed with {}", e),
                    _ => ()
                }
             }
        }
    }
}

fn main() {
    async_bench();
    let _ = supernote_tool_rs::sync_work(
        vec!["./test/01. Asset Allocation.note".into()],
        None, supernote_tool_rs::ServerConfig::default(),
        false, "./test/".into()
    );
}