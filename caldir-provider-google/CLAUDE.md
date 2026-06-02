# caldir-provider-google

Google Calendar provider — OAuth + the Google Calendar REST API.

## Auth modes

Two modes, decided per-account at `connect` time:

- **Hosted (default)**: OAuth flows through `caldir.org`, which holds the client_id/secret. Users get to a working calendar without registering anything in Google Cloud Console.
- **Self-hosted (`--hosted=false`)**: User registers their own OAuth app and provides client_id/secret in `app_config.toml`. Tokens refresh directly with Google.

The mode is recorded on the session so refresh logic knows which path to take.
