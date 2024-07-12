use anyhow::{anyhow, Result};
use chrono::DateTime;
use chrono::Datelike;
use chrono::Duration;
use chrono::Local;
use chrono::NaiveDate;
use chrono::Timelike;
use chrono::Utc;
use const_env::from_env;
use holochain_types::dna::{ActionHashB64, AgentPubKey, DnaHashB64};
use holochain_types::prelude::YamlProperties;
use holochain_types::prelude::{holochain_serial, SerializedBytes};
use serde::Deserialize;
use serde::Serialize;

// General Notes:
// These constants are defined here for use by all the other repos
// that depend on service-logger function.  They should only be overridden
// globally in holo-nixpkgs if the system wide determination is made to
// change the default values.
// The primary place that uses these values is hpos-api-rust, but other
// repos may also consume them.gs

// Time-bucket Notes:
// Currently the time-bucket size (SL_BUCKET_SIZE_DAYS) is a implemented as a global constant,
// that should apply for all service-logger instances, the sl-dna is built to be future-proofed
// and thus stores the value in its properties.  This is so that different apps could have different
// log rotation schedules.  But that is not currently implemented in the `sl-check` endpoint of
// `hpos-api-rust`.  But this is why all of these functions take the `days_in_bucket`` parameter
// rather than using the global directly.
#[from_env]
pub const SL_BUCKET_SIZE_DAYS: u32 = 14;
#[from_env]
pub const SL_MINUTES_BEFORE_BUCKET_TO_CLONE: i64 = 9;
#[from_env]
pub const SL_DELETING_LOG_WINDOW_SIZE_MINUTES: u32 = 10;
#[from_env]
pub const HOLO_EPOCH_YEAR: u16 = 2024;

/// This is the structure of the DNA properties for service logger instances
/// This struct must be kept up to date with the service-logger implementation
#[derive(Serialize, Deserialize, Debug, SerializedBytes)]
pub struct SlDnaProperties {
    pub cron_schedule: Option<String>,
    pub bound_happ_id: Option<ActionHashB64>,
    pub bound_hf_dna: Option<DnaHashB64>,
    pub bound_hha_dna: Option<DnaHashB64>,
    pub holo_admin: Option<AgentPubKey>,
    pub bucket_size: Option<u32>,
    pub time_bucket: Option<u32>,
}

pub fn sl_serialized_props(props: &SlDnaProperties) -> YamlProperties {
    YamlProperties::new(
        serde_yaml::to_value(props)
        .expect("should serialize"))
}

/// given the date in UTC timezone, return the current bucket
pub fn time_bucket_from_date(date: DateTime<Utc>, days_in_bucket: u32) -> u32 {
    let epoch_start = NaiveDate::from_ymd_opt(HOLO_EPOCH_YEAR.into(), 1, 1).unwrap();
    let days_since_epoch: u32 = (date.num_days_from_ce() - epoch_start.num_days_from_ce())
        .try_into()
        .expect("now should always be after Holo epoch");
    days_since_epoch / days_in_bucket
}

/// returns the current time bucket in a deterministic way so that all code elements
/// that rely on logging can know the service logger instance with which they should be
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
/// (this fn is used to check for cloning new service loggers)
/// note that `days_in_bucket` and `minutes_before` are passed in as parameters by
/// the caller as part of future-proofing, but currently they all just use the constants
/// provided above.
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

/// returns all the buckets that are inside the range of the `days` param
pub fn sl_get_bucket_range(bucket_size: u32, days: u32) -> (u32, u32) {
    let time_bucket: u32 = sl_get_current_time_bucket(bucket_size);
    let buckets_for_days_in_request = days / bucket_size;
    (time_bucket, buckets_for_days_in_request)
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

/// creates the clone name from the bucket and bucket size
pub fn sl_clone_name(spec: SlCloneSpec) -> String {
    format!("{}.{}", spec.days_in_bucket, spec.time_bucket)
}

pub struct SlCloneSpec {
    pub days_in_bucket: u32,
    pub time_bucket: u32,
}

/// returns the bucket size and bucket from a clone name
pub fn sl_clone_name_spec(name: &str) -> Result<SlCloneSpec> {
    let mut parts = name.split(".");
    let days_in_bucket = parts
        .next()
        .ok_or(anyhow!("no days in clone name"))?
        .parse::<u32>()?;
    let time_bucket = parts
        .next()
        .ok_or(anyhow!("no bucket in clone name"))?
        .parse::<u32>()?;
    Ok(SlCloneSpec {
        days_in_bucket,
        time_bucket,
    })
}
