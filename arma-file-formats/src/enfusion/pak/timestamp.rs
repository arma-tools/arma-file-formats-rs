use std::fmt;

const SECONDS_PER_DAY: i64 = 86_400;

/// Modification time (UTC) of a pak entry.
///
/// Packed as `year - 2000 : 6 | month : 4 | day : 5 | hour : 5 | minute : 6 | second : 6` from
/// the high bit down. Workbench stores the modification time of the source file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct PakTimestamp(pub u32);

impl PakTimestamp {
    /// Returns `None` for dates outside of 2000 to 2063 or out of range fields.
    #[must_use]
    pub fn new(year: u16, month: u8, day: u8, hour: u8, minute: u8, second: u8) -> Option<Self> {
        let valid = (2000..=2063).contains(&year)
            && (1..=12).contains(&month)
            && (1..=31).contains(&day)
            && hour < 24
            && minute < 60
            && second < 60;
        valid.then(|| {
            Self(
                (u32::from(year - 2000) << 26)
                    | (u32::from(month) << 22)
                    | (u32::from(day) << 17)
                    | (u32::from(hour) << 12)
                    | (u32::from(minute) << 6)
                    | u32::from(second),
            )
        })
    }

    /// Converts seconds since the Unix epoch, `None` outside of 2000 to 2063.
    #[must_use]
    pub fn from_unix_secs(secs: i64) -> Option<Self> {
        let (year, month, day) = civil_from_days(secs.div_euclid(SECONDS_PER_DAY));
        let time = secs.rem_euclid(SECONDS_PER_DAY);
        Self::new(
            u16::try_from(year).ok()?,
            month,
            day,
            (time / 3600) as u8,
            (time / 60 % 60) as u8,
            (time % 60) as u8,
        )
    }

    /// Seconds since the Unix epoch.
    #[must_use]
    pub fn to_unix_secs(self) -> i64 {
        let days = days_from_civil(i64::from(self.year()), self.month(), self.day());
        days * SECONDS_PER_DAY
            + i64::from(self.hour()) * 3600
            + i64::from(self.minute()) * 60
            + i64::from(self.second())
    }

    #[must_use]
    pub const fn year(self) -> u16 {
        2000 + (self.0 >> 26) as u16
    }

    #[must_use]
    pub const fn month(self) -> u8 {
        (self.0 >> 22 & 0xF) as u8
    }

    #[must_use]
    pub const fn day(self) -> u8 {
        (self.0 >> 17 & 0x1F) as u8
    }

    #[must_use]
    pub const fn hour(self) -> u8 {
        (self.0 >> 12 & 0x1F) as u8
    }

    #[must_use]
    pub const fn minute(self) -> u8 {
        (self.0 >> 6 & 0x3F) as u8
    }

    #[must_use]
    pub const fn second(self) -> u8 {
        (self.0 & 0x3F) as u8
    }
}

impl fmt::Display for PakTimestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
            self.year(),
            self.month(),
            self.day(),
            self.hour(),
            self.minute(),
            self.second()
        )
    }
}

// Proleptic Gregorian calendar conversions, see https://howardhinnant.github.io/date_algorithms.html

fn civil_from_days(days: i64) -> (i64, u8, u8) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u8;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u8;
    let year = yoe + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

fn days_from_civil(year: i64, month: u8, day: u8) -> i64 {
    let year = year - i64::from(month <= 2);
    let era = year.div_euclid(400);
    let yoe = year - era * 400;
    let mp = (i64::from(month) + 9) % 12;
    let doy = (153 * mp + 2) / 5 + i64::from(day) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}
