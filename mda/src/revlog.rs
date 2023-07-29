//! Build a data structure similar to the revlog format to implement version control and incremental storage.
 
use serde::{Deserialize, Serialize};
use sha1::{Digest, Sha1};
use std::cmp::{max, min};
use std::process;

mod constants {
    pub const BLOCK_SIZE: usize = 10; // TODO
    pub const NULLID: [u8; 20] = [0; 20];
}

///  Splitting large-scale data into fixed-size data blocks and recording the block numbers.
fn split_data_into_blocks(data: Vec<u8>, block_size: usize) -> (Vec<DataBlock>, Vec<usize>) {
    let mut blocks = Vec::new();
    let mut index = 0;
    let mut block_number = 0;
    let mut numbers: Vec<usize> = Vec::new();
    while index < data.len() {
        numbers.push(block_number);

        let end = std::cmp::min(index + block_size, data.len());
        blocks.push(DataBlock::new(block_number, data[index..end].to_vec()));
        index = end;
        block_number += 1;
    }

    (blocks, numbers)
}

/// Comparing data block lists to find newly added data blocks.
fn find_different_blocks(
    id: u8,
    entries: &Vec<Entry>,
    current_data: &[u8],
    _block_size: usize,
) -> Vec<DataBlock> {
    let blocks1 = get_data_blocks_up_to_id(id, entries);
    let (blocks2, _data_indices) =
        split_data_into_blocks(current_data.clone().to_vec(), constants::BLOCK_SIZE);
    // Find elements in block1 that are not in block2
    let elements_not_in_block1: Vec<DataBlock> = blocks2
        .iter()
        .filter(|block2_item| {
            !blocks1
                .iter()
                .any(|block1_item| block1_item.data == block2_item.data)
        })
        .cloned()
        .collect();
    // let elements_not_in_block1: Vec<DataBlock> = blocks2
    // .iter()
    // .filter(|block2_item| {
    //     blocks1
    //         .iter()
    //         .find(|block1_item| block1_item.data == block2_item.data)
    //         .is_none()
    // })
    // .cloned()
    // .collect();
    elements_not_in_block1
}

/// add new blocks to blocklist
fn add_to_block_list(
    mut block_list: Vec<DataBlock>,
    different_blocks: Vec<DataBlock>,
) -> (Vec<DataBlock>, Vec<usize>) {
    let mut diff_number = Vec::<usize>::new();
    for mut block in different_blocks {
        let last_block_number = block_list.last().map_or(0, |block| block.block_number);

        block.block_number = 1 + last_block_number;
        diff_number.push(block.block_number);
        block_list.push(block);
    }

    // block_list
    (block_list, diff_number)
}

/// extract index from data blocks
fn extract_index(vec_data1: &[DataBlock], vec_data2: &[DataBlock]) -> Vec<usize> {
    let mut index: Vec<usize> = Vec::new();
    for data_block1 in vec_data1.iter() {
        if let Some(index_in_vec_data2) = vec_data2
            .iter()
            .position(|data_block2| data_block1.data == data_block2.data)
        {
            index.push(vec_data2[index_in_vec_data2].block_number);
        }
    }

    index
}

impl Entry {
    /// new Entry
    fn new(id: u8, index: Vec<usize>, blocks: Vec<DataBlock>) -> Self {
        Entry { id, index, blocks }
    }

    /// add first Entry
    pub fn init(content: &str) -> (Vec<RevlogHeader>, Vec<Entry>) {
        // Config current content
        let data: Vec<u8> = content.as_bytes().to_vec();
        let (blocks, data_indices) = split_data_into_blocks(data.clone(), constants::BLOCK_SIZE);

        // Config enrty
        let entry = Entry::new(0, data_indices, blocks);
       
        let entries: Vec<Entry> = vec![entry];

        // Config Header
        let nodeid = compute_nodeid(&constants::NULLID, &constants::NULLID, &data);

        let revlog_header = RevlogHeader::new(
            0,
            0,
            data.len() as u32,
            0,
            0,
            constants::NULLID,
            constants::NULLID,
            nodeid,
        );
        let headers: Vec<RevlogHeader> = vec![revlog_header];
        (headers, entries)
    }

