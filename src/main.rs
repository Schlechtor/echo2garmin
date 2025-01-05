#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]

// use btleplug::api::{Central, CharPropFlags, Manager as _, Peripheral, ScanFilter, WriteType};
// use btleplug::platform::Manager;
// use std::fmt::write;
// use std::str::FromStr;
// use std::time::SystemTime;
// use futures::stream::StreamExt;
// use uuid::Uuid;
// use std::fs::File;
// use std::io::{Write, BufWriter};
// use byteorder::{LittleEndian, WriteBytesExt};
// use std::{fs, ptr};
// use chrono::{DateTime, Utc};

use std::os::raw::c_void;
// extern crate libc;
// use std::ffi::c_void;
use std::slice;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

use lazy_static::lazy_static;
use std::fs::File;
use std::io::{Seek, SeekFrom, Write};

const FIT_BASE_TYPE_UINT32: u8 = 0x86; 
const FIT_BASE_TYPE_UINT32Z: u8 = 0x8C; 
const FIT_BASE_TYPE_STRING: u8 = 0x07;
const FIT_BASE_TYPE_UINT16: u8 = 0x84; 
const FIT_BASE_TYPE_UINT8: u8 = 0x02;
const FIT_BASE_TYPE_UINT8Z: u8 = 0x0A;
const FIT_BASE_TYPE_UINT16Z: u8 = 0x8B; 
const FIT_BASE_TYPE_ENUM: u8 = 0x00;

lazy_static! {
    static ref DATA_CRC: Arc<Mutex<u16>> = Arc::new(Mutex::new(0)); 
}

#[repr(C, packed)]
struct FITFILEHDR {
    header_size: u8,        // 1 byte
    protocol_version: u8,   // 1 byte
    profile_version: u16,   // 2 bytes
    data_size: u32,         // 4 bytes
    data_type: [u8; 4],     // 4 bytes
    crc: u16,               // 2 bytes
}

fn write_file_header(file: &mut File) {
    const size: u32 = 14;
    let mut file_header = FITFILEHDR {
        header_size: size as u8,
        protocol_version: (2 << FIT_PROTOCOL_VERSION_MAJOR_SHIFT),
        profile_version: (FIT_PROFILE_VERSION_MAJOR * FIT_PROFILE_VERSION_SCALE + FIT_PROFILE_VERSION_MINOR) as u16,
        data_size: 0,
        data_type: *b".FIT",
        crc: 0,
    };
    
    file.seek(SeekFrom::End(0)).unwrap();
    let file_size = file.stream_position().unwrap();

    file_header.data_size = (size as u64 - std::mem::size_of::<u16>() as u64) as u32;

    if file_size < size as u64 {
        file_header.data_size = (size as u64 - std::mem::size_of::<u16>() as u64) as u32;
    } else {
        file_header.data_size = (file_size - size as u64 - std::mem::size_of::<u16>() as u64) as u32;
    }


    let mut buffer = vec![];
    buffer.write_all(&file_header.header_size.to_le_bytes()).unwrap();
    buffer.write_all(&file_header.profile_version.to_le_bytes()).unwrap();
    buffer.write_all(&file_header.protocol_version.to_le_bytes()).unwrap();
    buffer.write_all(&file_header.data_type).unwrap();
    buffer.write_all(&file_header.data_size.to_le_bytes()).unwrap();
    // buffer.write_all(&file_header.crc.to_le_bytes()).unwrap();


    let mut buffer2: Vec<u8> = vec![];
    buffer2.push(file_header.header_size);
    buffer2.push(file_header.protocol_version);
    buffer2.extend_from_slice(&file_header.profile_version.to_le_bytes());
    buffer2.extend_from_slice(&file_header.data_size.to_le_bytes());
    buffer2.extend_from_slice(&file_header.data_type);

    // println!("{:?}", fit_crc_calc16(as_bytes(&file_header)));
    // println!("{:?}", format!("{:X}", fit_crc_calc16(&buffer)));
    // println!("{:?}", format!("{:X}", fit_crc_calc16(as_bytes(&buffer))));

    // println!("{:?}", format!("{:X}", fit_crc_calc16(&buffer2)));
    // println!("{:?}", format!("{:X}", fit_crc_calc16(as_bytes(&buffer2))));


    file_header.crc = fit_crc_calc16(&buffer2);
    // file_header.crc = unsafe {FitCRC_Calc16(&file_header as *const _ as *const c_void, size)};

    file.seek(SeekFrom::Start(0)).unwrap();
    file.write_all(as_bytes(&file_header)).unwrap();
}

