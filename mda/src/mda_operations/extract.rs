use crate::{
    get_full_data, print_revlog_headers, save_audio_to_file, save_image_to_file, save_text_to_file,
    save_video_to_file, write_strings_to_file, DataType, Entry, Header, MDAIndex, Revlog,
    RevlogHeader,
};
use anyhow::Result;
use std::error::Error;
use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::process;

/// Read data from an MDA file.
pub fn read_info_from_mda(file_path: &str) -> Result<(MDAIndex, Header), Box<dyn Error>> {
    let file = File::open(file_path)?;
    let mut reader = BufReader::new(file);
    let index: MDAIndex = bincode::deserialize_from(&mut reader)?;
    reader.seek(SeekFrom::Start(index.header_offset))?;
    let header: Header = bincode::deserialize_from(&mut reader)?;
    Ok((index, header))
}

// Read annotations from an MDA file
pub fn read_anno_from_mda(file_path: &str, rev: i32) -> Result<(), Box<dyn Error>> {
    let file = File::open(file_path)?;
    let mut reader = BufReader::new(file);

    // Deserialize the MDAIndex structure from the file
    let index: MDAIndex = bincode::deserialize_from(&mut reader)?;

    // Move the reader to the start of annotation headers
    reader.seek(SeekFrom::Start(index.anno_headers_offset))?;

    // Read header information
    let mut header_bytes = Vec::new();
    reader.read_to_end(&mut header_bytes)?;

    let mut headers: Vec<RevlogHeader> = Vec::new();
    let mut offset = 0;
    while offset < header_bytes.len() {
        let header: RevlogHeader = bincode::deserialize(&header_bytes[offset..])?;
        headers.push(header.clone());

        offset += bincode::serialized_size(&header)? as usize;
    }

    // If the rev is -1, set it to the last header's index, otherwise, use the provided rev
    let mut rev = rev;
    if rev == -1 {
        rev = (headers.len() - 1) as i32;
    }
    let header_number = rev + 1;
    let headers: Vec<RevlogHeader> = headers.into_iter().take(header_number as usize).collect();

    reader.seek(SeekFrom::Start(index.anno_entries_offset))?;

    let mut entries_bytes = Vec::new();
    reader.read_to_end(&mut entries_bytes)?;

    let mut entries: Vec<Entry> = Vec::new();
    let mut offset = 0;
    for revlog_header in &headers {
        let entry_bytes = &&entries_bytes[offset..(offset + revlog_header.length as usize)];
        let entry: Entry = bincode::deserialize(entry_bytes)?;
        entries.push(entry);

        offset += revlog_header.length as usize;
    }

    // Print the RevlogHeaders to the console
    print_revlog_headers(&headers);

    Ok(())
}

/// Extract data from an MDA file.
pub fn extract_data_from_mda(
    mda_path: &str,
    training_data_path: &str,
    anno_data_path: &str,
    rev: i32,
) -> Result<(), Box<dyn Error>> {
    let _ = extract_anno_from_mda(mda_path, anno_data_path, rev);
    extract_train_from_mda(mda_path, training_data_path)
}
/// Extract anno data from mda
fn extract_anno_from_mda(
    file_path: &str,
    anno_data_path: &str,
    rev: i32,
) -> Result<(), Box<dyn Error>> {
    let revlog = match get_anno_revlog_from_mda(file_path, rev) {
        Ok(revlog) => revlog,
        Err(err) => {
            println!("error={:?}", err);
            process::exit(1);
        }
    };
    let mut origin_rev = rev;
    if origin_rev == -1 {
        origin_rev = (revlog.headers.len() - 1) as i32;
    }

    let full_data = get_full_data(origin_rev as u8, revlog.entries);

    let strings: Vec<String> = vec![full_data.to_string()];

    write_strings_to_file(&strings, anno_data_path)?;
    Ok(())
}

/// Extract training data from mda
fn extract_train_from_mda(mda_path: &str, training_data_path: &str) -> Result<(), Box<dyn Error>> {
    let file = File::open(mda_path)?;
    let mut reader = BufReader::new(file);
    let index: MDAIndex = bincode::deserialize_from(&mut reader)?;

    reader.seek(SeekFrom::Start(index.train_data_offset))?;
    let data_type: DataType = bincode::deserialize_from(&mut reader)?;
    match data_type {
        DataType::Text => {
            let text: String = bincode::deserialize_from(&mut reader)?;

            save_text_to_file(&text, training_data_path)?;
        }
        DataType::Image => {
            let image_data: Vec<u8> = bincode::deserialize_from(&mut reader)?;

            save_image_to_file(&image_data, training_data_path)?;
        }
        DataType::Video => {
            let video_data: Vec<u8> = bincode::deserialize_from(&mut reader)?;

            save_video_to_file(&video_data, training_data_path)?;
        }
        DataType::Audio => {
            let audio_data: Vec<u8> = bincode::deserialize_from(&mut reader)?;

            save_audio_to_file(&audio_data, training_data_path)?;
        }
    };

    println!("Extract data {:?} successfully", mda_path);
    Ok(())
}

// Function to retrieve the annotation revision log from an MDA file
fn get_anno_revlog_from_mda(file_path: &str, rev: i32) -> Result<Revlog, Box<dyn Error>> {
    let mut rev = rev;

    let file = File::open(file_path)?;
    let mut reader = BufReader::new(file);

    // Deserialize the MDAIndex structure from the file, which contains offsets for headers and entries
    let index: MDAIndex = bincode::deserialize_from(&mut reader)?;

    reader.seek(SeekFrom::Start(index.anno_headers_offset))?;

    // Read the bytes data of the header information
    let mut header_bytes = Vec::new();
    reader.read_to_end(&mut header_bytes)?;

    let mut headers: Vec<RevlogHeader> = Vec::new();
    let mut offset = 0;
    while offset < header_bytes.len() {
        let header: RevlogHeader = bincode::deserialize(&header_bytes[offset..])?;
        headers.push(header.clone());

        offset += bincode::serialized_size(&header)? as usize;
    }

    // If the rev is -1, set it to the last header's index, otherwise, use the provided rev
    if rev == -1 {
        rev = (headers.len() - 1) as i32;
    }
    let header_number = rev + 1;
    let headers: Vec<RevlogHeader> = headers.into_iter().take(header_number as usize).collect();

    reader.seek(SeekFrom::Start(index.anno_entries_offset))?;

    let mut entries_bytes = Vec::new();
    reader.read_to_end(&mut entries_bytes)?;

    let mut entries: Vec<Entry> = Vec::new();
    let mut offset = 0;
    for revlog_header in &headers {
        let entry_bytes = &&entries_bytes[offset..(offset + revlog_header.length as usize)];
        let entry: Entry = bincode::deserialize(entry_bytes)?;
        entries.push(entry);

        offset += revlog_header.length as usize;
    }

    let revlog = Revlog::new(headers, entries);

    Ok(revlog)
}
