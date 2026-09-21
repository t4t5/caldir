use serde::{Serialize, de::DeserializeOwned};
use std::path::Path;
use toml_edit::{DocumentMut, Item, Table, TableLike};

use super::atomic_write;

#[derive(Debug)]
pub(crate) enum TomlFileError {
    Read(std::io::Error),
    Write(std::io::Error),
    Deserialize(toml::de::Error),
    Serialize(toml::ser::Error),
    Parse(toml_edit::TomlError),
}

pub(crate) fn write_toml<T>(path: &Path, value: &T) -> Result<(), TomlFileError>
where
    T: Serialize + DeserializeOwned,
{
    let contents = match std::fs::read_to_string(path) {
        Ok(contents) => contents,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            let contents = toml::to_string(value).map_err(TomlFileError::Serialize)?;
            return atomic_write(path, contents.as_bytes()).map_err(TomlFileError::Write);
        }
        Err(err) => return Err(TomlFileError::Read(err)),
    };

    let old: T = toml::from_str(&contents).map_err(TomlFileError::Deserialize)?;
    let old = toml::Value::try_from(old).map_err(TomlFileError::Serialize)?;
    let new = toml::Value::try_from(value).map_err(TomlFileError::Serialize)?;

    if old == new {
        return Ok(());
    }

    let mut document = contents
        .parse::<DocumentMut>()
        .map_err(TomlFileError::Parse)?;
    apply_diff(document.as_table_mut(), &old, &new);

    atomic_write(path, document.to_string().as_bytes()).map_err(TomlFileError::Write)
}

fn apply_diff(document: &mut dyn TableLike, old: &toml::Value, new: &toml::Value) {
    let (toml::Value::Table(old), toml::Value::Table(new)) = (old, new) else {
        unreachable!("configuration roots must serialize as TOML tables");
    };

    apply_table_diff(document, old, new);
}

fn apply_table_diff(
    document: &mut dyn TableLike,
    old: &toml::map::Map<String, toml::Value>,
    new: &toml::map::Map<String, toml::Value>,
) {
    for (key, old_value) in old {
        let Some(new_value) = new.get(key) else {
            document.remove(key);
            continue;
        };

        if old_value == new_value {
            continue;
        }

        match (old_value, new_value) {
            (toml::Value::Table(old_table), toml::Value::Table(new_table)) => {
                if document.get(key).is_none_or(|item| !item.is_table_like()) {
                    document.insert(key, Item::Table(Table::new()));
                }

                let table = document
                    .get_mut(key)
                    .and_then(Item::as_table_like_mut)
                    .expect("a table was just inserted");
                apply_table_diff(table, old_table, new_table);
            }
            _ => replace_value(document, key, new_value),
        }
    }

    for (key, new_value) in new {
        if !old.contains_key(key) {
            document.insert(key, value_to_item(new_value));
        }
    }
}

fn replace_value(document: &mut dyn TableLike, key: &str, new: &toml::Value) {
    let decor = document
        .get(key)
        .and_then(Item::as_value)
        .map(|value| value.decor().clone());
    let mut replacement = value_to_item(new);

    if let (Some(decor), Some(value)) = (decor, replacement.as_value_mut()) {
        *value.decor_mut() = decor;
    }

    if let Some(item) = document.get_mut(key) {
        *item = replacement;
    } else {
        document.insert(key, replacement);
    }
}

