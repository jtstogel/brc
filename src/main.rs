#![feature(hint_prefetch)]

use brc::memops::memchr64_unchecked;
use brc::station_map::StationNameKey;
use brc::station_map::StationNameKeyView;
use brc::station_map::new_station_map;
use cmov::Cmov;
use memmap2::MmapOptions;
use std::usize;
use std::{cmp::Ordering, fmt::Display, fs::File, process::ExitCode};

use brc::error::{BrcError, BrcResult};
use brc::temperature_summary::TemperatureSummary;
use clap::Parser;
use itertools::Itertools;

pub struct WeatherStation {
    name: String,
    summary: TemperatureSummary,
}

impl PartialEq for WeatherStation {
    fn eq(&self, other: &Self) -> bool {
        self.name.eq(&other.name)
    }
}

impl Eq for WeatherStation {}

impl PartialOrd for WeatherStation {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for WeatherStation {
    fn cmp(&self, other: &Self) -> Ordering {
        self.name.cmp(&other.name)
    }
}

struct FloatAsIntEn1(i32);

impl Display for FloatAsIntEn1 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let a = self.0.abs() / 10;
        let b = self.0.abs() % 10;
        let sign = if self.0 < 0 { "-" } else { "" };
        write!(f, "{}{}.{}", sign, a.abs(), b)
    }
}

impl Display for WeatherStation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}={}/{:.01}/{}",
            self.name,
            FloatAsIntEn1(self.summary.min()),
            self.summary.avg(),
            FloatAsIntEn1(self.summary.max())
        )
    }
}

fn digit_to_i32(d: u8) -> i32 {
    d.wrapping_sub(b'0') as i32
}

/// Parses a float of the form ;[-][d]d.d from the end of a string.
#[cfg_attr(feature = "profiled", inline(never))]
fn parse_temperature(line: &[u8]) -> i32 {
    let p = line.as_ptr().wrapping_add(line.len() - 1);

    // options: ;-99.9 or ;-9.9 or ;99.9 or ;9.9
    //               ^        ^        ^       ^
    let c0 = unsafe { *p };

    // options: ;-99.9 or ;-9.9 or ;99.9 or ;9.9
    //             ^        ^        ^       ^
    let c1 = unsafe { *p.sub(2) };

    // options: ;-99.9 or ;-9.9 or ;99.9 or ;9.9
    //            ^        ^        ^       ^
    let c2 = unsafe { *p.sub(3) };

    // options: ;-99.9 or ;-9.9 or ;99.9 or ;9.9
    //           ^        ^        ^       ^
    let c3 = unsafe { *p.sub(4) };

    let is_two_digits = (c2 == b';') | (c2 == b'-');
    let is_negative = (c3 == b'-') | (c2 == b'-');

    let mut hundreds = digit_to_i32(c2);
    hundreds.cmovnz(&0, is_two_digits as u8);

    let mut result = 10 * (10 * hundreds + digit_to_i32(c1)) + digit_to_i32(c0);
    let negative_result = -result;
    result.cmovnz(&negative_result, is_negative as u8);

    result
}

enum IterationControl {
    Continue,
}

#[cfg_attr(feature = "profiled", inline(never))]
fn batched_process_lines<const N: usize, F>(file: File, mut callback: F) -> BrcResult<()>
where
    F: FnMut(&[&[u8]]) -> IterationControl,
{
    let mmap = unsafe { MmapOptions::new().map(&file)? };
    mmap.advise(memmap2::Advice::Sequential)?;

    let mut cursor: usize = 0;
    // Handle the boundary condition of the last bytes separately.
    let mmap_boundary = mmap.len() & !1023usize;
    while cursor < mmap_boundary {
        let mut slices: [&[u8]; N] = [&[]; N];

        for i in 0..N {
            let newline_idx = unsafe { memchr64_unchecked::<b'\n'>(&mmap.get_unchecked(cursor..)) };
            slices[i] = unsafe { &mmap.get_unchecked(cursor..cursor + newline_idx) };
            cursor += newline_idx + 1;
        }

        callback(&slices);
    }

    // Deal with boundary condition at end of mmap'd region.
    while cursor < mmap.len() {
        let remaining = unsafe { mmap.get_unchecked(cursor..) };
        let mut data = [0; 64];
        let remaining_with_safe_boundary = &mut data[..remaining.len().min(64)];
        (remaining_with_safe_boundary).copy_from_slice(&remaining[..remaining.len().min(64)]);

        let newline_idx = unsafe { memchr64_unchecked::<b'\n'>(remaining_with_safe_boundary) };
        let slices: [&[u8]; 1] =
            [unsafe { remaining_with_safe_boundary.get_unchecked(..newline_idx) }];
        callback(&slices);
        cursor += newline_idx + 1;
    }

    Ok(())
}