fn write_message_definition(file: &mut File, local_mesg_number: u8, mesg_def: &[u8]) {
    let header = local_mesg_number | 0x40;
    write_data(file, &[header], FIT_HDR_SIZE as usize);
    write_data(file, mesg_def, mesg_def.len());
}

// pub fn write_message_definition_with_dev_fields(
//     file: &mut File,
//     local_mesg_number: u8,
//     mesg_def: &[u8],
//     number_dev_fields: u8,
//     dev_field_definitions: &[FitDevFieldDef],
// ) {
//     let header = local_mesg_number | FIT_HDR_TYPE_DEF_BIT | FIT_HDR_DEV_DATA_BIT;
//     write_data(file, &[header], FIT_HDR_SIZE as usize);
//     write_data(file, mesg_def, mesg_def.len());

//     write_data(file, &[number_dev_fields], 1);
//     for dev_field_def in dev_field_definitions {
//         write_data(file, as_bytes(dev_field_def), std::mem::size_of::<FitDevFieldDef>());
//     }
// }

fn write_message(file: &mut File, local_mesg_number: u8, message: &[u8]) {
    write_data(file, &[local_mesg_number], FIT_HDR_SIZE as usize);
    write_data(file, message, message.len());
}

// pub fn write_developer_field(file: &mut File, data: &[u8]) {
//     write_data(file, data, data.len());
// }

fn write_data(file: &mut File, data: &[u8], data_size: usize) {
    // let x = copy_to_fixed_size_array(data, data_size);
    file.write_all(&data[..data_size]).unwrap();

    for &byte in &data[..data_size] {
        let current_crc = get_crc();
        let updated_crc = fit_crc_get16(current_crc, byte);
        set_crc(updated_crc);
    }
}

fn get_crc() -> u16 {
    let crc_guard = DATA_CRC.lock().unwrap();
    *crc_guard
}

fn set_crc(new_crc: u16) {
    let mut crc_guard = DATA_CRC.lock().unwrap();
    *crc_guard = new_crc;
}

fn as_bytes<T>(data: &T) -> &[u8] {
    unsafe { slice::from_raw_parts((data as *const T) as *const u8, std::mem::size_of::<T>()) }
}

fn fit_crc_get16(mut crc: u16, byte: u8) -> u16 {
    const CRC_TABLE: [u16; 16] = [
        0x0000, 0xCC01, 0xD801, 0x1400, 0xF001, 0x3C00, 0x2800, 0xE401,
        0xA001, 0x6C00, 0x7800, 0xB401, 0x5000, 0x9C01, 0x8801, 0x4400,
    ];

    // Compute checksum of lower four bits of byte
    let mut tmp = CRC_TABLE[(crc & 0xF) as usize];
    crc = (crc >> 4) & 0x0FFF;
    crc ^= tmp ^ CRC_TABLE[(byte & 0xF) as usize];

    // Compute checksum of upper four bits of byte
    tmp = CRC_TABLE[(crc & 0xF) as usize];
    crc = (crc >> 4) & 0x0FFF;
    crc ^= tmp ^ CRC_TABLE[((byte >> 4) & 0xF) as usize];

    crc
}

fn fit_crc_update16(mut crc: u16, data: &[u8]) -> u16 {
    for &byte in data {
        crc = fit_crc_get16(crc, byte);
    }
    crc
}

fn fit_crc_calc16(data: &[u8]) -> u16 {
    fit_crc_update16(0, data)
}

