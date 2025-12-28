use brc::memops::memchr64_unchecked;
use brc::station_map::StationNameKey;
use brc::station_map::StationNameKeyView;
use brc::station_map::new_station_map;
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
        let a = self.0 / 10;
        let b = self.0.abs() % 10;
        write!(f, "{}.{}", a, b)
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

/// Parses a float of the form [-][d]d.d from the string.
/// This will read up to two characters off the end of the provided slice,
/// so only provide slices with longer buffers.
#[cfg_attr(feature = "profiled", inline(never))]
fn parse_float(p: *const u8) -> i32 {
    let neg = unsafe { *p } == b'-';
    let idx = if neg { 1 } else { 0 };
    // def a digit: 99.9 or 9.9
    //              ^       ^
    let c0 = unsafe { *p.add(idx) };

    // maybe a digit: 99.9 or 9.9
    //                 ^       ^
    let c1 = unsafe { *p.add(idx + 1) };

    // maybe a digit: 99.9 or 9.9
    //                  ^       ^
    let c2 = unsafe { *p.add(idx + 2) };

    // maybe a digit: 99.9 or 9.9
    //                   ^       ^
    let c3 = unsafe { *p.add(idx + 3) };

    let is_three_digits = c1 != b'.';
    let three_digit_result = 100 * digit_to_i32(c0) + 10 * digit_to_i32(c1) + digit_to_i32(c3);
    let two_digit_result = 10 * digit_to_i32(c0) + 1 * digit_to_i32(c2);
    let result = if is_three_digits {
        three_digit_result
    } else {
        two_digit_result
    };
    if neg { -result } else { result }
}

enum IterationControl {
    Continue,
}

/// Iterates over the lines of the file.
///
/// Requires that no line in the input is longer than 64 bytes.
///
/// Optionally, specify limit to only process that number of lines.
#[cfg_attr(feature = "profiled", inline(never))]
fn process_lines<F>(file: File, mut callback: F) -> BrcResult<()>
where
    F: FnMut(&[u8]) -> IterationControl,
{
    let mmap = unsafe { MmapOptions::new().map(&file)? };
    mmap.advise(memmap2::Advice::Sequential)?;

    let mut cursor: usize = 0;
    let mmap_boundary = mmap.len() & !64usize;
    while cursor < mmap_boundary {
        let remaining = unsafe { mmap.get_unchecked(cursor..) };
        // TODO: This is prob inefficient. We're reading 64 bytes for the newline,
        // and we'll probably have to re-read the same chunk multiple times.
        // Experiment: instead read for all the newlines in a small chunk
        // of memory (say 256 bytes), then pop through the bitmask to find
        // the indices of newlines. That way we minimize the amount of
        // data we're re-scanning.
        let newline_idx = unsafe { memchr64_unchecked::<b'\n'>(remaining) };

        let ctrl = callback(unsafe { remaining.get_unchecked(..newline_idx) });
        cursor += newline_idx + 1;
        match ctrl {
            IterationControl::Continue => {
                continue;
            }
        }
    }

    // Deal with boundary condition at end of mmap'd region.
    while cursor < mmap.len() {
        let remaining = unsafe { mmap.get_unchecked(cursor..) };
        let mut data = [0; 64];
        let remaining_with_safe_boundary = &mut data[..remaining.len()];
        (remaining_with_safe_boundary).copy_from_slice(remaining);

        let newline_idx = unsafe { memchr64_unchecked::<b'\n'>(remaining_with_safe_boundary) };
        callback(unsafe { remaining_with_safe_boundary.get_unchecked(..newline_idx) });
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

    let mut temperatures = new_station_map::<TemperatureSummary>(20_000);

    process_lines(file, |line| {
        let delim_idx = unsafe { memchr64_unchecked::<b';'>(line) };
        let temperature = parse_float(unsafe { line.as_ptr().add(delim_idx + 1) });
        let station = unsafe { std::str::from_utf8_unchecked(line.get_unchecked(..delim_idx)) };

        if let Some(v) = temperatures.get_mut(StationNameKeyView::new(station)) {
            v.add_reading(temperature);
        } else {
            temperatures.insert(
                StationNameKey::new(station),
                TemperatureSummary::of(temperature),
            );
        }
        IterationControl::Continue
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
    // #[cfg(feature = "profiled")]
    // for _ in 0..4 {
    //     let _ = run();
    // }

    #[cfg(feature = "profiled")]
    let guard = pprof::ProfilerGuardBuilder::default()
        .frequency(99999)
        .blocklist(&["libc", "libgcc", "pthread", "vdso"])
        .build()
        .unwrap();

    let res = run();

    #[cfg(feature = "profiled")]
    if let Ok(report) = guard.report().build() {
        let file = std::fs::File::create("brc.svg").unwrap();
        report.flamegraph(file).unwrap();
    };

    if let Err(err) = res {
        println!("{err}");
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}
