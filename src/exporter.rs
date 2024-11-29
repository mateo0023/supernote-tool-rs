use std::collections::HashMap;
use std::error::Error;

use crate::data_structures::*;
use crate::decoder::{decode_separate, ColorMap, DecodedImage};
use crate::error::DecoderError;

const A4_WIDTH: u32 = crate::common::f_fmt::PAGE_WIDTH as u32;
const A4_HEIGHT: u32 = crate::common::f_fmt::PAGE_HEIGHT as u32;

mod potrace;

mod page_ranges;

pub use page_ranges::*;

pub use potrace::Word as PotraceWord;
pub use potrace::PotraceError;

use lopdf::content::Content;
use lopdf::{dictionary, Document, Object, ObjectId, Stream};

/// The [Title] but for the Table of Contents.
/// Used for exporting to PDF.
pub struct TitleToC {
    /// The level of the Title, 
    /// see [TitleLevel], for some basics.
    pub level: i8,
    /// The [transcription](Transcription) of the [Title]
    pub name: String,
    /// The index in the `.note` [Notebook].
    pub page_index: usize,
}

/// Exports the array of [Notebook] into a single **uncompressed** [PDF document](Document).
pub fn export_multiple(notebooks: Vec<&Notebook>, title_cols: Vec<Vec<TitleToC>>, page_map: MultiNotePageMap) -> Result<Document, Box<dyn Error>> {
    let mut doc = Document::with_version("1.7");
    let base_page_id = doc.new_object_id();

    let file_map = {
        let mut map = HashMap::new();
        notebooks.iter().zip(page_map.iter()).for_each(|n| {map.insert(n.0.file_id, n);});
        map
    };

    // Creating document catalog.
    // There are many more entries allowed in the catalog dictionary.
    let catalog_id = doc.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => base_page_id,
    });

    let mut pages = vec![];
    for (notebook, page_map) in notebooks.iter().zip(page_map.iter()) {
        pages.extend_from_slice(&add_pages(base_page_id, &mut doc, notebook, page_map)?);
    }

    // Add links to the notebook.
    for (notebook, map) in notebooks.iter().zip(page_map.iter()) {
        for link in &notebook.links {
            match &link.link_type {
                LinkType::SameFile { page_id } => {
                    let to_idx = notebook.get_page_index_from_id(*page_id).unwrap();
                    if let (Some(from_idx), Some(to_idx)) = (map.get_new_idx(link.start_page), map.get_new_idx(to_idx)) {
                        add_internal_link(
                            &mut doc, pages[from_idx],
                            link.coords, pages[to_idx]
                        )?;
                    }
                },
                // Link goes to into_note
                LinkType::OtherFile { page_id, file_id  } => if let Some(into_note) = file_map.get(file_id) {
                    if let (Some(from_idx), Some(to_idx)) = (
                        map.get_new_idx(link.start_page),
                        into_note.0.get_page_index_from_id(*page_id).and_then(|idx| into_note.1.get_new_idx(idx))
                    ) {
                        add_internal_link(
                            &mut doc, pages[from_idx],
                            link.coords, pages[to_idx]
                        )?;

                    }
                },
                LinkType::WebLink { link } => todo!("Haven't implemented linking to {}", link),
            }
        }
    }

    // Get a link of Titles to add to the document.
    let mut titles = vec![];
    for (toc_items, page_map) in title_cols.into_iter().zip(page_map.iter()) {
        // Filter un-included and map page indexes.
        titles.extend(toc_items.into_iter().filter_map(|mut t| page_map.get_new_idx(t.page_index).map(|n_idx| {
            t.page_index = n_idx;
            t
        })));
    }
    // Add the table of contents to the document
    add_toc(&mut doc, titles, &pages, catalog_id).map_err(|e| e.to_string())?;

    let page_count = pages.len();

    // Add the pages object to the document
    doc.objects.insert(base_page_id, Object::Dictionary(dictionary!{
        // Type of dictionary
        "Type" => "Pages",
        // Vector of page IDs in document. Normally would contain more than one ID
        // and be produced using a loop of some kind.
        "Kids" => pages.into_iter().map(|p| p.into()).collect::<Vec<_>>(),
        // Page count
        "Count" => page_count as i64,
        // A rectangle that defines the boundaries of the physical or digital media.
        // This is the "page size".
        "MediaBox" => vec![0.into(), 0.into(), A4_WIDTH.into(), A4_HEIGHT.into()]
    }));

    // The "Root" key in trailer is set to the ID of the document catalog,
    // the remainder of the trailer is set during `doc.save()`.
    doc.trailer.set("Root", catalog_id);

    // doc.compress();

    Ok(doc)
}

