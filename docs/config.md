We use a CaldirConfig / CaldirConfigFile / Caldir split.

Most experienced Rustaceans default to "value type + handle that knows where
it lives" because it shows up everywhere in the stdlib and ecosystem
(Path/File, Url/Client, serde_json::Value/from_reader). Once you've
internalized the pattern, you reach for it reflexively.

CaldirConfig       // data/schema
CaldirConfigFile   // persistence: data + path
Caldir             // runtime environment

It also makes the dependency direction better:

CaldirConfigFile -> CaldirConfig
Caldir           -> CaldirConfig

Why this model:

1. The mutation-and-persist methods are the design fighting back.
set_default_calendar_if_unset(&mut self) on Caldir is doing two unrelated
things — mutating config state and implicitly committing it to disk later via
save_config(). That's a config operation, not a Caldir operation. The fact
that it keeps wanting to live on Caldir is a smell. Move it to CaldirConfig
(or CaldirConfigFile) and Caldir stops being mutable at all. That's a big
simplification.

2. Option<PathBuf> is a question every reader has to answer. "When is it None?
 What does save_config do then?" The split removes the question — if you have
a CaldirConfigFile, you can save; if you have a Caldir, you can't. The type
system tells you the capability instead of a runtime branch + error.

3. The CLI/GUI sharing story is cleaner. Both call CaldirConfigFile::load() /
save(). There's a single canonical "load and persist the default config" path.
 With the hybrid, GUIs either construct a full Caldir just to mutate config
(weird) or write their own loader (drift risk).

4. Tests stop thinking about paths. ~95% of tests construct
Caldir::new(CaldirConfig::default(), providers) and never see a filesystem.
The remaining ~5% that test persistence reach for
CaldirConfigFile::load_from(tmpdir) directly — which is honest about what's
being tested.

I’d make CLI/MagiCal own persistence:

```rust
let mut config_file = CaldirConfigFile::load()?;
let providers = ProviderRegistry::discover(...);

let mut caldir = Caldir::new(config_file.config.clone(), providers)?;

commands::connect::run(&mut caldir, provider, hosted).await?;

config_file.config = caldir.config().clone();
config_file.save()?;
```

For most tests:

```rust
let caldir = Caldir::new(CaldirConfig::default(), ProviderRegistry::empty())?;
```

For persistence tests:

```rust
let file = CaldirConfigFile::load_from(tmp.path().join("config.toml"))?;
file.save()?;
```
