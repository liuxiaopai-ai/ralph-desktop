# Versioning

This project follows Semantic Versioning: `MAJOR.MINOR.PATCH`.

## When to bump
- Bump versions **only when tagging a release**.
- Do **not** auto-bump on every commit.

## Files to update
- `package.json`
- `src-tauri/Cargo.toml`
- `src-tauri/tauri.conf.json`

## Recommended workflow

```bash
node scripts/bump-version.mjs <MAJOR.MINOR.PATCH>
# example
node scripts/bump-version.mjs 0.1.1
```

Then commit the changes and tag:

```bash
git commit -am "chore: bump version to 0.1.1"
git tag v0.1.1
git push origin main --tags
```
