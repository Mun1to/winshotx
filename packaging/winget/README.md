# winget

Manifests for [`microsoft/winget-pkgs`](https://github.com/microsoft/winget-pkgs), so that
`winget install Mun1to.winshotx` works. They are kept here because winget-pkgs only holds the
merged copy, and the next version starts from this one.

`winget validate --manifest packaging/winget/0.1.6` passes. Installing straight from a local
manifest needs `winget settings --enable LocalManifestFiles` as administrator, which is why the
real check is the validation plus the SHA256 of the published installer.

## Publishing a new version

1. Publish the GitHub release first: the installer URL has to exist and be immutable.
2. Copy the folder, rename it to the new version and change `PackageVersion`, `InstallerUrl`,
   `InstallerSha256`, `ReleaseDate`, `DisplayVersion` and `ReleaseNotesUrl`.
   The hash comes from `(Get-FileHash winshotx_X.Y.Z_x64-setup.exe -Algorithm SHA256).Hash`.
3. `winget validate --manifest packaging/winget/X.Y.Z`
4. Fork winget-pkgs, copy the folder to `manifests/m/Mun1to/winshotx/X.Y.Z/` and open a pull
   request. The bots check the hash, install the package in a sandbox and merge on their own if
   everything is in order.
