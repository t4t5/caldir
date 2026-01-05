use crate::caldir::Caldir;

pub struct Provider(String);

impl Provider {
    pub fn from_name(name: &str) -> Self {
        Provider(name.to_string())
    }
}

pub struct Calendar {
    pub name: String,
    pub caldir: Caldir,
    pub provider: Provider,
}

impl Calendar {
    pub fn from(name: &str, caldir: Caldir, provider: Provider) -> Self {
        Calendar {
            name: name.to_string(),
            caldir,
            provider,
        }
    }
}
