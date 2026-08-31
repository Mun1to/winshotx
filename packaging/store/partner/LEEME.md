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