/// Exports a single [Notebook] and list of [TitleToC] into an **uncompressed** [Document].
pub fn to_pdf(notebook: &Notebook, titles: Vec<TitleToC>, page_map: &PageMap) -> Result<Document, Box<dyn Error>> {
    let mut doc = Document::with_version("1.7");
    let base_page_id = doc.new_object_id();

    // Creating document catalog.
    // There are many more entries allowed in the catalog dictionary.
    let catalog_id = doc.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => base_page_id,
    });

    let pages = add_pages(base_page_id, &mut doc, notebook, page_map)?;

    for link in &notebook.links {
        match &link.link_type {
            LinkType::SameFile { page_id } => {
                let &to_idx = notebook.page_id_map.get(page_id).unwrap();
                add_internal_link(
                    &mut doc, pages[link.start_page],
                    link.coords, pages[to_idx]
                )?;
            },
            // Don't have any other .note files to link to
            LinkType::OtherFile { .. } => continue,
            LinkType::WebLink { link } => todo!("Haven't implemented linking to {}", link),
        }
    }

    // Add the table of contents to the document
    add_toc(
        &mut doc, 
        titles.into_iter()
            .filter_map(|mut t| page_map.get_new_idx(t.page_index).map(|n_idx| {t.page_index = n_idx; t})).collect::<Vec<_>>(),
        &pages, catalog_id
    )?;

    let page_count = pages.len();

    // Add the pages object to the document
    doc.objects.insert(base_page_id, Object::Dictionary(dictionary!{
        // Type of dictionary
        "Type" => "Pages",
        // Vector of page IDs in document. Normally would contain more than one ID
        // and be produced using a loop of some kind.
        "Kids" => pages.into_iter().map(|p| p.into()).collect::<Vec<_>>(),
        // Page count
        "Count" => page_count as i64,
        // A rectangle that defines the boundaries of the physical or digital media.
        // This is the "page size".
        "MediaBox" => vec![0.into(), 0.into(), A4_WIDTH.into(), A4_HEIGHT.into()]
    }));

    // The "Root" key in trailer is set to the ID of the document catalog,
    // the remainder of the trailer is set during `doc.save()`.
    doc.trailer.set("Root", catalog_id);

    // doc.compress();

    Ok(doc)
}

