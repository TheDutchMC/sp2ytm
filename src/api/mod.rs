pub mod spotify;
pub mod youtube;

/// Call the provided function `f` as soon as the ratelimit it allows.
/// This macro call blocks until the ratelimit bucket `bucket` permits the request
#[macro_export]
macro_rules! try_rl {
    ($bucket:expr, $f:expr) => {
        {
            use ::ratelimit_meter::NonConformance;

            let mut bucket = $bucket.lock().expect("Failed to lock RL Bucket mutex");

            'rt_bucket: loop {
                match bucket.check() {
                    Ok(_) => {
                        break 'rt_bucket $f
                    },
                    Err(e) => {
                        let earliest = e.earliest_possible();
                        let sleep_for = earliest - ::std::time::Instant::now();
                        ::log::debug!("Bucket is full. Sleeping for {} miliseconds", sleep_for.as_millis());
                        std::thread::sleep(sleep_for);
                        continue 'rt_bucket;
                    }
                }
            }
        }
    }
}