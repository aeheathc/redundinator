use rand::Rng;

/**
Create an exponential backoff timetable with jitter.

Create a series of times to wait between retries of a fallible external service.
All input and output times are in seconds.

# Arguments
* `initial` - The first time in the series.
* `multiplier` - How much to multiply each time by to get the next one. Must be at least 1 to ensure that it increases.
* `max_retries` - The maximum number of values in the series.
* `max_wait` - The maximum individual value. If a calculated value exceeds this, it and all values after will be clamped to the maximum
* `max_total_wait` - The maximum total of all values. If this would be exceeded, the series ends, and the last value is cut to a value which brings the total exactly to the maximum.
* `jitter_factor` - A factor which is multiplied by each value to get the maximum jitter that can be added to that value. Must be greater than 0 and should generally be less than 1.

# Returns
A Vec<f32> of times, in seconds, that you should wait between each retry of whatever you're doing.

# Examples
```
use redundinator::backoff::calculate_backoff_series;

fn main()
{
    let mut backoff = calculate_backoff_series(0.5, 1.5, 10, 60.0, 600.0, 0.5);
    backoff.push(0.0);
    let mut result: Result<(),DothingError> = Result::Ok(());
    for time in backoff
    {
        result = do_thing();
        match &result
        {
            Ok(()) => {break;},
            Err(e) => {
                if *e == DothingError::Permanent {break;}
            }
        }
        sleep(time);
    }
    match result
    {
        Ok(()) => {println!("Dothing succeeded");},
        Err(e) => {
            match e
            {
                DothingError::Permanent => {println!("Dothing failed on unretryable error");},
                DothingError::Transient => {println!("Dothing retries exceeded");},
            }
        }
    }
}

fn do_thing() -> Result<(),DothingError>
{
    Ok(())
}
fn sleep(time: f32) {println!("fake sleeping for {}s", time);}

#[derive(PartialEq)]
enum DothingError
{
    Permanent,
    Transient
}
```
*/
pub fn calculate_backoff_series(initial: f32, multiplier: f32, max_retries: usize, max_wait: f32, max_total_wait: f32, jitter_factor: f32) -> Vec<f32>
{
    let initial = f32::max(initial, 0.1);
    let multiplier = f32::max(multiplier, 1.0);
    let max_wait = f32::max(max_wait, 0.1);
    let max_total_wait = f32::max(max_total_wait, 0.1);
    let jitter_factor = f32::max(jitter_factor, 0.0);

    let mut rng = rand::thread_rng();
    let mut series = Vec::new();
    let mut current = initial;
    let mut total: f32 = 0.0;
    loop {
        let next_total = total + current;
        if next_total > max_total_wait
        {
            series.push(max_total_wait - total);
            break;
        }
        series.push(current);
        total = next_total;
        if series.len() >= max_retries {break;}

        current *= multiplier;
        let rand_jitter_scale: f32 = rng.gen(); // a float between 0 and 1
        current += current * jitter_factor * rand_jitter_scale;
        current = f32::min(current, max_wait);
    }
    series
}

#[cfg(test)]
mod tests
{
    use super::*;

	#[test]
	fn max_retries()
	{
        let backoff = calculate_backoff_series(1.0, 2.0, 10, 600.0, 6000.0, 0.0);
        assert_eq!(backoff, vec!(1.0, 2.0, 4.0, 8.0, 16.0, 32.0, 64.0, 128.0, 256.0, 512.0));
    }

    #[test]
	fn max_wait()
	{
        let backoff = calculate_backoff_series(1.0, 2.0, 10, 60.0, 600.0, 0.0);
        assert_eq!(backoff, vec!(1.0, 2.0, 4.0, 8.0, 16.0, 32.0, 60.0, 60.0, 60.0, 60.0));
    }

	#[test]
	fn max_total_wait()
	{
        let backoff = calculate_backoff_series(1.0, 2.0, 10, 60.0, 100.0, 0.0);
        assert_eq!(backoff, vec!(1.0, 2.0, 4.0, 8.0, 16.0, 32.0, 37.0));
    }

	#[test]
	fn jitter()
	{
        for _ in 0..10
        {
            let backoff_min: f32 = calculate_backoff_series(1.0, 2.0, 5, 600.0, 6000.0, 0.0).iter().sum();
            let backoff_jitter: f32 = calculate_backoff_series(1.0, 2.0, 5, 600.0, 6000.0, 1.0).iter().sum();
            let backoff_max: f32 = vec!(1.0, 4.0, 16.0, 64.0, 256.0).iter().sum();
            assert!(backoff_min <= backoff_jitter && backoff_jitter <= backoff_max);
        }
    }
}