use chrono::{NaiveDate, NaiveDateTime};

pub trait DateBounds {
    fn start_of_date(self) -> NaiveDateTime;
    fn end_of_date(self) -> NaiveDateTime;
}

impl DateBounds for NaiveDate {
    fn start_of_date(self) -> NaiveDateTime {
        self.and_hms_opt(0, 0, 0).expect("0:0:0 is always valid")
    }

    fn end_of_date(self) -> NaiveDateTime {
        self.and_hms_opt(23, 59, 59)
            .expect("23:59:59 is always valid")
    }
}
