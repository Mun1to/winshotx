# De dónde sale lo que hay en esta carpeta

## `obturador.wav`

El clic de cámara que suena al capturar, si el ajuste **Sonido de obturador** está encendido.

- **Origen:** «5 Camera Shutter Sound Effects (No Copyright) Free To Use For Video Editing»,
  un paquete publicado como libre de derechos y de uso libre.
- **Qué se le hizo:** se recortó el tercer efecto (de 0,818 s a 0,953 s del original), se pasó
  de estéreo a mono, de 48.000 a 44.100 Hz, se subió el volumen un 40 % y se le pusieron un
  fundido de entrada de 3 ms y uno de salida de 20 ms, para que no chasque al empezar ni al
  acabar. Quedan 135 ms y 12 KB.
- **Por qué ese formato:** `PlaySoundW`, que es lo que trae Windows y lo que usa winshotx para
  no cargar con un motor de audio, **solo sabe leer WAV PCM**. La prueba
  `el_clic_es_un_wav_pcm_que_windows_sabe_tocar` lo comprueba en cada compilación.

Si alguna vez se cambia el sonido, hay que apuntar aquí el nuevo origen y su licencia: winshotx
se distribuye con licencia MIT y no puede empaquetar audio con derechos.