    /// add entries to list
    pub fn add(
        content: &str,
        mut record_table: Vec<Entry>,
        mut headers: Vec<RevlogHeader>,
    ) -> (Vec<RevlogHeader>, Vec<Entry>) {
        // Config data from last entry
        let last_entry = record_table.last().unwrap_or_else(|| {
            println!("The last data is empty!");
            process::exit(1);
        });
        let last_id = last_entry.id;
        let last_header = headers.last().unwrap_or_else(|| {
            println!("The last data is empty!");
            process::exit(1);
        });
        let mut last_p1 = last_header.p1rev;
        if last_id == 0 {
            last_p1 = last_header.nodeid;
        }
        // Config current data info
        let current_id = last_id + 1;

        // change to Vec<u8>
        let current_data: Vec<u8> = content.as_bytes().to_vec();
        let (current_data_blocks, _data_indices) =
            split_data_into_blocks(current_data.clone(), constants::BLOCK_SIZE);

        // Build a block list and record the construction number of the original data
        let different_blocks =
            find_different_blocks(last_id, &record_table, &current_data, constants::BLOCK_SIZE);

        let block_list = get_data_blocks_up_to_id(last_id, &record_table);
        let (records, diff) = add_to_block_list(block_list, different_blocks);

        // assign id to diff blocks
        let diff_blocks: Vec<DataBlock> = records
            .iter()
            .filter_map(|record| {
                if diff.contains(&record.block_number) {
                    Some(DataBlock {
                        block_number: record.block_number,
                        data: record.data.clone(),
                    })
                } else {
                    None
                }
            })
            .collect();

        // get current index
        let matching_block_numbers = extract_index(&current_data_blocks, &records);

        // Config entry

        let nodeid = compute_nodeid(&constants::NULLID, &constants::NULLID, &current_data);
        let entry = Entry {
            id: current_id,
            index: matching_block_numbers,
            blocks: diff_blocks,
        };
        record_table.push(entry);

        //Config header
        let revlog_header = RevlogHeader::new(
            current_id,
            0,
            current_data.len() as u32,
            0,
            last_id as i32,
            last_p1,
            constants::NULLID,
            nodeid,
        );
        headers.push(revlog_header);
        (headers, record_table)
    }
}

/// Compute nodeid hash using sha1
fn compute_nodeid(parent1: &[u8; 20], parent2: &[u8; 20], contents: &[u8]) -> [u8; 20] {
    let mut hasher = Sha1::new();
    hasher.update(min(parent1, parent2));
    hasher.update(max(parent1, parent2));
    hasher.update(contents);
    let result = hasher.finalize();
    let mut nodeid = [0u8; 20];
    nodeid.copy_from_slice(&result);
    nodeid
}

/// shorten nodeid
fn nodeid_to_short_hex(nodeid: &[u8; 20]) -> String {
    let nodeid_hex_string: String = nodeid
        .iter()
        .take(6)
        .map(|b| format!("{:02x}", b))
        .collect();
    nodeid_hex_string
}

/// Function to combine Vec<DataBlock> into text
fn combine_data_blocks_to_text(data_blocks: &Vec<DataBlock>) -> String {
    let mut combined_text = String::new();
    for data_block in data_blocks {
        combined_text.push_str(std::str::from_utf8(&data_block.data).unwrap());
    }
    combined_text
}

/// Find the corresponding indexes by ID.
fn find_index_by_id(id: u8, delta_list: &[Entry]) -> Option<Vec<usize>> {
    let delta_to_find = delta_list.iter().find(|entry| entry.id == id);

    delta_to_find.map(|entry| entry.index.clone())
}

/// Get all data blocks from ID 0 to the input ID.
fn get_data_blocks_up_to_id(id: u8, delta_list: &Vec<Entry>) -> Vec<DataBlock> {
    let mut data_blocks = Vec::new();
    for entry in delta_list {
        if entry.id <= id {
            data_blocks.extend(entry.blocks.iter().cloned());
        }
    }
    data_blocks
}