#[repr(C, packed)]
#[derive(Default, Debug)]
struct FITFILEIDMESG {
    pub serial_number: u32,                  // FIT_UINT32Z
    pub time_created: u32,                   // FIT_DATE_TIME (assumed to be a 32-bit integer)
    pub product_name: [u8; 20], // FIT_STRING (array of bytes)
    pub manufacturer: u16,                   // FIT_MANUFACTURER
    pub product: u16,                        // FIT_UINT16
    pub number: u16,                         // FIT_UINT16
    pub type_: u8,                       // FIT_FILE
}

#[repr(C, packed)]
struct FitFileIdMesgDef {
    reserved_1: u8,
    arch: u8,
    global_mesg_num: u16,
    num_fields: u8,
    fields: [u8; 7 * 3], 
}

fn write_file_id_message(file: &mut File, timestamp: u32) -> std::io::Result<()> {
    let file_id_mesg = FITFILEIDMESG { 
        serial_number: 1,
        time_created: timestamp,
        product_name: [b'.', b'F', b'I', b'T', 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        manufacturer: 200,
        product: 1,
        number: 1,
        type_: 0,
    };

    let def = FitFileIdMesgDef {
        reserved_1: 0,
        arch: 0,
        global_mesg_num: 1,
        num_fields: 7,
        fields: [
            3, size_of::<u32>() as u8, FIT_BASE_TYPE_UINT32Z,
            4, size_of::<u32>() as u8, FIT_BASE_TYPE_UINT32,
            8, 20, FIT_BASE_TYPE_STRING,
            1, size_of::<u16>() as u8, FIT_BASE_TYPE_UINT16,
            2, size_of::<u16>() as u8, FIT_BASE_TYPE_UINT16,
            5, size_of::<u16>() as u8, FIT_BASE_TYPE_UINT16,
            0, size_of::<u8>() as u8, FIT_BASE_TYPE_ENUM,
        ],
    };

    write_message_definition(
        file,
        0,
        as_bytes(&def),
    );

    write_message(file, 0, as_bytes(&file_id_mesg));

    Ok(())
}

#[repr(C, packed)]
#[derive(Debug, Default)]
pub struct FitDeviceInfoMesg {
    pub timestamp: u32,                // 1 * s + 0
    pub serial_number: u32,            // Zeroable integer
    pub cum_operating_time: u32,       // 1 * s + 0, Reset by new battery or charge
    pub product_name: [u8; 20], // Optional free-form string
    pub manufacturer: u16,             // Manufacturer ID
    pub product: u16,                  // Product ID
    pub software_version: u16,         // Version as 1.0 * 100
    pub battery_voltage: u16,          // 256 * V + 0
    pub ant_device_number: u32,        // ANT device number
    pub device_index: u8,              // Device index
    pub device_type: u8,               // Device type
    pub hardware_version: u8,          // Hardware version
    pub battery_status: u8,            // Battery status
    pub sensor_position: u8,           // Indicates sensor location
    pub descriptor: [u8; 20], // Sensor/location descriptor
    pub ant_transmission_type: u8,     // ANT transmission type
    pub ant_network: u8,               // ANT network type
    pub source_type: u8,               // Source type
}

#[repr(C, packed)]
struct FitDeviceInfoMesgDef {
    reserved_1: u8,
    arch: u8,
    global_mesg_num: u16,
    num_fields: u8,
    fields: [u8; 18 * 3], // Assuming FIT_FIELD_DEF_SIZE is 3
}

pub fn write_device_info_message(file: &mut File, timestamp: u32) {
    let mut device_info_mesg = FitDeviceInfoMesg {
        device_index: 1,
        manufacturer: 1,
        product: 0, // Use a unique ID for each of your products
        product_name: [0; 20], // Max 20 chars + null terminator
        serial_number: 123456,
        software_version: 100, // 1.0 * 100
        timestamp,
        cum_operating_time: 1,
        battery_voltage: 1,
        ant_device_number: 1,
        device_type: 1,
        hardware_version: 1,
        battery_status: 1,
        sensor_position: 1,
        descriptor: [0; 20],
        ant_transmission_type: 1,
        ant_network: 0,
        source_type: 0,
    };

    let def = FitDeviceInfoMesgDef {
        reserved_1: 0,
        arch: 0,
        global_mesg_num: 23,
        num_fields: 18,
        fields: [
            253, size_of::<u32>() as u8, FIT_BASE_TYPE_UINT32,
            3, size_of::<u32>() as u8, FIT_BASE_TYPE_UINT32Z,
            7, size_of::<u32>() as u8, FIT_BASE_TYPE_UINT32,
            27, 20, FIT_BASE_TYPE_STRING,
            2, size_of::<u16>() as u8, FIT_BASE_TYPE_UINT16,
            4, size_of::<u16>() as u8, FIT_BASE_TYPE_UINT16,
            5, size_of::<u16>() as u8, FIT_BASE_TYPE_UINT16,
            10, size_of::<u16>() as u8, FIT_BASE_TYPE_UINT16,
            21, size_of::<u16>() as u8, FIT_BASE_TYPE_UINT16Z,
            0, size_of::<u8>() as u8, FIT_BASE_TYPE_UINT8,
            1, size_of::<u8>() as u8, FIT_BASE_TYPE_UINT8,
            6, size_of::<u8>() as u8, FIT_BASE_TYPE_UINT8,
            11, size_of::<u8>() as u8, FIT_BASE_TYPE_UINT8,
            18, size_of::<u8>() as u8, FIT_BASE_TYPE_ENUM,
            19, 1, FIT_BASE_TYPE_STRING,
            20, size_of::<u8>() as u8, FIT_BASE_TYPE_UINT8Z,
            22, size_of::<u8>() as u8, FIT_BASE_TYPE_ENUM,
            25, size_of::<u8>() as u8, FIT_BASE_TYPE_ENUM,
        ],
    };

    let product_name = "Echo Bike";
    let name_bytes = product_name.as_bytes();
    let length = name_bytes.len().min(20);
    device_info_mesg.product_name[..length].copy_from_slice(&name_bytes[..length]);

    write_message_definition(
        file,
        0,
        as_bytes(&def),
    );

    write_message(file, 0, as_bytes(&device_info_mesg));
}

#[repr(C, packed)]
#[derive(Default)]
pub struct FITEVENTMESG {
    pub timestamp: u32,                     // FIT_DATE_TIME (32-bit integer, seconds since epoch)
    pub data: u32,                          // FIT_UINT32
    pub data16: u16,                        // FIT_UINT16
    pub score: u16,                         // FIT_UINT16, autogenerated by decoder for sport_point
    pub opponent_score: u16,                // FIT_UINT16, autogenerated by decoder for sport_point
    pub event: u8,                          // FIT_EVENT (single byte for event type)
    pub event_type: u8,                     // FIT_EVENT_TYPE (single byte for event type)
    pub event_group: u8,                    // FIT_UINT8
    pub front_gear_num: u8,         // FIT_UINT8Z, autogenerated for gear_change subfield
    pub front_gear: u8,             // FIT_UINT8Z, autogenerated for gear_change subfield
    pub rear_gear_num: u8,          // FIT_UINT8Z, autogenerated for gear_change subfield
    pub rear_gear: u8,              // FIT_UINT8Z, autogenerated for gear_change subfield
    pub radar_threat_level_max: u8, // FIT_RADAR_THREAT_LEVEL_TYPE, autogenerated for threat_alert
    pub radar_threat_count: u8,     // FIT_UINT8, autogenerated for threat_alert
}

fn write_event(file: &mut File, timestamp: u32) -> std::io::Result<()> {
    let mut event_mesg = FITEVENTMESG {
        timestamp: timestamp,
        event: 0,
        event_type: 0,
        data: 0,
        data16: 0,
        score: 0,
        opponent_score: 0,
        event_group: 0,
        front_gear_num: 0,
        front_gear: 0,
        rear_gear_num: 0,
        rear_gear: 0,
        radar_threat_level_max: 0,
        radar_threat_count: 0,
    };

    event_mesg.timestamp = timestamp;
    event_mesg.event = 0; // FIT_EVENT_TIMER
    event_mesg.event_type = 1; // FIT_EVENT_TYPE_START

    // write_message_definition(
    //     file,
    //     0,
    //     as_bytes(&event_mesg),
    // );

    // write_message(file, 0, as_bytes(&event_mesg));

    Ok(())
}

#[tokio::main]
async fn main() {
    let mut file = File::create("test.fit").unwrap();
    let timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() as u32;

    let _ = write_file_header(&mut file);
    let _ = write_file_id_message(&mut file, timestamp);
    let _ = write_device_info_message(&mut file, timestamp);
    // let _ = write_event(&mut file, timestamp);

    println!("{:?}", format!("{:X}", get_crc()));
    let _ = file.write_all(&get_crc().to_be_bytes());
    let _ = write_file_header(&mut file);

    // ---------------------------------------------
    // STOP HERE YOU FUCK

    // let manager = Manager::new().await?;

    // let adapters = manager.adapters().await?;
    // let central = adapters
    //     .into_iter()
    //     .nth(0)
    //     .expect("No Bluetooth adapters found");

    // central.start_scan(ScanFilter::default()).await?;

    // let peripherals = central.peripherals().await?;

    // for peripheral in peripherals.iter() {
    //     println!("{:?}", peripheral.properties().await?.unwrap().local_name);

    //     if peripheral.properties().await?.unwrap().local_name.is_some() {
    //         if peripheral.properties().await?.unwrap().local_name.unwrap() == "ECHO_BIKE_004130" {
    //             println!("found you");

    //             let properties = peripheral.properties().await?;
    //             let local_name = properties
    //             .unwrap()
    //             .local_name
    //             .unwrap_or(String::from("(peripheral name unknown)"));

    //             println!("Connecting to peripheral {:?}...", &local_name);
    //             if let Err(err) = peripheral.connect().await {
    //                 eprintln!("Error connecting to peripheral, skipping: {}", err);
    //                 continue;
    //             }

    //             let is_connected = peripheral.is_connected().await?;
    //             println!(
    //                 "Now connected ({:?}) to peripheral {:?}...",
    //                 is_connected, "stuff"
    //             );

    //             peripheral.discover_services().await?;
    //             println!("Discover peripheral {:?} services...", &local_name);

    //             for characteristic in peripheral.characteristics() {
    //                 if characteristic.uuid == Uuid::from_str("00002ad9-0000-1000-8000-00805f9b34fb").unwrap() {
    //                     println!("Writing");
    //                     peripheral.write(&characteristic, &vec![0x00], WriteType::WithResponse).await?;
    //                     peripheral.write(&characteristic, &vec![0x07], WriteType::WithResponse).await?;
    //                 }
    //             }

    //             for characteristic in peripheral.characteristics() {
    //                 if characteristic.uuid == Uuid::from_str("00002ad2-0000-1000-8000-00805f9b34fb").unwrap() && characteristic.properties.contains(CharPropFlags::NOTIFY)
    //                 {
    //                     println!("Subscribing to characteristic {:?}", characteristic.uuid);
    //                     peripheral.subscribe(&characteristic).await?;

    //                     let mut notification_stream =
    //                         peripheral.notifications().await?;

    //                     while let Some(x) = notification_stream.next().await {
    //                         println!("Received data from {:?} {:?}", local_name,  x.value);

    //                         let data = x.value;
    //                         let speed = u16::from_le_bytes([data[2], data[3]]) as f32 * 0.01;
    //                         // let avg_speed = u16::from_le_bytes([data[4], data[5]]) as f32 * 0.01;
    //                         let cadence = u16::from_le_bytes([data[6], data[7]]) as f32 * 0.5;
    //                         let dist = (data[10] as u32) | ((data[11] as u32) << 8) | ((data[12] as u32) << 16);
    //                         let d = dist as f32 * 0.0006213712;
    //                         let power = i16::from_le_bytes([data[13], data[14]]);                        
    //                         let time = u16::from_le_bytes([data[19], data[20]]);

    //                         println!("speed: {:?}, cad: {:?}, dist: {:?}, watts: {:?}, time: {:?}", speed, cadence, d, power, time);
    //                         tokio::signal::ctrl_c().await.expect("failed to listen for event");
    //                     }
    //                 }
    //             }
    //             println!("Disconnecting from peripheral {:?}...", local_name);
    //             peripheral.disconnect().await?;
    //         }
    //     }
    // }
}
