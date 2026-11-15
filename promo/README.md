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
2. **The video.** `src/Promo.tsx` is the chosen cut, 45 s: the maker's story over the app, then
   six questions the app answers, a new idea every 2.2 s. Its cuts are counted in beats of the
   music, not in seconds, so the same cut comes out on the beat of a track at another tempo
   (`Promo` at 120 beats per minute, `Promo-127` at 127.5). `src/` composes those frames with a
   camera and text that comes in word by word. `src/direcciones/` holds the three directions that
   were drafted to choose from.
3. **Sound effects.** Each one sits on its frame in the composition (`src/piezas/Sonidos.tsx`):
   keys on Win + Shift + S, a click and a shutter on the capture, a tick per step of a counting
   figure, a soft whoosh peaking on each cut. `sonidos.mjs` downloads the 14 of them from Mixkit,
   trims them and evens them to -1 dBFS.
4. **Rendering.** `render.mjs` bundles once and renders at below-normal priority: the picture,
   muted, and the effects on their own track (`out/<id>-efectos.wav`; rendering audio alone takes
   seconds, no frames are drawn).
5. **Music.** `musica.mjs` lays a track under the muted render without rendering again: levelled
   to -14 LUFS, kept 8 dB down during the story and in full from the logo (`--sube`), started at
   any second of the track (`--desde`) so its drop lands on the logo, with the effects mixed in
   under it.

```
pnpm build                          # in the repo root, once: the bench photographs dist/
cd promo
pnpm install --ignore-workspace
node capturas.mjs                   # about 40 s
node sonidos.mjs                    # the sound effects, from Mixkit, into public/sonido/mixkit/
node render.mjs muestras            # the opening of each direction plus a few stills
node render.mjs Promo               # the chosen cut (45 s): out/Promo.mp4 (muted) + out/Promo-efectos.wav
node render.mjs Promo-127           # the same cut on the beat of a 127.5 bpm track (42.4 s)
node musica.mjs <track.mp3> --desde=7.56 --sube=5.5  # out/Promo-<track>.mp4, about -14 LUFS
node musica.mjs <track.mp3> --video=Promo-127 --desde=7.57 --sube=5.18  # on the 127.5 cut
pnpm ver                            # Remotion Studio, to look around
```

## Rules this video follows

- English, 16:9 at 1920 x 1080, and it has to make sense **with the sound off** (Reddit plays it muted).
- Only the real app, and only measured, published figures: 114 ms against 920 ms, 33 MB against
  253 MB, a 3.00 MB installer, free and MIT.
- Text is centered, loose (never inside a box), two to five words, one word in colour.
- Sound effects, but discreet: each on its frame, under the music (a median of about 9 dB below
  it), never the same one twice in a row. They come from Mixkit under its Sound Effects Free
  License, which allows them in the video but not handing them out on their own, so they are not
  in git: `sonidos.mjs` fetches them. The five CC0 clicks from Kenney's "Interface Sounds"
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
  with a limiter at -3 dB and encodes AAC at 256 kbps (at 192 the native encoder pushed one
  limited mix up by 5 dB): the files end between -0.8 and -3.4 dBTP.
