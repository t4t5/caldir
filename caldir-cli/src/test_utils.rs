use anyhow::Result;

pub fn capture<F>(f: F) -> String
where
    F: FnOnce(&mut Vec<u8>) -> Result<()>,
{
    let mut out: Vec<u8> = Vec::new();
    f(&mut out).unwrap();
    String::from_utf8(out).unwrap()
}