/// Create a table of contents given the list of [titles](TitleToC) and [page_ids](ObjectId).
/// 
/// ## Titles MUST have thier indexes mapped beforehand
fn add_toc(doc: &mut Document, titles: Vec<TitleToC>, page_ids: &[ObjectId], catalog_id: ObjectId) -> Result<(), lopdf::Error>{
    let mut catalog = doc.get_object(catalog_id)?.as_dict()?.clone();
    let mut prev_at_level: HashMap<i8, ObjectId> = HashMap::new();
    
    // Create or get the /Outlines dictionary
    let outlines_id = {
        let outlines_id = doc.add_object(dictionary!{
            "Type" => "Outlines",
        });
        // Set the /Outlines entry in the catalog
        catalog.set("Outlines", Object::Reference(outlines_id));
        doc.objects.insert(catalog_id, Object::Dictionary(catalog));
        outlines_id
    };

    let mut title_id_stack = std::collections::VecDeque::new();
    let mut outline_count = (i8::MAX, 0);
    for TitleToC {level, name, page_index } in titles.into_iter() {
        // Update the count of the higher-level ToC Items.
        match level.cmp(&outline_count.0) {
            std::cmp::Ordering::Less => outline_count = (level, 1),
            std::cmp::Ordering::Equal => outline_count.1 += 1,
            std::cmp::Ordering::Greater => (),
        }

        while let Some((_id, queue_lvl)) = title_id_stack.back() {
            match level.cmp(queue_lvl) {
                // If title's level is not closer to root, break the loop
                std::cmp::Ordering::Greater => break,
                // If Title's Level is the same, continue popping
                std::cmp::Ordering::Equal => {
                    title_id_stack.pop_back();
                },
                // If now closer to root, also remove old level
                std::cmp::Ordering::Less => {
                    prev_at_level.remove(queue_lvl);
                    title_id_stack.pop_back();
                },
            }
        }
        let page = page_ids[page_index];
        let parent_id = title_id_stack.back().map(|(id, _lvl)| *id);

        // Create a new ObjectId for the bookmark
        let new_id = doc.new_object_id();
    
        // Create the bookmark dictionary
        let mut bookmark_dict = lopdf::Dictionary::new();
        bookmark_dict.set("Title", Object::string_literal(name));
        bookmark_dict.set("Parent", Object::Reference(parent_id.unwrap_or(outlines_id)));
        bookmark_dict.set(
            "Dest",
            Object::Array(vec![
                Object::Reference(page),
                Object::Name(b"Fit".to_vec()),
            ]),
        );
    
        // Set /Prev and /Next links
        if let Some(&prev_id) = prev_at_level.get(&level) {
            // Set /Prev
            bookmark_dict.set("Prev", Object::Reference(prev_id));
            // Update the previous bookmark's /Next to point to the new bookmark
            if let Some(Object::Dictionary(ref mut prev_dict)) = doc.objects.get_mut(&prev_id) {
                prev_dict.set("Next", Object::Reference(new_id));
            }
        }
    
        // Insert the new bookmark into the document
        doc.objects.insert(new_id, Object::Dictionary(bookmark_dict));
    
        // Update the parent's /First and /Last entries
        let parent_obj_id = parent_id.unwrap_or(outlines_id);
        if let Some(Object::Dictionary(ref mut parent_dict)) = doc.objects.get_mut(&parent_obj_id) {
            // Update /First if it doesn't exist
            if !parent_dict.has(b"First") {
                parent_dict.set("First", Object::Reference(new_id));
            }
            // Update /Last
            parent_dict.set("Last", Object::Reference(new_id));

            // Update /Count
            let count = parent_dict
                .get(b"Count")
                .and_then(|o| o.as_i64())
                .unwrap_or(0)
                + 1;
            parent_dict.set("Count", Object::Integer(count));
        }
    
        // Update the `prev_at_level` hashmap
        prev_at_level.insert(level, new_id);
    
        // Add it to the queue
        title_id_stack.push_back((new_id, level));
    }

    if let Some(Object::Dictionary(ref mut outlines_dict)) = doc.objects.get_mut(&outlines_id) {
        // Ensure /First and /Last are set
        if !outlines_dict.has(b"First") {
            if let Some(&first_id) = prev_at_level.values().next() {
                outlines_dict.set("First", Object::Reference(first_id));
            }
        }
        if !outlines_dict.has(b"Last") {
            if let Some(&last_id) = prev_at_level.values().last() {
                outlines_dict.set("Last", Object::Reference(last_id));
            }
        }
        // Set /Count to the total number of top-level bookmarks
        outlines_dict.set("Count", Object::Integer(outline_count.1));
    }

    Ok(())
}

