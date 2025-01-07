use std::slice;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use lazy_static::lazy_static;
use std::fs::File;
use std::io::{Seek, SeekFrom, Write};

const BASE_TYPE_UINT32: u8 = 0x86; 
const BASE_TYPE_UINT32Z: u8 = 0x8C; 
const BASE_TYPE_STRING: u8 = 0x07;
const BASE_TYPE_UINT16: u8 = 0x84; 
const BASE_TYPE_UINT8: u8 = 0x02;
const BASE_TYPE_UINT8Z: u8 = 0x0A;
const BASE_TYPE_UINT16Z: u8 = 0x8B; 
const BASE_TYPE_ENUM: u8 = 0x00;
const BASE_TYPE_BYTE: u8 = 0x0D;
const BASE_TYPE_SINT32: u8 = 0x85;
const BASE_TYPE_SINT16: u8 = 0x83;
const BASE_TYPE_SINT8: u8 = 0x01;

lazy_static! {
    static ref DATA_CRC: Arc<Mutex<u16>> = Arc::new(Mutex::new(0)); 
}
#[repr(C, packed)]
struct FILEHDR {
    header_size: u8,        // 1 byte
    protocol_version: u8,   // 1 byte
    profile_version: u16,   // 2 bytes
    data_size: u32,         // 4 bytes
    data_type: [u8; 4],     // 4 bytes
    crc: u16,               // 2 bytes
}

fn write_file_header(file: &mut File) {
    const HEADERSIZE: u32 = 14;
    let mut file_header: FILEHDR = FILEHDR {
        header_size: HEADERSIZE as u8,
        protocol_version: 2,
        profile_version: 21158 as u16,
        data_size: 0,
        data_type: *b".FIT",
        crc: 0,
    };
    
    file.seek(SeekFrom::End(0)).unwrap();
    let file_size = file.stream_position().unwrap();

    file_header.data_size = (HEADERSIZE as u64 - size_of::<u16>() as u64) as u32;

    if file_size < HEADERSIZE as u64 {
        file_header.data_size = (HEADERSIZE as u64 - size_of::<u16>() as u64) as u32;
    } else {
        file_header.data_size = (file_size - HEADERSIZE as u64 - size_of::<u16>() as u64) as u32;
    }

    let mut buffer: Vec<u8> = vec![];
    buffer.push(file_header.header_size);
    buffer.push(file_header.protocol_version);
    buffer.extend_from_slice(&file_header.profile_version.to_le_bytes());
    buffer.extend_from_slice(&file_header.data_size.to_le_bytes());
    buffer.extend_from_slice(&file_header.data_type);

    file_header.crc = crc_calc16(&buffer);

    file.seek(SeekFrom::Start(0)).unwrap();
    file.write_all(as_bytes(&file_header)).unwrap();
}

fn write_message_definition(file: &mut File, local_mesg_number: u8, mesg_def: &[u8]) {
    let header = local_mesg_number | 0x40;
    write_data(file, &[header], 1);
    write_data(file, mesg_def, mesg_def.len());
}

fn write_message(file: &mut File, local_mesg_number: u8, message: &[u8]) {
    write_data(file, &[local_mesg_number], 1);
    write_data(file, message, message.len());
}