/// Get the Vec<DataBlock> corresponding to the indexes.
fn get_data_blocks_by_index(index: &Vec<usize>, data_blocks: &[DataBlock]) -> Vec<DataBlock> {
    let mut result_blocks = Vec::new();
    for &idx in index {
        if let Some(data_block) = data_blocks.iter().find(|block| block.block_number == idx) {
            result_blocks.push(data_block.clone());
        }
    }
    result_blocks
}
/// Get full data(string)
pub fn get_full_data(id: u8, entries: Vec<Entry>) -> String {
    if let Some(index) = find_index_by_id(id, &entries) {
        let data_blocks = get_data_blocks_up_to_id(id, &entries);
        let selected_blocks = get_data_blocks_by_index(&index, &data_blocks);
        combine_data_blocks_to_text(&selected_blocks)
    } else {
        println!("No data blocks found for ID {}", id);
        process::exit(1);
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Entry {
    pub id: u8,
    pub index: Vec<usize>,
    pub blocks: Vec<DataBlock>,
}
/// Structure for a data block
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct DataBlock {
    /// Block number of the data block
    pub block_number: usize,
    /// Content of the data block
    pub data: Vec<u8>,
}

impl DataBlock {
    fn new(block_number: usize, data: Vec<u8>) -> Self {
        DataBlock { block_number, data }
    }
}
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RevlogHeader {
    pub rev: u8,
    pub offset: u64,
    pub length: u32,
    pub baserev: i32,
    pub linkrev: i32,
    pub p1rev: [u8; 20],
    pub p2rev: [u8; 20],
    pub nodeid: [u8; 20],
}
impl RevlogHeader {
    #![allow(clippy::too_many_arguments)]
     fn new(
        rev: u8,
        offset: u64,
        length: u32,
        baserev: i32,
        linkrev: i32,
        p1rev: [u8; 20],
        p2rev: [u8; 20],
        nodeid: [u8; 20],
    ) -> RevlogHeader {
        RevlogHeader {
            rev: (rev),
            offset: (offset),
            length,
            baserev,
            linkrev,
            p1rev,
            p2rev,
            nodeid,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]

pub struct RevlogIndex {
    pub headers_offset: u64,
    pub entries_offset: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Revlog {
    pub revlog_index: RevlogIndex,
    pub headers: Vec<RevlogHeader>,
    pub entries: Vec<Entry>,
}

impl Revlog {
    pub fn new(revlog_header: Vec<RevlogHeader>, entries: Vec<Entry>) -> Self {
        let revlog_index = RevlogIndex {
            headers_offset: 0,
            entries_offset: 0,
        };
        Revlog {
            revlog_index: (revlog_index),
            headers: (revlog_header),
            entries: (entries),
        }
    }
}
 
pub fn print_revlog_headers(headers: &Vec<RevlogHeader>) {
    println!(
        "{:<6} {:<8} {:<7} {:<6} {:<7} {:<12} {:<12} {:<40}",
        "rev", "offset", "length", "delta", "linkrev", "nodeid", "p1", "p2"
    );

    // let mut count = 0;
    // for header in headers {
    //      let mut rev = header.rev.to_string();
    //     if count == headers.len() - 1 {
    //         rev = header.rev.to_string() + "*";
    //     }
    //     println!(
    //         "{:<6} {:<8} {:<7} {:<6} {:<7} {:<12} {:<12} {:<40}",
    //         rev,
    //         header.offset,
    //         header.length,
    //         header.baserev,
    //         header.linkrev,
    //         nodeid_to_short_hex(&header.nodeid),
    //         nodeid_to_short_hex(&header.p1rev),
    //         nodeid_to_short_hex(&header.p2rev),
    //     );
    //     count += 1;
    // }
    for (count, header) in headers.iter().enumerate() {
        let mut rev = header.rev.to_string();
        if count == headers.len() - 1 {
            rev = header.rev.to_string() + "*";
        }
        println!(
            "{:<6} {:<8} {:<7} {:<6} {:<7} {:<12} {:<12} {:<40}",
            rev,
            header.offset,
            header.length,
            header.baserev,
            header.linkrev,
            nodeid_to_short_hex(&header.nodeid),
            nodeid_to_short_hex(&header.p1rev),
            nodeid_to_short_hex(&header.p2rev),
        );
    }
}
