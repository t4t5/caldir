Providers should write data to the path specified by the `CALDIR_PROVIDER_STORAGE_DIR` env variable.

Why env variable?

1. It's not domain data. The protocol carries commands and their params — "list events from X to Y." Where the provider stores its private state is infrastructure config. Mixing those is the same smell as putting $HOME in every HTTP request body.
2. It's invariant per process. Sending the same provider_storage_dir on every call is redundant. Set it once at spawn time and it's done.
3. Standalone debuggability. A developer can run caldir-provider-google by hand to poke at it (the provider-rpc skill does this). With env var + sensible default, that's just:
echo '{"command":"list_calendars","params":{...}}' | caldir-provider-google
4. It's the standard Unix pattern. XDG_CONFIG_HOME, GIT_DIR, npm_config_* — infrastructure paths go in env, not in the protocol.

Concrete shape:

- Core spawns provider with `CALDIR_PROVIDER_STORAGE_DIR=<path>` set.
- Provider reads the env var at startup. Falls back to ~/.config/caldir/providers/{name}/ if unset (or whatever XDG-correct default).
- Tests inject a tempdir by setting the env var before spawning, same as they would for any other env-controlled tool.
- The Request struct loses the field, becoming purely {command, params}.

With a fallback default, a provider works standalone without core. Without a default, every invocation needs the env var set, which makes "run the provider directly to debug" annoying.

Tests would look like this: `Command::new(binary).env("CALDIR_PROVIDER_STATE_DIR", tempdir.path())`

