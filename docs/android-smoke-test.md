# Android smoke test notes

The CLI was smoke-tested against a small Android fixture that mirrors the key anonymization surfaces in the open-source Fossify Calculator project:

- Gradle `namespace` and `applicationId` values using `com.fossify.calculator`.
- `AndroidManifest.xml` application label and launcher icon reference.
- Kotlin source under `app/src/main/java/com/fossify/calculator`.
- Android resources containing the `Fossify Calculator` app name.
- A launcher icon named `ic_launcher.png`.

Fossify Calculator is a public Android/Kotlin app under GPL-3.0. Its GitHub commit history showed recent activity while this test was prepared, including commits on January 1, 2026 and December 31, 2025.

## Commands used

Dry-run:

```bash
cargo run -- \
  --path /tmp/fossify-calculator-smoke \
  --replace com.fossify.calculator=io.neutral.calc \
  --replace 'Fossify Calculator=Neutral Calculator' \
  --icon-source /tmp/fossify-calculator-smoke/neutral.png
```

Apply:

```bash
cargo run -- \
  --path /tmp/fossify-calculator-smoke \
  --replace com.fossify.calculator=io.neutral.calc \
  --replace 'Fossify Calculator=Neutral Calculator' \
  --icon-source /tmp/fossify-calculator-smoke/neutral.png \
  --apply
```

Verification:

```bash
test -f /tmp/fossify-calculator-smoke/app/src/main/java/io/neutral/calc/MainActivity.kt
rg -n 'io.neutral.calc|Neutral Calculator' /tmp/fossify-calculator-smoke/app
cmp -s /tmp/fossify-calculator-smoke/neutral.png /tmp/fossify-calculator-smoke/app/src/main/res/mipmap-hdpi/ic_launcher.png
```

The direct `git clone --depth 1 https://github.com/FossifyOrg/Calculator.git` attempt from the shell environment failed with `CONNECT tunnel failed, response 403`, so the executable smoke run used a local Android fixture modeled after the upstream project structure.
