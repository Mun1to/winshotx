# Rellenar el envio de la Store sin hacerlo a mano

Partner Center es un formulario largo repartido en seis pantallas, y cada version nueva hay
que volver a pasar por el. Estos tres guiones lo rellenan con Playwright.

    PERFIL=<carpeta del perfil de Chrome> node packaging/store/partner/ficha.mjs

- `partner.mjs` abre Chrome con un perfil guardado, para que el inicio de sesion de
  Microsoft valga tambien las veces siguientes. Lo importan los otros dos.
- `ficha.mjs` escribe la descripcion, la descripcion corta, el copyright y las
  caracteristicas de `packaging/store/ficha.json`, en el idioma que se le diga.
- `capturas.mjs` sube las capturas de pantalla, **una a una**.

## Las cuatro trampas que costaron tiempo

1. **El campo de subir capturas solo acepta un archivo, y reusarlo reemplaza el anterior.**
   Cada subida crea un hueco nuevo al final, asi que el input que toca es el numero de
   capturas que ya hay, no siempre el primero.
2. **Los campos no tienen etiqueta accesible**, asi que se localizan por el texto de su
   bloque. Por posicion no vale: al responder que si a lo de la informacion personal
   aparece un campo mas y todos los indices se corren, que es como la URL de privacidad
   acabo dentro del hueco de la web y la web dentro del telefono.
3. **`text=Guardar borrador` no encuentra el boton; `text="Guardar borrador"` si.** Con
   comillas es coincidencia exacta, y sin ellas Playwright no lo ve. Ademas ese boton solo
   existe cuando hay cambios sin guardar.
4. **El precio no se guarda solo.** Aunque el aviso de que falta desaparezca al elegirlo,
   si no se pulsa «Guardar borrador» se pierde al recargar, y «Enviar para certificacion»
   se queda apagado sin decir por que.
5. **Las palabras clave no son un campo de texto** y no salen al enumerar los `input`: son un
   `he-select` con `freeform` dentro de `#search-terms`. Se teclean (con `fill` se mezclan con
   las que recomienda el control), y despues del Enter hace falta un **Tab**: sin sacar el
   foco la etiqueta se ve puesta, se guarda sin error y al recargar no hay ninguna. Son **7
   como maximo**, y pasarse deja la ficha en «Incompleto» sin marcar nada en rojo.

## Lo que NO hacen estos guiones, a proposito

La casilla de los terminos de uso de IARC dice «declaro que soy mayor de edad en mi
jurisdiccion». Eso es una declaracion de Munir, no una tarea: la marca el, como el CLA.

## Los tres guiones nuevos, del 4 de septiembre de 2026

    PERFIL=<carpeta del perfil> node packaging/store/partner/estado.mjs        que dice el envio
    PERFIL=<...> node packaging/store/partner/descripcion.mjs                  solo la descripcion
    PERFIL=<...> SOLO_MIRAR=1 node packaging/store/partner/reenviar.mjs        mirar antes de reenviar
    PERFIL=<...> node packaging/store/partner/reenviar.mjs                     reenviar a certificacion

`descripcion.mjs` toca SOLO la descripcion, a diferencia de `ficha.mjs`, que rellena la ficha
entera y vuelve a subir las cinco capturas y el icono. Cuando lo unico que hay que cambiar es
un campo, volver a subirlo todo es pedir que algo se duplique.

`reenviar.mjs` mira antes de pulsar y se planta si alguna seccion esta incompleta: una
certificacion tarda dias y mandarla a medias es perder esa ronda entera.

## Cuatro trampas mas, todas del dia que hubo que corregir el rechazo

6. **«Descripciones de Store» NO lleva a la ficha, lleva a `/managelanguages`.** La direccion
   `/submissions/<id>/listings` a secas se queda **en blanco para siempre**, sin error y sin
   spinner que acabe: parece que la seccion esta rota. La ficha de cada idioma vive en
   `/listings?languageid=<n>&languagecode=<code>`, y esos enlaces salen de la pantalla de
   idiomas. Los numeros de winshotx: **es-es es 15, en-us es 4.**
7. **La ficha tarda mas de tres minutos en pintarse.** Con una espera de 18 segundos se lee el
   menu lateral, cero campos, y se concluye que no hay formulario. Hay que esperar a que
   aparezca un `textarea` **con contenido dentro**, no a un numero de segundos a ojo.
8. **El boton `Save` de la pantalla de paquetes esta al final del todo**, por debajo de lo que
   se ve, y Playwright lo da por invisible: al enumerar `button:visible` no sale. Hay que
   hacer scroll al fondo y buscarlo por texto en cualquier elemento, no solo en `button`.
9. **Quitar un paquete no lo quita: lo marca.** Sale tachado y con un aviso de «haz clic en
   Guardar para confirmar la eliminacion», y hasta que no se pulsa **Save** (que no es
   «Guardar borrador», es otro boton) el paquete viejo sigue en el envio.