/// Add [page](Page)s of the [Notebook] to the [Document], ignoring the pages that should not be exported
/// according to the [PageMap].
fn add_pages(pages_id: ObjectId, doc: &mut Document, notebook: &Notebook, page_map: &PageMap) -> Result<Vec<ObjectId>, Box<dyn Error>> {
    let mut page_commands = Vec::with_capacity(notebook.pages.len());
    for (idx, page) in notebook.pages.iter().enumerate() {
        // Only add the included pages
        if page_map.get_new_idx(idx).is_some() {
            page_commands.push(page.command());
        }
    }

    let mut pages: Vec<ObjectId> = Vec::with_capacity(page_map.get_len());
    for content in page_commands {
        let encoded = content.encode()?;

        let content_id = doc.add_object(Stream::new(dictionary! {}, encoded));

        let page_id = doc.add_object(dictionary!{
            "Type" => "Page",
            "Parent" => pages_id,
            "MediaBox" => vec![0.into(), 0.into(), A4_WIDTH.into(), A4_HEIGHT.into()],
            "Contents" => content_id,
        });
        pages.push(page_id);
    }

    Ok(pages)
}


/// Function to add an internal link annotation to a page
fn add_internal_link(
    doc: &mut Document,
    from_page_id: ObjectId,
    rect: [u32; 4],
    destination_page_id: ObjectId,
) -> Result<(), Box<dyn Error>> {
    // Define the GoTo action
    let action = dictionary! {
        "Type" => "Action",
        "S" => "GoTo",
        "D" => vec![Object::Reference(destination_page_id), Object::Name("Fit".into())],
    };

    let action_id = doc.add_object(action);

    // Need to invert the y axis
    let processed_rect: Vec<Object> = vec![
        rect[0].into(),
        (A4_HEIGHT - rect[1]).into(),
        rect[2].into(),
        (A4_HEIGHT - rect[3]).into(),
    ];

    // Define the link annotation
    let annotation = dictionary! {
        "Type" => "Annot",
        "Subtype" => "Link",
        "Rect" => processed_rect,
        "Border" => vec![0.into(), 0.into(), 0.into()], // No border
        "A" => Object::Reference(action_id),
    };

    let annotation_id = doc.add_object(annotation);

    // Add the annotation to the page's /Annots array
    if let Some(Object::Dictionary(ref mut page_dict)) = doc.objects.get_mut(&from_page_id) {
        // Retrieve or create the /Annots array
        let annots = page_dict.as_hashmap_mut().entry("Annots".into()).or_insert_with(|| Object::Array(vec![]));

        if let Object::Array(ref mut annots_array) = annots {
            annots_array.push(Object::Reference(annotation_id));
        } else {
            // If /Annots exists but is not an array, return an error
            return Err("Page /Annots is not an array".into());
        }
    } else {
        return Err("Page object is not a dictionary".into());
    }

    Ok(())
}

/// Exports a given page to the PDF Vector Commands
pub fn page_to_commands(page: Page, colormap: ColorMap) -> Result<Content, Box<dyn Error>> {
    use file_format_consts::{PAGE_HEIGHT, PAGE_WIDTH};

    let mut image = DecodedImage::default();
    for data in page.layers.iter()
        .filter(|l| !l.is_background())
        .filter_map(|l| l.content.as_ref())
    {
        image += decode_separate(data, PAGE_WIDTH, PAGE_HEIGHT)?;
    }

    potrace::trace_and_generate(image, &colormap).map(|operations| {
        Content {
            operations,
        }
    })
}

impl Title {
    pub fn render_bitmap(&self) -> Result<Option<Vec<u8>>, DecoderError> {
        match &self.content {
            Some(data) => {
                let width = (self.coords[2] - self.coords[0]) as usize;
                let height = (self.coords[3] - self.coords[1]) as usize;
                let decoded = decode_separate(data, width, height)?;
                Ok(Some(decoded.into_color(&ColorMap::default())))
            },
            None => Ok(None),
        }
    }
}

impl From<&Title> for TitleToC {
    fn from(value: &Title) -> Self {
        TitleToC {
            level: value.title_level as i8,
            name: value.get_name(),
            page_index: value.page_index,
        }
    }
}

impl TitleToC {
    /// Create a new [TitleToC] that will be used to indicate a file.
    pub fn new_for_file(name: &str, index: usize) -> Self {
        TitleToC {
            level: TitleLevel::FileLevel as i8,
            page_index: index,
            name: name.to_string(),
        }
    }
}
