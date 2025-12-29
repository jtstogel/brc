#![feature(hint_prefetch)]

use brc::memops::memchr64_unchecked;
use brc::station_map::StationMap;
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

// This is rarely called (10k times out of 1B rows),
// so make sure it's outlined from the hot path.
#[inline(never)]
fn insert_temperature(m: &mut StationMap<TemperatureSummary>, k: &str, temp: i32) {
    m.entry(StationNameKey::new(k))
        .or_default()
        .add_reading(temp)
}

#[cfg_attr(feature = "profiled", inline(never))]
pub fn temperature_reading_summaries(
    input_path: &str,
) -> BrcResult<impl Iterator<Item = WeatherStation>> {
    let file = File::open(input_path)
        .map_err(|err| BrcError::new(format!("Failed to open {input_path}: {err}")))?;

    let mut temperatures = new_station_map::<TemperatureSummary>(12_500);

    const N: usize = 4;
    batched_process_lines::<N, _>(file, |lines: &[&[u8]]| {
        if lines.len() == N {
            let mut delim_indexes = [0usize; N];
            for i in 0..N {
                delim_indexes[i] = unsafe { memchr64_unchecked::<b';'>(lines[i]) };
            }

            let mut station_temperatures = [0i32; N];
            for i in 0..N {
                station_temperatures[i] = parse_temperature(lines[i]);
            }

            let mut stations = [""; N];
            for i in 0..N {
                stations[i] = unsafe {
                    std::str::from_utf8_unchecked(lines[i].get_unchecked(..delim_indexes[i]))
                };
            }

            let mut hashes = [0u64; N];
            for i in 0..N {
                hashes[i] = StationNameKeyView::new(stations[i]).hash_u64();
            }

            let mut entries: [Option<(&StationNameKey, &TemperatureSummary)>; N] = [None; N];
            for i in 0..N {
                entries[i] = temperatures.raw_entry().from_hash(hashes[i], |k| {
                    k.view() == StationNameKeyView::new(stations[i])
                });
            }

            let mut found = [false; N];
            for i in 0..N {
                found[i] = entries[i].is_some();
            }

            for i in 0..N {
                if let Some(e) = entries[i] {
                    e.1.add_reading(station_temperatures[i]);
                }
            }

            for i in 0..N {
                if !found[i] {
                    insert_temperature(&mut temperatures, stations[i], station_temperatures[i]);
                }
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
