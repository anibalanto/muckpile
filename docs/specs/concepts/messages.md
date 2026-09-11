# Los mensajes

Todo lo que ve el usuario sale de archivos de mensajes, uno por idioma.

## Los catálogos

### Lo que ve el usuario sale de un catálogo por idioma

Hay dos catálogos, `crates/muckpile-core/i18n/en.toml` y `crates/muckpile-core/i18n/es-AR.toml`. El código nombra cada mensaje con una clave en inglés, y el texto vive en el catálogo, con sus datos entre llaves: `{id}`. Las dos tienen las mismas claves, con los mismos datos en cada una, y un test lo exige. Un mensaje que falta en `es-AR` sale en `en`.

### El idioma sale de `MUCKPILE_LANG`, o del locale

El binario elige el idioma al arrancar, con la primera variable que no esté vacía de estas cuatro, en este orden: `MUCKPILE_LANG`, `LC_ALL`, `LC_MESSAGES` y `LANG`. Un valor que empiece con `es` es `es-AR`, y cualquier otro es `en`. La biblioteca, si nadie elige, habla `es-AR`.

### Los mensajes de los commits quedan en castellano

Los mensajes de los commits que hace `muckpile` y la plantilla de `muckpile.toml` que deja `init` no pasan por los catálogos: son historia, no algo que se le muestra al usuario.