fn value_to_item(value: &toml::Value) -> Item {
    let value = Serialize::serialize(value, toml_edit::ser::ValueSerializer::new())
        .expect("toml::Value always serializes as a toml_edit::Value");
    let item = Item::Value(value);

    match item.into_table() {
        Ok(table) => Item::Table(table),
        Err(item) => item,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    #[serde(default)]
    struct Config {
        name: Option<String>,
        mode: String,
        values: Vec<String>,
        nested: Option<Nested>,
    }

    impl Default for Config {
        fn default() -> Self {
            Self {
                name: None,
                mode: "standard".to_string(),
                values: Vec::new(),
                nested: None,
            }
        }
    }

    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    struct Nested {
        value: String,
        stale: Option<String>,
    }

    fn fixture(contents: &str) -> (tempfile::TempDir, std::path::PathBuf, Config) {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("config.toml");
        std::fs::write(&path, contents).unwrap();
        let config = toml::from_str(contents).unwrap();
        (tmp, path, config)
    }

    #[test]
    fn scalar_change_preserves_comments_and_formatting() {
        let before = "# header\nname   =   \"Old\"   # inline\n\n# trailing\n";
        let (_tmp, path, mut config) = fixture(before);
        config.name = Some("New".to_string());

        write_toml(&path, &config).unwrap();

        assert_eq!(
            std::fs::read_to_string(path).unwrap(),
            "# header\nname   =   \"New\"   # inline\n\n# trailing\n"
        );
    }

    #[test]
    fn unrelated_change_does_not_insert_absent_defaults() {
        let before = "name = \"Old\"\n";
        let (_tmp, path, mut config) = fixture(before);
        config.name = Some("New".to_string());

        write_toml(&path, &config).unwrap();

        assert_eq!(std::fs::read_to_string(path).unwrap(), "name = \"New\"\n");
    }

    #[test]
    fn changed_default_is_inserted() {
        let before = "name = \"Old\"\n";
        let (_tmp, path, mut config) = fixture(before);
        config.mode = "custom".to_string();

        write_toml(&path, &config).unwrap();

        assert_eq!(
            std::fs::read_to_string(path).unwrap(),
            "name = \"Old\"\nmode = \"custom\"\n"
        );
    }

    #[test]
    fn clearing_optional_field_preserves_sibling_comment() {
        let before = "name = \"Old\"\n# mode comment\nmode = \"custom\"\n";
        let (_tmp, path, mut config) = fixture(before);
        config.name = None;

        write_toml(&path, &config).unwrap();

        assert_eq!(
            std::fs::read_to_string(path).unwrap(),
            "# mode comment\nmode = \"custom\"\n"
        );
    }

    #[test]
    fn unknown_root_keys_and_nested_fields_are_untouched() {
        let before =
            "unknown = 'root'\n\n[nested]\nvalue = \"old\"\nunknown = { spelling = 'kept' }\n";
        let (_tmp, path, mut config) = fixture(before);
        config.nested.as_mut().unwrap().value = "new".to_string();

        write_toml(&path, &config).unwrap();

        assert_eq!(
            std::fs::read_to_string(path).unwrap(),
            "unknown = 'root'\n\n[nested]\nvalue = \"new\"\nunknown = { spelling = 'kept' }\n"
        );
    }

    #[test]
    fn unrelated_root_change_leaves_nested_table_byte_identical() {
        let before =
            "name = \"Old\"\n\n[nested] # table comment\nvalue = 'kept'\nstale = \"also kept\"\n";
        let (_tmp, path, mut config) = fixture(before);
        config.name = Some("New".to_string());

        write_toml(&path, &config).unwrap();

        assert_eq!(
            std::fs::read_to_string(path).unwrap(),
            "name = \"New\"\n\n[nested] # table comment\nvalue = 'kept'\nstale = \"also kept\"\n"
        );
    }

    #[test]
    fn replacing_nested_config_removes_stale_known_fields() {
        let before = "[nested]\nvalue = \"old\"\nstale = \"remove me\"\n";
        let (_tmp, path, mut config) = fixture(before);
        config.nested = Some(Nested {
            value: "new".to_string(),
            stale: None,
        });

        write_toml(&path, &config).unwrap();

        assert_eq!(
            std::fs::read_to_string(path).unwrap(),
            "[nested]\nvalue = \"new\"\n"
        );
    }

    #[test]
    fn clearing_nested_config_removes_its_table() {
        let before = "name = \"kept\"\n\n[nested]\nvalue = \"old\"\n";
        let (_tmp, path, mut config) = fixture(before);
        config.nested = None;

        write_toml(&path, &config).unwrap();

        assert_eq!(std::fs::read_to_string(path).unwrap(), "name = \"kept\"\n");
    }

    #[test]
    fn changed_array_keeps_outer_decoration() {
        let before = "values = [ \"one\", # inner\n  \"two\" ] # outer\n";
        let (_tmp, path, mut config) = fixture(before);
        config.values = vec!["new".to_string()];

        write_toml(&path, &config).unwrap();

        assert_eq!(
            std::fs::read_to_string(path).unwrap(),
            "values = [\"new\"] # outer\n"
        );
    }

    #[test]
    fn no_op_preserves_missing_final_newline() {
        let before = "name = 'same'";
        let (_tmp, path, config) = fixture(before);

        write_toml(&path, &config).unwrap();

        assert_eq!(std::fs::read_to_string(path).unwrap(), before);
    }

    #[test]
    fn missing_file_and_parent_directories_are_created() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("nested/dir/config.toml");
        let config = Config {
            name: Some("new".to_string()),
            ..Config::default()
        };

        write_toml(&path, &config).unwrap();

        assert_eq!(
            toml::from_str::<Config>(&std::fs::read_to_string(path).unwrap()).unwrap(),
            config
        );
    }

    #[test]
    fn malformed_file_errors_without_changing_contents() {
        let before = "name = [";
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("config.toml");
        std::fs::write(&path, before).unwrap();

        let error = write_toml(&path, &Config::default()).unwrap_err();

        assert!(matches!(error, TomlFileError::Deserialize(_)));
        assert_eq!(std::fs::read_to_string(path).unwrap(), before);
    }

    #[test]
    fn schema_invalid_file_errors_without_changing_contents() {
        let before = "mode = 12\n";
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("config.toml");
        std::fs::write(&path, before).unwrap();

        let error = write_toml(&path, &Config::default()).unwrap_err();

        assert!(matches!(error, TomlFileError::Deserialize(_)));
        assert_eq!(std::fs::read_to_string(path).unwrap(), before);
    }
}
