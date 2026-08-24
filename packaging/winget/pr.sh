#!/usr/bin/env bash
# Abre el pull request de winshotx 0.1.6 a microsoft/winget-pkgs sin clonar el repo,
# que pesa varios gigas. Los manifiestos salen de packaging/winget/0.1.6.
set -euo pipefail

VERSION=0.1.6
ORIGEN="packaging/winget/$VERSION"
DESTINO="manifests/m/Mun1to/winshotx/$VERSION"
RAMA="winshotx-$VERSION"
YO="$(gh api user --jq .login)"

echo "1/4 · fork"
gh repo fork microsoft/winget-pkgs --clone=false --remote=false >/dev/null 2>&1 || true
sleep 5

echo "2/4 · rama $RAMA"
BASE="$(gh api repos/microsoft/winget-pkgs --jq .default_branch)"
SHA="$(gh api "repos/$YO/winget-pkgs/git/ref/heads/$BASE" --jq .object.sha)"
gh api "repos/$YO/winget-pkgs/git/refs" -f ref="refs/heads/$RAMA" -f sha="$SHA" >/dev/null

echo "3/4 · manifiestos"
for f in "$ORIGEN"/*.yaml; do
  nombre="$(basename "$f")"
  gh api "repos/$YO/winget-pkgs/contents/$DESTINO/$nombre" -X PUT \
    -f message="New version: Mun1to.winshotx version $VERSION" \
    -f branch="$RAMA" \
    -f content="$(base64 -w0 < "$f")" >/dev/null
  echo "    $nombre"
done

echo "4/4 · pull request"
gh pr create --repo microsoft/winget-pkgs --head "$YO:$RAMA" --base "$BASE" \
  --title "New version: Mun1to.winshotx version $VERSION" \
  --body "winshotx is a free and open source screenshot and screen recording tool for Windows.

- [x] Have you signed the [Contributor License Agreement](https://cla.opensource.microsoft.com/microsoft/winget-pkgs)?
- [x] Have you checked that there aren't other open [pull requests](https://github.com/microsoft/winget-pkgs/pulls) for the same manifest update/add?
- [x] Have you validated your manifest locally with \`winget validate --manifest <path>\`?
- [ ] Have you tested your manifest locally with \`winget install --manifest <path>\`?
- [x] Does your manifest conform to the [1.6 schema](https://github.com/microsoft/winget-pkgs/tree/master/doc/manifest/schema/1.6.0)?

Source: https://github.com/Mun1to/winshotx
Home page: https://mun1to.github.io/winshotx/en/"
