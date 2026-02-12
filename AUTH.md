# Authentication

blippy resolves authentication on startup in this order:

1. `gh auth token --hostname github.com`
2. OS keychain (`service=blippy`, `account=github.com`)
3. Prompt for a PAT (input hidden), then store it in keychain

Tokens are never written to config files or the local database.

## Recommended Setup

GitHub CLI (`gh`) is heavily recommended. Run:

```bash
gh auth login
```

If `gh` is unavailable, blippy falls back to keychain or PAT prompt.

## Personal Access Token (PAT)

If prompted for a PAT, create one in GitHub settings.

### Fine-grained token (recommended)

Path: `Settings -> Developer settings -> Personal access tokens -> Fine-grained tokens -> Generate new token`

Suggested repository permissions:

- Repository metadata: `Read`
- Issues: `Read and write`
- Pull requests: `Read and write`

### Classic token

Path: `Settings -> Developer settings -> Personal access tokens -> Tokens (classic) -> Generate new token`

Suggested scopes:

- `repo` (private repos and full issues/PR access)
- `read:org` (organization repositories)

## Verify Auth

```bash
gh auth status
gh auth token --hostname github.com
```

## Debug Auth Source

```bash
BLIPPY_AUTH_DEBUG=1 blippy
```

## Reset Auth

Remove the stored keychain token:

```bash
blippy auth reset
```

To force the PAT prompt:

- Log out from GitHub CLI: `gh auth logout`
- Remove the `blippy` / `github.com` keychain entry in your OS keychain UI
