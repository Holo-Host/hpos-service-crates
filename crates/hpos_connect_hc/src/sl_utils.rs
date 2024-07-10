use chrono::DateTime;
use chrono::Datelike;
use chrono::Duration;
use chrono::Local;
use chrono::NaiveDate;
use chrono::Timelike;
use chrono::Utc;
use const_env::from_env;
use holochain_types::prelude::ClonedCell;

#[from_env]
pub const SL_BUCKET_SIZE_DAYS: u32 = 14;
#[from_env]
pub const SL_MINUTES_BEFORE_BUCKET_TO_CLONE: i64 = 9;
#[from_env]
pub const SL_DELETING_LOG_WINDOW_SIZE_MINUTES: u32 = 10;
#[from_env]
pub const HOLO_EPOCH_YEAR: u16 = 2024;

/// given the date in UTC timezone, return the current bucket
pub fn time_bucket_from_date(date: DateTime<Utc>, days_in_bucket: u32) -> u32 {
    let epoch_start = NaiveDate::from_ymd_opt(HOLO_EPOCH_YEAR.into(), 1, 1).unwrap();
    let days_since_epoch: u32 = (date.num_days_from_ce() - epoch_start.num_days_from_ce())
        .try_into()
        .expect("now should always be after Holo epoch");
    days_since_epoch / days_in_bucket
}

/// returns the current time bucket in a deterministic way so that all code elements
/// that rely on logging can know which service logger instance they should be
/// interacting
pub fn sl_get_current_time_bucket(days_in_bucket: u32) -> u32 {
    let now_utc = Utc::now();
    if std::env::var("IS_TEST_ENV").is_ok() {
        if let Ok(test_time_bucket_str) = std::env::var("SL_TEST_TIME_BUCKET") {
            test_time_bucket_str
                .parse::<u32>()
                .expect("wanted an int for SL_TEST_TIME_BUCKET")
        } else {
            10
        }
    } else {
        time_bucket_from_date(now_utc, days_in_bucket)
    }
}

/// returns whether we are within `minutes_before` minutes of the next time bucket
/// (used to check for cloning new service loggers)
pub fn sl_within_min_of_next_time_bucket(days_in_bucket: u32, minutes_before: i64) -> bool {
    if let Ok(val) = std::env::var("SL_TEST_IS_BEFORE_NEXT_BUCKET") {
        if val == "true" {
            return true;
        }
    }
    let now_utc = Utc::now();
    let current_time_bucket = time_bucket_from_date(now_utc, days_in_bucket);
    let time_bucket_soon =
        time_bucket_from_date(now_utc + Duration::minutes(minutes_before), days_in_bucket);
    current_time_bucket != time_bucket_soon
}

/// returns all the buckets that are indide the range of the `days` param
pub fn sl_get_bucket_range(_clone_cells: Vec<ClonedCell>, days: u32) -> (u32, u32, u32) {
    let bucket_size = SL_BUCKET_SIZE_DAYS; // TODO: get this from: clone_cells[0].dna_modifiers.properties;
    let time_bucket: u32 = sl_get_current_time_bucket(bucket_size);
    let buckets_for_days_in_request = days / bucket_size;
    (bucket_size, time_bucket, buckets_for_days_in_request)
}

/// returns whether the local time is within the deleting window which is`windows_size`` min after midnight.
pub fn sl_within_deleting_check_window(window_size: u32) -> bool {
    if let Ok(val) = std::env::var("SL_TEST_IS_IN_DELETING_WINDOW") {
        if val == "true" {
            return true;
        }
    }
    let now = Local::now();
    let min = now.minute();
    now.hour() == 0 && min >= 1 && min <= window_size
}