#[cfg_attr(feature = "profiled", inline(never))]
pub fn temperature_reading_summaries(
    input_path: &str,
) -> BrcResult<impl Iterator<Item = WeatherStation>> {
    let file = File::open(input_path)
        .map_err(|err| BrcError::new(format!("Failed to open {input_path}: {err}")))?;

    let mut temperatures = new_station_map::<TemperatureSummary>(12_500);

    batched_process_lines::<4, _>(file, |lines: &[&[u8]]| {
        if lines.len() == 4 {
            let l0 = lines[0];
            let l1 = lines[1];
            let l2 = lines[2];
            let l3 = lines[3];

            let delim_idx0 = unsafe { memchr64_unchecked::<b';'>(l0) };
            let delim_idx1 = unsafe { memchr64_unchecked::<b';'>(l1) };
            let delim_idx2 = unsafe { memchr64_unchecked::<b';'>(l2) };
            let delim_idx3 = unsafe { memchr64_unchecked::<b';'>(l3) };

            let temperature0 = parse_temperature(l0);
            let temperature1 = parse_temperature(l1);
            let temperature2 = parse_temperature(l2);
            let temperature3 = parse_temperature(l3);

            let station0 = unsafe { std::str::from_utf8_unchecked(l0.get_unchecked(..delim_idx0)) };
            let station1 = unsafe { std::str::from_utf8_unchecked(l1.get_unchecked(..delim_idx1)) };
            let station2 = unsafe { std::str::from_utf8_unchecked(l2.get_unchecked(..delim_idx2)) };
            let station3 = unsafe { std::str::from_utf8_unchecked(l3.get_unchecked(..delim_idx3)) };

            let hash0 = StationNameKeyView::new(station0).hash_u64();
            let hash1 = StationNameKeyView::new(station1).hash_u64();
            let hash2 = StationNameKeyView::new(station2).hash_u64();
            let hash3 = StationNameKeyView::new(station3).hash_u64();

            let e0 = temperatures
                .raw_entry()
                .from_hash(hash0, |k| k.view() == StationNameKeyView::new(station0));
            let e1 = temperatures
                .raw_entry()
                .from_hash(hash1, |k| k.view() == StationNameKeyView::new(station1));
            let e2 = temperatures
                .raw_entry()
                .from_hash(hash2, |k| k.view() == StationNameKeyView::new(station2));
            let e3 = temperatures
                .raw_entry()
                .from_hash(hash3, |k| k.view() == StationNameKeyView::new(station3));

            let e0_found = e0.is_some();
            let e1_found = e1.is_some();
            let e2_found = e2.is_some();
            let e3_found = e3.is_some();

            if let Some(e) = e0 {
                e.1.add_reading(temperature0);
            }
            if let Some(e) = e1 {
                e.1.add_reading(temperature1);
            }
            if let Some(e) = e2 {
                e.1.add_reading(temperature2);
            }
            if let Some(e) = e3 {
                e.1.add_reading(temperature3);
            }

            if !e0_found || !e1_found || !e2_found || !e3_found {
                temperatures.insert(
                    StationNameKey::new(station0),
                    TemperatureSummary::of(temperature0),
                );
            }
            if !e1_found {
                temperatures.insert(
                    StationNameKey::new(station1),
                    TemperatureSummary::of(temperature1),
                );
            }
            if !e2_found {
                temperatures.insert(
                    StationNameKey::new(station2),
                    TemperatureSummary::of(temperature2),
                );
            }
            if !e3_found {
                temperatures.insert(
                    StationNameKey::new(station3),
                    TemperatureSummary::of(temperature3),
                );
            }

            IterationControl::Continue
        } else {
            let delim_idx = unsafe { memchr64_unchecked::<b';'>(lines[0]) };
            let temperature = parse_temperature(lines[0]);
            let station =
                unsafe { std::str::from_utf8_unchecked(lines[0].get_unchecked(..delim_idx)) };

            if let Some(v) = temperatures.get_mut(StationNameKeyView::new(station)) {
                v.add_reading(temperature);
            } else {
                temperatures.insert(
                    StationNameKey::new(station),
                    TemperatureSummary::of(temperature),
                );
            }

            IterationControl::Continue
        }
    })?;

    Ok(temperatures
        .into_iter()
        .map(|(station, summary)| WeatherStation {
            name: station.into(),
            summary,
        })
        .sorted_unstable())
}

#[derive(Parser, Debug)]
struct Args {
    #[arg(long, default_value = "measurements.txt")]
    input: String,
}

#[cfg_attr(feature = "profiled", inline(never))]
fn run() -> BrcResult {
    let args = Args::try_parse()?;

    println!(
        "{{{}}}",
        temperature_reading_summaries(&args.input)?
            .map(|station| format!("{station}"))
            .join(", ")
    );
    Ok(())
}

fn main() -> ExitCode {
    #[cfg(feature = "profiled")]
    for _ in 0..4 {
        let _ = run();
    }

    let res = run();

    if let Err(err) = res {
        println!("{err}");
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

#[cfg(test)]
mod test {
    use crate::parse_temperature;

    #[test]
    fn test_parse_float() {
        assert_eq!(parse_temperature("  ;-99.9".as_bytes()), -999);
        assert_eq!(parse_temperature("  ;99.9".as_bytes()), 999);
        assert_eq!(parse_temperature("  ;-9.9".as_bytes()), -99);
        assert_eq!(parse_temperature("  ;9.9".as_bytes()), 99);
    }
}
