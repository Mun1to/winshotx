# winshotx promo video

The promo video for winshotx, made with [Remotion](https://www.remotion.dev/) out of frames of
the **real app**: nothing on screen is redrawn by hand. It lives outside the app's pnpm workspace
on purpose, because Remotion pulls a headless Chrome that nobody building winshotx needs.

## How it is made

1. **Frames of the real app.** `capturas.mjs` serves the app's own `dist/` bundle through the
   photo bench (`scripts/ver-ventana.mjs`, a fake Tauri with fixed answers) and drives it with
   Playwright: a real mouse over the selection overlay, one screenshot per video frame, with the
   page clock stepped exactly one frame between shots (`page.clock`), so animations play at their
   real speed however long each screenshot takes. It also photographs the editor and the
   settings. Nothing opens on anyone's desktop. Output: `public/app/` (not in git).
2. **The video.** `src/Promo.tsx` is the chosen cut, 40.6 s: the maker's story over the app, then
   five questions the app answers, a new idea every 2.2 s and no sound effects. `src/` composes
   those frames with a camera and text that comes in word by word. `src/direcciones/` holds the
   three directions that were drafted to choose from.
3. **Rendering.** `render.mjs` bundles once and renders at below-normal priority.
4. **Music.** `musica.mjs` lays a track under the muted render without rendering again: levelled
   to -14 LUFS, kept 8 dB down during the story and in full from the logo (`--sube`), started at
   any second of the track (`--desde`) so its drop lands on the logo.

```
pnpm build                          # in the repo root, once: the bench photographs dist/
cd promo
pnpm install --ignore-workspace
node capturas.mjs                   # about 40 s
node render.mjs muestras            # the opening of each direction plus a few stills
node render.mjs Promo               # the chosen cut (40.6 s), muted, in out/Promo.mp4
node musica.mjs <track.mp3> --desde=4.6 --sube=5.6   # out/Promo-<track>.mp4, about -14 LUFS
pnpm ver                            # Remotion Studio, to look around
```

## Rules this video follows

- English, 16:9 at 1920 x 1080, and it has to make sense **with the sound off** (Reddit plays it muted).
- Only the real app, and only measured, published figures: 114 ms against 920 ms, 33 MB against
  253 MB, a 3.00 MB installer, free and MIT.
- Text is centered, loose (never inside a box), two to five words, one word in colour.
- No sound effects in the chosen cut. The five CC0 clicks from Kenney's "Interface Sounds"
  (`public/sonido/kenney/`, licence alongside) are only used by the drafted directions. The music
  is chosen by a person from licensed stock (`musica-candidatas/`, not in git), not generated here.

## Traps already paid for

- The overlay ignores synthetic `dispatchEvent` mouse events; only Playwright's real input moves
  its magnifier and selection.
- The bench fakes two open windows ("Bloc de notas", "Explorador") to test window snapping; they
  are not in the photo, so `capturas.mjs` removes them from the overlay's bootstrap.
- The bench disables CSS transitions for its still photos; with the stepped clock that keeps every
  frame deterministic, so they stay disabled here too.
- Text over the moving app needs something behind it: a blur in the middle (`Velo`) or a soft
  shade coming down from the top edge (`Arriba`). Never a box.
- Remotion's packages all at the same version (4.0.507), and TypeScript stays on 5.x.
- One-pass `loudnorm` does not hold the true peak, and AAC pushes it up again: `musica.mjs` ends
  with a limiter at -3 dB, which leaves the files between -0.8 and -2.6 dBTP.