fn write_data(file: &mut File, data: &[u8], data_size: usize) {
    file.write_all(&data[..data_size]).unwrap();

    for &byte in &data[..data_size] {
        let current_crc = get_crc();
        let updated_crc = crc_get16(current_crc, byte);
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
    unsafe { slice::from_raw_parts((data as *const T) as *const u8, size_of::<T>()) }
}

fn crc_get16(mut crc: u16, byte: u8) -> u16 {
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

fn crc_update16(mut crc: u16, data: &[u8]) -> u16 {
    for &byte in data {
        crc = crc_get16(crc, byte);
    }
    crc
}

fn crc_calc16(data: &[u8]) -> u16 {
    crc_update16(0, data)
}

#[repr(C, packed)]
#[derive(Default, Debug)]
struct FileIdMesg {
    pub serial_number: u32,                  // UINT32Z
    pub time_created: u32,                   // DATE_TIME (assumed to be a 32-bit integer)
    pub product_name: [u8; 20], // STRING (array of bytes)
    pub manufacturer: u16,                   // MANUFACTURER
    pub product: u16,                        // UINT16
    pub number: u16,                         // UINT16
    pub type_: u8,                       // FILE
}

#[repr(C, packed)]
struct FileIdMesgDef {
    reserved_1: u8,
    arch: u8,
    global_mesg_num: u16,
    num_fields: u8,
    fields: [u8; 7 * 3], 
}

fn write_file_id_message(file: &mut File) -> std::io::Result<()> {
    let file_id_mesg = FileIdMesg { 
        serial_number: 3469062800,
        time_created: get_timestamp(),
        product_name: [b'E', b'c', b'h', b'o', 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        manufacturer: 1,
        product: 4376,
        number: 0,
        type_: 4,
    };

    let def = FileIdMesgDef {
        reserved_1: 0,
        arch: 0,
        global_mesg_num: 0,
        num_fields: 7,
        fields: [
            3, size_of::<u32>() as u8, BASE_TYPE_UINT32Z,
            4, size_of::<u32>() as u8, BASE_TYPE_UINT32,
            8, 20, BASE_TYPE_STRING,
            1, size_of::<u16>() as u8, BASE_TYPE_UINT16,
            2, size_of::<u16>() as u8, BASE_TYPE_UINT16,
            5, size_of::<u16>() as u8, BASE_TYPE_UINT16,
            0, size_of::<u8>() as u8, BASE_TYPE_ENUM,
        ],
    };

    write_message_definition(file, 0, as_bytes(&def));
    write_message(file, 0, as_bytes(&file_id_mesg));

    Ok(())
}

#[repr(C, packed)]
#[derive(Debug, Default)]
pub struct DeviceInfoMesg {
    pub timestamp: u32,                // 1 * s + 0
    pub serial_number: u32,            // Zeroable integer
    pub cum_operating_time: u32,       // 1 * s + 0, Reset by new battery or charge
    pub product_name: [u8; 20], // Optional free-form string
    pub manufacturer: u16,             // Manufacturer ID
    pub product: u16,                  // Product ID
    pub software_version: u16,         // Version as 1.0 * 100
    pub battery_voltage: u16,          // 256 * V + 0
    pub ant_device_number: u16,        // ANT device number
    pub device_index: u8,              // Device index
    pub device_type: u8,               // Device type
    pub hardware_version: u8,          // Hardware version
    pub battery_status: u8,            // Battery status
    pub sensor_position: u8,           // Indicates sensor location
    pub descriptor: [u8; 1], // Sensor/location descriptor
    pub ant_transmission_type: u8,     // ANT transmission type
    pub ant_network: u8,               // ANT network type
    pub source_type: u8,               // Source type
}

#[repr(C, packed)]
struct DeviceInfoMesgDef {
    reserved_1: u8,
    arch: u8,
    global_mesg_num: u16,
    num_fields: u8,
    fields: [u8; 18 * 3], // Assuming FIELD_DEF_SIZE is 3
}

pub fn write_device_info_message(file: &mut File) {
    let mut device_info_mesg = DeviceInfoMesg {
        device_index: 1,
        manufacturer: 2,
        product: 0, // Use a unique ID for each of your products
        product_name: [0; 20], // Max 20 chars + null terminator
        serial_number: 123456,
        software_version: 100, // 1.0 * 100
        timestamp: get_timestamp(),
        cum_operating_time: 1,
        battery_voltage: 1,
        ant_device_number: 1,
        device_type: 1,
        hardware_version: 1,
        battery_status: 1,
        sensor_position: 1,
        descriptor: [0; 1],
        ant_transmission_type: 1,
        ant_network: 0,
        source_type: 0,
    };

    let def = DeviceInfoMesgDef {
        reserved_1: 0,
        arch: 0,
        global_mesg_num: 23,
        num_fields: 18,
        fields: [
            253, size_of::<u32>() as u8, BASE_TYPE_UINT32,
            3, size_of::<u32>() as u8, BASE_TYPE_UINT32Z,
            7, size_of::<u32>() as u8, BASE_TYPE_UINT32,
            27, 20, BASE_TYPE_STRING,
            2, size_of::<u16>() as u8, BASE_TYPE_UINT16,
            4, size_of::<u16>() as u8, BASE_TYPE_UINT16,
            5, size_of::<u16>() as u8, BASE_TYPE_UINT16,
            10, size_of::<u16>() as u8, BASE_TYPE_UINT16,
            21, size_of::<u16>() as u8, BASE_TYPE_UINT16Z,
            0, size_of::<u8>() as u8, BASE_TYPE_UINT8,
            1, size_of::<u8>() as u8, BASE_TYPE_UINT8,
            6, size_of::<u8>() as u8, BASE_TYPE_UINT8,
            11, size_of::<u8>() as u8, BASE_TYPE_UINT8,
            18, size_of::<u8>() as u8, BASE_TYPE_ENUM,
            19, 1, BASE_TYPE_STRING,
            20, size_of::<u8>() as u8, BASE_TYPE_UINT8Z,
            22, size_of::<u8>() as u8, BASE_TYPE_ENUM,
            25, size_of::<u8>() as u8, BASE_TYPE_ENUM,
        ],
    };

    let product_name = "Echo Bike".as_bytes();
    let length = product_name.len().min(20);
    device_info_mesg.product_name[..length].copy_from_slice(&product_name[..length]);

    write_message_definition(file, 0, as_bytes(&def));

    write_message(file, 0, as_bytes(&device_info_mesg));
}

#[repr(C, packed)]
#[derive(Default)]
pub struct EventMesg {
    pub timestamp: u32,                     // DATE_TIME (32-bit integer, seconds since epoch)
    pub data: u32,                          // UINT32
    pub data16: u16,                        // UINT16
    pub score: u16,                         // UINT16, autogenerated by decoder for sport_point
    pub opponent_score: u16,                // UINT16, autogenerated by decoder for sport_point
    pub event: u8,                          // EVENT (single byte for event type)
    pub event_type: u8,                     // EVENT_TYPE (single byte for event type)
    pub event_group: u8,                    // UINT8
    pub front_gear_num: u8,         // UINT8Z, autogenerated for gear_change subfield
    pub front_gear: u8,             // UINT8Z, autogenerated for gear_change subfield
    pub rear_gear_num: u8,          // UINT8Z, autogenerated for gear_change subfield
    pub rear_gear: u8,              // UINT8Z, autogenerated for gear_change subfield
    pub radar_threat_level_max: u8, // RADAR_THREAT_LEVEL_TYPE, autogenerated for threat_alert
    pub radar_threat_count: u8,     // UINT8, autogenerated for threat_alert
}
#[repr(C, packed)]
struct EventMesgDef {
    reserved_1: u8,
    arch: u8,
    global_mesg_num: u16,
    num_fields: u8,
    fields: [u8; 14 * 3], // Assuming FIELD_DEF_SIZE is 3
}

fn write_start_event(file: &mut File) -> std::io::Result<()> {
    let mut event_mesg = EventMesg {
        timestamp: get_timestamp(),
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
        radar_threat_count: 1,
    };

    event_mesg.timestamp = get_timestamp();
    event_mesg.event = 0; // EVENT_TIMER
    event_mesg.event_type = 0; // EVENT_TYPE_START

    let def = EventMesgDef { 
        reserved_1: 0,
        arch: 0, 
        global_mesg_num: 21, 
        num_fields: 14, 
        fields: [
            253, size_of::<u32>() as u8, BASE_TYPE_UINT32,
            3, size_of::<u32>() as u8, BASE_TYPE_UINT32,
            2, size_of::<u16>() as u8, BASE_TYPE_UINT16,
            7, size_of::<u16>() as u8, BASE_TYPE_UINT16,
            8, size_of::<u16>() as u8, BASE_TYPE_UINT16,
            0, size_of::<u8>() as u8, BASE_TYPE_ENUM,
            1, size_of::<u8>() as u8, BASE_TYPE_ENUM,
            4, size_of::<u8>() as u8, BASE_TYPE_UINT8,
            9, size_of::<u8>() as u8, BASE_TYPE_UINT8Z,
            10, size_of::<u8>() as u8, BASE_TYPE_UINT8Z,
            11, size_of::<u8>() as u8, BASE_TYPE_UINT8Z,
            12, size_of::<u8>() as u8, BASE_TYPE_UINT8Z,
            21, size_of::<u8>() as u8, BASE_TYPE_ENUM,
            22, size_of::<u8>() as u8, BASE_TYPE_UINT8,
        ],
    };

    write_message_definition(file, 0, as_bytes(&def));

    write_message(file, 0, as_bytes(&event_mesg));

    Ok(())
}

fn write_stop_event(file: &mut File) -> std::io::Result<()> {
    let mut event_mesg = EventMesg {
        timestamp: get_timestamp(),
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
        radar_threat_count: 1,
    };

    event_mesg.timestamp = get_timestamp() + 50;
    event_mesg.event = 0; // EVENT_TIMER
    event_mesg.event_type = 1; // EVENT_TYPE_START

    let def = EventMesgDef { 
        reserved_1: 0,
        arch: 0, 
        global_mesg_num: 21, 
        num_fields: 14, 
        fields: [
            253, size_of::<u32>() as u8, BASE_TYPE_UINT32,
            3, size_of::<u32>() as u8, BASE_TYPE_UINT32,
            2, size_of::<u16>() as u8, BASE_TYPE_UINT16,
            7, size_of::<u16>() as u8, BASE_TYPE_UINT16,
            8, size_of::<u16>() as u8, BASE_TYPE_UINT16,
            0, size_of::<u8>() as u8, BASE_TYPE_ENUM,
            1, size_of::<u8>() as u8, BASE_TYPE_ENUM,
            4, size_of::<u8>() as u8, BASE_TYPE_UINT8,
            9, size_of::<u8>() as u8, BASE_TYPE_UINT8Z,
            10, size_of::<u8>() as u8, BASE_TYPE_UINT8Z,
            11, size_of::<u8>() as u8, BASE_TYPE_UINT8Z,
            12, size_of::<u8>() as u8, BASE_TYPE_UINT8Z,
            21, size_of::<u8>() as u8, BASE_TYPE_ENUM,
            22, size_of::<u8>() as u8, BASE_TYPE_UINT8,
        ],
    };

    write_message_definition(file, 0, as_bytes(&def));
    write_message(file, 0, as_bytes(&event_mesg));

    Ok(())
}

#[repr(C, packed)]
pub struct RecordMesg {
    pub timestamp: u32, // DATE_TIME: 1 * s + 0
    pub position_lat: i32, // SINT32: 1 * semicircles + 0
    pub position_long: i32, // SINT32: 1 * semicircles + 0
    pub distance: u32, // UINT32: 100 * m + 0
    pub time_from_course: i32, // SINT32: 1000 * s + 0
    pub total_cycles: u32, // UINT32: 1 * cycles + 0
    pub accumulated_power: u32, // UINT32: 1 * watts + 0
    pub enhanced_speed: u32, // UINT32: 1000 * m/s + 0
    pub enhanced_altitude: u32, // UINT32: 5 * m + 500
    pub altitude: u16, // UINT16: 5 * m + 500
    pub speed: u16, // UINT16: 1000 * m/s + 0
    pub power: u16, // UINT16: 1 * watts + 0
    pub grade: i16, // SINT16: 100 * % + 0
    pub compressed_accumulated_power: u16, // UINT16: 1 * watts + 0
    pub vertical_speed: i16, // SINT16: 1000 * m/s + 0
    pub calories: u16, // UINT16: 1 * kcal + 0
    pub vertical_oscillation: u16, // UINT16: 10 * mm + 0
    pub stance_time_percent: u16, // UINT16: 100 * percent + 0
    pub stance_time: u16, // UINT16: 10 * ms + 0
    pub ball_speed: u16, // UINT16: 100 * m/s + 0
    pub cadence256: u16, // UINT16: 256 * rpm + 0
    pub total_hemoglobin_conc: u16, // UINT16: 100 * g/dL + 0
    pub total_hemoglobin_conc_min: u16, // UINT16: 100 * g/dL + 0
    pub total_hemoglobin_conc_max: u16, // UINT16: 100 * g/dL + 0
    pub saturated_hemoglobin_percent: u16, // UINT16: 10 * % + 0
    pub saturated_hemoglobin_percent_min: u16, // UINT16: 10 * % + 0
    pub saturated_hemoglobin_percent_max: u16, // UINT16: 10 * % + 0
    pub heart_rate: u8, // UINT8: 1 * bpm + 0
    pub cadence: u8, // UINT8: 1 * rpm + 0
    pub compressed_speed_distance: [u8; 3], // Replace with actual count
    pub resistance: u8, // Relative: 0 is none, 254 is Max
    pub cycle_length: u8, // UINT8: 100 * m + 0
    pub temperature: i8, // SINT8: 1 * C + 0
    pub speed_1s: [u8; 5], // Replace with actual count
    pub cycles: u8, // UINT8: 1 * cycles + 0
    pub left_right_balance: u8, // Replace with the actual enum or struct
    pub gps_accuracy: u8, // UINT8: 1 * m + 0
    pub activity_type: u8, // Replace with the actual enum or struct
    pub left_torque_effectiveness: u8, // UINT8: 2 * percent + 0
    pub right_torque_effectiveness: u8, // UINT8: 2 * percent + 0
    pub left_pedal_smoothness: u8, // UINT8: 2 * percent + 0
    pub right_pedal_smoothness: u8, // UINT8: 2 * percent + 0
    pub combined_pedal_smoothness: u8, // UINT8: 2 * percent + 0
    pub time128: u8, // UINT8: 128 * s + 0
    pub stroke_type: u8, // Replace with the actual enum or struct
    pub zone: u8, // UINT8
    pub fractional_cadence: u8, // UINT8: 128 * rpm + 0
    pub device_index: u8, // Replace with the actual enum or struct
}

#[repr(C, packed)]
struct RecordMesgDef {
    reserved_1: u8,
    arch: u8,
    global_mesg_num: u16,
    num_fields: u8,
    fields: [u8; 48 * 3], // Assuming FIELD_DEF_SIZE is 3
}

fn write_record(file: &mut File) {
    let record_mesg = RecordMesg {
        timestamp: get_timestamp(),
        position_lat: 0,
        position_long: 0,
        distance: 0,
        time_from_course: 0,
        total_cycles: 0,
        accumulated_power: 0,
        enhanced_speed: 0,
        enhanced_altitude: 0,
        altitude: 0,
        speed: 0,
        power: 0,
        grade: 0,
        compressed_accumulated_power: 0,
        vertical_speed: 0,
        calories: 0,
        vertical_oscillation: 0,
        stance_time_percent: 0,
        stance_time: 0,
        ball_speed: 0,
        cadence256: 0,
        total_hemoglobin_conc: 0,
        total_hemoglobin_conc_min: 0,
        total_hemoglobin_conc_max: 0,
        saturated_hemoglobin_percent: 0,
        saturated_hemoglobin_percent_min: 0,
        saturated_hemoglobin_percent_max: 0,
        heart_rate: 0,
        cadence: 0,
        compressed_speed_distance: [0; 3],
        resistance: 0,
        cycle_length: 0,
        temperature: 0,
        speed_1s: [0; 5],
        cycles: 0,
        left_right_balance: 0,
        gps_accuracy: 0,
        activity_type: 0,
        left_torque_effectiveness: 0,
        right_torque_effectiveness: 0,
        left_pedal_smoothness: 0,
        right_pedal_smoothness: 0,
        combined_pedal_smoothness: 0,
        time128: 0,
        stroke_type: 0,
        zone: 0,
        fractional_cadence: 0,
        device_index: 0,
    };

    let def = RecordMesgDef { 
        reserved_1: 0,
        arch: 0, 
        global_mesg_num: 20, 
        num_fields: 48, 
        fields: [
            253, size_of::<u32>() as u8, BASE_TYPE_UINT32,
            0, size_of::<i32>() as u8, BASE_TYPE_SINT32,
            1, size_of::<i32>() as u8, BASE_TYPE_SINT32,
            5, size_of::<u32>() as u8, BASE_TYPE_UINT32,
            11, size_of::<i32>() as u8, BASE_TYPE_SINT32,
            19, size_of::<u32>() as u8, BASE_TYPE_UINT32,
            29, size_of::<u32>() as u8, BASE_TYPE_UINT32,
            73, size_of::<u32>() as u8, BASE_TYPE_UINT32,
            78, size_of::<u32>() as u8, BASE_TYPE_UINT32,
            2, size_of::<u16>() as u8, BASE_TYPE_UINT16,
            6, size_of::<u16>() as u8, BASE_TYPE_UINT16,
            7, size_of::<u16>() as u8, BASE_TYPE_UINT16,
            9, size_of::<i16>() as u8, BASE_TYPE_SINT16,
            28, size_of::<u16>() as u8, BASE_TYPE_UINT16,
            32, size_of::<i16>() as u8, BASE_TYPE_SINT16,
            33, size_of::<u16>() as u8, BASE_TYPE_UINT16,
            39, size_of::<u16>() as u8, BASE_TYPE_UINT16,
            40, size_of::<u16>() as u8, BASE_TYPE_UINT16,
            41, size_of::<u16>() as u8, BASE_TYPE_UINT16,
            51, size_of::<u16>() as u8, BASE_TYPE_UINT16,
            52, size_of::<u16>() as u8, BASE_TYPE_UINT16,
            54, size_of::<u16>() as u8, BASE_TYPE_UINT16,
            55, size_of::<u16>() as u8, BASE_TYPE_UINT16,
            56, size_of::<u16>() as u8, BASE_TYPE_UINT16,
            57, size_of::<u16>() as u8, BASE_TYPE_UINT16,
            58, size_of::<u16>() as u8, BASE_TYPE_UINT16,
            59, size_of::<u16>() as u8, BASE_TYPE_UINT16,
            3, size_of::<u8>() as u8, BASE_TYPE_UINT8,
            4, size_of::<u8>() as u8, BASE_TYPE_UINT8,
            8, (size_of::<u8>() * 3) as u8, BASE_TYPE_BYTE,
            10, size_of::<u8>() as u8, BASE_TYPE_UINT8,
            12, size_of::<u8>() as u8, BASE_TYPE_UINT8,
            13, size_of::<i8>() as u8, BASE_TYPE_SINT8,
            17, (size_of::<u8>() * 5) as u8, BASE_TYPE_UINT8,
            18, size_of::<u8>() as u8, BASE_TYPE_UINT8,
            30, size_of::<u8>() as u8, BASE_TYPE_UINT8,
            31, size_of::<u8>() as u8, BASE_TYPE_UINT8,
            42, size_of::<u8>() as u8, BASE_TYPE_ENUM,
            43, size_of::<u8>() as u8, BASE_TYPE_UINT8,
            44, size_of::<u8>() as u8, BASE_TYPE_UINT8,
            45, size_of::<u8>() as u8, BASE_TYPE_UINT8,
            46, size_of::<u8>() as u8, BASE_TYPE_UINT8,
            47, size_of::<u8>() as u8, BASE_TYPE_UINT8,
            48, size_of::<u8>() as u8, BASE_TYPE_UINT8,
            49, size_of::<u8>() as u8, BASE_TYPE_ENUM,
            50, size_of::<u8>() as u8, BASE_TYPE_UINT8,
            53, size_of::<u8>() as u8, BASE_TYPE_UINT8,
            62, size_of::<u8>() as u8, BASE_TYPE_UINT8,
        ],
    };

    write_message_definition(file, 0, as_bytes(&def));
    write_message(file, 0, as_bytes(&record_mesg));
}

#[repr(C, packed)]
struct LapMesg {
    timestamp: u32, // 1 * s + 0, Lap end time.
    start_time: u32, //
    start_position_lat: i32, // 1 * semicircles + 0,
    start_position_long: i32, // 1 * semicircles + 0,
    end_position_lat: i32, // 1 * semicircles + 0,
    end_position_long: i32, // 1 * semicircles + 0,
    total_elapsed_time: u32, // 1000 * s + 0, Time (includes pauses)
    total_timer_time: u32, // 1000 * s + 0, Timer Time (excludes pauses)
    total_distance: u32, // 100 * m + 0,
    total_cycles: u32, // 1 * cycles + 0,
    total_work: u32, // 1 * J + 0,
    total_moving_time: u32, // 1000 * s + 0,
    time_in_hr_zone: u32,// 1000 * s + 0,
    time_in_speed_zone: u32, // 1000 * s + 0,
    time_in_cadence_zone: u32, // 1000 * s + 0,
    time_in_power_zone: u32, // 1000 * s + 0,
    enhanced_avg_speed: u32, // 1000 * m/s + 0,
    enhanced_max_speed: u32, // 1000 * m/s + 0,
    enhanced_avg_altitude: u32, // 5 * m + 500,
    enhanced_min_altitude: u32, // 5 * m + 500,
    enhanced_max_altitude: u32, // 5 * m + 500,
    message_index: u16, //
    total_calories: u16, // 1 * kcal + 0,
    total_fat_calories: u16, // 1 * kcal + 0, If New Leaf
    avg_speed: u16, // 1000 * m/s + 0,
    max_speed: u16, // 1000 * m/s + 0,
    avg_power: u16, // 1 * watts + 0, total_power / total_timer_time if non_zero_avg_power otherwise total_power / total_elapsed_time
    max_power: u16, // 1 * watts + 0,
    total_ascent: u16, // 1 * m + 0,
    total_descent: u16, // 1 * m + 0,
    num_lengths: u16, // 1 * lengths + 0, # of lengths of swim pool
    normalized_power: u16, // 1 * watts + 0,
    left_right_balance: u16, //
    first_length_index: u16, //
    avg_stroke_distance: u16, // 100 * m + 0,
    num_active_lengths: u16, // 1 * lengths + 0, # of active lengths of swim pool
    avg_altitude: u16, // 5 * m + 500,
    max_altitude: u16, // 5 * m + 500,
    avg_grade: i16, // 100 * % + 0,
    avg_pos_grade: i16, // 100 * % + 0,
    avg_neg_grade: i16, // 100 * % + 0,
    max_pos_grade: i16, // 100 * % + 0,
    max_neg_grade: i16, // 100 * % + 0,
    avg_pos_vertical_speed: i16, // 1000 * m/s + 0,
    avg_neg_vertical_speed: i16, // 1000 * m/s + 0,
    max_pos_vertical_speed: i16, // 1000 * m/s + 0,
    max_neg_vertical_speed: i16, // 1000 * m/s + 0,
    repetition_num: u16, //
    min_altitude: u16, // 5 * m + 500,
    wkt_step_index: u16, //
    opponent_score: u16, //
    stroke_count: u16, // 1 * counts + 0, stroke_type enum used as the index
    zone_count: u16, // 1 * counts + 0, zone number used as the index
    avg_vertical_oscillation: u16, // 10 * mm + 0,
    avg_stance_time_percent: u16, // 100 * percent + 0,
    avg_stance_time: u16, // 10 * ms + 0,
    player_score: u16, //
    avg_total_hemoglobin_conc: u16, // 100 * g/dL + 0, Avg saturated and unsaturated hemoglobin
    min_total_hemoglobin_conc: u16, // 100 * g/dL + 0, Min saturated and unsaturated hemoglobin
    max_total_hemoglobin_conc: u16, // 100 * g/dL + 0, Max saturated and unsaturated hemoglobin
    avg_saturated_hemoglobin_percent: u16, // 10 * % + 0, Avg percentage of hemoglobin saturated with oxygen
    min_saturated_hemoglobin_percent: u16,// 10 * % + 0, Min percentage of hemoglobin saturated with oxygen
    max_saturated_hemoglobin_percent: u16, // 10 * % + 0, Max percentage of hemoglobin saturated with oxygen
    avg_vam: u16,// 1000 * m/s + 0,
    event: u8, //
    event_type: u8, //
    avg_heart_rate: u8, // 1 * bpm + 0,
    max_heart_rate: u8, // 1 * bpm + 0,
    avg_cadence: u8, // 1 * rpm + 0, total_cycles / total_timer_time if non_zero_avg_cadence otherwise total_cycles / total_elapsed_time
    max_cadence: u8,// 1 * rpm + 0,
    intensity: u8, //
    lap_trigger: u8, //
    sport: u8, //
    event_group: u8, //
    swim_stroke: u8, //
    sub_sport: u8, //
    gps_accuracy: u8, // 1 * m + 0,
    avg_temperature: i8, // 1 * C + 0,
    max_temperature: i8, // 1 * C + 0,
    min_heart_rate: u8, // 1 * bpm + 0,
    avg_fractional_cadence: u8, // 128 * rpm + 0, fractional part of the avg_cadence
    max_fractional_cadence: u8, // 128 * rpm + 0, fractional part of the max_cadence
    total_fractional_cycles: u8, // 128 * cycles + 0, fractional part of the total_cycles
    min_temperature: i8, // 1 * C + 0,
}

#[repr(C, packed)]
struct LogDefMesg {
    reserved_1: u8,
    arch: u8,
    global_mesg_num: u16,
    num_fields: u8,
    fields: [u8; 84 * 3], // Assuming FIELD_DEF_SIZE is 3
}

fn write_lap(file: &mut File) {
    let start_time = get_timestamp();
    let lap_mesg = LapMesg {
        timestamp: get_timestamp(),
        start_time,
        start_position_lat: 0,
        start_position_long: 0,
        end_position_lat: 0,
        end_position_long: 0,
        total_elapsed_time: 0,
        total_timer_time: 0,
        total_distance: 0,
        total_cycles: 0,
        total_work: 0,
        total_moving_time: 0,
        time_in_hr_zone: 0,
        time_in_speed_zone: 0,
        time_in_cadence_zone: 0,
        time_in_power_zone: 0,
        enhanced_avg_speed: 0,
        enhanced_max_speed: 0,
        enhanced_avg_altitude: 0,
        enhanced_min_altitude: 0,
        enhanced_max_altitude: 0,
        message_index: 0,
        total_calories: 0,
        total_fat_calories: 0,
        avg_speed: 0,
        max_speed: 0,
        avg_power: 0,
        max_power: 0,
        total_ascent: 0,
        total_descent: 0,
        num_lengths: 0,
        normalized_power: 0,
        left_right_balance: 0,
        first_length_index: 0,
        avg_stroke_distance: 0,
        num_active_lengths: 0,
        avg_altitude: 0,
        max_altitude: 0,
        avg_grade: 0,
        avg_pos_grade: 0,
        avg_neg_grade: 0,
        max_pos_grade: 0,
        max_neg_grade: 0,
        avg_pos_vertical_speed: 0,
        avg_neg_vertical_speed: 0,
        max_pos_vertical_speed: 0,
        max_neg_vertical_speed: 0,
        repetition_num: 0,
        min_altitude: 0,
        wkt_step_index: 0,
        opponent_score: 0,
        stroke_count: 0,
        zone_count: 0,
        avg_vertical_oscillation: 0,
        avg_stance_time_percent: 0,
        avg_stance_time: 0,
        player_score: 0,
        avg_total_hemoglobin_conc: 0,
        min_total_hemoglobin_conc: 0,
        max_total_hemoglobin_conc: 0,
        avg_saturated_hemoglobin_percent: 0,
        min_saturated_hemoglobin_percent: 0,
        max_saturated_hemoglobin_percent: 0,
        avg_vam: 0,
        event: 0,
        event_type: 0,
        avg_heart_rate: 0,
        max_heart_rate: 0,
        avg_cadence: 0,
        max_cadence: 0,
        intensity: 0,
        lap_trigger: 0,
        sport: 0,
        event_group: 0,
        swim_stroke: 0,
        sub_sport: 0,
        gps_accuracy: 0,
        avg_temperature: 0,
        max_temperature: 0,
        min_heart_rate: 0,
        avg_fractional_cadence: 0,
        max_fractional_cadence: 0,
        total_fractional_cycles: 0,
        min_temperature: 0,
    };

    let def = LogDefMesg { 
        reserved_1: 0,
        arch: 0, 
        global_mesg_num: 19, 
        num_fields: 84, 
        fields: [
            253, size_of::<u32>() as u8, BASE_TYPE_UINT32,
            2, size_of::<u32>() as u8, BASE_TYPE_UINT32,
            3, size_of::<i32>() as u8, BASE_TYPE_SINT32,
            4, size_of::<i32>() as u8, BASE_TYPE_SINT32,
            5, size_of::<i32>() as u8, BASE_TYPE_SINT32,
            6, size_of::<i32>() as u8, BASE_TYPE_SINT32,
            7, size_of::<u32>() as u8, BASE_TYPE_UINT32,
            8, size_of::<u32>() as u8, BASE_TYPE_UINT32,
            9, size_of::<u32>() as u8, BASE_TYPE_UINT32,
            10, size_of::<u32>() as u8, BASE_TYPE_UINT32,
            41, size_of::<u32>() as u8, BASE_TYPE_UINT32,
            52, size_of::<u32>() as u8, BASE_TYPE_UINT32,
            57, size_of::<u32>() as u8, BASE_TYPE_UINT32,
            58, size_of::<u32>() as u8, BASE_TYPE_UINT32,
            59, size_of::<u32>() as u8, BASE_TYPE_UINT32,
            60, size_of::<u32>() as u8, BASE_TYPE_UINT32,
            110, size_of::<u32>() as u8, BASE_TYPE_UINT32,
            111, size_of::<u32>() as u8, BASE_TYPE_UINT32,
            112, size_of::<u32>() as u8, BASE_TYPE_UINT32,
            113, size_of::<u32>() as u8, BASE_TYPE_UINT32,
            114, size_of::<u32>() as u8, BASE_TYPE_UINT32,
            254, size_of::<u16>() as u8, BASE_TYPE_UINT16,
            11, size_of::<u16>() as u8, BASE_TYPE_UINT16,
            12, size_of::<u16>() as u8, BASE_TYPE_UINT16,
            13, size_of::<u16>() as u8, BASE_TYPE_UINT16,
            14, size_of::<u16>() as u8, BASE_TYPE_UINT16,
            19, size_of::<u16>() as u8, BASE_TYPE_UINT16,
            20, size_of::<u16>() as u8, BASE_TYPE_UINT16,
            21, size_of::<u16>() as u8, BASE_TYPE_UINT16,
            22, size_of::<u16>() as u8, BASE_TYPE_UINT16,
            32, size_of::<u16>() as u8, BASE_TYPE_UINT16,
            33, size_of::<u16>() as u8, BASE_TYPE_UINT16,
            34, size_of::<u16>() as u8, BASE_TYPE_UINT16,
            35, size_of::<u16>() as u8, BASE_TYPE_UINT16,
            37, size_of::<u16>() as u8, BASE_TYPE_UINT16,
            40, size_of::<u16>() as u8, BASE_TYPE_UINT16,
            42, size_of::<u16>() as u8, BASE_TYPE_UINT16,
            43, size_of::<u16>() as u8, BASE_TYPE_UINT16,
            45, size_of::<i16>() as u8, BASE_TYPE_SINT16,
            46, size_of::<i16>() as u8, BASE_TYPE_SINT16,
            47, size_of::<i16>() as u8, BASE_TYPE_SINT16,
            48, size_of::<i16>() as u8, BASE_TYPE_SINT16,
            49, size_of::<i16>() as u8, BASE_TYPE_SINT16,
            53, size_of::<i16>() as u8, BASE_TYPE_SINT16,
            54, size_of::<i16>() as u8, BASE_TYPE_SINT16,
            55, size_of::<i16>() as u8, BASE_TYPE_SINT16,
            56, size_of::<i16>() as u8, BASE_TYPE_SINT16,
            61, size_of::<u16>() as u8, BASE_TYPE_UINT16,
            62, size_of::<u16>() as u8, BASE_TYPE_UINT16,
            71, size_of::<u16>() as u8, BASE_TYPE_UINT16,
            74, size_of::<u16>() as u8, BASE_TYPE_UINT16,
            75, size_of::<u16>() as u8, BASE_TYPE_UINT16,
            76, size_of::<u16>() as u8, BASE_TYPE_UINT16,
            77, size_of::<u16>() as u8, BASE_TYPE_UINT16,
            78, size_of::<u16>() as u8, BASE_TYPE_UINT16,
            79, size_of::<u16>() as u8, BASE_TYPE_UINT16,
            83, size_of::<u16>() as u8, BASE_TYPE_UINT16,
            84, size_of::<u16>() as u8, BASE_TYPE_UINT16,
            85, size_of::<u16>() as u8, BASE_TYPE_UINT16,
            86, size_of::<u16>() as u8, BASE_TYPE_UINT16,
            87, size_of::<u16>() as u8, BASE_TYPE_UINT16,
            88, size_of::<u16>() as u8, BASE_TYPE_UINT16,
            89, size_of::<u16>() as u8, BASE_TYPE_UINT16,
            121, size_of::<u16>() as u8, BASE_TYPE_UINT16,
            0, size_of::<u8>() as u8, BASE_TYPE_ENUM,
            1, size_of::<u8>() as u8, BASE_TYPE_ENUM,
            15, size_of::<u8>() as u8, BASE_TYPE_UINT8,
            16, size_of::<u8>() as u8, BASE_TYPE_UINT8,
            17, size_of::<u8>() as u8, BASE_TYPE_UINT8,
            18, size_of::<u8>() as u8, BASE_TYPE_UINT8,
            23, size_of::<u8>() as u8, BASE_TYPE_ENUM,
            24, size_of::<u8>() as u8, BASE_TYPE_ENUM,
            25, size_of::<u8>() as u8, BASE_TYPE_ENUM,
            26, size_of::<u8>() as u8, BASE_TYPE_UINT8,
            38, size_of::<u8>() as u8, BASE_TYPE_ENUM,
            39, size_of::<u8>() as u8, BASE_TYPE_ENUM,
            44, size_of::<u8>() as u8, BASE_TYPE_UINT8,
            50, size_of::<i8>() as u8, BASE_TYPE_SINT8,
            51, size_of::<i8>() as u8, BASE_TYPE_SINT8,
            63, size_of::<u8>() as u8, BASE_TYPE_UINT8,
            80, size_of::<u8>() as u8, BASE_TYPE_UINT8,
            81, size_of::<u8>() as u8, BASE_TYPE_UINT8,
            82, size_of::<u8>() as u8, BASE_TYPE_UINT8,
            124, size_of::<i8>() as u8, BASE_TYPE_SINT8,
        ],
    };

    write_message_definition(file, 0, as_bytes(&def));
    write_message(file, 0, as_bytes(&lap_mesg));
}
#[repr(C, packed)]
struct SessionMesg {  
    timestamp: u32, // 1 * s + 0, Sesson end time.
    start_time: u32, //
    start_position_lat: i32, // 1 * semicircles + 0,
    start_position_long: i32, // 1 * semicircles + 0,
    total_elapsed_time: u32, // 1000 * s + 0, Time (includes pauses)
    total_timer_time: u32, // 1000 * s + 0, Timer Time (excludes pauses)
    total_distance: u32, // 100 * m + 0,
    total_cycles: u32,// 1 * cycles + 0,
    nec_lat: i32, // 1 * semicircles + 0, North east corner latitude
    nec_long: i32, // 1 * semicircles + 0, North east corner longitude
    swc_lat: i32, // 1 * semicircles + 0, South west corner latitude
    swc_long: i32, // 1 * semicircles + 0, South west corner longitude
    end_position_lat: i32, // 1 * semicircles + 0,
    end_position_long: i32, // 1 * semicircles + 0,
    avg_stroke_count: u32, // 10 * strokes/lap + 0,
    total_work: u32, // 1 * J + 0,
    total_moving_time: u32, // 1000 * s + 0,
    time_in_hr_zone: u32, // 1000 * s + 0,
    time_in_speed_zone: u32, // 1000 * s + 0,
    time_in_cadence_zone: u32, // 1000 * s + 0,
    time_in_power_zone: u32, // 1000 * s + 0,
    avg_lap_time: u32, // 1000 * s + 0,
    sport_profile_name: [u8; 16],  // Sport name from associated sport mesg
    enhanced_avg_speed: u32, // 1000 * m/s + 0, total_distance / total_timer_time
    enhanced_max_speed: u32, // 1000 * m/s + 0,
    enhanced_avg_altitude: u32, // 5 * m + 500,
    enhanced_min_altitude: u32, // 5 * m + 500,
    enhanced_max_altitude: u32, // 5 * m + 500,
    message_index: u16,// Selected bit is set for the current session.
    total_calories: u16,// 1 * kcal + 0,
    total_fat_calories: u16, // 1 * kcal + 0,
    avg_speed: u16, // 1000 * m/s + 0, total_distance / total_timer_time
    max_speed: u16,// 1000 * m/s + 0,
    avg_power: u16, // 1 * watts + 0, total_power / total_timer_time if non_zero_avg_power otherwise total_power / total_elapsed_time
    max_power: u16,// 1 * watts + 0,
    total_ascent: u16, // 1 * m + 0,
    total_descent: u16, // 1 * m + 0,
    first_lap_index: u16, //
    num_laps: u16, //
    num_lengths: u16, // 1 * lengths + 0, # of lengths of swim pool
    normalized_power: u16, // 1 * watts + 0,
    training_stress_score: u16, // 10 * tss + 0,
    intensity_factor: u16, // 1000 * if + 0,
    left_right_balance: u16, //
    avg_stroke_distance: u16, // 100 * m + 0,
    pool_length: u16, // 100 * m + 0,
    threshold_power: u16, // 1 * watts + 0,
    num_active_lengths: u16, // 1 * lengths + 0, # of active lengths of swim pool
    avg_altitude: u16, // 5 * m + 500,
    max_altitude: u16, // 5 * m + 500,
    avg_grade: i16, // 100 * % + 0,
    avg_pos_grade: i16, // 100 * % + 0,
    avg_neg_grade: i16, // 100 * % + 0,
    max_pos_grade: i16, // 100 * % + 0,
    max_neg_grade: i16, // 100 * % + 0,
    avg_pos_vertical_speed: i16, // 1000 * m/s + 0,
    avg_neg_vertical_speed: i16, // 1000 * m/s + 0,
    max_pos_vertical_speed: i16, // 1000 * m/s + 0,
    max_neg_vertical_speed: i16, // 1000 * m/s + 0,
    best_lap_index: u16, //
    min_altitude: u16, // 5 * m + 500,
    player_score: u16, //
    opponent_score: u16, //
    stroke_count: u16, // 1 * counts + 0, stroke_type enum used as the index
    zone_count: u16, // 1 * counts + 0, zone number used as the index
    max_ball_speed: u16, // 100 * m/s + 0,
    avg_ball_speed: u16, // 100 * m/s + 0,
    avg_vertical_oscillation: u16, // 10 * mm + 0,
    avg_stance_time_percent: u16, // 100 * percent + 0,
    avg_stance_time: u16, // 10 * ms + 0,
    avg_vam: u16, // 1000 * m/s + 0,
    event: u8, // session
    event_type: u8, // stop
    sport: u8, //
    sub_sport: u8, //
    avg_heart_rate: u8, // 1 * bpm + 0, average heart rate (excludes pause time)
    max_heart_rate: u8, // 1 * bpm + 0,
    avg_cadence: u8, // 1 * rpm + 0, total_cycles / total_timer_time if non_zero_avg_cadence otherwise total_cycles / total_elapsed_time
    max_cadence: u8, // 1 * rpm + 0,
    total_training_effect: u8, //
    event_group: u8, //
    trigger: u8, //
    swim_stroke: u8, // 1 * swim_stroke + 0,
    pool_length_unit: u8, //
    gps_accuracy: u8, // 1 * m + 0,
    avg_temperature: i8, // 1 * C + 0,
    max_temperature: i8, // 1 * C + 0,
    min_heart_rate: u8, // 1 * bpm + 0,
    opponent_name: [u8; 1], //
    avg_fractional_cadence: u8, // 128 * rpm + 0, fractional part of the avg_cadence
    max_fractional_cadence: u8, // 128 * rpm + 0, fractional part of the max_cadence
    total_fractional_cycles: u8, // 128 * cycles + 0, fractional part of the total_cycles
    sport_index: u8, //
    total_anaerobic_training_effect: u8, //
    min_temperature: i8, // 1 * C + 0,
}
#[repr(C, packed)]
struct SessionMesgDef {
    reserved_1: u8,
    arch: u8,
    global_mesg_num: u16,
    num_fields: u8,
    fields: [u8; 95 * 3],
}

fn write_session(file: &mut File) {
    let start_time = get_timestamp();
    let mut session_mesg = SessionMesg {
        timestamp: get_timestamp(),
        start_time: start_time,
        start_position_lat: 0,
        start_position_long: 0,
        total_elapsed_time: 0,
        total_timer_time: 0,
        total_distance: 0,
        total_cycles: 0,
        nec_lat: 0,
        nec_long: 0,
        swc_lat: 0,
        swc_long: 0,
        end_position_lat: 0,
        end_position_long: 0,
        avg_stroke_count: 0,
        total_work: 0,
        total_moving_time: 0,
        time_in_hr_zone: 0,
        time_in_speed_zone: 0,
        time_in_cadence_zone: 0,
        time_in_power_zone: 0,
        avg_lap_time: 0,
        sport_profile_name: [0; 16],
        enhanced_avg_speed: 0,
        enhanced_max_speed: 0,
        enhanced_avg_altitude: 0,
        enhanced_min_altitude: 0,
        enhanced_max_altitude: 0,
        message_index: 0,
        total_calories: 0,
        total_fat_calories: 0,
        avg_speed: 0,
        max_speed: 0,
        avg_power: 0,
        max_power: 0,
        total_ascent: 0,
        total_descent: 0,
        first_lap_index: 0,
        num_laps: 0,
        num_lengths: 0,
        normalized_power: 0,
        training_stress_score: 0,
        intensity_factor: 0,
        left_right_balance: 0,
        avg_stroke_distance: 0,
        pool_length: 0,
        threshold_power: 0,
        num_active_lengths: 0,
        avg_altitude: 0,
        max_altitude: 0,
        avg_grade: 0,
        avg_pos_grade: 0,
        avg_neg_grade: 0,
        max_pos_grade: 0,
        max_neg_grade: 0,
        avg_pos_vertical_speed: 0,
        avg_neg_vertical_speed: 0,
        max_pos_vertical_speed: 0,
        max_neg_vertical_speed: 0,
        best_lap_index: 0,
        min_altitude: 0,
        player_score: 0,
        opponent_score: 0,
        stroke_count: 0,
        zone_count: 0,
        max_ball_speed: 0,
        avg_ball_speed: 0,
        avg_vertical_oscillation: 0,
        avg_stance_time_percent: 0,
        avg_stance_time: 0,
        avg_vam: 0,
        event: 0,
        event_type: 0,
        sport: 0,
        sub_sport: 0,
        avg_heart_rate: 0,
        max_heart_rate: 0,
        avg_cadence: 0,
        max_cadence: 0,
        total_training_effect: 0,
        event_group: 0,
        trigger: 0,
        swim_stroke: 0,
        pool_length_unit: 0,
        gps_accuracy: 0,
        avg_temperature: 0,
        max_temperature: 0,
        min_heart_rate: 0,
        opponent_name: [0; 1],
        avg_fractional_cadence: 0,
        max_fractional_cadence: 0,
        total_fractional_cycles: 0,
        sport_index: 0,
        total_anaerobic_training_effect: 0,
        min_temperature: 0,
    };

    session_mesg.total_elapsed_time = (get_timestamp() - start_time) * 1000;
    session_mesg.total_timer_time = (get_timestamp() - start_time) * 1000;
    session_mesg.sport = 0;
    session_mesg.sub_sport = 0;
    session_mesg.first_lap_index = 0;
    session_mesg.num_laps = 1;

    let def = SessionMesgDef { 
        reserved_1: 0,
        arch: 0, 
        global_mesg_num: 18, 
        num_fields: 95, 
        fields: [
            253, size_of::<u32>() as u8, BASE_TYPE_UINT32,
            2, size_of::<u32>() as u8, BASE_TYPE_UINT32,
            3, size_of::<i32>() as u8, BASE_TYPE_SINT32,
            4, size_of::<i32>() as u8, BASE_TYPE_SINT32,
            7, size_of::<u32>() as u8, BASE_TYPE_UINT32,
            8, size_of::<u32>() as u8, BASE_TYPE_UINT32,
            9, size_of::<u32>() as u8, BASE_TYPE_UINT32,
            10, size_of::<u32>() as u8, BASE_TYPE_UINT32,
            29, size_of::<i32>() as u8, BASE_TYPE_SINT32,
            30, size_of::<i32>() as u8, BASE_TYPE_SINT32,
            31, size_of::<i32>() as u8, BASE_TYPE_SINT32,
            32, size_of::<i32>() as u8, BASE_TYPE_SINT32,
            38, size_of::<i32>() as u8, BASE_TYPE_SINT32,
            39, size_of::<i32>() as u8, BASE_TYPE_SINT32,
            41, size_of::<u32>() as u8, BASE_TYPE_UINT32,
            48, size_of::<u32>() as u8, BASE_TYPE_UINT32,
            59, size_of::<u32>() as u8, BASE_TYPE_UINT32,
            65, size_of::<u32>() as u8, BASE_TYPE_UINT32,
            66, size_of::<u32>() as u8, BASE_TYPE_UINT32,
            67, size_of::<u32>() as u8, BASE_TYPE_UINT32,
            68, size_of::<u32>() as u8, BASE_TYPE_UINT32,
            69, size_of::<u32>() as u8, BASE_TYPE_UINT32,
            110, 16, BASE_TYPE_STRING,
            124, size_of::<u32>() as u8, BASE_TYPE_UINT32,
            125, size_of::<u32>() as u8, BASE_TYPE_UINT32,
            126, size_of::<u32>() as u8, BASE_TYPE_UINT32,
            127, size_of::<u32>() as u8, BASE_TYPE_UINT32,
            128, size_of::<u32>() as u8, BASE_TYPE_UINT32,
            254, size_of::<u16>() as u8, BASE_TYPE_UINT16,
            11, size_of::<u16>() as u8, BASE_TYPE_UINT16,
            13, size_of::<u16>() as u8, BASE_TYPE_UINT16,
            14, size_of::<u16>() as u8, BASE_TYPE_UINT16,
            15, size_of::<u16>() as u8, BASE_TYPE_UINT16,
            20, size_of::<u16>() as u8, BASE_TYPE_UINT16,
            21, size_of::<u16>() as u8, BASE_TYPE_UINT16,
            22, size_of::<u16>() as u8, BASE_TYPE_UINT16,
            23, size_of::<u16>() as u8, BASE_TYPE_UINT16,
            25, size_of::<u16>() as u8, BASE_TYPE_UINT16,
            26, size_of::<u16>() as u8, BASE_TYPE_UINT16,
            33, size_of::<u16>() as u8, BASE_TYPE_UINT16,
            34, size_of::<u16>() as u8, BASE_TYPE_UINT16,
            35, size_of::<u16>() as u8, BASE_TYPE_UINT16,
            36, size_of::<u16>() as u8, BASE_TYPE_UINT16,
            37, size_of::<u16>() as u8, BASE_TYPE_UINT16,
            42, size_of::<u16>() as u8, BASE_TYPE_UINT16,
            44, size_of::<u16>() as u8, BASE_TYPE_UINT16,
            45, size_of::<u16>() as u8, BASE_TYPE_UINT16,
            47, size_of::<u16>() as u8, BASE_TYPE_UINT16,
            49, size_of::<u16>() as u8, BASE_TYPE_UINT16,
            50, size_of::<u16>() as u8, BASE_TYPE_UINT16,
            52, size_of::<i16>() as u8, BASE_TYPE_SINT16,
            53, size_of::<i16>() as u8, BASE_TYPE_SINT16,
            54, size_of::<i16>() as u8, BASE_TYPE_SINT16,
            55, size_of::<i16>() as u8, BASE_TYPE_SINT16,
            56, size_of::<i16>() as u8, BASE_TYPE_SINT16,
            60, size_of::<i16>() as u8, BASE_TYPE_SINT16,
            61, size_of::<i16>() as u8, BASE_TYPE_SINT16,
            62, size_of::<i16>() as u8, BASE_TYPE_SINT16,
            63, size_of::<i16>() as u8, BASE_TYPE_SINT16,
            70, size_of::<u16>() as u8, BASE_TYPE_UINT16,
            71, size_of::<u16>() as u8, BASE_TYPE_UINT16,
            82, size_of::<u16>() as u8, BASE_TYPE_UINT16,
            83, size_of::<u16>() as u8, BASE_TYPE_UINT16,
            85, size_of::<u16>() as u8, BASE_TYPE_UINT16,
            86, size_of::<u16>() as u8, BASE_TYPE_UINT16,
            87, size_of::<u16>() as u8, BASE_TYPE_UINT16,
            88, size_of::<u16>() as u8, BASE_TYPE_UINT16,
            89, size_of::<u16>() as u8, BASE_TYPE_UINT16,
            90, size_of::<u16>() as u8, BASE_TYPE_UINT16,
            91, size_of::<u16>() as u8, BASE_TYPE_UINT16,
            139, size_of::<u16>() as u8, BASE_TYPE_UINT16,
            0, size_of::<u8>() as u8, BASE_TYPE_ENUM,
            1, size_of::<u8>() as u8, BASE_TYPE_ENUM,
            5, size_of::<u8>() as u8, BASE_TYPE_ENUM,
            6, size_of::<u8>() as u8, BASE_TYPE_ENUM,
            16, size_of::<u8>() as u8, BASE_TYPE_UINT8,
            17, size_of::<u8>() as u8, BASE_TYPE_UINT8,
            18, size_of::<u8>() as u8, BASE_TYPE_UINT8,
            19, size_of::<u8>() as u8, BASE_TYPE_UINT8,
            24, size_of::<u8>() as u8, BASE_TYPE_UINT8,
            27, size_of::<u8>() as u8, BASE_TYPE_UINT8,
            28, size_of::<u8>() as u8, BASE_TYPE_ENUM,
            43, size_of::<u8>() as u8, BASE_TYPE_ENUM,
            46, size_of::<u8>() as u8, BASE_TYPE_ENUM,
            51, size_of::<u8>() as u8, BASE_TYPE_UINT8,
            57, size_of::<i8>() as u8, BASE_TYPE_SINT8,
            58, size_of::<i8>() as u8, BASE_TYPE_SINT8,
            64, size_of::<u8>() as u8, BASE_TYPE_UINT8,
            84, 1, BASE_TYPE_STRING,
            92, size_of::<u8>() as u8, BASE_TYPE_UINT8,
            93, size_of::<u8>() as u8, BASE_TYPE_UINT8,
            94, size_of::<u8>() as u8, BASE_TYPE_UINT8,
            111, size_of::<u8>() as u8, BASE_TYPE_UINT8,
            137, size_of::<u8>() as u8, BASE_TYPE_UINT8,
            150, size_of::<i8>() as u8, BASE_TYPE_SINT8,
        ],
    };

    write_message_definition(file, 0, as_bytes(&def));
    write_message(file, 0, as_bytes(&session_mesg));
} 

#[repr(C, packed)]
struct ActivityMesg {
    timestamp: u32, //
    total_timer_time: u32, // 1000 * s + 0, Exclude pauses
    local_timestamp: u32, // timestamp epoch expressed in local time, used to convert activity timestamps to local time
    num_sessions: u16, //
    _type: u8, //
    event: u8, //
    event_type: u8, //
    event_group: u8, //
}

#[repr(C, packed)]
struct ActivityDefMesg {
    reserved_1: u8,
    arch: u8,
    global_mesg_num: u16,
    num_fields: u8,
    fields: [u8; 8 * 3],
}

fn write_activity(file: &mut File) {
    let act_mesg = ActivityMesg {
        timestamp: get_timestamp(),
        total_timer_time: 0,
        local_timestamp: get_timestamp(),
        num_sessions: 0,
        _type: 0,
        event: 0,
        event_type: 0,
        event_group: 0,
    };

    let def = ActivityDefMesg { 
        reserved_1: 0,
        arch: 0, 
        global_mesg_num: 34, 
        num_fields: 8, 
        fields: [
            253, size_of::<u32>() as u8, BASE_TYPE_UINT32,
            0, size_of::<u32>() as u8, BASE_TYPE_UINT32,
            5, size_of::<u32>() as u8, BASE_TYPE_UINT32,
            1, size_of::<u16>() as u8, BASE_TYPE_UINT16,
            2, size_of::<u8>() as u8, BASE_TYPE_ENUM,
            3, size_of::<u8>() as u8, BASE_TYPE_ENUM,
            4, size_of::<u8>() as u8, BASE_TYPE_ENUM,
            6, size_of::<u8>() as u8, BASE_TYPE_UINT8,
        ],
    };

    write_message_definition(file, 0, as_bytes(&def));
    write_message(file, 0, as_bytes(&act_mesg));
}

fn get_timestamp() -> u32 {
    const GARMINEPOCH: u32 = 631065600;
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs().saturating_sub(GARMINEPOCH as u64) as u32
}

fn main() {
    let mut file = File::create("test.fit").unwrap();

    let _ = write_file_header(&mut file);
    let _ = write_file_id_message(&mut file);
    let _ = write_device_info_message(&mut file);
    let _ = write_start_event(&mut file);
    let _ = write_record(&mut file);
    let _ = write_stop_event(&mut file);
    let _ = write_lap(&mut file);
    let _ = write_session(&mut file);
    let _ = write_activity(&mut file);

    let _ = file.write_all(&get_crc().to_le_bytes());
    let _ = write_file_header(&mut file);
}
