mod from_google;
mod to_google;

/// Convert from Google API types to caldir types
pub trait FromGoogle<T> {
    fn from_google(value: T) -> anyhow::Result<Self>
    where
        Self: Sized;
}

/// Convert to Google API types from caldir types
pub trait ToGoogle<T> {
    fn to_google(&self) -> T;
}
