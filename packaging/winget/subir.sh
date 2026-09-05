#!/usr/bin/env bash
# Abre la propuesta de winget de una version nueva partiendo de la anterior YA FUSIONADA en el
# catalogo, sin clonar nada (winget-pkgs pesa gigas). Para cuando la version anterior ya esta
# dentro; si la anterior sigue en un PR abierto, ese PR se actualiza en vez de abrir otro.
#
#   packaging/winget/subir.sh <vieja> <nueva> <SHA256 en mayusculas> <fecha YYYY-MM-DD>
#
# La huella es la del instalador CON la version en el nombre (winshotx_X.Y.Z_x64-setup.exe),
# y sale de PowerShell: (Get-FileHash <ruta> -Algorithm SHA256).Hash
#
# Comprueba antes que el instalador publicado responde 200 y tiene esa huella, valida los
# manifiestos con `winget validate`, y despues de cada cambio RELEE la linea que cambio: la
# primera version de esto buscaba `InstallerSha256` al principio de linea, en el YAML va con
# sangria, y la propuesta 429971 salio con la huella de la version anterior.
set -euo pipefail

VIEJA="$1"; NUEVA="$2"; SHA="$3"; FECHA="$4"
YO="Mun1to"
RAMA="winshotx-$NUEVA"
CARPETA="manifests/m/Mun1to/winshotx"
TMP="$(mktemp -d)"

echo "1/5 · comprobar que el instalador publicado responde y tiene ese hash"
URL="https://github.com/Mun1to/winshotx/releases/download/v$NUEVA/winshotx_${NUEVA}_x64-setup.exe"
CODIGO="$(curl -sIL -o /dev/null -w '%{http_code}' "$URL")"
[ "$CODIGO" = "200" ] || { echo "el instalador responde $CODIGO"; exit 1; }
mkdir -p "$TMP/dl" "$TMP/m"
curl -sL -o "$TMP/dl/setup.exe" "$URL"
REAL="$(sha256sum "$TMP/dl/setup.exe" | cut -d' ' -f1 | tr a-z A-Z)"
[ "$REAL" = "$SHA" ] || { echo "el hash publicado es $REAL y no $SHA"; exit 1; }
echo "    200 y hash correcto"

echo "2/5 · sincronizar el fork y crear la rama $RAMA"
gh repo sync "$YO/winget-pkgs" --source microsoft/winget-pkgs --branch master >/dev/null
BASE_SHA="$(gh api "repos/$YO/winget-pkgs/git/ref/heads/master" --jq .object.sha)"
gh api "repos/$YO/winget-pkgs/git/refs" -f ref="refs/heads/$RAMA" -f sha="$BASE_SHA" --jq .ref

echo "3/5 · manifiestos de la $NUEVA a partir de los de la $VIEJA"
for NOMBRE in Mun1to.winshotx.yaml Mun1to.winshotx.locale.en-US.yaml Mun1to.winshotx.installer.yaml; do
  gh api "repos/microsoft/winget-pkgs/contents/$CARPETA/$VIEJA/$NOMBRE" --jq .content | base64 -d > "$TMP/m/$NOMBRE"
  sed -i "s/$VIEJA/$NUEVA/g" "$TMP/m/$NOMBRE"
  if [ "$NOMBRE" = "Mun1to.winshotx.installer.yaml" ]; then
    # Con la sangria que lleve cada linea: InstallerSha256 va debajo de Installers.
    sed -i "s/^\(\s*\)InstallerSha256: .*/\1InstallerSha256: $SHA/" "$TMP/m/$NOMBRE"
    sed -i "s/^\(\s*\)ReleaseDate: .*/\1ReleaseDate: $FECHA/" "$TMP/m/$NOMBRE"
    grep -q "InstallerSha256: $SHA" "$TMP/m/$NOMBRE" || { echo "la huella no se ha escrito"; exit 1; }
    grep -q "ReleaseDate: $FECHA" "$TMP/m/$NOMBRE" || { echo "la fecha no se ha escrito"; exit 1; }
  fi
  grep -q "$VIEJA" "$TMP/m/$NOMBRE" && { echo "queda un $VIEJA suelto en $NOMBRE"; exit 1; }
  grep -q "DisplayVersion" "$TMP/m/$NOMBRE" && { echo "DisplayVersion no puede ir"; exit 1; }
  echo "    $NOMBRE"
done
grep -n "PackageVersion\|InstallerUrl\|InstallerSha256\|ReleaseDate" "$TMP/m/Mun1to.winshotx.installer.yaml"
echo "    validar con winget"
winget validate --manifest "$(cygpath -w "$TMP/m")" || { echo "winget validate ha fallado"; exit 1; }

echo "4/5 · subirlos"
for NOMBRE in Mun1to.winshotx.yaml Mun1to.winshotx.locale.en-US.yaml Mun1to.winshotx.installer.yaml; do
  TAM="$(gh api "repos/$YO/winget-pkgs/contents/$CARPETA/$NUEVA/$NOMBRE" -X PUT \
    -f message="New version: Mun1to.winshotx version $NUEVA" \
    -f branch="$RAMA" \
    -f content="$(base64 -w0 < "$TMP/m/$NOMBRE")" --jq .content.size)"
  [ "${TAM:-0}" -gt 0 ] || { echo "$NOMBRE se ha subido vacio"; exit 1; }
  echo "    $NOMBRE ($TAM bytes)"
done

echo "5/5 · pull request"
gh pr create --repo microsoft/winget-pkgs --head "$YO:$RAMA" --base master \
  --title "New version: Mun1to.winshotx version $NUEVA" \
  --body "winshotx is a free and open source screenshot and screen recording tool for Windows.

- [x] Have you signed the [Contributor License Agreement](https://cla.opensource.microsoft.com/microsoft/winget-pkgs)?
- [x] Have you checked that there aren't other open [pull requests](https://github.com/microsoft/winget-pkgs/pulls) for the same manifest update/add?
- [x] Have you validated your manifest locally with \`winget validate --manifest <path>\`?
- [ ] Have you tested your manifest locally with \`winget install --manifest <path>\`?
- [x] Does your manifest conform to the [1.6 schema](https://github.com/microsoft/winget-pkgs/tree/master/doc/manifest/schema/1.6.0)?

Source: https://github.com/Mun1to/winshotx
Home page: https://winshotx.com/en/"
rm -rf "$TMP"
