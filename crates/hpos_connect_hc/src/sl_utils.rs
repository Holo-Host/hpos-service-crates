use chrono::Datelike;
use chrono::NaiveDate;
use chrono::Utc;
use chrono::Timelike;
use holochain_types::prelude::ClonedCell;

pub const SL_BUCKET_SIZE_DAYS: u32 = 14;
pub const HOLO_EPOCH_YEAR: u16 = 2024;

pub fn sl_get_current_time_bucket(days_in_bucket: u32) -> u32 {
    let now_utc = Utc::now();
    if std::env::var("IS_TEST_ENV").is_ok() {
        10
    } else {
        let epoch_start = NaiveDate::from_ymd_opt(HOLO_EPOCH_YEAR.into(), 1, 1).unwrap();
        let days_since_epoch : u32 = (now_utc.num_days_from_ce()-epoch_start.num_days_from_ce()).try_into().expect("now should always be after Holo epoch");
        days_since_epoch/days_in_bucket
    }
}


pub fn sl_get_bucket_range(_clone_cells: Vec<ClonedCell>, days: u32) -> (u32, u32, u32){
    let bucket_size = SL_BUCKET_SIZE_DAYS; // TODO: get this from: clone_cells[0].dna_modifiers.properties;
    let time_bucket: u32 = sl_get_current_time_bucket(bucket_size);
    let buckets_for_days_in_request = days/bucket_size;
    (bucket_size, time_bucket, buckets_for_days_in_request)
}